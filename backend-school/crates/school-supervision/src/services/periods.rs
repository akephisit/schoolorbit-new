//! Canonical saved periods; legacy snapshots are read without rewriting their raw JSON.
use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, NaiveDate, Utc};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, PgConnection};
use uuid::Uuid;

use crate::models::{LessonSnapshot, SupervisionObservationPeriod, SupervisionPeriodManualEvidence};
use super::booking::ResolvedPeriod;

#[derive(sqlx::FromRow)]
struct PeriodRow {
    id: Uuid,
    observation_id: Uuid,
    position: i32,
    timetable_block_group_id: Option<Uuid>,
    learning_group_id: Option<Uuid>,
    homeroom_id: Option<Uuid>,
    observed_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    manual_subject_name: Option<String>,
    manual_classroom_label: Option<String>,
    manual_room_label: Option<String>,
    manual_period_label: Option<String>,
    manual_reason: Option<String>,
    lesson_snapshot: Json<LessonSnapshot>,
}

pub(super) async fn load(
    connection: &mut PgConnection,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<SupervisionObservationPeriod>>, AppError> {
    let rows: Vec<PeriodRow> = DbSession::new(connection).query_as(
        "supervision.periods.load",
        "SELECT id,observation_id,position,timetable_block_group_id,learning_group_id,homeroom_id,
         observed_at,ends_at,manual_subject_name,manual_classroom_label,manual_room_label,
         manual_period_label,manual_reason,lesson_snapshot FROM supervision_observation_periods
         WHERE observation_id=ANY($1) ORDER BY observation_id,position,id",
    ).bind(ids).fetch_all().await?;
    let mut result: HashMap<Uuid, Vec<SupervisionObservationPeriod>> = HashMap::new();
    for row in rows {
        let has_manual = row.manual_subject_name.is_some() || row.manual_classroom_label.is_some()
            || row.manual_room_label.is_some() || row.manual_period_label.is_some() || row.manual_reason.is_some();
        let manual_lesson = has_manual.then_some(SupervisionPeriodManualEvidence {
            subject_name: row.manual_subject_name,
            classroom_label: row.manual_classroom_label,
            room_label: row.manual_room_label,
            period_label: row.manual_period_label,
            reason: row.manual_reason,
        });
        result.entry(row.observation_id).or_default().push(SupervisionObservationPeriod {
            id: row.id, observation_id: row.observation_id, position: row.position,
            timetable_block_group_id: row.timetable_block_group_id,
            learning_group_id: row.learning_group_id, homeroom_id: row.homeroom_id,
            observed_at: row.observed_at, ends_at: row.ends_at,
            manual_lesson, lesson_snapshot: row.lesson_snapshot.0,
        });
    }
    Ok(result)
}

pub(super) async fn replace(
    connection: &mut PgConnection,
    observation: Uuid,
    year: Uuid,
    term: Uuid,
    periods: &[ResolvedPeriod],
) -> Result<(), AppError> {
    if periods.is_empty() {
        return Err(AppError::ValidationError("ต้องเลือกคาบนิเทศอย่างน้อย 1 คาบ".into()));
    }
    DbSession::new(connection).query("supervision.periods.replace_remove",
        "DELETE FROM supervision_observation_periods WHERE observation_id=$1")
        .bind(observation).execute().await?;
    let allocations = periods.iter().map(|p| p.allocation_id).collect::<Vec<_>>();
    let offerings = periods.iter().map(|p| p.offering_id).collect::<Vec<_>>();
    let groups = periods.iter().map(|p| p.learning_group_id).collect::<Vec<_>>();
    let homerooms = periods.iter().map(|p| p.homeroom_id).collect::<Vec<_>>();
    let starts = periods.iter().map(|p| p.observed_at).collect::<Vec<_>>();
    let ends = periods.iter().map(|p| p.ends_at).collect::<Vec<_>>();
    let snapshots = periods.iter().map(|p| p.snapshot.clone()).collect::<Vec<_>>();
    DbSession::new(connection).query("supervision.periods.replace_insert", r#"
        INSERT INTO supervision_observation_periods (observation_id,position,academic_year_id,academic_term_id,
            timetable_block_group_id,learning_offering_id,learning_group_id,homeroom_id,observed_at,ends_at,lesson_snapshot)
        SELECT $1,p.position::integer,$2,$3,p.allocation,p.offering,p.learning_group,p.homeroom,p.starts,p.ends,
            $10::jsonb->((p.position-1)::integer)
        FROM unnest($4::uuid[],$5::uuid[],$6::uuid[],$7::uuid[],$8::timestamptz[],$9::timestamptz[])
            WITH ORDINALITY AS p(allocation,offering,learning_group,homeroom,starts,ends,position)
    "#).bind(observation).bind(year).bind(term).bind(allocations).bind(offerings).bind(groups)
        .bind(homerooms).bind(starts).bind(ends).bind(Json(snapshots)).execute().await?;
    DbSession::new(connection).query("supervision.periods.touch_observation",
        "UPDATE supervision_observations SET updated_at=now() WHERE id=$1").bind(observation).execute().await?;
    Ok(())
}

/// Serialize the same evaluator's civil-date decisions across every cycle/year.
/// Unknown legacy ends stay exact points; no inferred duration is persisted.
pub(super) async fn lock_evaluator_dates(
    connection: &mut PgConnection,
    evaluators: &[Uuid],
    dates: impl IntoIterator<Item = NaiveDate>,
) -> Result<(), AppError> {
    let dates = dates.into_iter().collect::<BTreeSet<_>>();
    let keys = evaluators.iter().flat_map(|user| dates.iter().map(move |date| (*user,*date)))
        .collect::<BTreeSet<_>>();
    for (user,date) in keys {
        DbSession::new(connection).query("supervision.periods.lock_evaluator_date",
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("supervision-evaluator:{user}:{date}")).execute().await?;
    }
    Ok(())
}

pub(super) async fn ensure_no_own_duplicate(
    connection: &mut PgConnection,
    teacher: Uuid,
    except: Option<Uuid>,
    date: NaiveDate,
    periods: &[ResolvedPeriod],
) -> Result<(), AppError> {
    DbSession::new(connection).query("supervision.periods.lock_booking_date",
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("supervision-booking:{teacher}:{date}")).execute().await?;
    let starts = periods.iter().map(|p|p.observed_at).collect::<Vec<_>>();
    let ends = periods.iter().map(|p|p.ends_at).collect::<Vec<_>>();
    let duplicate: bool = DbSession::new(connection).scalar("supervision.periods.own_duplicate", r#"
        SELECT EXISTS(SELECT 1 FROM supervision_observations o
            JOIN supervision_observation_periods p ON p.observation_id=o.id
            CROSS JOIN unnest($3::timestamptz[],$4::timestamptz[]) chosen(starts,ends)
            WHERE o.observed_user_id=$1 AND ($2::uuid IS NULL OR o.id<>$2) AND o.status<>'cancelled'
              AND ((p.ends_at IS NOT NULL AND p.observed_at<chosen.ends AND chosen.starts<p.ends_at)
                OR (p.ends_at IS NULL AND chosen.starts<=p.observed_at AND p.observed_at<chosen.ends)))
    "#).bind(teacher).bind(except).bind(starts).bind(ends).fetch_one().await?;
    if duplicate {
        return Err(AppError::Conflict("มีรายการนิเทศของตนเองในคาบที่เลือกแล้ว".into()));
    }
    Ok(())
}
