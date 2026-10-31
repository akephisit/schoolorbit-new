use school_database::DbSession;
use std::collections::{HashMap, HashSet};

use bigdecimal::BigDecimal;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::models::{
    AssessmentCoordinatorOption, AssessmentExamArrangement, AssessmentOfferingScopeRow,
    AssessmentPhase, AssessmentPhaseCode, AssessmentPhaseControl, AssessmentPhaseControlRow,
    AssessmentPlanDetail, AssessmentPlanListQuery, AssessmentPlanRow, AssessmentPlanSummary,
    AssessmentReadiness, AssessmentReadinessFinding, SaveAssessmentPhaseRequest,
    SaveAssessmentPlanRequest, UpdateAssessmentPhaseControlRequest,
};
use school_academic_core::ports::{
    TermPreparationContext, TermPreparationMappingKind, TermPreparationModule,
    TermPreparationModuleOutcome,
};
use school_academic_core::services::{lifecycle_guard, validate_canonical_decimal};
use school_authorization::AcademicResourceListFilter;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;

#[derive(Debug, FromRow)]
struct AssessmentPlanSummaryRow {
    plan_id: Option<Uuid>,
    offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    subject_id: Uuid,
    subject_version_display_label: String,
    offering_code: String,
    offering_name: String,
    row_version: Option<i64>,
    assessment_coordinator_id: Option<Uuid>,
    assessment_coordinator_name: Option<String>,
    learning_group_ids: Vec<Uuid>,
    learning_group_count: i64,
    assessment_total_score: BigDecimal,
}

#[derive(Debug, FromRow)]
struct CoordinatorCandidateRow {
    offering_id: Uuid,
    teacher_id: Uuid,
    display_name: String,
    learning_group_count: i64,
    primary_learning_group_count: i64,
    primary_assignment_count: i64,
}

#[derive(Debug, Clone)]
struct CoordinatorCandidate {
    option: AssessmentCoordinatorOption,
    primary_assignment_count: i64,
}

pub fn default_phases() -> Vec<AssessmentPhase> {
    AssessmentPhaseCode::ALL
        .into_iter()
        .map(|phase_code| AssessmentPhase {
            id: None,
            phase_code,
            label: phase_code.label_th().to_string(),
            order: phase_code.order(),
            max_score: "0.00".to_string(),
            exam_arrangement: AssessmentExamArrangement::None,
            exam_duration_minutes: None,
            row_version: None,
        })
        .collect()
}

pub async fn apply_term_preparation(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    context: &TermPreparationContext,
    entity_mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
) -> Result<TermPreparationModuleOutcome, AppError> {
    #[derive(FromRow)]
    struct SourcePlan {
        id: Uuid,
        offering_id: Uuid,
        coordinator_id: Option<Uuid>,
    }

    let plans: Vec<SourcePlan> = DbSession::new(&mut **transaction).query_as("academic_assessment.assessment.services.apply_term_preparation",
        "SELECT id,learning_offering_id AS offering_id,assessment_coordinator_id AS coordinator_id
         FROM course_assessment_plans WHERE academic_term_id=$1 ORDER BY id",
    )
    .bind(context.source_term_id)
    .fetch_all()
    .await?;
    let mut ids = Vec::new();
    for plan in plans {
        let target_offering = entity_mappings
            .get(&(
                TermPreparationMappingKind::LearningOffering,
                plan.offering_id,
            ))
            .copied()
            .ok_or_else(|| {
                AppError::Conflict(format!("ไม่พบการจับคู่ learning_offering {}", plan.offering_id))
            })?;
        let (subject_version_id, target_year_id): (Uuid, Uuid) = DbSession::new(&mut **transaction)
            .query_as(
                "academic_assessment.assessment.services.apply_term_preparation",
                "SELECT detail.subject_version_id,offering.academic_year_id
             FROM course_offering_details detail
             JOIN learning_offerings offering ON offering.id=detail.learning_offering_id
             WHERE offering.id=$1 AND offering.academic_term_id=$2",
            )
            .bind(target_offering)
            .bind(context.target_term_id)
            .fetch_optional()
            .await?
            .ok_or_else(|| {
                AppError::Conflict("รายการเปิดสอนเป้าหมายของโครงสร้างคะแนนไม่พร้อม".into())
            })?;
        let coordinator = plan
            .coordinator_id
            .map(|id| {
                entity_mappings
                    .get(&(TermPreparationMappingKind::Teacher, id))
                    .copied()
                    .ok_or_else(|| AppError::Conflict(format!("ไม่พบการจับคู่ teacher {id}")))
            })
            .transpose()?;
        let id = Uuid::now_v7();
        DbSession::new(&mut **transaction)
            .query(
                "academic_assessment.assessment.services.apply_term_preparation",
                "INSERT INTO course_assessment_plans(
                 id,academic_term_id,subject_version_id,learning_offering_id,academic_year_id,
                 assessment_coordinator_id,row_version,migration_provenance,created_at,updated_at
             ) VALUES($1,$2,$3,$4,$5,$6,1,'{}'::jsonb,now(),now())",
            )
            .bind(id)
            .bind(context.target_term_id)
            .bind(subject_version_id)
            .bind(target_offering)
            .bind(target_year_id)
            .bind(coordinator)
            .execute()
            .await?;
        DbSession::new(&mut **transaction)
            .query(
                "academic_assessment.assessment.services.apply_term_preparation",
                r#"INSERT INTO course_assessment_phases(
                   id,plan_id,phase_code,max_score,exam_arrangement,exam_duration_minutes,
                   row_version,created_by,updated_by,created_at,updated_at
               ) SELECT gen_random_uuid(),$1,phase_code,max_score,exam_arrangement,
                        exam_duration_minutes,1,$2,$2,now(),now()
                 FROM course_assessment_phases WHERE plan_id=$3"#,
            )
            .bind(id)
            .bind(actor_user_id)
            .bind(plan.id)
            .execute()
            .await?;
        ids.push(id);
    }
    Ok(TermPreparationModuleOutcome {
        module: TermPreparationModule::Assessments,
        created_count: ids.len(),
        target_ids: ids,
    })
}

