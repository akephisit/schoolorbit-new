//! Application-owned Attendance -> Behavior delivery and score-cycle scheduling.
use chrono::{NaiveDate, Utc};
use school_attendance::{
    models::{AttendanceKind, ScoreAction},
    services::{finalization, score_changes},
};
use school_behavior::{
    models::{
        BehaviorAttendanceDay, BehaviorAttendanceFact, BehaviorResetReadiness,
        BehaviorScoreAction, BehaviorScoreOverride, BehaviorSource,
    },
    services::{integration, schedules},
};
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};

fn behavior_fact(
    fact: score_changes::AttendanceScoreFact,
) -> Result<BehaviorAttendanceFact, AppError> {
    let source_session_id = if fact.kind == AttendanceKind::Arrival {
        None
    } else {
        Some(uuid::Uuid::parse_str(&fact.source_key).map_err(|_| {
            AppError::InternalServerError("รหัสรอบเช็กชื่อสำหรับคะแนนไม่ถูกต้อง".into())
        })?)
    };
    Ok(BehaviorAttendanceFact {
        source_key: fact.source_key,
        source_session_id,
        kind: match fact.kind {
            AttendanceKind::Arrival => BehaviorSource::Arrival,
            AttendanceKind::Lesson => BehaviorSource::Lesson,
            AttendanceKind::Special => BehaviorSource::Special,
        },
        student_id: fact.student_id,
        status: fact.result.code().into(),
        counted: fact.counted,
        grade_level_id: fact.grade_level_id,
        homeroom_id: fact.homeroom_id,
        overrides: fact.scoring.map(|rules| {
            rules
                .into_iter()
                .map(|rule| BehaviorScoreOverride {
                    status: rule.result.code().into(),
                    action: match rule.action {
                        ScoreAction::None => BehaviorScoreAction::None,
                        ScoreAction::Merit => BehaviorScoreAction::Merit,
                        ScoreAction::Deduction => BehaviorScoreAction::Deduction,
                    },
                    points: rule.points,
                })
                .collect()
        }),
    })
}

pub(crate) async fn run(pool: &PgPool) -> Result<(), AppError> {
    let changes = {
        let mut connection = pool.acquire().await?;
        score_changes::pending_on_connection(&mut connection, 20).await?
    };
    for change in changes {
        let mut transaction = pool.begin().await?;
        // Module -> academic lifecycle -> Attendance term -> Behavior. Never take
        // an Attendance lock after the Behavior lock held by the consumer.
        school_attendance::services::lock_term(
            &mut transaction,
            change.academic_term_id,
            false,
            false,
        )
        .await?;
        let facts = if change.reset {
            Vec::new()
        } else {
            score_changes::day_facts_on_connection(
                &mut transaction,
                change.academic_term_id,
                change.date,
            )
            .await?
            .into_iter()
            .map(behavior_fact)
            .collect::<Result<Vec<_>, _>>()?
        };
        integration::replace_attendance_day(
            &mut transaction,
            BehaviorAttendanceDay {
                generation: change.generation,
                academic_term_id: change.academic_term_id,
                date: change.date,
                revision: change.row_version,
                facts,
            },
        )
        .await?;
        let acknowledged = score_changes::acknowledge(&mut transaction, &change).await?;
        transaction.commit().await?;
        if !acknowledged {
            tracing::debug!(
                event_code = "behavior_source_requeued",
                "An Attendance correction remains queued for reconciliation"
            );
        }
    }
    let today = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let mut transaction = pool.begin().await?;
    school_attendance::services::lock_module_exclusive(&mut transaction).await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut transaction)
        .await?;
    let readiness = reset_readiness(&mut transaction, today).await?;
    schedules::run_due_resets_on_connection(&mut transaction, today, readiness, 8).await?;
    transaction.commit().await?;
    Ok(())
}

async fn reset_readiness(
    connection: &mut PgConnection,
    today: NaiveDate,
) -> Result<BehaviorResetReadiness, AppError> {
    let finalized_through =
        finalization::ready_through_on_connection(connection, today).await?;
    let tomorrow = today.succ_opt().ok_or_else(|| {
        AppError::ValidationError("วันที่เกินขอบเขตงานรีเซ็ตคะแนน".into())
    })?;
    let delivered_through =
        score_changes::delivered_through_on_connection(connection, tomorrow).await?;
    let (term_ends, year_ends) =
        school_academic_core::services::score_calendar::reset_boundaries(connection).await?;
    Ok(BehaviorResetReadiness {
        finalized_through,
        delivered_through,
        term_ends,
        year_ends,
    })
}

pub(crate) async fn reset_scores(
    pool: &PgPool,
    actor: uuid::Uuid,
    payload: school_behavior::models::ResetBehaviorScores,
) -> Result<school_behavior::models::BehaviorCleanupOutcome, AppError> {
    let mut transaction = pool.begin().await?;
    school_attendance::services::lock_module_exclusive(&mut transaction).await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut transaction)
        .await?;
    let today = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let readiness = reset_readiness(&mut transaction, today).await?;
    let outcome = schedules::reset_on_connection(&mut transaction, actor, payload, readiness).await?;
    transaction.commit().await?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use school_attendance::models::{AttendanceResult, AttendanceScoreRule};
    use uuid::Uuid;

    #[test]
    fn delivery_preserves_status_nullable_room_and_explicit_special_overrides() {
        let student_id = Uuid::new_v4();
        let converted = behavior_fact(score_changes::AttendanceScoreFact {
            student_id,
            student_academic_year_id: Uuid::new_v4(),
            source_key: Uuid::new_v4().to_string(),
            kind: AttendanceKind::Special,
            result: AttendanceResult::Skipped,
            counted: true,
            grade_level_id: Uuid::new_v4(),
            homeroom_id: None,
            scoring: Some(vec![AttendanceScoreRule {
                result: AttendanceResult::Skipped,
                action: ScoreAction::Deduction,
                points: 5,
            }]),
        })
        .unwrap();
        assert_eq!(converted.student_id, student_id);
        assert_eq!(converted.kind, BehaviorSource::Special);
        assert_eq!(converted.status, "skipped");
        assert_eq!(converted.homeroom_id, None);
        let rules = converted.overrides.unwrap();
        assert_eq!(rules[0].action, BehaviorScoreAction::Deduction);
        assert_eq!(rules[0].points, 5);
    }
}
