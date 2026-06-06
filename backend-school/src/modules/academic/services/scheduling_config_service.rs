use crate::error::AppError;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow)]
pub struct InstructorConstraintView {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub hard_unavailable_slots: Option<serde_json::Value>,
    pub max_periods_per_day: Option<i32>,
    pub min_periods_per_day: Option<i32>,
    pub assigned_room_id: Option<Uuid>,
    pub assigned_room_name: Option<String>,
    pub priority: i32,
    pub primary_course_count: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SubjectConstraintView {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub min_consecutive_periods: i32,
    pub max_consecutive_periods: Option<i32>,
    pub allow_single_period: Option<bool>,
    pub periods_per_week: Option<i32>,
    pub allowed_period_ids: Option<serde_json::Value>,
    pub allowed_days: Option<serde_json::Value>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ClassroomCourseConstraintView {
    pub id: Uuid,
    pub classroom_id: Uuid,
    pub classroom_name: String,
    pub subject_id: Uuid,
    pub subject_code: String,
    pub subject_name: String,
    pub periods_per_week: Option<i32>,
    pub primary_instructor_id: Option<Uuid>,
    pub primary_instructor_name: Option<String>,
    pub consecutive_pattern: Option<serde_json::Value>,
    pub same_day_unique: bool,
    pub hard_unavailable_slots: serde_json::Value,
    pub team_unavailable_slots: serde_json::Value,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CcPreferredRoomView {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub room_id: Uuid,
    pub room_code: String,
    pub room_name: String,
    pub rank: i32,
    pub is_required: bool,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RoomView {
    pub id: Uuid,
    pub code: String,
    pub name_th: String,
    pub room_type: Option<String>,
}

pub async fn get_active_year_id(pool: &PgPool) -> Result<Uuid, AppError> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM academic_years WHERE is_active = true LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    id.ok_or_else(|| AppError::NotFound("Active academic year not found".to_string()))
}

pub async fn get_active_year_id_tx(tx: &mut sqlx::PgConnection) -> Result<Uuid, AppError> {
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM academic_years WHERE is_active = true LIMIT 1")
            .fetch_optional(&mut *tx)
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

    let q = sqlx::query_as::<_, ClassroomCourseConstraintView>(&sql).bind(year_id);
    let result = if let Some(iid) = instructor_id {
        q.bind(iid).fetch_all(pool).await
    } else {
        q.fetch_all(pool).await
    };
    result.map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn validate_consecutive_pattern(
    pool: &PgPool,
    cc_id: Uuid,
    pattern: &serde_json::Value,
) -> Result<(), AppError> {
    let sum = consecutive_pattern_sum(pattern)?;

    let pw: Option<i32> = sqlx::query_scalar(
        "SELECT s.periods_per_week FROM classroom_courses cc JOIN subjects s ON s.id = cc.subject_id WHERE cc.id = $1"
    )
    .bind(cc_id).fetch_optional(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?
    .flatten();

    validate_consecutive_sum_matches_periods_per_week(sum, pw)
}

pub async fn update_classroom_course_constraints(
    pool: &PgPool,
    cc_id: Uuid,
    consecutive_pattern: Option<serde_json::Value>,
    same_day_unique: Option<bool>,
    hard_unavailable_slots: Option<serde_json::Value>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"UPDATE classroom_courses SET
            consecutive_pattern = COALESCE($2, consecutive_pattern),
            same_day_unique = COALESCE($3, same_day_unique),
            hard_unavailable_slots = COALESCE($4, hard_unavailable_slots),
            updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(cc_id)
    .bind(consecutive_pattern)
    .bind(same_day_unique)
    .bind(hard_unavailable_slots)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
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

pub async fn set_cc_preferred_rooms(
    pool: &PgPool,
    cc_id: Uuid,
    rooms: Vec<(Uuid, i32, bool)>,
) -> Result<usize, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    sqlx::query("DELETE FROM classroom_course_preferred_rooms WHERE classroom_course_id = $1")
        .bind(cc_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if !rooms.is_empty() {
        let room_ids: Vec<Uuid> = rooms.iter().map(|r| r.0).collect();
        let ranks: Vec<i32> = rooms.iter().map(|r| r.1).collect();
        let required: Vec<bool> = rooms.iter().map(|r| r.2).collect();

        sqlx::query(
            r#"INSERT INTO classroom_course_preferred_rooms (classroom_course_id, room_id, rank, is_required)
               SELECT $1, room_id, rk, req
               FROM UNNEST($2::uuid[], $3::int[], $4::bool[]) AS t(room_id, rk, req)"#
        )
        .bind(cc_id).bind(&room_ids).bind(&ranks).bind(&required)
        .execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    let count = rooms.len();
    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(count)
}

pub async fn list_all_rooms(pool: &PgPool) -> Result<Vec<RoomView>, AppError> {
    sqlx::query_as::<_, RoomView>(
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
    sqlx::query_as::<_, InstructorConstraintView>(
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
        LEFT JOIN instructor_room_assignments ra ON u.id = ra.instructor_id AND ra.academic_year_id = $1 AND ra.is_required = true
        LEFT JOIN rooms r ON ra.room_id = r.id
        LEFT JOIN primary_counts pc ON pc.instructor_id = u.id
        WHERE u.user_type = 'staff' AND u.status = 'active'
        ORDER BY COALESCE(ip.priority, 100), u.first_name"#
    )
    .bind(year_id).fetch_all(pool).await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn reorder_instructor_priority(
    pool: &PgPool,
    instructor_ids: Vec<Uuid>,
) -> Result<usize, AppError> {
    if instructor_ids.is_empty() {
        return Ok(0);
    }
    let year_id = get_active_year_id(pool).await?;
    let priorities: Vec<i32> = (1..=instructor_ids.len() as i32).collect();

    sqlx::query(
        r#"INSERT INTO instructor_preferences (instructor_id, academic_year_id, priority)
           SELECT instr_id, $2, prio
           FROM UNNEST($1::uuid[], $3::int[]) AS t(instr_id, prio)
           ON CONFLICT (instructor_id, academic_year_id)
           DO UPDATE SET priority = EXCLUDED.priority, updated_at = NOW()"#,
    )
    .bind(&instructor_ids)
    .bind(year_id)
    .bind(&priorities)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(instructor_ids.len())
}

pub async fn get_scheduler_settings(pool: &PgPool) -> Result<i32, AppError> {
    let rows = sqlx::query_as::<_, (String, serde_json::Value)>(
        "SELECT key, value FROM scheduler_settings",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    Ok(scheduler_default_max_consecutive(rows))
}

pub async fn update_scheduler_settings(
    pool: &PgPool,
    default_max_consecutive: Option<i32>,
) -> Result<(), AppError> {
    if let Some(v) = default_max_consecutive {
        if !(1..=20).contains(&v) {
            return Err(AppError::BadRequest(
                "default_max_consecutive ต้องอยู่ระหว่าง 1-20".to_string(),
            ));
        }
        sqlx::query(
            "INSERT INTO scheduler_settings (key, value) VALUES ('default_max_consecutive', $1::jsonb)
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()"
        )
        .bind(serde_json::Value::from(v)).execute(pool).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }
    Ok(())
}

pub struct InstructorConstraintUpdate {
    pub hard_unavailable_slots: Option<serde_json::Value>,
    pub max_periods_per_day: Option<i32>,
    pub preferred_slots: Option<serde_json::Value>,
    pub priority: Option<i32>,
    pub assigned_room_id: Option<Uuid>,
    pub clear_assigned_room: bool,
}

pub async fn update_instructor_constraints(
    pool: &PgPool,
    instructor_id: Uuid,
    update: InstructorConstraintUpdate,
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    let year_id = get_active_year_id_tx(&mut tx).await?;

    sqlx::query(
        r#"INSERT INTO instructor_preferences (
               instructor_id, academic_year_id,
               hard_unavailable_slots, max_periods_per_day, preferred_slots, priority
           )
           VALUES ($1, $2,
                   COALESCE($3, '[]'::jsonb), $4,
                   COALESCE($5, '[]'::jsonb), COALESCE($6, 100))
           ON CONFLICT (instructor_id, academic_year_id)
           DO UPDATE SET
               hard_unavailable_slots = COALESCE($3, instructor_preferences.hard_unavailable_slots),
               max_periods_per_day = COALESCE($4, instructor_preferences.max_periods_per_day),
               preferred_slots = COALESCE($5, instructor_preferences.preferred_slots),
               priority = COALESCE($6, instructor_preferences.priority),
               updated_at = NOW()"#,
    )
    .bind(instructor_id)
    .bind(year_id)
    .bind(update.hard_unavailable_slots)
    .bind(update.max_periods_per_day)
    .bind(update.preferred_slots)
    .bind(update.priority)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    if let Some(room_id) = update.assigned_room_id {
        sqlx::query(
            "DELETE FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true"
        ).bind(instructor_id).bind(year_id).execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO instructor_room_assignments (instructor_id, academic_year_id, room_id, is_required)
             VALUES ($1, $2, $3, true)"
        ).bind(instructor_id).bind(year_id).bind(room_id).execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    } else if update.clear_assigned_room {
        sqlx::query(
            "DELETE FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true"
        ).bind(instructor_id).bind(year_id).execute(&mut *tx).await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn list_subject_constraints(
    pool: &PgPool,
) -> Result<Vec<SubjectConstraintView>, AppError> {
    sqlx::query_as::<_, SubjectConstraintView>(
        r#"SELECT id, code, name_th as name,
                  COALESCE(min_consecutive_periods, 1) as min_consecutive_periods,
                  max_consecutive_periods, allow_single_period, periods_per_week,
                  allowed_period_ids, allowed_days
           FROM subjects WHERE is_active = true ORDER BY code"#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn update_subject_constraints(
    pool: &PgPool,
    subject_id: Uuid,
    min_consecutive_periods: Option<i32>,
    max_consecutive_periods: Option<i32>,
    allow_single_period: Option<bool>,
    allowed_period_ids: Option<serde_json::Value>,
    allowed_days: Option<serde_json::Value>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"UPDATE subjects SET
            min_consecutive_periods = COALESCE($2, min_consecutive_periods),
            max_consecutive_periods = $3,
            allow_single_period = COALESCE($4, allow_single_period),
            allowed_period_ids = $5,
            allowed_days = $6,
            updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(subject_id)
    .bind(min_consecutive_periods)
    .bind(max_consecutive_periods)
    .bind(allow_single_period)
    .bind(allowed_period_ids)
    .bind(allowed_days)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

fn consecutive_pattern_sum(pattern: &serde_json::Value) -> Result<i64, AppError> {
    let arr = pattern
        .as_array()
        .ok_or_else(|| AppError::BadRequest("consecutive_pattern ต้องเป็น array".to_string()))?;
    let mut sum: i64 = 0;
    for value in arr {
        let number = value
            .as_i64()
            .ok_or_else(|| AppError::BadRequest("consecutive_pattern มีค่าที่ไม่ใช่ตัวเลข".to_string()))?;
        if !(1..=20).contains(&number) {
            return Err(AppError::BadRequest(
                "consecutive_pattern แต่ละค่าต้องอยู่ระหว่าง 1-20".to_string(),
            ));
        }
        sum += number;
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

fn scheduler_default_max_consecutive(rows: Vec<(String, serde_json::Value)>) -> i32 {
    rows.into_iter()
        .find_map(|(key, value)| {
            if key == "default_max_consecutive" {
                value.as_i64().map(|value| value as i32)
            } else {
                None
            }
        })
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_pattern_sum_accepts_integer_array() {
        let pattern = serde_json::json!([2, 1, 2]);

        assert_eq!(consecutive_pattern_sum(&pattern).unwrap(), 5);
    }

    #[test]
    fn consecutive_pattern_sum_rejects_non_array_values() {
        assert!(matches!(
            consecutive_pattern_sum(&serde_json::json!({"days": [1]})),
            Err(AppError::BadRequest(message)) if message == "consecutive_pattern ต้องเป็น array"
        ));
    }

    #[test]
    fn consecutive_pattern_sum_rejects_out_of_range_items() {
        assert!(matches!(
            consecutive_pattern_sum(&serde_json::json!([0, 2])),
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
    fn scheduler_setting_value_defaults_to_four_when_missing_or_invalid() {
        assert_eq!(
            scheduler_default_max_consecutive(vec![(
                "default_max_consecutive".to_string(),
                serde_json::json!(6)
            )]),
            6
        );
        assert_eq!(scheduler_default_max_consecutive(Vec::new()), 4);
        assert_eq!(
            scheduler_default_max_consecutive(vec![(
                "default_max_consecutive".to_string(),
                serde_json::json!("bad")
            )]),
            4
        );
    }
}