pub fn validate_plan_payload(payload: &SaveAssessmentPlanRequest) -> Result<(), AppError> {
    if payload.phases.len() != AssessmentPhaseCode::ALL.len() {
        return Err(AppError::ValidationError(
            "โครงสร้างคะแนนต้องมี 4 ช่วงมาตรฐานครบถ้วน".to_string(),
        ));
    }

    let zero = BigDecimal::from(0);
    let mut phase_codes = HashSet::new();
    let mut phase_ids = HashSet::new();
    for phase in &payload.phases {
        if !phase_codes.insert(phase.phase_code) {
            return Err(AppError::ValidationError(
                "โครงสร้างคะแนนต้องมี 4 ช่วงมาตรฐานโดยไม่ซ้ำกัน".to_string(),
            ));
        }
        if phase.id.is_some_and(|id| !phase_ids.insert(id)) {
            return Err(AppError::ValidationError(
                "ช่วงคะแนนอ้างอิงข้อมูลซ้ำกัน".to_string(),
            ));
        }

        let max_score = validate_canonical_decimal(&phase.max_score, 2)?;
        if max_score < zero {
            return Err(AppError::ValidationError(
                "คะแนนสูงสุดของแต่ละช่วงต้องไม่ติดลบ".to_string(),
            ));
        }

        if !phase.phase_code.supports_exam_arrangement()
            && phase.exam_arrangement != AssessmentExamArrangement::None
        {
            return Err(AppError::ValidationError(format!(
                "ช่วง{}ไม่ใช่ช่วงสอบกลางหรือปลายภาค",
                phase.phase_code.label_th()
            )));
        }
        if phase.exam_arrangement != AssessmentExamArrangement::InTimetable
            && phase.exam_duration_minutes.is_some()
        {
            return Err(AppError::ValidationError(format!(
                "ช่วง{}กำหนดเวลาได้เฉพาะการสอบในตารางสอบ",
                phase.phase_code.label_th()
            )));
        }
        if phase
            .exam_duration_minutes
            .is_some_and(|duration| duration <= 0)
        {
            return Err(AppError::ValidationError(
                "ระยะเวลาสอบต้องมากกว่า 0 นาที".to_string(),
            ));
        }
    }

    if AssessmentPhaseCode::ALL
        .into_iter()
        .any(|phase_code| !phase_codes.contains(&phase_code))
    {
        return Err(AppError::ValidationError(
            "โครงสร้างคะแนนต้องมี 4 ช่วงมาตรฐานครบถ้วน".to_string(),
        ));
    }
    Ok(())
}

pub fn plan_readiness(
    payload: &SaveAssessmentPlanRequest,
    expected_total: &BigDecimal,
) -> Result<AssessmentReadiness, AppError> {
    validate_plan_payload(payload)?;
    let mut findings = Vec::new();
    if payload.assessment_coordinator_id.is_none() {
        findings.push(AssessmentReadinessFinding::MissingCoordinator);
    }

    let total = payload.phases.iter().try_fold(
        BigDecimal::from(0),
        |total, phase| -> Result<BigDecimal, AppError> {
            Ok(total + validate_canonical_decimal(&phase.max_score, 2)?)
        },
    )?;
    if total != *expected_total {
        findings.push(AssessmentReadinessFinding::TotalMismatch);
    }

    for phase in &payload.phases {
        if phase.exam_arrangement == AssessmentExamArrangement::InTimetable
            && phase.exam_duration_minutes.is_none()
        {
            match phase.phase_code {
                AssessmentPhaseCode::Midterm => {
                    findings.push(AssessmentReadinessFinding::MidtermMissingExamDuration)
                }
                AssessmentPhaseCode::Final => {
                    findings.push(AssessmentReadinessFinding::FinalMissingExamDuration)
                }
                AssessmentPhaseCode::BeforeMidterm | AssessmentPhaseCode::AfterMidterm => {}
            }
        }
    }

    Ok(AssessmentReadiness {
        ready: findings.is_empty(),
        findings,
        total_score: decimal_string(&total),
        expected_total_score: decimal_string(expected_total),
    })
}

pub async fn list_assessment_plans(
    pool: &PgPool,
    query: &AssessmentPlanListQuery,
    access: &AcademicResourceListFilter,
    current_actor_id: Uuid,
) -> Result<Vec<AssessmentPlanSummary>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_assessment.assessment.services.list_assessment_plans",
    )
    .await?;
    list_assessment_plans_on_connection(&mut connection, query, access, current_actor_id).await
}

pub async fn list_assessment_plans_on_connection(
    connection: &mut sqlx::PgConnection,
    query: &AssessmentPlanListQuery,
    access: &AcademicResourceListFilter,
    current_actor_id: Uuid,
) -> Result<Vec<AssessmentPlanSummary>, AppError> {
    let owner_ids = access.allowed_organization_unit_ids();
    let rows: Vec<AssessmentPlanSummaryRow> = DbSession::new(connection)
        .query_as(
            "academic_assessment.assessment.services.list_assessment_plans_on_connection",
            r#"
        SELECT plan.id AS plan_id,
               offering.id AS offering_id,
               offering.academic_term_id,
               offering.academic_year_id,
               detail.subject_id,
               concat(
                   coalesce(version.name_th, version.name_en, offering.name_snapshot),
                   ' · v', version.version_no
               ) AS subject_version_display_label,
               offering.code_snapshot AS offering_code,
               offering.name_snapshot AS offering_name,
               plan.row_version,
               plan.assessment_coordinator_id,
               CASE WHEN coordinator.id IS NULL THEN NULL ELSE coalesce(
                   nullif(coordinator.display_name, ''),
                   coordinator.username
               ) END AS assessment_coordinator_name,
               ARRAY(
                   SELECT learning_group.id
                   FROM learning_groups learning_group
                   WHERE learning_group.learning_offering_id = offering.id
                     AND learning_group.status <> 'closed'
                   ORDER BY learning_group.code, learning_group.id
               ) AS learning_group_ids,
               (
                   SELECT count(*)
                   FROM learning_groups learning_group
                   WHERE learning_group.learning_offering_id = offering.id
                     AND learning_group.status <> 'closed'
               )::bigint AS learning_group_count,
               detail.assessment_total_score
        FROM learning_offerings offering
        JOIN course_offering_details detail ON detail.learning_offering_id = offering.id
        JOIN subject_versions version ON version.id = detail.subject_version_id
        LEFT JOIN course_assessment_plans plan ON plan.learning_offering_id = offering.id
        LEFT JOIN user_display_names coordinator ON coordinator.id = plan.assessment_coordinator_id
        WHERE offering.academic_term_id = $1
          AND offering.kind = 'course'
          AND ($2::uuid IS NULL OR detail.subject_id = $2)
          AND ($3::uuid IS NULL OR EXISTS (
              SELECT 1
              FROM learning_groups learning_group
              JOIN learning_group_teachers teacher
                ON teacher.learning_group_id = learning_group.id
              WHERE learning_group.learning_offering_id = offering.id
                AND teacher.teacher_id = $3
          ))
          AND ($4 OR offering.owning_organization_unit_id = ANY($5) OR EXISTS (
              SELECT 1
              FROM learning_groups learning_group
              JOIN learning_group_teachers teacher
                ON teacher.learning_group_id = learning_group.id
              WHERE learning_group.learning_offering_id = offering.id
                AND teacher.teacher_id = $6
          ))
        ORDER BY CASE
                     WHEN plan.assessment_coordinator_id = $7 THEN 0
                     ELSE 1
                 END,
                 offering.code_snapshot,
                 offering.id
        LIMIT 500
        "#,
        )
        .bind(query.academic_term_id)
        .bind(query.subject_id)
        .bind(query.instructor_id)
        .bind(access.includes_school_owned)
        .bind(owner_ids)
        .bind(access.assigned_actor_id)
        .bind(current_actor_id)
        .fetch_all()
        .await?;

    let plan_ids = rows
        .iter()
        .filter_map(|row| row.plan_id)
        .collect::<Vec<_>>();
    let offering_ids = rows.iter().map(|row| row.offering_id).collect::<Vec<_>>();
    let mut phases_by_plan = load_phases_by_plan(connection, &plan_ids).await?;
    let candidates_by_offering = load_coordinator_candidates(connection, &offering_ids).await?;

    let mut summaries = Vec::with_capacity(rows.len());
    for row in rows {
        let phases = row
            .plan_id
            .and_then(|plan_id| phases_by_plan.remove(&plan_id))
            .unwrap_or_else(default_phases);
        let candidates = candidates_by_offering
            .get(&row.offering_id)
            .cloned()
            .unwrap_or_default();
        let (suggested_id, suggested_name) =
            suggested_coordinator(&candidates, row.learning_group_count);
        let coordinator_is_candidate =
            row.assessment_coordinator_id.is_some_and(|coordinator_id| {
                candidates
                    .iter()
                    .any(|candidate| candidate.option.teacher_id == coordinator_id)
            });
        let readiness = response_readiness(
            row.assessment_coordinator_id,
            coordinator_is_candidate,
            &phases,
            &row.assessment_total_score,
        )?;
        if query.ready.is_some_and(|ready| readiness.ready != ready) {
            continue;
        }
        if query.exam_arrangement.is_some_and(|arrangement| {
            !phases.iter().any(|phase| {
                phase.phase_code.supports_exam_arrangement()
                    && phase.exam_arrangement == arrangement
            })
        }) {
            continue;
        }

        summaries.push(AssessmentPlanSummary {
            plan_id: row.plan_id,
            offering_id: row.offering_id,
            academic_term_id: row.academic_term_id,
            academic_year_id: row.academic_year_id,
            subject_id: row.subject_id,
            subject_version_display_label: row.subject_version_display_label,
            offering_code: row.offering_code,
            offering_name: row.offering_name,
            row_version: row.row_version,
            learning_group_ids: row.learning_group_ids,
            learning_group_count: row.learning_group_count,
            assessment_coordinator_id: row.assessment_coordinator_id,
            assessment_coordinator_name: row.assessment_coordinator_name,
            suggested_coordinator_id: suggested_id,
            suggested_coordinator_name: suggested_name,
            phases,
            readiness,
        });
    }
    Ok(summaries)
}

