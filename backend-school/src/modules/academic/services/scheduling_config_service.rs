use crate::error::AppError;
use crate::modules::academic::models::scheduling::TimeSlot;
use crate::modules::academic::models::scheduling_config::{
    CcPreferredRoomView, ClassroomCourseConstraintView, InstructorConstraintView, Patch,
    SaveSchedulingConfigurationRequest, SchedulerSettingsView, SchedulingConfigurationSaveResult,
    SchedulingRoomView, SubjectConstraintView,
};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct InstructorConstraintRow {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub hard_unavailable_slots: Option<Json<Vec<TimeSlot>>>,
    pub max_periods_per_day: Option<i32>,
    pub min_periods_per_day: Option<i32>,
    pub assigned_room_id: Option<Uuid>,
    pub assigned_room_name: Option<String>,
    pub priority: i32,
    pub primary_course_count: i64,
}

#[derive(sqlx::FromRow)]
struct SubjectConstraintRow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub min_consecutive_periods: i32,
    pub max_consecutive_periods: Option<i32>,
    pub allow_single_period: Option<bool>,
    pub periods_per_week: Option<i32>,
    pub allowed_period_ids: Option<Json<Vec<Uuid>>>,
    pub allowed_days: Option<Json<Vec<String>>>,
}

#[derive(sqlx::FromRow)]
struct ClassroomCourseConstraintRow {
    pub id: Uuid,
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub subject_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    pub periods_per_week: Option<i32>,
    pub primary_instructor_id: Option<Uuid>,
    pub primary_instructor_name: Option<String>,
    pub consecutive_pattern: Option<Json<Vec<i32>>>,
    pub same_day_unique: bool,
    pub hard_unavailable_slots: Option<Json<Vec<TimeSlot>>>,
    pub team_unavailable_slots: Option<Json<Vec<TimeSlot>>>,
}

fn json_vec_or_default<T>(value: Option<Json<Vec<T>>>) -> Vec<T> {
    value.map(|Json(values)| values).unwrap_or_default()
}

fn instructor_constraint_view_from_row(
    row: InstructorConstraintRow,
) -> Result<InstructorConstraintView, AppError> {
    Ok(InstructorConstraintView {
        id: row.id,
        first_name: row.first_name,
        last_name: row.last_name,
        hard_unavailable_slots: row.hard_unavailable_slots.map(|Json(values)| values),
        max_periods_per_day: row.max_periods_per_day,
        min_periods_per_day: row.min_periods_per_day,
        assigned_room_id: row.assigned_room_id,
        assigned_room_name: row.assigned_room_name,
        priority: row.priority,
        primary_course_count: row.primary_course_count,
    })
}

fn subject_constraint_view_from_row(
    row: SubjectConstraintRow,
) -> Result<SubjectConstraintView, AppError> {
    Ok(SubjectConstraintView {
        id: row.id,
        code: row.code,
        name: row.name,
        min_consecutive_periods: row.min_consecutive_periods,
        max_consecutive_periods: row.max_consecutive_periods,
        allow_single_period: row.allow_single_period,
        periods_per_week: row.periods_per_week,
        allowed_period_ids: row.allowed_period_ids.map(|Json(values)| values),
        allowed_days: row.allowed_days.map(|Json(values)| values),
    })
}

fn classroom_course_constraint_view_from_row(
    row: ClassroomCourseConstraintRow,
) -> Result<ClassroomCourseConstraintView, AppError> {
    Ok(ClassroomCourseConstraintView {
        id: row.id,
        classroom_id: row.classroom_id,
        classroom_name: row.classroom_name,
        subject_id: row.subject_id,
        subject_code: row.subject_code,
        subject_name: row.subject_name,
        periods_per_week: row.periods_per_week,
        primary_instructor_id: row.primary_instructor_id,
        primary_instructor_name: row.primary_instructor_name,
        consecutive_pattern: row.consecutive_pattern.map(|Json(values)| values),
        same_day_unique: row.same_day_unique,
        hard_unavailable_slots: json_vec_or_default(row.hard_unavailable_slots),
        team_unavailable_slots: json_vec_or_default(row.team_unavailable_slots),
    })
}

pub async fn get_active_year_id(pool: &PgPool) -> Result<Uuid, AppError> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM academic_years WHERE is_active = true LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    id.ok_or_else(|| AppError::NotFound("Active academic year not found".to_string()))
}

