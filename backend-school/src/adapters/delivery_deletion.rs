//! Application composition of the exact current Delivery deletion command.
use async_trait::async_trait;
use school_academic_delivery::{models::current::*, ports::*};
use school_academic_timetable::collaboration::{
    MutationAuthorizationPort, TimetableCollaborationRuntime, TimetableDeletionGuard,
    TimetableMutationContext,
};
use school_errors::AppError;
use sqlx::{PgConnection, Postgres, Transaction};
use std::sync::Arc;

pub(crate) struct ApplicationDeliveryDeletion {
    pub collaboration: Arc<TimetableCollaborationRuntime>,
}
struct Authorization(Arc<dyn DeliveryDeletionAuthorizationPort>);
#[async_trait]
impl MutationAuthorizationPort for Authorization {
    async fn validate_admission(
        &self,
        actor: &school_authorization::ActorContext,
    ) -> Result<(), AppError> {
        self.0.validate_admission(actor).await
    }
}
struct Admission {
    guard: TimetableDeletionGuard,
    pool: school_tenancy::TenantPool,
}
fn behavior_scope(
    scope: &DeliveryDeletionScope,
    sources: &school_attendance::services::deletion::AttendanceDeletionSources,
) -> school_behavior::services::lesson_deletion::LessonDeletionScope {
    school_behavior::services::lesson_deletion::LessonDeletionScope {
        generation: sources.generation,
        academic_term_id: scope.academic_term_id,
        session_ids: sources.session_ids.clone(),
        affected_students: sources
            .affected_students
            .iter()
            .map(
                |student| school_behavior::services::lesson_deletion::LessonDeletionStudent {
                    date: student.date,
                    student_id: student.student_id,
                },
            )
            .collect(),
        archived_student_ids: sources.archived_student_ids.clone(),
        session_students: sources
            .session_students
            .iter()
            .map(|source| {
                school_behavior::services::lesson_deletion::LessonDeletionSessionStudent {
                    session_id: source.session_id,
                    date: source.date,
                    student_id: source.student_id,
                }
            })
            .collect(),
    }
}
#[async_trait]
impl DeliveryDeletionPort for ApplicationDeliveryDeletion {
    async fn preview(
        &self,
        conn: &mut PgConnection,
        scope: &DeliveryDeletionScope,
    ) -> Result<DeliveryDeletionDependencies, AppError> {
        let timetable =
            school_academic_timetable::services::deletion::deletion_impact_on_connection(
                conn, scope,
            )
            .await?;
        let assessment =
            school_academic_assessment::deletion::preview_on_connection(conn, scope).await?;
        let results =
            school_academic_results::services::deletion::preview_current_contribution_removal(
                conn, scope,
            )
            .await?;
        let attendance =
            school_attendance::services::deletion::deletion_impact_on_connection(conn, scope)
                .await?;
        let sources =
            school_attendance::services::deletion::deletion_sources_on_connection(conn, scope)
                .await?;
        let behavior = school_behavior::services::lesson_deletion::preview_on_connection(
            conn,
            &behavior_scope(scope, &sources),
        )
        .await?;
        let supervision =
            school_supervision::services::current_source::deletion_impact_on_connection(
                conn, scope,
            )
            .await?;
        use sha2::{Digest, Sha256};
        let source_digest = hex::encode(Sha256::digest(
            serde_json::to_vec(&(
                &timetable.source_digest,
                &assessment.source_digest,
                &results.source_digest,
                &attendance.source_digest,
                &behavior.source_digest,
                &supervision.source_digest,
            ))
            .map_err(|_| {
                AppError::InternalServerError("Deletion evidence serialization failed".into())
            })?,
        ));
        Ok(DeliveryDeletionDependencies {
            source_digest,
            impact: OfferingDeletionImpact {
                timetable_blocks: timetable.blocks,
                exam_schedule_items: assessment.exam_items,
                assessment_scores: assessment.scores,
                attendance_sessions: attendance.sessions,
                attendance_summaries: attendance.summaries,
                evidence_files: attendance.evidence_files,
                official_results: results.official_results,
                result_corrections: results.result_corrections,
                term_aggregates: results.term_aggregates,
                annual_aggregates: results.annual_aggregates,
                supervision_links: supervision.observations,
                derived_behavior_events: behavior.entries,
                ..Default::default()
            },
        })
    }
    async fn acquire(
        &self,
        context: &DeliveryDeletionCommandContext,
        target: DeliveryDeletionTarget,
    ) -> Result<Box<dyn DeliveryDeletionAdmission>, AppError> {
        let mutation = TimetableMutationContext {
            runtime: self.collaboration.clone(),
            actor: context.actor.clone(),
            tenant_id: context.tenant_id,
            session_id: context.session_id,
            identity: None,
            staff_directory: &super::staff_directory::STAFF_DIRECTORY,
            lesson_source_detachment: &super::lesson_sources::LESSON_SOURCE_DETACHMENT,
            authorization: Arc::new(Authorization(context.authorization.clone())),
            pool_lease: context.pool_lease.clone(),
        };
        let guard = mutation
            .admit_delivery_deletion(
                &context.pool_lease,
                target.academic_year_id,
                target.academic_term_id,
            )
            .await?;
        Ok(Box::new(Admission {
            guard,
            pool: context.pool_lease.clone(),
        }))
    }
}
#[async_trait]
impl DeliveryDeletionAdmission for Admission {
    async fn lock_dependencies_before_term(
        &mut self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        school_attendance::services::deletion::lock_before_term_in_transaction(tx).await
    }
    async fn lock_dependencies_after_term(
        &mut self,
        tx: &mut Transaction<'_, Postgres>,
        scope: &DeliveryDeletionScope,
    ) -> Result<(), AppError> {
        self.guard
            .validate_current_on_connection(&mut **tx, scope)
            .await?;
        school_attendance::services::lock_term(tx, scope.academic_term_id, false, true).await?;
        let sources =
            school_attendance::services::deletion::deletion_sources_on_connection(&mut **tx, scope)
                .await?;
        school_behavior::services::lesson_deletion::require_removal_in_transaction(
            tx,
            &behavior_scope(scope, &sources),
        )
        .await?;
        Ok(())
    }
    async fn delete_dependencies(
        &mut self,
        tx: &mut Transaction<'_, Postgres>,
        scope: &DeliveryDeletionScope,
    ) -> Result<(), AppError> {
        let sources =
            school_attendance::services::deletion::deletion_sources_on_connection(&mut **tx, scope)
                .await?;
        // Official frozen contributions must be transformed before their source IDs disappear.
        school_academic_results::services::deletion::remove_current_contributions(tx, scope)
            .await?;
        school_supervision::services::current_source::detach_deleted_source_in_transaction(
            tx, scope,
        )
        .await?;
        self.guard.delete_in_transaction(tx, scope).await?;
        school_academic_assessment::deletion::delete_in_transaction(tx, scope).await?;
        let repository =
            school_file_platform::repository::SqlFileRepository::new((*self.pool).clone());
        for file_id in &sources.evidence_file_ids {
            repository
                .request_delete_in_transaction(tx, *file_id)
                .await
                .map_err(|error| AppError::InternalServerError(error.log_safe_code().into()))?;
        }
        let removed =
            school_attendance::services::deletion::delete_in_transaction(tx, scope).await?;
        for date in removed.affected_dates {
            if let Some(change) =
                school_attendance::services::score_changes::current_day_on_connection(
                    &mut **tx,
                    removed.generation,
                    scope.academic_term_id,
                    date,
                )
                .await?
            {
                if change.reset {
                    return Err(AppError::ConflictWithCode {
                        code: "ACADEMIC_DELIVERY_ATTENDANCE_RESET_PENDING",
                        message: "กำลังล้างข้อมูลเช็กชื่อ กรุณารอให้เสร็จก่อนลบรายการเปิดสอน".into(),
                    });
                }
                let facts = school_attendance::services::score_changes::day_facts_on_connection(
                    &mut **tx,
                    scope.academic_term_id,
                    date,
                )
                .await?
                .into_iter()
                .map(super::behavior::behavior_fact)
                .collect::<Result<Vec<_>, _>>()?;
                school_behavior::services::integration::replace_attendance_day(
                    &mut **tx,
                    school_behavior::models::BehaviorAttendanceDay {
                        generation: change.generation,
                        academic_term_id: change.academic_term_id,
                        date: change.date,
                        revision: change.row_version,
                        facts,
                    },
                )
                .await?;
                if !school_attendance::services::score_changes::acknowledge(tx, &change).await? {
                    return Err(AppError::ConflictWithCode {
                        code: "ACADEMIC_DELIVERY_ATTENDANCE_CHANGED",
                        message: "ข้อมูลเช็กชื่อเปลี่ยนไประหว่างการลบ กรุณาตรวจสอบผลกระทบใหม่".into(),
                    });
                }
            }
        }
        Ok(())
    }
    async fn commit(
        self: Box<Self>,
        tx: school_tenancy::ObservedTransaction,
        changed: bool,
    ) -> Result<(), AppError> {
        self.guard.commit(tx, changed).await
    }
}