pub async fn get_plan_detail(
    pool: &PgPool,
    offering_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_assessment.assessment.services.get_plan_detail",
    )
    .await?;
    get_plan_detail_on_connection(&mut connection, offering_id).await
}

pub async fn get_plan_detail_on_connection(
    connection: &mut sqlx::PgConnection,
    offering_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    fetch_plan_detail(connection, offering_id).await
}

/// Initializes only missing coordinators, using the same dated suggestion as plan reads.
/// Delivery owns the transaction and publishes teacher episodes before invoking this operation.
pub async fn fill_missing_coordinators(
    result_locks: &dyn crate::ports::ResultLockPort,
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    offering_ids: &[Uuid],
) -> Result<(), AppError> {
    let mut offering_ids = offering_ids.to_vec();
    offering_ids.sort_unstable();
    offering_ids.dedup();
    if offering_ids.is_empty() {
        return Ok(());
    }
    let contexts: Vec<(Uuid, Uuid)> = DbSession::new(&mut **transaction).query_as(
        "academic_assessment.assessment.services.fill_missing_coordinators",
        "SELECT DISTINCT academic_year_id,academic_term_id FROM learning_offerings WHERE id=ANY($1) AND kind='course' ORDER BY academic_year_id,academic_term_id",
    ).bind(&offering_ids).fetch_all().await?;
    let mut writable_terms = Vec::new();
    for (year, term) in contexts {
        if lifecycle_guard::lock_context(transaction, year, term)
            .await?
            .is_writable()
        {
            writable_terms.push(term);
        }
    }
    #[derive(FromRow)]
    struct PublicationOfferingScope {
        offering_id: Uuid,
        academic_term_id: Uuid,
        academic_year_id: Uuid,
        subject_version_id: Uuid,
    }
    let scopes: Vec<PublicationOfferingScope> = DbSession::new(&mut **transaction).query_as(
        "academic_assessment.assessment.services.fill_missing_coordinators",
        r#"SELECT offering.id AS offering_id,offering.academic_term_id,offering.academic_year_id,
                  detail.subject_version_id
           FROM learning_offerings offering
           JOIN course_offering_details detail ON detail.learning_offering_id=offering.id
           WHERE offering.id=ANY($1) AND offering.academic_term_id=ANY($2)
             AND offering.kind='course' AND offering.status='published'
           ORDER BY offering.id FOR UPDATE OF offering"#,
    ).bind(&offering_ids).bind(&writable_terms).fetch_all().await?;
    let eligible_ids = scopes
        .iter()
        .map(|scope| scope.offering_id)
        .collect::<Vec<_>>();
    let mut candidates = load_coordinator_candidates(&mut **transaction, &eligible_ids).await?;
    let group_counts: Vec<(Uuid, i64)> = DbSession::new(&mut **transaction).query_as(
        "academic_assessment.assessment.services.fill_missing_coordinators",
        "SELECT learning_offering_id,count(*)::bigint FROM learning_groups WHERE learning_offering_id=ANY($1) AND status<>'closed' GROUP BY learning_offering_id",
    ).bind(&eligible_ids).fetch_all().await?;
    let group_counts = group_counts.into_iter().collect::<HashMap<_, _>>();
    for scope in scopes {
        if result_locks
            .course_offering_is_locked(
                transaction,
                scope.offering_id,
                scope.academic_term_id,
                scope.academic_year_id,
            )
            .await?
        {
            continue;
        }
        let existing = load_plan_in_tx(transaction, scope.offering_id).await?;
        if existing
            .as_ref()
            .is_some_and(|plan| plan.assessment_coordinator_id.is_some())
        {
            continue;
        }
        let (coordinator_id, _) = suggested_coordinator(
            &candidates.remove(&scope.offering_id).unwrap_or_default(),
            group_counts.get(&scope.offering_id).copied().unwrap_or(0),
        );
        let Some(coordinator_id) = coordinator_id else {
            continue;
        };
        if let Some(plan) = existing {
            DbSession::new(&mut **transaction).query(
                "academic_assessment.assessment.services.fill_missing_coordinators",
                "UPDATE course_assessment_plans SET assessment_coordinator_id=$2,row_version=row_version+1,updated_at=now() WHERE id=$1 AND assessment_coordinator_id IS NULL",
            ).bind(plan.id).bind(coordinator_id).execute().await?;
        } else {
            let plan_id = Uuid::now_v7();
            DbSession::new(&mut **transaction)
                .query(
                    "academic_assessment.assessment.services.fill_missing_coordinators",
                    r#"INSERT INTO course_assessment_plans(
                    id,academic_term_id,subject_version_id,learning_offering_id,academic_year_id,
                    assessment_coordinator_id,row_version,migration_provenance,created_at,updated_at
                ) VALUES($1,$2,$3,$4,$5,$6,1,'{}'::jsonb,now(),now())"#,
                )
                .bind(plan_id)
                .bind(scope.academic_term_id)
                .bind(scope.subject_version_id)
                .bind(scope.offering_id)
                .bind(scope.academic_year_id)
                .bind(coordinator_id)
                .execute()
                .await?;
            let phases = default_phases()
                .into_iter()
                .map(|phase| SaveAssessmentPhaseRequest {
                    id: None,
                    phase_code: phase.phase_code,
                    max_score: phase.max_score,
                    exam_arrangement: phase.exam_arrangement,
                    exam_duration_minutes: phase.exam_duration_minutes,
                })
                .collect::<Vec<_>>();
            replace_plan_phases(transaction, plan_id, actor_user_id, &phases, None).await?;
        }
    }
    Ok(())
}