pub async fn list_classroom_course_constraints(
    pool: &PgPool,
    instructor_id: Option<Uuid>,
) -> Result<Vec<ClassroomCourseConstraintView>, AppError> {
    let year_id = get_active_year_id(pool).await?;

    let mut sql = String::from(
        r#"WITH team_unavail AS (
            SELECT cci.classroom_course_id,
                   COALESCE(jsonb_agg(elem) FILTER (WHERE elem IS NOT NULL), '[]'::jsonb) AS slots
            FROM classroom_course_instructors cci
            JOIN classroom_courses cc2 ON cc2.id = cci.classroom_course_id
            JOIN academic_semesters sem2 ON sem2.id = cc2.academic_semester_id
            LEFT JOIN instructor_preferences ip2
                ON ip2.instructor_id = cci.instructor_id
                AND ip2.academic_year_id = sem2.academic_year_id
            LEFT JOIN LATERAL jsonb_array_elements(COALESCE(ip2.hard_unavailable_slots, '[]'::jsonb)) elem ON true
            WHERE sem2.academic_year_id = $1
            GROUP BY cci.classroom_course_id
        ),
        primary_instr AS (
            SELECT cci.classroom_course_id, cci.instructor_id
            FROM classroom_course_instructors cci
            WHERE cci.role = 'primary'
        )
        SELECT cc.id, cc.classroom_id, cls.name AS classroom_name,
               cc.subject_id, s.code AS subject_code, s.name_th AS subject_name,
               COALESCE(s.periods_per_week,
                   CASE WHEN s.hours_per_semester > 0 THEN CEIL(s.hours_per_semester::float / 20.0)::int
                        WHEN s.credit > 0 THEN CEIL(s.credit * 2.0)::int
                        ELSE 2 END
               ) AS periods_per_week,
               pi.instructor_id AS primary_instructor_id,
               CASE WHEN u.id IS NOT NULL THEN u.first_name || ' ' || u.last_name ELSE NULL END AS primary_instructor_name,
               cc.consecutive_pattern, cc.same_day_unique, cc.hard_unavailable_slots,
               COALESCE(tu.slots, '[]'::jsonb) AS team_unavailable_slots
        FROM classroom_courses cc
        JOIN class_rooms cls ON cls.id = cc.classroom_id
        JOIN subjects s ON s.id = cc.subject_id
        JOIN academic_semesters sem ON sem.id = cc.academic_semester_id
        LEFT JOIN primary_instr pi ON pi.classroom_course_id = cc.id
        LEFT JOIN users u ON u.id = pi.instructor_id
        LEFT JOIN team_unavail tu ON tu.classroom_course_id = cc.id
        WHERE sem.academic_year_id = $1"#,
    );

    if instructor_id.is_some() {
        sql.push_str(" AND pi.instructor_id = $2");
    }
    sql.push_str(" ORDER BY cls.name, s.code");

    let q = sqlx::query_as::<_, ClassroomCourseConstraintRow>(&sql).bind(year_id);
    let result = if let Some(iid) = instructor_id {
        q.bind(iid).fetch_all(pool).await
    } else {
        q.fetch_all(pool).await
    };
    let rows = result.map_err(|e| AppError::InternalServerError(e.to_string()))?;
    rows.into_iter()
        .map(classroom_course_constraint_view_from_row)
        .collect()
}

