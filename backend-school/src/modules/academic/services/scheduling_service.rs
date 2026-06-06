use crate::error::AppError;
use crate::modules::academic::models::scheduling::*;
use crate::modules::academic::services::scheduler_data::SchedulerDataLoader;
use crate::modules::academic::services::SchedulerBuilder;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use sqlx::{types::Json, FromRow, PgPool};
use uuid::Uuid;

fn jsonb_or_default<T>(value: serde_json::Value, field_name: &str) -> Result<T, AppError>
where
    T: DeserializeOwned + Default,
{
    if value.is_null() {
        return Ok(T::default());
    }

    serde_json::from_value(value).map_err(|error| {
        AppError::InternalServerError(format!("Invalid {field_name} JSONB shape: {error}"))
    })
}

fn optional_jsonb_vec<T>(
    value: serde_json::Value,
    field_name: &str,
) -> Result<Option<Vec<T>>, AppError>
where
    T: DeserializeOwned,
{
    if value.is_null() {
        return Ok(None);
    }

    serde_json::from_value(value).map(Some).map_err(|error| {
        AppError::InternalServerError(format!("Invalid {field_name} JSONB shape: {error}"))
    })
}

#[derive(Debug, FromRow)]
struct InstructorPreferenceRow {
    id: Uuid,
    instructor_id: Uuid,
    academic_year_id: Uuid,
    hard_unavailable_slots: serde_json::Value,
    preferred_slots: serde_json::Value,
    max_periods_per_day: Option<i32>,
    min_periods_per_day: Option<i32>,
    preferred_days: serde_json::Value,
    avoid_days: serde_json::Value,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct InstructorRoomAssignmentRow {
    id: Uuid,
    instructor_id: Uuid,
    room_id: Uuid,
    academic_year_id: Uuid,
    is_preferred: Option<bool>,
    is_required: Option<bool>,
    for_subjects: serde_json::Value,
    reason: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct TimetableLockedSlotRow {
    id: Uuid,
    academic_semester_id: Uuid,
    scope_type: String,
    scope_ids: serde_json::Value,
    subject_id: Uuid,
    day_of_week: String,
    period_ids: serde_json::Value,
    room_id: Option<Uuid>,
    instructor_id: Option<Uuid>,
    reason: Option<String>,
    locked_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct TimetableSchedulingJobRow {
    id: Uuid,
    academic_semester_id: Uuid,
    classroom_ids: serde_json::Value,
    algorithm: String,
    config: serde_json::Value,
    status: String,
    progress: Option<i32>,
    quality_score: Option<f32>,
    scheduled_courses: Option<i32>,
    total_courses: Option<i32>,
    failed_courses: serde_json::Value,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    duration_seconds: Option<i32>,
    error_message: Option<String>,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn instructor_preference_from_row(
    row: InstructorPreferenceRow,
) -> Result<InstructorPreference, AppError> {
    Ok(InstructorPreference {
        id: row.id,
        instructor_id: row.instructor_id,
        academic_year_id: row.academic_year_id,
        hard_unavailable_slots: jsonb_or_default(
            row.hard_unavailable_slots,
            "instructor_preferences.hard_unavailable_slots",
        )?,
        preferred_slots: jsonb_or_default(
            row.preferred_slots,
            "instructor_preferences.preferred_slots",
        )?,
        max_periods_per_day: row.max_periods_per_day,
        min_periods_per_day: row.min_periods_per_day,
        preferred_days: jsonb_or_default(
            row.preferred_days,
            "instructor_preferences.preferred_days",
        )?,
        avoid_days: jsonb_or_default(row.avoid_days, "instructor_preferences.avoid_days")?,
        notes: row.notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn instructor_room_assignment_from_row(
    row: InstructorRoomAssignmentRow,
) -> Result<InstructorRoomAssignment, AppError> {
    Ok(InstructorRoomAssignment {
        id: row.id,
        instructor_id: row.instructor_id,
        room_id: row.room_id,
        academic_year_id: row.academic_year_id,
        is_preferred: row.is_preferred,
        is_required: row.is_required,
        for_subjects: jsonb_or_default(
            row.for_subjects,
            "instructor_room_assignments.for_subjects",
        )?,
        reason: row.reason,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn locked_slot_from_row(row: TimetableLockedSlotRow) -> Result<TimetableLockedSlot, AppError> {
    Ok(TimetableLockedSlot {
        id: row.id,
        academic_semester_id: row.academic_semester_id,
        scope_type: row.scope_type,
        scope_ids: optional_jsonb_vec(row.scope_ids, "timetable_locked_slots.scope_ids")?,
        subject_id: row.subject_id,
        day_of_week: row.day_of_week,
        period_ids: jsonb_or_default(row.period_ids, "timetable_locked_slots.period_ids")?,
        room_id: row.room_id,
        instructor_id: row.instructor_id,
        reason: row.reason,
        locked_by: row.locked_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn scheduling_job_from_row(
    row: TimetableSchedulingJobRow,
) -> Result<TimetableSchedulingJob, AppError> {
    Ok(TimetableSchedulingJob {
        id: row.id,
        academic_semester_id: row.academic_semester_id,
        classroom_ids: jsonb_or_default(
            row.classroom_ids,
            "timetable_scheduling_jobs.classroom_ids",
        )?,
        algorithm: row.algorithm,
        config: jsonb_or_default(row.config, "timetable_scheduling_jobs.config")?,
        status: row.status,
        progress: row.progress,
        quality_score: row.quality_score,
        scheduled_courses: row.scheduled_courses,
        total_courses: row.total_courses,
        failed_courses: jsonb_or_default(
            row.failed_courses,
            "timetable_scheduling_jobs.failed_courses",
        )?,
        started_at: row.started_at,
        completed_at: row.completed_at,
        duration_seconds: row.duration_seconds,
        error_message: row.error_message,
        created_by: row.created_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub fn scheduling_job_response(job: TimetableSchedulingJob) -> SchedulingJobResponse {
    SchedulingJobResponse {
        id: job.id,
        academic_semester_id: job.academic_semester_id,
        classroom_ids: job.classroom_ids,
        algorithm: match job.algorithm.as_str() {
            "GREEDY" => SchedulingAlgorithm::Greedy,
            "BACKTRACKING" => SchedulingAlgorithm::Backtracking,
            "HYBRID" => SchedulingAlgorithm::Hybrid,
            _ => SchedulingAlgorithm::Backtracking,
        },
        status: match job.status.as_str() {
            "PENDING" => SchedulingStatus::Pending,
            "RUNNING" => SchedulingStatus::Running,
            "COMPLETED" => SchedulingStatus::Completed,
            "FAILED" => SchedulingStatus::Failed,
            "CANCELLED" => SchedulingStatus::Cancelled,
            _ => SchedulingStatus::Pending,
        },
        progress: job.progress.unwrap_or(0),
        quality_score: job.quality_score.map(f64::from),
        scheduled_courses: job.scheduled_courses.unwrap_or(0),
        total_courses: job.total_courses.unwrap_or(0),
        failed_courses: job.failed_courses,
        started_at: job.started_at,
        completed_at: job.completed_at,
        duration_seconds: job.duration_seconds,
        error_message: job.error_message,
        created_by: job.created_by,
        created_at: job.created_at,
    }
}

fn scheduler_config_from_request(
    config: &SchedulingConfig,
    algorithm: crate::modules::academic::services::SchedulingAlgorithm,
) -> crate::modules::academic::services::scheduler::types::SchedulerConfig {
    crate::modules::academic::services::scheduler::types::SchedulerConfig {
        algorithm,
        timeout_seconds: config.timeout_seconds.unwrap_or(300),
        min_quality_score: config.min_quality_score.unwrap_or(70.0),
        allow_partial: config.allow_partial.unwrap_or(false),
        ..Default::default()
    }
}

pub async fn create_scheduling_job(
    pool: &PgPool,
    job_id: Uuid,
    semester_id: Uuid,
    classroom_ids: &[Uuid],
    algorithm_label: &str,
    config: &SchedulingConfig,
    user_id: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO timetable_scheduling_jobs
               (id, academic_semester_id, classroom_ids, algorithm, config, status, progress, created_by)
           VALUES ($1, $2, $3, $4::scheduling_algorithm, $5, 'PENDING'::scheduling_status, 0, $6)"#
    )
    .bind(job_id).bind(semester_id).bind(Json(classroom_ids))
    .bind(algorithm_label).bind(Json(config)).bind(user_id)
    .execute(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn get_scheduling_job(
    pool: &PgPool,
    job_id: Uuid,
) -> Result<TimetableSchedulingJob, AppError> {
    let row = sqlx::query_as::<_, TimetableSchedulingJobRow>(
        r#"SELECT id, academic_semester_id, classroom_ids, algorithm::TEXT, config,
                  status::TEXT, progress, quality_score::REAL, scheduled_courses, total_courses,
                  failed_courses, started_at, completed_at, duration_seconds,
                  error_message, created_by, created_at, updated_at
           FROM timetable_scheduling_jobs WHERE id = $1"#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or(AppError::NotFound("Job not found".to_string()))?;

    scheduling_job_from_row(row)
}

pub async fn undo_scheduling_job(
    pool: &PgPool,
    job_id: Uuid,
) -> Result<(Option<Uuid>, u64), AppError> {
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries WHERE scheduler_job_id = $1 LIMIT 1"
    )
    .bind(job_id).fetch_optional(pool).await.unwrap_or(None);

    let result = sqlx::query("DELETE FROM academic_timetable_entries WHERE scheduler_job_id = $1")
        .bind(job_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok((semester_id, result.rows_affected()))
}

pub async fn list_scheduling_jobs(
    pool: &PgPool,
    semester_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<TimetableSchedulingJob>, AppError> {
    let select_fields = "id, academic_semester_id, classroom_ids, algorithm::TEXT, config, status::TEXT, progress, quality_score::REAL, scheduled_courses, total_courses, failed_courses, started_at, completed_at, duration_seconds, error_message, created_by, created_at, updated_at";

    let result = if let Some(sid) = semester_id {
        let sql = format!("SELECT {} FROM timetable_scheduling_jobs WHERE academic_semester_id = $1 ORDER BY created_at DESC LIMIT $2", select_fields);
        sqlx::query_as::<_, TimetableSchedulingJobRow>(&sql)
            .bind(sid)
            .bind(limit)
            .fetch_all(pool)
            .await
    } else {
        let sql = format!(
            "SELECT {} FROM timetable_scheduling_jobs ORDER BY created_at DESC LIMIT $1",
            select_fields
        );
        sqlx::query_as::<_, TimetableSchedulingJobRow>(&sql)
            .bind(limit)
            .fetch_all(pool)
            .await
    };

    result
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .into_iter()
        .map(scheduling_job_from_row)
        .collect()
}

pub async fn create_instructor_preference(
    pool: &PgPool,
    payload: CreateInstructorPreferenceRequest,
) -> Result<InstructorPreference, AppError> {
    let hard_slots = payload.hard_unavailable_slots.unwrap_or_default();
    let pref_slots = payload.preferred_slots.unwrap_or_default();
    let pref_days = payload.preferred_days.unwrap_or_default();
    let avoid_days = payload.avoid_days.unwrap_or_default();

    let row = sqlx::query_as::<_, InstructorPreferenceRow>(
        r#"INSERT INTO instructor_preferences
               (instructor_id, academic_year_id, hard_unavailable_slots, preferred_slots,
                max_periods_per_day, min_periods_per_day, preferred_days, avoid_days, notes)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           ON CONFLICT (instructor_id, academic_year_id) DO UPDATE
           SET hard_unavailable_slots = EXCLUDED.hard_unavailable_slots,
               preferred_slots = EXCLUDED.preferred_slots,
               max_periods_per_day = EXCLUDED.max_periods_per_day,
               min_periods_per_day = EXCLUDED.min_periods_per_day,
               preferred_days = EXCLUDED.preferred_days,
               avoid_days = EXCLUDED.avoid_days,
               notes = EXCLUDED.notes, updated_at = NOW()
           RETURNING *"#,
    )
    .bind(payload.instructor_id)
    .bind(payload.academic_year_id)
    .bind(Json(hard_slots))
    .bind(Json(pref_slots))
    .bind(payload.max_periods_per_day)
    .bind(payload.min_periods_per_day)
    .bind(Json(pref_days))
    .bind(Json(avoid_days))
    .bind(payload.notes)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    instructor_preference_from_row(row)
}

pub async fn create_instructor_room_assignment(
    pool: &PgPool,
    payload: CreateInstructorRoomAssignmentRequest,
) -> Result<InstructorRoomAssignment, AppError> {
    let for_subjects = payload.for_subjects.unwrap_or_default();

    let row = sqlx::query_as::<_, InstructorRoomAssignmentRow>(
        r#"INSERT INTO instructor_room_assignments
               (instructor_id, room_id, academic_year_id, is_preferred, is_required, for_subjects, reason)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#
    )
    .bind(payload.instructor_id).bind(payload.room_id).bind(payload.academic_year_id)
    .bind(payload.is_preferred).bind(payload.is_required)
    .bind(Json(for_subjects)).bind(payload.reason)
    .fetch_one(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    instructor_room_assignment_from_row(row)
}

pub async fn create_locked_slot(
    pool: &PgPool,
    payload: CreateLockedSlotRequest,
    user_id: Option<Uuid>,
) -> Result<TimetableLockedSlot, AppError> {
    let scope_type = match payload.scope_type {
        LockedSlotScope::Classroom => "CLASSROOM",
        LockedSlotScope::GradeLevel => "GRADE_LEVEL",
        LockedSlotScope::AllSchool => "ALL_SCHOOL",
    };

    let row = sqlx::query_as::<_, TimetableLockedSlotRow>(
        r#"INSERT INTO timetable_locked_slots
               (academic_semester_id, scope_type, scope_ids, subject_id, day_of_week,
                period_ids, room_id, instructor_id, reason, locked_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING *"#,
    )
    .bind(payload.academic_semester_id)
    .bind(scope_type)
    .bind(Json(payload.scope_ids))
    .bind(payload.subject_id)
    .bind(payload.day_of_week)
    .bind(Json(payload.period_ids))
    .bind(payload.room_id)
    .bind(payload.instructor_id)
    .bind(payload.reason)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    locked_slot_from_row(row)
}

pub async fn list_locked_slots(
    pool: &PgPool,
    semester_id: Option<Uuid>,
) -> Result<Vec<TimetableLockedSlot>, AppError> {
    let result = if let Some(sid) = semester_id {
        sqlx::query_as::<_, TimetableLockedSlotRow>(
            "SELECT * FROM timetable_locked_slots WHERE academic_semester_id = $1",
        )
        .bind(sid)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, TimetableLockedSlotRow>("SELECT * FROM timetable_locked_slots")
            .fetch_all(pool)
            .await
    };

    result
        .map_err(|e| AppError::InternalServerError(e.to_string()))?
        .into_iter()
        .map(locked_slot_from_row)
        .collect()
}

pub async fn delete_locked_slot(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM timetable_locked_slots WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

/// Background scheduling task
pub async fn run_scheduling_job(
    job_id: Uuid,
    semester_id: Uuid,
    classroom_ids: Vec<Uuid>,
    algorithm: crate::modules::academic::services::SchedulingAlgorithm,
    config: SchedulingConfig,
    pool: &PgPool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sqlx::query(
        "UPDATE timetable_scheduling_jobs SET status = 'RUNNING', started_at = NOW(), updated_at = NOW() WHERE id = $1"
    ).bind(job_id).execute(pool).await?;

    let loader = SchedulerDataLoader::new(pool);

    let academic_year_id: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM academic_semesters WHERE id = $1")
            .bind(semester_id)
            .fetch_one(pool)
            .await?;

    let mut courses = loader.load_courses(&classroom_ids, semester_id).await?;
    let activities = loader
        .load_independent_activities(&classroom_ids, semester_id)
        .await?;
    courses.extend(activities);

    let available_slots = loader.load_available_slots(semester_id).await?;

    let mut locked_slots = loader
        .load_locked_slots(semester_id, &classroom_ids)
        .await?;
    let existing = loader
        .load_existing_entries_as_locked(semester_id, &classroom_ids)
        .await?;
    locked_slots.extend(existing);

    let instructor_prefs = loader.load_instructor_preferences(academic_year_id).await?;
    let default_max_consecutive = loader.load_default_max_consecutive().await.unwrap_or(4);

    sqlx::query(
        "UPDATE timetable_scheduling_jobs SET progress = 10, updated_at = NOW() WHERE id = $1",
    )
    .bind(job_id)
    .execute(pool)
    .await?;

    let scheduler_config = scheduler_config_from_request(&config, algorithm.clone());

    let scheduler = SchedulerBuilder::new()
        .algorithm(algorithm)
        .timeout_seconds(scheduler_config.timeout_seconds)
        .min_quality_score(scheduler_config.min_quality_score)
        .allow_partial(scheduler_config.allow_partial)
        .build();

    sqlx::query(
        "UPDATE timetable_scheduling_jobs SET progress = 20, updated_at = NOW() WHERE id = $1",
    )
    .bind(job_id)
    .execute(pool)
    .await?;

    let result = scheduler.schedule_with_settings(
        courses,
        available_slots,
        locked_slots,
        instructor_prefs,
        default_max_consecutive,
    );

    if config.force_overwrite.unwrap_or(false) {
        sqlx::query(
            "DELETE FROM academic_timetable_entries
             WHERE classroom_course_id IN (
                 SELECT id FROM classroom_courses
                 WHERE classroom_id = ANY($1) AND academic_semester_id = $2
             )",
        )
        .bind(&classroom_ids)
        .bind(semester_id)
        .execute(pool)
        .await?;
    }

    for assignment in &result.assignments {
        let inserted_id: Option<Uuid> = if let Some(slot_id) = assignment.activity_slot_id {
            sqlx::query_scalar(
                r#"INSERT INTO academic_timetable_entries
                       (id, classroom_id, academic_semester_id, day_of_week, period_id, room_id,
                        entry_type, activity_slot_id, is_active, scheduler_job_id)
                   VALUES ($1, $2, $3, $4, $5, $6, 'ACTIVITY', $7, true, $8)
                   ON CONFLICT (classroom_id, academic_semester_id, day_of_week, period_id) WHERE is_active = true
                   DO UPDATE
                   SET room_id = EXCLUDED.room_id, activity_slot_id = EXCLUDED.activity_slot_id,
                       entry_type = 'ACTIVITY', scheduler_job_id = EXCLUDED.scheduler_job_id,
                       updated_at = NOW()
                   RETURNING id"#
            )
            .bind(assignment.id).bind(assignment.classroom_id).bind(semester_id)
            .bind(&assignment.time_slot.day).bind(assignment.time_slot.period_id)
            .bind(assignment.room_id).bind(slot_id).bind(job_id)
            .fetch_optional(pool).await?
        } else {
            sqlx::query_scalar(
                r#"INSERT INTO academic_timetable_entries
                       (id, classroom_course_id, day_of_week, period_id, room_id,
                        classroom_id, academic_semester_id, entry_type, scheduler_job_id)
                   SELECT $1, $2, $3, $4, $5,
                          cc.classroom_id, cc.academic_semester_id, 'COURSE', $6
                   FROM classroom_courses cc WHERE cc.id = $2
                   ON CONFLICT (classroom_id, academic_semester_id, day_of_week, period_id) WHERE is_active = true
                   DO UPDATE
                   SET room_id = EXCLUDED.room_id, scheduler_job_id = EXCLUDED.scheduler_job_id, updated_at = NOW()
                   RETURNING id"#
            )
            .bind(assignment.id).bind(assignment.classroom_course_id)
            .bind(&assignment.time_slot.day).bind(assignment.time_slot.period_id)
            .bind(assignment.room_id).bind(job_id)
            .fetch_optional(pool).await?
        };

        if let Some(entry_id) = inserted_id {
            if let Some(slot_id) = assignment.activity_slot_id {
                sqlx::query(
                    "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                     SELECT $1, instructor_id, 'primary'
                     FROM activity_slot_classroom_assignments
                     WHERE slot_id = $2 AND classroom_id = $3
                     ON CONFLICT DO NOTHING",
                )
                .bind(entry_id)
                .bind(slot_id)
                .bind(assignment.classroom_id)
                .execute(pool)
                .await?;
            } else {
                sqlx::query(
                    "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                     SELECT $1, instructor_id, role FROM classroom_course_instructors
                     WHERE classroom_course_id = $2
                     ON CONFLICT DO NOTHING",
                )
                .bind(entry_id)
                .bind(assignment.classroom_course_id)
                .execute(pool)
                .await?;
            }
        }
    }

    sqlx::query(
        r#"UPDATE timetable_scheduling_jobs
           SET status = 'COMPLETED', progress = 100,
               quality_score = $1, scheduled_courses = $2, total_courses = $3,
               failed_courses = $4, completed_at = NOW(),
               duration_seconds = $5, updated_at = NOW()
           WHERE id = $6"#,
    )
    .bind(result.quality_score as f32)
    .bind(result.scheduled_courses as i32)
    .bind(result.total_courses as i32)
    .bind(Json(&result.failed_courses))
    .bind((result.duration_ms / 1000) as i32)
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_job_failed(pool: &PgPool, job_id: Uuid, error: String) {
    let _ = sqlx::query(
        "UPDATE timetable_scheduling_jobs SET status = 'FAILED', error_message = $1, updated_at = NOW() WHERE id = $2"
    ).bind(error).bind(job_id).execute(pool).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::services::SchedulingAlgorithm;

    #[test]
    fn scheduler_config_from_request_applies_defaults() {
        let config = SchedulingConfig {
            force_overwrite: None,
            respect_preferences: None,
            allow_partial: None,
            min_quality_score: None,
            timeout_seconds: None,
            weight_distribution: None,
            weight_consecutive: None,
            weight_time_of_day: None,
            weight_instructor_preference: None,
            weight_daily_load: None,
        };

        let scheduler_config =
            scheduler_config_from_request(&config, SchedulingAlgorithm::Backtracking);

        assert_eq!(scheduler_config.timeout_seconds, 300);
        assert_eq!(scheduler_config.min_quality_score, 70.0);
        assert!(!scheduler_config.allow_partial);
        assert_eq!(
            scheduler_config.algorithm,
            crate::modules::academic::services::scheduler::types::SchedulingAlgorithm::Backtracking
        );
    }

    #[test]
    fn scheduler_config_from_request_uses_explicit_values() {
        let config = SchedulingConfig {
            force_overwrite: Some(false),
            respect_preferences: Some(true),
            allow_partial: Some(true),
            min_quality_score: Some(85.0),
            timeout_seconds: Some(45),
            weight_distribution: None,
            weight_consecutive: None,
            weight_time_of_day: None,
            weight_instructor_preference: None,
            weight_daily_load: None,
        };

        let scheduler_config = scheduler_config_from_request(&config, SchedulingAlgorithm::Greedy);

        assert_eq!(scheduler_config.timeout_seconds, 45);
        assert_eq!(scheduler_config.min_quality_score, 85.0);
        assert!(scheduler_config.allow_partial);
        assert_eq!(
            scheduler_config.algorithm,
            crate::modules::academic::services::scheduler::types::SchedulingAlgorithm::Greedy
        );
    }
}