pub async fn save_plan(
    result_locks: &dyn crate::ports::ResultLockPort,
    pool: &PgPool,
    offering_id: Uuid,
    actor_user_id: Uuid,
    can_manage_school: bool,
    payload: SaveAssessmentPlanRequest,
) -> Result<AssessmentPlanDetail, AppError> {
    validate_plan_payload(&payload)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.assessment.services.save_plan",
    )
    .await?;
    let scope = resolve_offering_scope_in_tx(&mut transaction, offering_id).await?;
    result_locks
        .require_course_offering_unlocked(
            &mut transaction,
            scope.offering_id,
            scope.academic_term_id,
            scope.academic_year_id,
        )
        .await?;
    let candidate_ids = load_candidate_ids_in_tx(&mut transaction, offering_id).await?;
    if payload
        .assessment_coordinator_id
        .is_some_and(|coordinator_id| !candidate_ids.contains(&coordinator_id))
    {
        return Err(AppError::ValidationError(
            "ผู้รับผิดชอบต้องเป็นครูที่กำลังสอนอย่างน้อยหนึ่งกลุ่มของรายวิชานี้".to_string(),
        ));
    }

    let existing = load_plan_in_tx(&mut transaction, offering_id).await?;
    let editable_phase_codes = if !can_manage_school {
        let allowed_coordinator = match &existing {
            Some(plan) => plan.assessment_coordinator_id == Some(actor_user_id),
            None => payload.assessment_coordinator_id == Some(actor_user_id),
        };
        if !allowed_coordinator || payload.assessment_coordinator_id != Some(actor_user_id) {
            return Err(AppError::Forbidden(
                "เฉพาะผู้รับผิดชอบโครงสร้างคะแนนหรือผู้ดูแลวิชาการเท่านั้นที่แก้ไขได้".to_string(),
            ));
        }
        Some(
            require_enabled_plan_phase_changes(
                &mut transaction,
                scope.academic_term_id,
                existing.as_ref().map(|plan| plan.id),
                &payload.phases,
            )
            .await?,
        )
    } else {
        None
    };

    let plan_id = match existing {
        Some(plan) => {
            let expected_version = payload.row_version.ok_or_else(|| {
                AppError::Conflict("ต้องระบุ rowVersion ของแผนคะแนนเดิม".to_string())
            })?;
            if expected_version != plan.row_version {
                return Err(AppError::Conflict(
                    "โครงสร้างคะแนนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string(),
                ));
            }
            let updated = DbSession::new(&mut *transaction)
                .query(
                    "academic_assessment.assessment.services.save_plan",
                    r#"UPDATE course_assessment_plans
                   SET assessment_coordinator_id = $2,
                       row_version = row_version + 1,
                       updated_at = now()
                   WHERE id = $1 AND row_version = $3"#,
                )
                .bind(plan.id)
                .bind(payload.assessment_coordinator_id)
                .bind(expected_version)
                .execute()
                .await?;
            if updated.rows_affected() != 1 {
                return Err(AppError::Conflict(
                    "โครงสร้างคะแนนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string(),
                ));
            }
            plan.id
        }
        None => {
            if payload.row_version.is_some() {
                return Err(AppError::Conflict(
                    "ยังไม่มีโครงสร้างคะแนนสำหรับรายวิชาที่เปิดสอนนี้".to_string(),
                ));
            }
            let plan_id = Uuid::now_v7();
            DbSession::new(&mut *transaction)
                .query(
                    "academic_assessment.assessment.services.save_plan",
                    r#"INSERT INTO course_assessment_plans (
                       id, academic_term_id, subject_version_id, learning_offering_id,
                       academic_year_id, assessment_coordinator_id, row_version,
                       migration_provenance, created_at, updated_at
                   ) VALUES ($1, $2, $3, $4, $5, $6, 1, '{}'::jsonb, now(), now())"#,
                )
                .bind(plan_id)
                .bind(scope.academic_term_id)
                .bind(scope.subject_version_id)
                .bind(scope.offering_id)
                .bind(scope.academic_year_id)
                .bind(payload.assessment_coordinator_id)
                .execute()
                .await?;
            plan_id
        }
    };

    replace_plan_phases(
        &mut transaction,
        plan_id,
        actor_user_id,
        &payload.phases,
        editable_phase_codes.as_ref(),
    )
    .await?;
    transaction.commit().await?;
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_assessment.assessment.services.save_plan",
    )
    .await?;
    fetch_plan_detail(&mut connection, offering_id).await
}

pub async fn list_phase_controls(
    pool: &PgPool,
    academic_term_id: Uuid,
) -> Result<Vec<AssessmentPhaseControl>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_assessment.assessment.services.list_phase_controls",
    )
    .await?;
    list_phase_controls_on_connection(&mut connection, academic_term_id).await
}

pub async fn list_phase_controls_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_term_id: Uuid,
) -> Result<Vec<AssessmentPhaseControl>, AppError> {
    let rows: Vec<AssessmentPhaseControlRow> = DbSession::new(connection)
        .query_as(
            "academic_assessment.assessment.services.list_phase_controls_on_connection",
            r#"SELECT id, academic_term_id, academic_year_id, phase_code,
                  plan_editing_enabled, exam_arrangement_editing_enabled, row_version
           FROM academic_assessment_phase_controls
           WHERE academic_term_id = $1
           ORDER BY CASE phase_code
               WHEN 'before_midterm' THEN 1
               WHEN 'midterm' THEN 2
               WHEN 'after_midterm' THEN 3
               WHEN 'final' THEN 4
               ELSE 99
           END"#,
        )
        .bind(academic_term_id)
        .fetch_all()
        .await?;
    rows.into_iter().map(phase_control_from_row).collect()
}