pub async fn list_cc_preferred_rooms(
    pool: &PgPool,
    cc_id: Uuid,
) -> Result<Vec<CcPreferredRoomView>, AppError> {
    sqlx::query_as::<_, CcPreferredRoomView>(
        r#"SELECT pr.id, pr.classroom_course_id, pr.room_id,
                  r.code AS room_code, r.name_th AS room_name,
                  pr.rank, pr.is_required
           FROM classroom_course_preferred_rooms pr
           JOIN rooms r ON r.id = pr.room_id
           WHERE pr.classroom_course_id = $1
           ORDER BY pr.rank ASC"#,
    )
    .bind(cc_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn list_all_rooms(pool: &PgPool) -> Result<Vec<SchedulingRoomView>, AppError> {
    sqlx::query_as::<_, SchedulingRoomView>(
        "SELECT id, code, name_th, room_type FROM rooms WHERE status = 'ACTIVE' ORDER BY code",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn list_instructor_constraints(
    pool: &PgPool,
) -> Result<Vec<InstructorConstraintView>, AppError> {
    let year_id = get_active_year_id(pool).await?;
    let rows = sqlx::query_as::<_, InstructorConstraintRow>(
        r#"WITH primary_counts AS (
            SELECT cci.instructor_id, COUNT(*)::bigint AS cnt
            FROM classroom_course_instructors cci
            JOIN classroom_courses cc ON cc.id = cci.classroom_course_id
            JOIN academic_semesters s ON s.id = cc.academic_semester_id
            WHERE cci.role = 'primary' AND s.academic_year_id = $1
            GROUP BY cci.instructor_id
        )
        SELECT u.id, u.first_name, u.last_name,
               ip.hard_unavailable_slots, ip.max_periods_per_day, ip.min_periods_per_day,
               ra.room_id AS assigned_room_id, r.name_th AS assigned_room_name,
               COALESCE(ip.priority, 100) AS priority,
               COALESCE(pc.cnt, 0) AS primary_course_count
        FROM users u
        LEFT JOIN instructor_preferences ip ON u.id = ip.instructor_id AND ip.academic_year_id = $1
        LEFT JOIN instructor_room_assignments ra ON u.id = ra.instructor_id AND ra.academic_year_id = $1 AND ra.is_required = true AND ra.subject_id IS NULL
        LEFT JOIN rooms r ON ra.room_id = r.id
        LEFT JOIN primary_counts pc ON pc.instructor_id = u.id
        WHERE u.user_type = 'staff' AND u.status = 'active'
        ORDER BY COALESCE(ip.priority, 100), u.first_name"#
    )
    .bind(year_id).fetch_all(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    rows.into_iter()
        .map(instructor_constraint_view_from_row)
        .collect()
}

pub async fn get_scheduler_settings(pool: &PgPool) -> Result<SchedulerSettingsView, AppError> {
    let value: Option<Json<i32>> = sqlx::query_scalar(
        "SELECT value FROM scheduler_settings WHERE key = 'default_max_consecutive'",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(SchedulerSettingsView {
        default_max_consecutive: scheduler_default_max_consecutive(
            value.map(|Json(setting)| setting),
        ),
    })
}

pub async fn list_subject_constraints(
    pool: &PgPool,
) -> Result<Vec<SubjectConstraintView>, AppError> {
    let rows = sqlx::query_as::<_, SubjectConstraintRow>(
        r#"SELECT id, code, name_th as name,
                  COALESCE(min_consecutive_periods, 1) as min_consecutive_periods,
                  max_consecutive_periods, allow_single_period, periods_per_week,
                  allowed_period_ids, allowed_days
           FROM subjects WHERE is_active = true ORDER BY code"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    rows.into_iter()
        .map(subject_constraint_view_from_row)
        .collect()
}

#[derive(sqlx::FromRow)]
struct InstructorPreferenceState {
    hard_unavailable_slots: Option<Json<Vec<TimeSlot>>>,
    max_periods_per_day: Option<i32>,
    preferred_slots: Option<Json<Vec<TimeSlot>>>,
}

#[derive(sqlx::FromRow)]
struct SubjectConstraintState {
    min_consecutive_periods: Option<i32>,
    max_consecutive_periods: Option<i32>,
    allow_single_period: Option<bool>,
    allowed_period_ids: Option<Json<Vec<Uuid>>>,
    allowed_days: Option<Json<Vec<String>>>,
}

#[derive(sqlx::FromRow)]
struct ClassroomCourseConstraintState {
    consecutive_pattern: Option<Json<Vec<i32>>>,
    same_day_unique: bool,
    hard_unavailable_slots: Option<Json<Vec<TimeSlot>>>,
}

#[derive(sqlx::FromRow)]
struct ClassroomCourseValidationRow {
    id: Uuid,
    periods_per_week: Option<i32>,
    hours_per_semester: Option<i32>,
    credit: f64,
}

fn write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database_error) = &error {
        if matches!(
            database_error.code().as_deref(),
            Some("23503" | "23505" | "40001" | "40P01")
        ) {
            return AppError::Conflict(
                "Scheduling configuration changed concurrently; please reload and retry"
                    .to_string(),
            );
        }
    }
    AppError::InternalServerError(error.to_string())
}

fn ensure_unique_ids<I>(ids: I, label: &str) -> Result<(), AppError>
where
    I: IntoIterator<Item = Uuid>,
{
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(AppError::BadRequest(format!(
                "Duplicate {label} target: {id}"
            )));
        }
    }
    Ok(())
}

fn validate_range(value: i32, label: &str, min: i32, max: i32) -> Result<(), AppError> {
    if !(min..=max).contains(&value) {
        return Err(AppError::BadRequest(format!(
            "{label} must be between {min} and {max}"
        )));
    }
    Ok(())
}

fn validate_slots(slots: &[TimeSlot], period_ids: &mut HashSet<Uuid>) -> Result<(), AppError> {
    let valid_days = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let mut seen = HashSet::new();
    for slot in slots {
        if !valid_days.contains(&slot.day.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid scheduling day: {}",
                slot.day
            )));
        }
        if !seen.insert((slot.day.as_str(), slot.period_id)) {
            return Err(AppError::BadRequest(
                "Duplicate time slot in one constraint".to_string(),
            ));
        }
        period_ids.insert(slot.period_id);
    }
    Ok(())
}

fn effective_periods_per_week(
    periods_per_week: Option<i32>,
    hours_per_semester: Option<i32>,
    credit: f64,
) -> i32 {
    periods_per_week.unwrap_or_else(|| {
        if hours_per_semester.unwrap_or_default() > 0 {
            (f64::from(hours_per_semester.unwrap_or_default()) / 20.0).ceil() as i32
        } else if credit > 0.0 {
            (credit * 2.0).ceil() as i32
        } else {
            2
        }
    })
}

async fn validate_reference_count(
    tx: &mut Transaction<'_, Postgres>,
    sql: &str,
    ids: &[Uuid],
    label: &str,
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    let count: i64 = sqlx::query_scalar(sql)
        .bind(ids)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    if count != ids.len() as i64 {
        return Err(AppError::NotFound(format!(
            "One or more active {label} targets were not found"
        )));
    }
    Ok(())
}

