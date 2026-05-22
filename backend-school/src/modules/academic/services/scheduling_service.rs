use crate::error::AppError;
use crate::modules::academic::models::scheduling::*;
use crate::modules::academic::services::scheduler_data::SchedulerDataLoader;
use crate::modules::academic::services::SchedulerBuilder;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create_scheduling_job(
    pool: &PgPool,
    job_id: Uuid,
    semester_id: Uuid,
    classroom_ids: &[Uuid],
    algorithm_label: &str,
    config: &SchedulingConfig,
    user_id: Option<Uuid>,
) -> Result<(), AppError> {
    let classroom_ids_json = serde_json::to_value(classroom_ids)
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let config_json = serde_json::to_value(config)
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query(
        r#"INSERT INTO timetable_scheduling_jobs
               (id, academic_semester_id, classroom_ids, algorithm, config, status, progress, created_by)
           VALUES ($1, $2, $3, $4::scheduling_algorithm, $5, 'PENDING'::scheduling_status, 0, $6)"#
    )
    .bind(job_id).bind(semester_id).bind(classroom_ids_json)
    .bind(algorithm_label).bind(config_json).bind(user_id)
    .execute(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn get_scheduling_job(pool: &PgPool, job_id: Uuid) -> Result<TimetableSchedulingJob, AppError> {
    sqlx::query_as::<_, TimetableSchedulingJob>(
        r#"SELECT id, academic_semester_id, classroom_ids, algorithm::TEXT, config,
                  status::TEXT, progress, quality_score::REAL, scheduled_courses, total_courses,
                  failed_courses, started_at, completed_at, duration_seconds,
                  error_message, created_by, created_at, updated_at
           FROM timetable_scheduling_jobs WHERE id = $1"#
    )
    .bind(job_id).fetch_optional(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .ok_or(AppError::NotFound("Job not found".to_string()))
}

pub async fn undo_scheduling_job(pool: &PgPool, job_id: Uuid) -> Result<(Option<Uuid>, u64), AppError> {
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries WHERE scheduler_job_id = $1 LIMIT 1"
    )
    .bind(job_id).fetch_optional(pool).await.unwrap_or(None);

    let result = sqlx::query("DELETE FROM academic_timetable_entries WHERE scheduler_job_id = $1")
        .bind(job_id).execute(pool).await
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
        sqlx::query_as::<_, TimetableSchedulingJob>(&sql)
            .bind(sid).bind(limit).fetch_all(pool).await
    } else {
        let sql = format!("SELECT {} FROM timetable_scheduling_jobs ORDER BY created_at DESC LIMIT $1", select_fields);
        sqlx::query_as::<_, TimetableSchedulingJob>(&sql)
            .bind(limit).fetch_all(pool).await
    };
    result.map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn create_instructor_preference(pool: &PgPool, payload: CreateInstructorPreferenceRequest) -> Result<InstructorPreference, AppError> {
    let hard_slots = serde_json::to_value(payload.hard_unavailable_slots.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let pref_slots = serde_json::to_value(payload.preferred_slots.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let pref_days = serde_json::to_value(payload.preferred_days.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let avoid_days = serde_json::to_value(payload.avoid_days.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query_as::<_, InstructorPreference>(
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
           RETURNING *"#
    )
    .bind(payload.instructor_id).bind(payload.academic_year_id)
    .bind(hard_slots).bind(pref_slots)
    .bind(payload.max_periods_per_day).bind(payload.min_periods_per_day)
    .bind(pref_days).bind(avoid_days).bind(payload.notes)
    .fetch_one(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn create_instructor_room_assignment(pool: &PgPool, payload: CreateInstructorRoomAssignmentRequest) -> Result<InstructorRoomAssignment, AppError> {
    let for_subjects = serde_json::to_value(payload.for_subjects.unwrap_or_default())
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query_as::<_, InstructorRoomAssignment>(
        r#"INSERT INTO instructor_room_assignments
               (instructor_id, room_id, academic_year_id, is_preferred, is_required, for_subjects, reason)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#
    )
    .bind(payload.instructor_id).bind(payload.room_id).bind(payload.academic_year_id)
    .bind(payload.is_preferred).bind(payload.is_required)
    .bind(for_subjects).bind(payload.reason)
    .fetch_one(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn create_locked_slot(pool: &PgPool, payload: CreateLockedSlotRequest, user_id: Option<Uuid>) -> Result<TimetableLockedSlot, AppError> {
    let scope_type = match payload.scope_type {
        LockedSlotScope::Classroom => "CLASSROOM",
        LockedSlotScope::GradeLevel => "GRADE_LEVEL",
        LockedSlotScope::AllSchool => "ALL_SCHOOL",
    };
    let scope_ids = serde_json::to_value(&payload.scope_ids)
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let period_ids = serde_json::to_value(&payload.period_ids)
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query_as::<_, TimetableLockedSlot>(
        r#"INSERT INTO timetable_locked_slots
               (academic_semester_id, scope_type, scope_ids, subject_id, day_of_week,
                period_ids, room_id, instructor_id, reason, locked_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING *"#
    )
    .bind(payload.academic_semester_id).bind(scope_type).bind(scope_ids)
    .bind(payload.subject_id).bind(payload.day_of_week).bind(period_ids)
    .bind(payload.room_id).bind(payload.instructor_id)
    .bind(payload.reason).bind(user_id)
    .fetch_one(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn list_locked_slots(pool: &PgPool, semester_id: Option<Uuid>) -> Result<Vec<TimetableLockedSlot>, AppError> {
    let result = if let Some(sid) = semester_id {
        sqlx::query_as::<_, TimetableLockedSlot>(
            "SELECT * FROM timetable_locked_slots WHERE academic_semester_id = $1"
        ).bind(sid).fetch_all(pool).await
    } else {
        sqlx::query_as::<_, TimetableLockedSlot>("SELECT * FROM timetable_locked_slots").fetch_all(pool).await
    };
    result.map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn delete_locked_slot(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM timetable_locked_slots WHERE id = $1")
        .bind(id).execute(pool).await
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

    let academic_year_id: Uuid = sqlx::query_scalar(
        "SELECT academic_year_id FROM academic_semesters WHERE id = $1"
    ).bind(semester_id).fetch_one(pool).await?;

    let mut courses = loader.load_courses(&classroom_ids, semester_id).await?;
    let activities = loader.load_independent_activities(&classroom_ids, semester_id).await?;
    courses.extend(activities);

    let available_slots = loader.load_available_slots(semester_id).await?;
    let periods = loader.load_periods(academic_year_id).await?;

    let mut locked_slots = loader.load_locked_slots(semester_id, &classroom_ids).await?;
    let existing = loader.load_existing_entries_as_locked(semester_id, &classroom_ids).await?;
    locked_slots.extend(existing);

    let instructor_prefs = loader.load_instructor_preferences(academic_year_id).await?;
    let rooms = loader.load_rooms().await?;
    let default_max_consecutive = loader.load_default_max_consecutive().await.unwrap_or(4);

    sqlx::query("UPDATE timetable_scheduling_jobs SET progress = 10, updated_at = NOW() WHERE id = $1")
        .bind(job_id).execute(pool).await?;

    let mut scheduler_config = crate::modules::academic::services::scheduler::types::SchedulerConfig::default();
    scheduler_config.algorithm = algorithm.clone();
    scheduler_config.timeout_seconds = config.timeout_seconds.unwrap_or(300);
    scheduler_config.min_quality_score = config.min_quality_score.unwrap_or(70.0);
    scheduler_config.allow_partial = config.allow_partial.unwrap_or(false);
    scheduler_config.force_overwrite = config.force_overwrite.unwrap_or(false);

    let scheduler = SchedulerBuilder::new()
        .algorithm(algorithm)
        .timeout_seconds(scheduler_config.timeout_seconds)
        .min_quality_score(scheduler_config.min_quality_score)
        .allow_partial(scheduler_config.allow_partial)
        .build();

    sqlx::query("UPDATE timetable_scheduling_jobs SET progress = 20, updated_at = NOW() WHERE id = $1")
        .bind(job_id).execute(pool).await?;

    let result = scheduler.schedule_with_settings(
        courses, available_slots, locked_slots, instructor_prefs, periods, rooms, default_max_consecutive,
    );

    if config.force_overwrite.unwrap_or(false) {
        sqlx::query(
            "DELETE FROM academic_timetable_entries
             WHERE classroom_course_id IN (
                 SELECT id FROM classroom_courses
                 WHERE classroom_id = ANY($1) AND academic_semester_id = $2
             )"
        )
        .bind(&classroom_ids).bind(semester_id).execute(pool).await?;
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
                     ON CONFLICT DO NOTHING"
                )
                .bind(entry_id).bind(slot_id).bind(assignment.classroom_id)
                .execute(pool).await?;
            } else {
                sqlx::query(
                    "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
                     SELECT $1, instructor_id, role FROM classroom_course_instructors
                     WHERE classroom_course_id = $2
                     ON CONFLICT DO NOTHING"
                )
                .bind(entry_id).bind(assignment.classroom_course_id)
                .execute(pool).await?;
            }
        }
    }

    let failed_courses_json = serde_json::to_value(&result.failed_courses)?;
    sqlx::query(
        r#"UPDATE timetable_scheduling_jobs
           SET status = 'COMPLETED', progress = 100,
               quality_score = $1, scheduled_courses = $2, total_courses = $3,
               failed_courses = $4, completed_at = NOW(),
               duration_seconds = $5, updated_at = NOW()
           WHERE id = $6"#
    )
    .bind(result.quality_score as f32)
    .bind(result.scheduled_courses as i32)
    .bind(result.total_courses as i32)
    .bind(failed_courses_json)
    .bind((result.duration_ms / 1000) as i32)
    .bind(job_id).execute(pool).await?;
    Ok(())
}

pub async fn mark_job_failed(pool: &PgPool, job_id: Uuid, error: String) {
    let _ = sqlx::query(
        "UPDATE timetable_scheduling_jobs SET status = 'FAILED', error_message = $1, updated_at = NOW() WHERE id = $2"
    ).bind(error).bind(job_id).execute(pool).await;
}