pub async fn update_phase_control(
    pool: &PgPool,
    control_id: Uuid,
    actor_user_id: Uuid,
    payload: UpdateAssessmentPhaseControlRequest,
) -> Result<AssessmentPhaseControl, AppError> {
    validate_phase_control_payload(&payload)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_assessment.assessment.services.update_phase_control",
    )
    .await?;
    let (year, term, phase_code): (Uuid, Uuid, String) = DbSession::new(&mut *transaction).query_as("academic_assessment.assessment.services.update_phase_control",
        "SELECT academic_year_id,academic_term_id,phase_code FROM academic_assessment_phase_controls WHERE id=$1",
    ).bind(control_id).fetch_optional().await?
        .ok_or_else(|| AppError::NotFound("ไม่พบการตั้งค่าช่วงคะแนน".into()))?;
    lifecycle_guard::require_term_write(&mut transaction, year, term).await?;
    if payload.exam_arrangement_editing_enabled == Some(true)
        && !matches!(phase_code.as_str(), "midterm" | "final")
    {
        return Err(AppError::ValidationError(
            "เปิดการแก้รูปแบบสอบได้เฉพาะช่วงกลางภาคและปลายภาค".to_string(),
        ));
    }
    let row: AssessmentPhaseControlRow = DbSession::new(&mut *transaction)
        .query_as(
            "academic_assessment.assessment.services.update_phase_control",
            r#"UPDATE academic_assessment_phase_controls
           SET plan_editing_enabled = coalesce($2, plan_editing_enabled),
               exam_arrangement_editing_enabled = coalesce($5, exam_arrangement_editing_enabled),
               row_version = row_version + 1,
               updated_by = $3,
               updated_at = now()
           WHERE id = $1 AND row_version = $4
           RETURNING id, academic_term_id, academic_year_id, phase_code,
                     plan_editing_enabled, exam_arrangement_editing_enabled, row_version"#,
        )
        .bind(control_id)
        .bind(payload.plan_editing_enabled)
        .bind(actor_user_id)
        .bind(payload.row_version)
        .bind(payload.exam_arrangement_editing_enabled)
        .fetch_optional()
        .await?
        .ok_or_else(|| {
            AppError::Conflict("การตั้งค่าช่วงคะแนนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string())
        })?;
    let result = phase_control_from_row(row)?;
    transaction.commit().await?;
    Ok(result)
}

pub fn validate_phase_control_payload(
    payload: &UpdateAssessmentPhaseControlRequest,
) -> Result<(), AppError> {
    if payload.row_version < 1 {
        return Err(AppError::ValidationError(
            "rowVersion ต้องมากกว่า 0".to_string(),
        ));
    }
    if payload.plan_editing_enabled.is_none() && payload.exam_arrangement_editing_enabled.is_none()
    {
        return Err(AppError::ValidationError(
            "ต้องระบุการตั้งค่าที่ต้องการแก้ไขอย่างน้อยหนึ่งรายการ".to_string(),
        ));
    }
    Ok(())
}

pub fn require_phase_controls_read_access(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
        codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_READ_SCHOOL,
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
    ])
}

pub fn require_phase_controls_manage_access(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL])
}

pub fn actor_can_manage_all_plans(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL])
}

async fn resolve_offering_scope(
    connection: &mut sqlx::PgConnection,
    offering_id: Uuid,
) -> Result<AssessmentOfferingScopeRow, AppError> {
    DbSession::new(connection)
        .query_as(
            "academic_assessment.assessment.services.resolve_offering_scope",
            sqlx::AssertSqlSafe(offering_scope_sql(false)),
        )
        .bind(offering_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()))
}

async fn resolve_offering_scope_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<AssessmentOfferingScopeRow, AppError> {
    // Resolve immutable context without taking an offering lock before the lifecycle lock.
    let (year, term): (Uuid, Uuid) = DbSession::new(&mut **transaction).query_as("academic_assessment.assessment.services.resolve_offering_scope_in_tx",
        "SELECT academic_year_id,academic_term_id FROM learning_offerings WHERE id=$1 AND kind='course'",
    ).bind(offering_id).fetch_optional().await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".into()))?;
    lifecycle_guard::require_term_write(transaction, year, term).await?;
    let scope: AssessmentOfferingScopeRow = DbSession::new(&mut **transaction)
        .query_as(
            "academic_assessment.assessment.services.resolve_offering_scope_in_tx",
            sqlx::AssertSqlSafe(offering_scope_sql(true)),
        )
        .bind(offering_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()))?;
    Ok(scope)
}

fn offering_scope_sql(for_update: bool) -> String {
    format!(
        r#"SELECT offering.id AS offering_id,
                  offering.academic_term_id,
                  offering.academic_year_id,
                  detail.subject_version_id,
                  detail.subject_id,
                  concat(
                      coalesce(version.name_th, version.name_en, offering.name_snapshot),
                      ' · v', version.version_no
                  ) AS subject_version_display_label,
                  offering.code_snapshot AS offering_code,
                  offering.name_snapshot AS offering_name,
                  detail.assessment_total_score
           FROM learning_offerings offering
           JOIN course_offering_details detail ON detail.learning_offering_id = offering.id
           JOIN subject_versions version ON version.id = detail.subject_version_id
           WHERE offering.id = $1 AND offering.kind = 'course'{}"#,
        if for_update {
            " FOR UPDATE OF offering"
        } else {
            ""
        }
    )
}

async fn load_plan_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<Option<AssessmentPlanRow>, AppError> {
    Ok(DbSession::new(&mut **transaction).query_as("academic_assessment.assessment.services.load_plan_in_tx",
        r#"SELECT plan.id, plan.assessment_coordinator_id,
                  CASE WHEN coordinator.id IS NULL THEN NULL ELSE coalesce(
                      nullif(coordinator.display_name, ''),
                      coordinator.username
                  ) END AS assessment_coordinator_name,
                  plan.row_version
           FROM course_assessment_plans plan
           LEFT JOIN user_display_names coordinator ON coordinator.id = plan.assessment_coordinator_id
           WHERE plan.learning_offering_id = $1
           FOR UPDATE OF plan"#,
    )
    .bind(offering_id)
    .fetch_optional()
    .await?)
}