async fn validate_configuration_request(
    tx: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    request: &SaveSchedulingConfigurationRequest,
) -> Result<HashMap<Uuid, i32>, AppError> {
    ensure_unique_ids(
        request.instructors.iter().map(|patch| patch.id),
        "instructor",
    )?;
    ensure_unique_ids(request.subjects.iter().map(|patch| patch.id), "subject")?;
    ensure_unique_ids(
        request.classroom_courses.iter().map(|patch| patch.id),
        "classroom course",
    )?;
    ensure_unique_ids(
        request
            .preferred_rooms
            .iter()
            .map(|patch| patch.classroom_course_id),
        "preferred-room classroom course",
    )?;

    if let Some(settings) = &request.scheduler_settings {
        if let Patch::Set(value) = settings.default_max_consecutive {
            validate_range(value, "default_max_consecutive", 1, 20)?;
        }
    }

    let mut instructor_ids: HashSet<Uuid> =
        request.instructors.iter().map(|patch| patch.id).collect();
    if let Patch::Set(ids) = &request.instructor_order {
        ensure_unique_ids(ids.iter().copied(), "instructor order")?;
        instructor_ids.extend(ids.iter().copied());
    }

    let mut period_ids = HashSet::new();
    let mut room_ids = HashSet::new();
    for patch in &request.instructors {
        if let Patch::Set(value) = patch.max_periods_per_day {
            validate_range(value, "max_periods_per_day", 1, 20)?;
        }
        for slots in [&patch.hard_unavailable_slots, &patch.preferred_slots] {
            if let Patch::Set(slots) = slots {
                validate_slots(slots, &mut period_ids)?;
            }
        }
        if let Patch::Set(room_id) = patch.assigned_room_id {
            room_ids.insert(room_id);
        }
    }

    for patch in &request.subjects {
        if let Patch::Set(value) = patch.min_consecutive_periods {
            validate_range(value, "min_consecutive_periods", 1, 20)?;
        }
        if let Patch::Set(value) = patch.max_consecutive_periods {
            validate_range(value, "max_consecutive_periods", 1, 20)?;
        }
        if let Patch::Set(days) = &patch.allowed_days {
            let valid_days = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
            let mut seen = HashSet::new();
            for day in days {
                if !valid_days.contains(&day.as_str()) {
                    return Err(AppError::BadRequest(format!(
                        "Invalid scheduling day: {day}"
                    )));
                }
                if !seen.insert(day) {
                    return Err(AppError::BadRequest(
                        "Duplicate day in subject constraint".to_string(),
                    ));
                }
            }
        }
        if let Patch::Set(ids) = &patch.allowed_period_ids {
            ensure_unique_ids(ids.iter().copied(), "allowed period")?;
            period_ids.extend(ids.iter().copied());
        }
    }

    for patch in &request.classroom_courses {
        if let Patch::Set(pattern) = &patch.consecutive_pattern {
            consecutive_pattern_sum(pattern)?;
        }
        if let Patch::Set(slots) = &patch.hard_unavailable_slots {
            validate_slots(slots, &mut period_ids)?;
        }
    }

    for patch in &request.preferred_rooms {
        let mut rooms_seen = HashSet::new();
        let mut ranks_seen = HashSet::new();
        for room in &patch.rooms {
            validate_range(room.rank, "preferred room rank", 1, 100)?;
            if !rooms_seen.insert(room.room_id) {
                return Err(AppError::BadRequest(
                    "Duplicate room in preferred-room set".to_string(),
                ));
            }
            if !ranks_seen.insert(room.rank) {
                return Err(AppError::BadRequest(
                    "Duplicate rank in preferred-room set".to_string(),
                ));
            }
            room_ids.insert(room.room_id);
        }
    }

    let instructor_ids: Vec<_> = instructor_ids.into_iter().collect();
    validate_reference_count(
        tx,
        "SELECT COUNT(DISTINCT id) FROM users WHERE id = ANY($1) AND user_type = 'staff' AND status = 'active'",
        &instructor_ids,
        "instructor",
    )
    .await?;
    let subject_ids: Vec<_> = request.subjects.iter().map(|patch| patch.id).collect();
    validate_reference_count(
        tx,
        "SELECT COUNT(DISTINCT id) FROM subjects WHERE id = ANY($1) AND is_active = true",
        &subject_ids,
        "subject",
    )
    .await?;
    let room_ids: Vec<_> = room_ids.into_iter().collect();
    validate_reference_count(
        tx,
        "SELECT COUNT(DISTINCT id) FROM rooms WHERE id = ANY($1) AND status = 'ACTIVE'",
        &room_ids,
        "room",
    )
    .await?;
    let period_ids: Vec<_> = period_ids.into_iter().collect();
    if !period_ids.is_empty() {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT id) FROM academic_periods WHERE id = ANY($1) AND academic_year_id = $2",
        )
        .bind(&period_ids)
        .bind(academic_year_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
        if count != period_ids.len() as i64 {
            return Err(AppError::NotFound(
                "One or more periods do not belong to the active academic year".to_string(),
            ));
        }
    }

    let mut classroom_course_ids: HashSet<Uuid> = request
        .classroom_courses
        .iter()
        .map(|patch| patch.id)
        .collect();
    classroom_course_ids.extend(
        request
            .preferred_rooms
            .iter()
            .map(|patch| patch.classroom_course_id),
    );
    let classroom_course_ids: Vec<_> = classroom_course_ids.into_iter().collect();
    let course_rows = if classroom_course_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, ClassroomCourseValidationRow>(
            r#"SELECT cc.id, s.periods_per_week, s.hours_per_semester, s.credit
               FROM classroom_courses cc
               JOIN academic_semesters sem ON sem.id = cc.academic_semester_id
               JOIN subjects s ON s.id = cc.subject_id
               WHERE cc.id = ANY($1) AND sem.academic_year_id = $2"#,
        )
        .bind(&classroom_course_ids)
        .bind(academic_year_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?
    };
    if course_rows.len() != classroom_course_ids.len() {
        return Err(AppError::NotFound(
            "One or more classroom courses do not belong to the active academic year".to_string(),
        ));
    }
    let periods_by_course: HashMap<_, _> = course_rows
        .into_iter()
        .map(|row| {
            (
                row.id,
                effective_periods_per_week(
                    row.periods_per_week,
                    row.hours_per_semester,
                    row.credit,
                ),
            )
        })
        .collect();
    for patch in &request.classroom_courses {
        if let Patch::Set(pattern) = &patch.consecutive_pattern {
            let sum = consecutive_pattern_sum(pattern)?;
            validate_consecutive_sum_matches_periods_per_week(
                sum,
                periods_by_course.get(&patch.id).copied(),
            )?;
        }
    }

    Ok(periods_by_course)
}