async fn require_enabled_plan_phase_changes(
    transaction: &mut Transaction<'_, Postgres>,
    academic_term_id: Uuid,
    plan_id: Option<Uuid>,
    phases: &[SaveAssessmentPhaseRequest],
) -> Result<HashSet<String>, AppError> {
    let controls: Vec<(String, bool, bool)> = DbSession::new(&mut **transaction)
        .query_as(
            "academic_assessment.assessment.services.require_enabled_plan_phase_changes",
            r#"SELECT phase_code, plan_editing_enabled, exam_arrangement_editing_enabled
           FROM academic_assessment_phase_controls
           WHERE academic_term_id = $1
           FOR SHARE"#,
        )
        .bind(academic_term_id)
        .fetch_all()
        .await?;
    if controls.len() != AssessmentPhaseCode::ALL.len() {
        return Err(AppError::InternalServerError(
            "การตั้งค่าช่วงคะแนนของภาคเรียนไม่ครบถ้วน".to_string(),
        ));
    }
    let editing_by_code = controls
        .into_iter()
        .map(|(code, score, exam)| (code, (score, exam)))
        .collect::<HashMap<_, _>>();
    let editable_phase_codes = editing_by_code
        .iter()
        .filter(|(_, (score, exam))| *score || *exam)
        .map(|(phase_code, _)| phase_code.clone())
        .collect::<HashSet<_>>();
    let existing_phases: Vec<super::models::AssessmentPhaseRow> = if let Some(plan_id) = plan_id {
        DbSession::new(&mut **transaction)
            .query_as(
                "academic_assessment.assessment.services.require_enabled_plan_phase_changes",
                r#"SELECT id, plan_id, phase_code, max_score, exam_arrangement,
                          exam_duration_minutes, row_version
                   FROM course_assessment_phases
                   WHERE plan_id = $1
                   FOR UPDATE"#,
            )
            .bind(plan_id)
            .fetch_all()
            .await?
    } else {
        Vec::new()
    };
    let existing_by_code = existing_phases
        .into_iter()
        .map(|phase| (phase.phase_code.clone(), phase))
        .collect::<HashMap<_, _>>();

    for phase in phases {
        let (score_enabled, exam_enabled) = editing_by_code
            .get(phase.phase_code.as_str())
            .copied()
            .unwrap_or((false, false));
        let requested_max_score = validate_canonical_decimal(&phase.max_score, 2)?;
        let (score_changed, exam_changed) = if plan_id.is_some() {
            let existing = existing_by_code
                .get(phase.phase_code.as_str())
                .ok_or_else(|| {
                    AppError::InternalServerError("โครงสร้างคะแนนของรายวิชาไม่ครบถ้วน".to_string())
                })?;
            (
                existing.max_score != requested_max_score,
                existing.exam_arrangement != phase.exam_arrangement.as_str()
                    || existing.exam_duration_minutes != phase.exam_duration_minutes,
            )
        } else {
            (
                requested_max_score != BigDecimal::from(0),
                phase.exam_arrangement != AssessmentExamArrangement::None
                    || phase.exam_duration_minutes.is_some(),
            )
        };
        if score_changed && !score_enabled {
            return Err(AppError::Forbidden(format!(
                "ปิดการแก้คะแนนสูงสุดช่วง{}อยู่",
                phase.phase_code.label_th()
            )));
        }
        if exam_changed && !exam_enabled {
            return Err(AppError::Forbidden(format!(
                "ปิดการแก้รูปแบบสอบช่วง{}อยู่",
                phase.phase_code.label_th()
            )));
        }
    }
    Ok(editable_phase_codes)
}

async fn replace_plan_phases(
    transaction: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    actor_user_id: Uuid,
    phases: &[SaveAssessmentPhaseRequest],
    editable_phase_codes: Option<&HashSet<String>>,
) -> Result<(), AppError> {
    let existing: Vec<(Uuid, String)> = DbSession::new(&mut **transaction)
        .query_as(
            "academic_assessment.assessment.services.replace_plan_phases",
            "SELECT id, phase_code FROM course_assessment_phases WHERE plan_id = $1 FOR UPDATE",
        )
        .bind(plan_id)
        .fetch_all()
        .await?;
    let existing_by_code = existing
        .into_iter()
        .map(|(id, code)| (code, id))
        .collect::<HashMap<_, _>>();

    for phase in phases {
        let existing_id = existing_by_code.get(phase.phase_code.as_str()).copied();
        if phase.id.is_some_and(|id| Some(id) != existing_id) {
            return Err(AppError::ValidationError(
                "ช่วงคะแนนไม่ได้อยู่ในโครงสร้างคะแนนนี้".to_string(),
            ));
        }
        if existing_id.is_some()
            && editable_phase_codes.is_some_and(|codes| !codes.contains(phase.phase_code.as_str()))
        {
            continue;
        }
        let phase_id = existing_id.unwrap_or_else(Uuid::now_v7);
        let max_score = validate_canonical_decimal(&phase.max_score, 2)?;
        DbSession::new(&mut **transaction)
            .query(
                "academic_assessment.assessment.services.replace_plan_phases",
                r#"INSERT INTO course_assessment_phases (
                   id, plan_id, phase_code, max_score, exam_arrangement,
                   exam_duration_minutes, row_version, created_by, updated_by,
                   created_at, updated_at
               ) VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $7, now(), now())
               ON CONFLICT (plan_id, phase_code) DO UPDATE
               SET max_score = EXCLUDED.max_score,
                   exam_arrangement = EXCLUDED.exam_arrangement,
                   exam_duration_minutes = EXCLUDED.exam_duration_minutes,
                   row_version = course_assessment_phases.row_version + 1,
                   updated_by = EXCLUDED.updated_by,
                   updated_at = now()
               WHERE (course_assessment_phases.max_score,
                      course_assessment_phases.exam_arrangement,
                      course_assessment_phases.exam_duration_minutes)
                   IS DISTINCT FROM (EXCLUDED.max_score, EXCLUDED.exam_arrangement,
                                     EXCLUDED.exam_duration_minutes)"#,
            )
            .bind(phase_id)
            .bind(plan_id)
            .bind(phase.phase_code.as_str())
            .bind(max_score)
            .bind(phase.exam_arrangement.as_str())
            .bind(phase.exam_duration_minutes)
            .bind(actor_user_id)
            .execute()
            .await?;
    }
    Ok(())
}

async fn fetch_plan_detail(
    connection: &mut sqlx::PgConnection,
    offering_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    let scope = resolve_offering_scope(connection, offering_id).await?;
    let plan: Option<AssessmentPlanRow> = DbSession::new(connection).query_as("academic_assessment.assessment.services.fetch_plan_detail",
        r#"SELECT plan.id, plan.assessment_coordinator_id,
                  CASE WHEN coordinator.id IS NULL THEN NULL ELSE coalesce(
                      nullif(coordinator.display_name, ''),
                      coordinator.username
                  ) END AS assessment_coordinator_name,
                  plan.row_version
           FROM course_assessment_plans plan
           LEFT JOIN user_display_names coordinator ON coordinator.id = plan.assessment_coordinator_id
           WHERE plan.learning_offering_id = $1"#,
    )
    .bind(offering_id)
    .fetch_optional()
    .await?;
    let learning_group_ids: Vec<Uuid> = DbSession::new(connection)
        .scalar(
            "academic_assessment.assessment.services.fetch_plan_detail",
            r#"SELECT id FROM learning_groups
           WHERE learning_offering_id = $1 AND status <> 'closed'
           ORDER BY code, id"#,
        )
        .bind(offering_id)
        .fetch_all()
        .await?;
    let candidates = load_coordinator_candidates(connection, &[offering_id])
        .await?
        .remove(&offering_id)
        .unwrap_or_default();
    let (suggested_id, suggested_name) =
        suggested_coordinator(&candidates, learning_group_ids.len() as i64);

    let (id, coordinator_id, coordinator_name, row_version, phases) = match plan {
        Some(plan) => {
            let phases = load_phases_by_plan(connection, &[plan.id])
                .await?
                .remove(&plan.id)
                .unwrap_or_default();
            (
                Some(plan.id),
                plan.assessment_coordinator_id,
                plan.assessment_coordinator_name,
                Some(plan.row_version),
                phases,
            )
        }
        None => (None, None, None, None, default_phases()),
    };
    let coordinator_is_candidate = coordinator_id.is_some_and(|selected_id| {
        candidates
            .iter()
            .any(|candidate| candidate.option.teacher_id == selected_id)
    });
    let readiness = response_readiness(
        coordinator_id,
        coordinator_is_candidate,
        &phases,
        &scope.assessment_total_score,
    )?;

    Ok(AssessmentPlanDetail {
        id,
        offering_id: scope.offering_id,
        academic_term_id: scope.academic_term_id,
        academic_year_id: scope.academic_year_id,
        subject_id: scope.subject_id,
        subject_version_display_label: scope.subject_version_display_label,
        offering_code: scope.offering_code,
        offering_name: scope.offering_name,
        assessment_total_score: decimal_string(&scope.assessment_total_score),
        row_version,
        learning_group_ids,
        assessment_coordinator_id: coordinator_id,
        assessment_coordinator_name: coordinator_name,
        suggested_coordinator_id: suggested_id,
        suggested_coordinator_name: suggested_name,
        coordinator_candidates: candidates
            .into_iter()
            .map(|candidate| candidate.option)
            .collect(),
        phases,
        readiness,
    })
}