fn patched_json_vec<T: Clone>(
    patch: &Patch<Vec<T>>,
    current: Option<Json<Vec<T>>>,
    default: Vec<T>,
) -> Option<Json<Vec<T>>> {
    match patch {
        Patch::Unchanged => current.or_else(|| Some(Json(default))),
        Patch::Clear => Some(Json(Vec::new())),
        Patch::Set(values) => Some(Json(values.clone())),
    }
}

pub async fn save_scheduling_configuration(
    pool: &PgPool,
    request: SaveSchedulingConfigurationRequest,
) -> Result<SchedulingConfigurationSaveResult, AppError> {
    let mut tx = pool.begin().await.map_err(write_error)?;
    let academic_year_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_years WHERE is_active = true ORDER BY year DESC, id LIMIT 1 FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| AppError::InternalServerError(error.to_string()))?;
    let academic_year_id = academic_year_id
        .ok_or_else(|| AppError::NotFound("Active academic year not found".to_string()))?;

    validate_configuration_request(&mut tx, academic_year_id, &request).await?;

    let mut result = SchedulingConfigurationSaveResult::default();

    if let Some(settings) = &request.scheduler_settings {
        let value = match settings.default_max_consecutive {
            Patch::Unchanged => None,
            Patch::Clear => Some(4),
            Patch::Set(value) => Some(value),
        };
        if let Some(value) = value {
            let changed = sqlx::query(
                r#"INSERT INTO scheduler_settings (key, value)
                   VALUES ('default_max_consecutive', $1)
                   ON CONFLICT (key) DO UPDATE
                   SET value = EXCLUDED.value, updated_at = NOW()
                   WHERE scheduler_settings.value IS DISTINCT FROM EXCLUDED.value"#,
            )
            .bind(Json(value))
            .execute(&mut *tx)
            .await
            .map_err(write_error)?
            .rows_affected();
            result.scheduler_settings_changed = changed > 0;
        }
    }

    match &request.instructor_order {
        Patch::Unchanged => {}
        Patch::Clear => {
            result.instructor_order_updated = sqlx::query(
                "UPDATE instructor_preferences SET priority = 100, updated_at = NOW() WHERE academic_year_id = $1 AND priority IS DISTINCT FROM 100",
            )
            .bind(academic_year_id)
            .execute(&mut *tx)
            .await
            .map_err(write_error)?
            .rows_affected() as usize;
        }
        Patch::Set(instructor_ids) => {
            result.instructor_order_updated += sqlx::query(
                "UPDATE instructor_preferences SET priority = 100, updated_at = NOW() WHERE academic_year_id = $1 AND NOT (instructor_id = ANY($2)) AND priority IS DISTINCT FROM 100",
            )
            .bind(academic_year_id)
            .bind(instructor_ids)
            .execute(&mut *tx)
            .await
            .map_err(write_error)?
            .rows_affected() as usize;
            for (index, instructor_id) in instructor_ids.iter().enumerate() {
                result.instructor_order_updated += sqlx::query(
                    r#"INSERT INTO instructor_preferences (instructor_id, academic_year_id, priority)
                       VALUES ($1, $2, $3)
                       ON CONFLICT (instructor_id, academic_year_id) DO UPDATE
                       SET priority = EXCLUDED.priority, updated_at = NOW()
                       WHERE instructor_preferences.priority IS DISTINCT FROM EXCLUDED.priority"#,
                )
                .bind(instructor_id)
                .bind(academic_year_id)
                .bind((index + 1) as i32)
                .execute(&mut *tx)
                .await
                .map_err(write_error)?
                .rows_affected() as usize;
            }
        }
    }

    for patch in &request.instructors {
        let preference_patch_present = !matches!(patch.hard_unavailable_slots, Patch::Unchanged)
            || !matches!(patch.max_periods_per_day, Patch::Unchanged)
            || !matches!(patch.preferred_slots, Patch::Unchanged);
        let current = sqlx::query_as::<_, InstructorPreferenceState>(
            "SELECT hard_unavailable_slots, max_periods_per_day, preferred_slots FROM instructor_preferences WHERE instructor_id = $1 AND academic_year_id = $2",
        )
        .bind(patch.id)
        .bind(academic_year_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;

        let mut row_changed = false;
        if preference_patch_present {
            let existed = current.is_some();
            let current = current.unwrap_or(InstructorPreferenceState {
                hard_unavailable_slots: None,
                max_periods_per_day: None,
                preferred_slots: None,
            });
            let hard_unavailable_slots = patched_json_vec(
                &patch.hard_unavailable_slots,
                current.hard_unavailable_slots,
                Vec::new(),
            );
            let max_periods_per_day = match patch.max_periods_per_day {
                Patch::Unchanged => current.max_periods_per_day.or(Some(7)),
                Patch::Clear => Some(7),
                Patch::Set(value) => Some(value),
            };
            let preferred_slots =
                patched_json_vec(&patch.preferred_slots, current.preferred_slots, Vec::new());
            let changed = sqlx::query(
                r#"INSERT INTO instructor_preferences (
                       instructor_id, academic_year_id, hard_unavailable_slots,
                       max_periods_per_day, preferred_slots
                   ) VALUES ($1, $2, $3, $4, $5)
                   ON CONFLICT (instructor_id, academic_year_id) DO UPDATE SET
                       hard_unavailable_slots = EXCLUDED.hard_unavailable_slots,
                       max_periods_per_day = EXCLUDED.max_periods_per_day,
                       preferred_slots = EXCLUDED.preferred_slots,
                       updated_at = NOW()
                   WHERE instructor_preferences.hard_unavailable_slots IS DISTINCT FROM EXCLUDED.hard_unavailable_slots
                      OR instructor_preferences.max_periods_per_day IS DISTINCT FROM EXCLUDED.max_periods_per_day
                      OR instructor_preferences.preferred_slots IS DISTINCT FROM EXCLUDED.preferred_slots"#,
            )
            .bind(patch.id)
            .bind(academic_year_id)
            .bind(hard_unavailable_slots)
            .bind(max_periods_per_day)
            .bind(preferred_slots)
            .execute(&mut *tx)
            .await
            .map_err(write_error)?
            .rows_affected();
            row_changed = changed > 0 || !existed;
        }

        if !matches!(patch.assigned_room_id, Patch::Unchanged) {
            let current_rooms: Vec<Uuid> = sqlx::query_scalar(
                "SELECT room_id FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true AND subject_id IS NULL ORDER BY room_id",
            )
            .bind(patch.id)
            .bind(academic_year_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|error| AppError::InternalServerError(error.to_string()))?;
            let desired_room = match patch.assigned_room_id {
                Patch::Set(room_id) => Some(room_id),
                Patch::Clear => None,
                Patch::Unchanged => unreachable!(),
            };
            let already_matches = match desired_room {
                Some(room_id) => current_rooms.as_slice() == [room_id],
                None => current_rooms.is_empty(),
            };
            if !already_matches {
                sqlx::query(
                    "DELETE FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true AND subject_id IS NULL",
                )
                .bind(patch.id)
                .bind(academic_year_id)
                .execute(&mut *tx)
                .await
                .map_err(write_error)?;
                if let Some(room_id) = desired_room {
                    sqlx::query(
                        "INSERT INTO instructor_room_assignments (instructor_id, academic_year_id, room_id, is_required) VALUES ($1, $2, $3, true)",
                    )
                    .bind(patch.id)
                    .bind(academic_year_id)
                    .bind(room_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(write_error)?;
                }
                row_changed = true;
            }
        }
        if row_changed {
            result.instructor_constraints_updated += 1;
        }
    }

    for patch in &request.subjects {
        let current = sqlx::query_as::<_, SubjectConstraintState>(
            "SELECT min_consecutive_periods, max_consecutive_periods, allow_single_period, allowed_period_ids, allowed_days FROM subjects WHERE id = $1",
        )
        .bind(patch.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
        let min_consecutive_periods = match patch.min_consecutive_periods {
            Patch::Unchanged => current.min_consecutive_periods,
            Patch::Clear => Some(1),
            Patch::Set(value) => Some(value),
        };
        let max_consecutive_periods = match patch.max_consecutive_periods {
            Patch::Unchanged => current.max_consecutive_periods,
            Patch::Clear => None,
            Patch::Set(value) => Some(value),
        };
        let allow_single_period = match patch.allow_single_period {
            Patch::Unchanged => current.allow_single_period,
            Patch::Clear => Some(true),
            Patch::Set(value) => Some(value),
        };
        let allowed_period_ids = match &patch.allowed_period_ids {
            Patch::Unchanged => current.allowed_period_ids,
            Patch::Clear => None,
            Patch::Set(values) => Some(Json(values.clone())),
        };
        let allowed_days = match &patch.allowed_days {
            Patch::Unchanged => current.allowed_days,
            Patch::Clear => None,
            Patch::Set(values) => Some(Json(values.clone())),
        };
        result.subject_constraints_updated += sqlx::query(
            r#"UPDATE subjects SET min_consecutive_periods = $2,
                   max_consecutive_periods = $3, allow_single_period = $4,
                   allowed_period_ids = $5, allowed_days = $6, updated_at = NOW()
               WHERE id = $1 AND (
                   min_consecutive_periods IS DISTINCT FROM $2 OR
                   max_consecutive_periods IS DISTINCT FROM $3 OR
                   allow_single_period IS DISTINCT FROM $4 OR
                   allowed_period_ids IS DISTINCT FROM $5 OR
                   allowed_days IS DISTINCT FROM $6
               )"#,
        )
        .bind(patch.id)
        .bind(min_consecutive_periods)
        .bind(max_consecutive_periods)
        .bind(allow_single_period)
        .bind(allowed_period_ids)
        .bind(allowed_days)
        .execute(&mut *tx)
        .await
        .map_err(write_error)?
        .rows_affected() as usize;
    }

    for patch in &request.classroom_courses {
        let current = sqlx::query_as::<_, ClassroomCourseConstraintState>(
            "SELECT consecutive_pattern, same_day_unique, hard_unavailable_slots FROM classroom_courses WHERE id = $1",
        )
        .bind(patch.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
        let consecutive_pattern = match &patch.consecutive_pattern {
            Patch::Unchanged => current.consecutive_pattern,
            Patch::Clear => None,
            Patch::Set(values) => Some(Json(values.clone())),
        };
        let same_day_unique = match patch.same_day_unique {
            Patch::Unchanged => current.same_day_unique,
            Patch::Clear => true,
            Patch::Set(value) => value,
        };
        let hard_unavailable_slots = patched_json_vec(
            &patch.hard_unavailable_slots,
            current.hard_unavailable_slots,
            Vec::new(),
        );
        result.classroom_course_constraints_updated += sqlx::query(
            r#"UPDATE classroom_courses SET consecutive_pattern = $2,
                   same_day_unique = $3, hard_unavailable_slots = $4, updated_at = NOW()
               WHERE id = $1 AND (
                   consecutive_pattern IS DISTINCT FROM $2 OR
                   same_day_unique IS DISTINCT FROM $3 OR
                   hard_unavailable_slots IS DISTINCT FROM $4
               )"#,
        )
        .bind(patch.id)
        .bind(consecutive_pattern)
        .bind(same_day_unique)
        .bind(hard_unavailable_slots)
        .execute(&mut *tx)
        .await
        .map_err(write_error)?
        .rows_affected() as usize;
    }

    for patch in &request.preferred_rooms {
        let mut current: Vec<(Uuid, i32, bool)> = sqlx::query_as(
            "SELECT room_id, rank, is_required FROM classroom_course_preferred_rooms WHERE classroom_course_id = $1 ORDER BY rank, room_id",
        )
        .bind(patch.classroom_course_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| AppError::InternalServerError(error.to_string()))?;
        let mut desired: Vec<_> = patch
            .rooms
            .iter()
            .map(|room| (room.room_id, room.rank, room.is_required))
            .collect();
        current.sort_by_key(|row| (row.1, row.0));
        desired.sort_by_key(|row| (row.1, row.0));
        if current != desired {
            sqlx::query(
                "DELETE FROM classroom_course_preferred_rooms WHERE classroom_course_id = $1",
            )
            .bind(patch.classroom_course_id)
            .execute(&mut *tx)
            .await
            .map_err(write_error)?;
            if !desired.is_empty() {
                let room_ids: Vec<_> = desired.iter().map(|row| row.0).collect();
                let ranks: Vec<_> = desired.iter().map(|row| row.1).collect();
                let required: Vec<_> = desired.iter().map(|row| row.2).collect();
                sqlx::query(
                    r#"INSERT INTO classroom_course_preferred_rooms
                           (classroom_course_id, room_id, rank, is_required)
                       SELECT $1, room_id, rank, is_required
                       FROM UNNEST($2::uuid[], $3::int[], $4::bool[])
                           AS replacement(room_id, rank, is_required)"#,
                )
                .bind(patch.classroom_course_id)
                .bind(room_ids)
                .bind(ranks)
                .bind(required)
                .execute(&mut *tx)
                .await
                .map_err(write_error)?;
            }
            result.preferred_room_sets_updated += 1;
        }
    }

    result.changed = result.scheduler_settings_changed
        || result.instructor_order_updated > 0
        || result.instructor_constraints_updated > 0
        || result.subject_constraints_updated > 0
        || result.classroom_course_constraints_updated > 0
        || result.preferred_room_sets_updated > 0;

    tx.commit().await.map_err(write_error)?;
    Ok(result)
}

fn consecutive_pattern_sum(pattern: &[i32]) -> Result<i64, AppError> {
    let mut sum: i64 = 0;
    for number in pattern {
        if !(1..=20).contains(number) {
            return Err(AppError::BadRequest(
                "consecutive_pattern แต่ละค่าต้องอยู่ระหว่าง 1-20".to_string(),
            ));
        }
        sum += i64::from(*number);
    }
    Ok(sum)
}

fn validate_consecutive_sum_matches_periods_per_week(
    sum: i64,
    periods_per_week: Option<i32>,
) -> Result<(), AppError> {
    if let Some(periods_per_week) = periods_per_week {
        if sum != periods_per_week as i64 {
            return Err(AppError::BadRequest(format!(
                "ผลรวมของ pattern ({}) ต้องเท่ากับ periods_per_week ของวิชา ({})",
                sum, periods_per_week
            )));
        }
    }
    Ok(())
}

fn scheduler_default_max_consecutive(value: Option<i32>) -> i32 {
    value.unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_pattern_sum_accepts_integer_array() {
        assert_eq!(consecutive_pattern_sum(&[2, 1, 2]).unwrap(), 5);
    }

    #[test]
    fn consecutive_pattern_sum_rejects_out_of_range_items() {
        assert!(matches!(
            consecutive_pattern_sum(&[0, 2]),
            Err(AppError::BadRequest(message)) if message == "consecutive_pattern แต่ละค่าต้องอยู่ระหว่าง 1-20"
        ));
    }

    #[test]
    fn validate_consecutive_sum_matches_subject_periods_per_week() {
        assert!(validate_consecutive_sum_matches_periods_per_week(5, Some(5)).is_ok());
        assert!(matches!(
            validate_consecutive_sum_matches_periods_per_week(4, Some(5)),
            Err(AppError::BadRequest(message)) if message.contains("ต้องเท่ากับ periods_per_week")
        ));
        assert!(validate_consecutive_sum_matches_periods_per_week(4, None).is_ok());
    }

    #[test]
    fn json_vec_or_default_reads_typed_json_arrays() {
        let period_id = Uuid::new_v4();
        let slot = TimeSlot {
            day: "MON".to_string(),
            period_id,
        };

        assert_eq!(
            json_vec_or_default(Some(Json(vec![slot.clone()]))),
            vec![slot]
        );
        assert!(json_vec_or_default::<TimeSlot>(None).is_empty());
    }

    #[test]
    fn instructor_constraint_view_from_row_maps_optional_typed_slots() {
        let period_id = Uuid::new_v4();
        let slot = TimeSlot {
            day: "WED".to_string(),
            period_id,
        };
        let room_id = Uuid::new_v4();
        let row = InstructorConstraintRow {
            id: Uuid::new_v4(),
            first_name: "สมชาย".to_string(),
            last_name: "ใจดี".to_string(),
            hard_unavailable_slots: Some(Json(vec![slot.clone()])),
            max_periods_per_day: Some(6),
            min_periods_per_day: Some(2),
            assigned_room_id: Some(room_id),
            assigned_room_name: Some("ห้องวิทย์".to_string()),
            priority: 10,
            primary_course_count: 4,
        };

        let view = instructor_constraint_view_from_row(row).expect("instructor row should map");

        assert_eq!(view.hard_unavailable_slots, Some(vec![slot]));
        assert_eq!(view.assigned_room_id, Some(room_id));
        assert_eq!(view.priority, 10);
        assert_eq!(view.primary_course_count, 4);
    }

    #[test]
    fn subject_constraint_view_from_row_preserves_typed_optional_arrays() {
        let allowed_period_id = Uuid::new_v4();
        let row = SubjectConstraintRow {
            id: Uuid::new_v4(),
            code: "MATH".to_string(),
            name: "คณิตศาสตร์".to_string(),
            min_consecutive_periods: 1,
            max_consecutive_periods: Some(2),
            allow_single_period: Some(true),
            periods_per_week: Some(5),
            allowed_period_ids: Some(Json(vec![allowed_period_id])),
            allowed_days: Some(Json(vec!["MON".to_string(), "TUE".to_string()])),
        };

        let view = subject_constraint_view_from_row(row).unwrap();

        assert_eq!(view.allowed_period_ids, Some(vec![allowed_period_id]));
        assert_eq!(
            view.allowed_days,
            Some(vec!["MON".to_string(), "TUE".to_string()])
        );
    }

    #[test]
    fn classroom_course_constraint_view_from_row_defaults_missing_slot_arrays() {
        let row = ClassroomCourseConstraintRow {
            id: Uuid::new_v4(),
            classroom_id: Uuid::new_v4(),
            classroom_name: "ม.1/1".to_string(),
            subject_id: Uuid::new_v4(),
            subject_code: "MATH101".to_string(),
            subject_name: "คณิตศาสตร์".to_string(),
            periods_per_week: Some(5),
            primary_instructor_id: None,
            primary_instructor_name: None,
            consecutive_pattern: Some(Json(vec![2, 1, 2])),
            same_day_unique: true,
            hard_unavailable_slots: None,
            team_unavailable_slots: None,
        };

        let view = classroom_course_constraint_view_from_row(row)
            .expect("classroom course row should map");

        assert_eq!(view.consecutive_pattern, Some(vec![2, 1, 2]));
        assert!(view.hard_unavailable_slots.is_empty());
        assert!(view.team_unavailable_slots.is_empty());
        assert!(view.same_day_unique);
    }

    #[test]
    fn classroom_course_constraint_view_from_row_preserves_typed_team_slots() {
        let hard_slot = TimeSlot {
            day: "MON".to_string(),
            period_id: Uuid::new_v4(),
        };
        let team_slot = TimeSlot {
            day: "FRI".to_string(),
            period_id: Uuid::new_v4(),
        };
        let row = ClassroomCourseConstraintRow {
            id: Uuid::new_v4(),
            classroom_id: Uuid::new_v4(),
            classroom_name: "ม.2/1".to_string(),
            subject_id: Uuid::new_v4(),
            subject_code: "SCI201".to_string(),
            subject_name: "วิทยาศาสตร์".to_string(),
            periods_per_week: Some(4),
            primary_instructor_id: Some(Uuid::new_v4()),
            primary_instructor_name: Some("ครูสมหญิง ใจดี".to_string()),
            consecutive_pattern: None,
            same_day_unique: false,
            hard_unavailable_slots: Some(Json(vec![hard_slot.clone()])),
            team_unavailable_slots: Some(Json(vec![team_slot.clone()])),
        };

        let view = classroom_course_constraint_view_from_row(row)
            .expect("classroom course row should map");

        assert_eq!(view.hard_unavailable_slots, vec![hard_slot]);
        assert_eq!(view.team_unavailable_slots, vec![team_slot]);
        assert_eq!(view.primary_instructor_name.as_deref(), Some("ครูสมหญิง ใจดี"));
    }

    #[test]
    fn scheduler_setting_value_defaults_to_four_when_missing() {
        assert_eq!(scheduler_default_max_consecutive(Some(6)), 6);
        assert_eq!(scheduler_default_max_consecutive(None), 4);
    }
}