async fn load_phases_by_plan(
    connection: &mut sqlx::PgConnection,
    plan_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<AssessmentPhase>>, AppError> {
    if plan_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<super::models::AssessmentPhaseRow> = DbSession::new(connection)
        .query_as(
            "academic_assessment.assessment.services.load_phases_by_plan",
            r#"SELECT id, plan_id, phase_code, max_score, exam_arrangement,
                      exam_duration_minutes, row_version
               FROM course_assessment_phases
               WHERE plan_id = ANY($1)
               ORDER BY plan_id,
                   CASE phase_code
                       WHEN 'before_midterm' THEN 1
                       WHEN 'midterm' THEN 2
                       WHEN 'after_midterm' THEN 3
                       WHEN 'final' THEN 4
                       ELSE 99
                   END"#,
        )
        .bind(plan_ids)
        .fetch_all()
        .await?;
    let mut by_plan: HashMap<Uuid, Vec<AssessmentPhase>> = HashMap::new();
    for row in rows {
        let plan_id = row.plan_id;
        by_plan
            .entry(plan_id)
            .or_default()
            .push(phase_from_row(row)?);
    }
    Ok(by_plan)
}

fn phase_from_row(row: super::models::AssessmentPhaseRow) -> Result<AssessmentPhase, AppError> {
    let phase_code = AssessmentPhaseCode::try_from(row.phase_code.as_str()).map_err(|reason| {
        tracing::error!(%reason, "invalid assessment phase code stored in database");
        AppError::InternalServerError("รหัสช่วงคะแนนในระบบไม่ถูกต้อง".to_string())
    })?;
    let exam_arrangement = AssessmentExamArrangement::try_from(row.exam_arrangement.as_str())
        .map_err(|reason| {
            tracing::error!(%reason, "invalid exam arrangement stored in database");
            AppError::InternalServerError("รูปแบบการสอบในระบบไม่ถูกต้อง".to_string())
        })?;
    Ok(AssessmentPhase {
        id: Some(row.id),
        phase_code,
        label: phase_code.label_th().to_string(),
        order: phase_code.order(),
        max_score: decimal_string(&row.max_score),
        exam_arrangement,
        exam_duration_minutes: row.exam_duration_minutes,
        row_version: Some(row.row_version),
    })
}

fn response_readiness(
    coordinator_id: Option<Uuid>,
    coordinator_is_candidate: bool,
    phases: &[AssessmentPhase],
    expected_total: &BigDecimal,
) -> Result<AssessmentReadiness, AppError> {
    if phases.len() != 4 {
        let total = phases.iter().try_fold(
            BigDecimal::from(0),
            |total, phase| -> Result<BigDecimal, AppError> {
                Ok(total + validate_canonical_decimal(&phase.max_score, 2)?)
            },
        )?;
        return Ok(AssessmentReadiness {
            ready: false,
            findings: vec![AssessmentReadinessFinding::MissingPhase],
            total_score: decimal_string(&total),
            expected_total_score: decimal_string(expected_total),
        });
    }
    let payload = SaveAssessmentPlanRequest {
        row_version: None,
        assessment_coordinator_id: coordinator_id,
        phases: phases
            .iter()
            .map(|phase| SaveAssessmentPhaseRequest {
                id: phase.id,
                phase_code: phase.phase_code,
                max_score: phase.max_score.clone(),
                exam_arrangement: phase.exam_arrangement,
                exam_duration_minutes: phase.exam_duration_minutes,
            })
            .collect(),
    };
    let mut readiness = plan_readiness(&payload, expected_total)?;
    if coordinator_id.is_some() && !coordinator_is_candidate {
        readiness
            .findings
            .insert(0, AssessmentReadinessFinding::CoordinatorNotCandidate);
        readiness.ready = false;
    }
    Ok(readiness)
}

async fn load_coordinator_candidates(
    connection: &mut sqlx::PgConnection,
    offering_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<CoordinatorCandidate>>, AppError> {
    if offering_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<CoordinatorCandidateRow> = DbSession::new(connection)
        .query_as(
            "academic_assessment.assessment.services.load_coordinator_candidates",
            r#"SELECT offering.id AS offering_id,
                  teacher.teacher_id,
                  coalesce(
                      nullif(user_account.display_name, ''),
                      user_account.username
                  ) AS display_name,
                  count(DISTINCT learning_group.id)::bigint AS learning_group_count,
                  count(DISTINCT learning_group.id)
                      FILTER (WHERE teacher.role = 'primary')::bigint
                      AS primary_learning_group_count,
                  count(*) FILTER (WHERE teacher.role = 'primary')::bigint
                      AS primary_assignment_count
           FROM learning_offerings offering
           JOIN academic_terms term ON term.id = offering.academic_term_id
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = offering.id
            AND learning_group.status <> 'closed'
           JOIN learning_group_teachers teacher
             ON teacher.learning_group_id = learning_group.id
            AND teacher.starts_on <= LEAST(
                GREATEST(current_date, term.start_date), term.planned_end_date
            )
            AND (
                teacher.ends_on IS NULL
                OR teacher.ends_on >= LEAST(
                    GREATEST(current_date, term.start_date), term.planned_end_date
                )
            )
           JOIN user_display_names user_account
             ON user_account.id = teacher.teacher_id
            AND user_account.status = 'active'
           WHERE offering.id = ANY($1)
           GROUP BY offering.id, teacher.teacher_id,
                    user_account.display_name, user_account.username
           ORDER BY offering.id, display_name, teacher.teacher_id"#,
        )
        .bind(offering_ids)
        .fetch_all()
        .await?;
    let mut by_offering: HashMap<Uuid, Vec<CoordinatorCandidate>> = HashMap::new();
    for row in rows {
        by_offering
            .entry(row.offering_id)
            .or_default()
            .push(CoordinatorCandidate {
                option: AssessmentCoordinatorOption {
                    teacher_id: row.teacher_id,
                    display_name: row.display_name,
                    learning_group_count: row.learning_group_count,
                    primary_learning_group_count: row.primary_learning_group_count,
                },
                primary_assignment_count: row.primary_assignment_count,
            });
    }
    Ok(by_offering)
}

async fn load_candidate_ids_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<HashSet<Uuid>, AppError> {
    Ok(DbSession::new(&mut **transaction)
        .scalar(
            "academic_assessment.assessment.services.load_candidate_ids_in_tx",
            r#"SELECT DISTINCT teacher.teacher_id
           FROM learning_offerings offering
           JOIN academic_terms term ON term.id = offering.academic_term_id
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = offering.id
            AND learning_group.status <> 'closed'
           JOIN learning_group_teachers teacher
             ON teacher.learning_group_id = learning_group.id
            AND teacher.starts_on <= LEAST(
                GREATEST(current_date, term.start_date), term.planned_end_date
            )
            AND (
                teacher.ends_on IS NULL
                OR teacher.ends_on >= LEAST(
                    GREATEST(current_date, term.start_date), term.planned_end_date
                )
            )
           JOIN user_display_names user_account
             ON user_account.id = teacher.teacher_id
            AND user_account.status = 'active'
           WHERE offering.id = $1"#,
        )
        .bind(offering_id)
        .fetch_all()
        .await?
        .into_iter()
        .collect())
}

fn suggested_coordinator(
    candidates: &[CoordinatorCandidate],
    total_learning_group_count: i64,
) -> (Option<Uuid>, Option<String>) {
    if total_learning_group_count == 0 {
        return (None, None);
    }
    let matches = candidates
        .iter()
        .filter(|candidate| {
            candidate.option.primary_learning_group_count == total_learning_group_count
                && candidate.primary_assignment_count == total_learning_group_count
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return (None, None);
    }
    (
        Some(matches[0].option.teacher_id),
        Some(matches[0].option.display_name.clone()),
    )
}

fn phase_control_from_row(
    row: AssessmentPhaseControlRow,
) -> Result<AssessmentPhaseControl, AppError> {
    let phase_code = AssessmentPhaseCode::try_from(row.phase_code.as_str()).map_err(|reason| {
        tracing::error!(%reason, "invalid assessment phase control code stored in database");
        AppError::InternalServerError("รหัสช่วงคะแนนในระบบไม่ถูกต้อง".to_string())
    })?;
    Ok(AssessmentPhaseControl {
        id: row.id,
        academic_term_id: row.academic_term_id,
        academic_year_id: row.academic_year_id,
        phase_code,
        plan_editing_enabled: row.plan_editing_enabled,
        exam_arrangement_editing_enabled: row.exam_arrangement_editing_enabled,
        row_version: row.row_version,
    })
}

fn decimal_string(value: &BigDecimal) -> String {
    value.normalized().to_string()
}

#[cfg(test)]
mod tests {
    use super::{default_phases, plan_readiness, validate_plan_payload};
    use crate::assessment::models::{
        AssessmentExamArrangement, AssessmentPhaseCode, AssessmentReadinessFinding,
        SaveAssessmentPhaseRequest, SaveAssessmentPlanRequest,
    };
    use bigdecimal::BigDecimal;
    use school_errors::AppError;
    use uuid::Uuid;

    fn phase(
        phase_code: AssessmentPhaseCode,
        score: &str,
        exam_arrangement: AssessmentExamArrangement,
        exam_duration_minutes: Option<i32>,
    ) -> SaveAssessmentPhaseRequest {
        SaveAssessmentPhaseRequest {
            id: None,
            max_score: score.to_string(),
            phase_code,
            exam_arrangement,
            exam_duration_minutes,
        }
    }

    fn complete_payload() -> SaveAssessmentPlanRequest {
        SaveAssessmentPlanRequest {
            row_version: None,
            assessment_coordinator_id: Some(Uuid::new_v4()),
            phases: vec![
                phase(
                    AssessmentPhaseCode::BeforeMidterm,
                    "30.00",
                    AssessmentExamArrangement::None,
                    None,
                ),
                phase(
                    AssessmentPhaseCode::Midterm,
                    "20.00",
                    AssessmentExamArrangement::InTimetable,
                    Some(60),
                ),
                phase(
                    AssessmentPhaseCode::AfterMidterm,
                    "20.00",
                    AssessmentExamArrangement::None,
                    None,
                ),
                phase(
                    AssessmentPhaseCode::Final,
                    "30.00",
                    AssessmentExamArrangement::OutsideTimetable,
                    None,
                ),
            ],
        }
    }

    #[test]
    fn fixed_phase_payload_accepts_temporary_under_or_over_allocation() {
        let expected = "100.00".parse::<BigDecimal>().unwrap();
        let mut payload = complete_payload();
        assert!(validate_plan_payload(&payload).is_ok());

        payload.phases[0].max_score = "10.00".to_string();
        assert!(validate_plan_payload(&payload).is_ok());
        assert!(!plan_readiness(&payload, &expected).unwrap().ready);

        payload.phases[0].max_score = "70.00".to_string();
        assert!(validate_plan_payload(&payload).is_ok());
        assert!(!plan_readiness(&payload, &expected).unwrap().ready);
    }

    #[test]
    fn readiness_is_derived_from_coordinator_total_and_exam_duration() {
        let expected = "100.00".parse::<BigDecimal>().unwrap();
        let mut payload = complete_payload();
        let ready = plan_readiness(&payload, &expected).unwrap();
        assert!(ready.ready);
        assert!(ready.findings.is_empty());

        payload.assessment_coordinator_id = None;
        payload.phases[1].exam_duration_minutes = None;
        let incomplete = plan_readiness(&payload, &expected).unwrap();
        assert!(!incomplete.ready);
        assert_eq!(
            incomplete.findings,
            vec![
                AssessmentReadinessFinding::MissingCoordinator,
                AssessmentReadinessFinding::MidtermMissingExamDuration,
            ]
        );
    }

    #[test]
    fn exam_duration_is_only_allowed_for_in_timetable_exams() {
        let mut payload = complete_payload();
        payload.phases[3].exam_duration_minutes = Some(60);

        assert!(matches!(
            validate_plan_payload(&payload),
            Err(AppError::ValidationError(message))
                if message.contains("เฉพาะการสอบในตารางสอบ")
        ));
    }

    #[test]
    fn duplicate_or_noncanonical_phase_shape_is_rejected() {
        let mut payload = complete_payload();
        payload.phases[3].phase_code = AssessmentPhaseCode::Midterm;
        assert!(matches!(
            validate_plan_payload(&payload),
            Err(AppError::ValidationError(message)) if message.contains("4 ช่วง")
        ));

        let mut invalid_arrangement = complete_payload();
        invalid_arrangement.phases[0].exam_arrangement = AssessmentExamArrangement::InTimetable;
        assert!(matches!(
            validate_plan_payload(&invalid_arrangement),
            Err(AppError::ValidationError(message)) if message.contains("ก่อนกลางภาค")
        ));
    }

    #[test]
    fn defaults_are_four_system_owned_phase_rows() {
        let phases = default_phases();
        assert_eq!(phases.len(), 4);
        assert_eq!(
            phases
                .iter()
                .map(|phase| phase.phase_code)
                .collect::<Vec<_>>(),
            AssessmentPhaseCode::ALL
        );
        assert!(phases
            .iter()
            .all(|phase| phase.id.is_none() && phase.max_score == "0.00"));
    }
}
