use crate::error::AppError;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
pub struct MyActivityInstructor {
    pub id: Uuid,
    pub name: String,
}

#[derive(Serialize)]
pub struct MyActivityForEntry {
    pub enrolled: bool,
    pub slot_id: Uuid,
    pub group_id: Option<Uuid>,
    pub group_name: Option<String>,
    pub max_capacity: Option<i32>,
    pub member_count: Option<i64>,
    pub instructor_name: Option<String>,
    pub instructors: Option<Vec<MyActivityInstructor>>,
}

/// ผลของ add_entry_instructor — handler ใช้ broadcast EntryInstructorAdded
pub struct AddInstructorResult {
    pub semester_id: Option<Uuid>,
    pub instructor_name: String,
}

/// เพิ่มครูเข้า entry — return semester_id + instructor_name สำหรับ broadcast
pub async fn add_entry_instructor(
    pool: &PgPool,
    entry_id: Uuid,
    instructor_id: Uuid,
    role: &str,
) -> Result<AddInstructorResult, AppError> {
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(entry_id)
    .bind(instructor_id)
    .bind(role)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries WHERE id = $1",
    )
    .bind(entry_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let instructor_name: String =
        sqlx::query_scalar("SELECT CONCAT(first_name, ' ', last_name) FROM users WHERE id = $1")
            .bind(instructor_id)
            .fetch_one(pool)
            .await
            .unwrap_or_default();

    Ok(AddInstructorResult {
        semester_id,
        instructor_name,
    })
}

/// ผลของ remove_entry_instructor
pub struct RemoveInstructorResult {
    pub semester_id: Option<Uuid>,
    pub entry_deleted: bool,
}

/// ลบครูออกจาก entry — ถ้าไม่เหลือครู + เป็น course entry ก็ลบ entry ทั้งหมด
pub async fn remove_entry_instructor(
    pool: &PgPool,
    entry_id: Uuid,
    instructor_id: Uuid,
) -> Result<RemoveInstructorResult, AppError> {
    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries WHERE id = $1",
    )
    .bind(entry_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    sqlx::query(
        "DELETE FROM timetable_entry_instructors WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(instructor_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM timetable_entry_instructors WHERE entry_id = $1")
            .bind(entry_id)
            .fetch_one(pool)
            .await
            .unwrap_or(1);

    let mut entry_deleted = false;
    if remaining == 0 {
        let is_course: bool = sqlx::query_scalar(
            "SELECT classroom_course_id IS NOT NULL FROM academic_timetable_entries WHERE id = $1",
        )
        .bind(entry_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
        if is_course {
            sqlx::query("DELETE FROM academic_timetable_entries WHERE id = $1")
                .bind(entry_id)
                .execute(pool)
                .await
                .ok();
            entry_deleted = true;
        }
    }

    Ok(RemoveInstructorResult {
        semester_id,
        entry_deleted,
    })
}

/// เพิ่มครูกลับเข้าทุก entry ของ slot — return rows inserted
pub async fn restore_instructor_to_slot(
    pool: &PgPool,
    slot_id: Uuid,
    instructor_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         SELECT te.id, $2, 'primary' FROM academic_timetable_entries te
         WHERE te.activity_slot_id = $1 AND te.is_active = true
         ON CONFLICT DO NOTHING",
    )
    .bind(slot_id)
    .bind(instructor_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(result.rows_affected())
}

/// ลบครูออกจากทุก entry ของ slot — return (rows_deleted, semester_id?)
pub async fn hide_instructor_from_slot(
    pool: &PgPool,
    slot_id: Uuid,
    instructor_id: Uuid,
) -> Result<(u64, Option<Uuid>), AppError> {
    let result = sqlx::query(
        "DELETE FROM timetable_entry_instructors
         WHERE instructor_id = $1
           AND entry_id IN (
               SELECT id FROM academic_timetable_entries
               WHERE activity_slot_id = $2 AND is_active = true
           )",
    )
    .bind(instructor_id)
    .bind(slot_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries
         WHERE activity_slot_id = $1 LIMIT 1",
    )
    .bind(slot_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    Ok((result.rows_affected(), semester_id))
}

/// ลบครูออกจาก entries ของ slot ที่ตรงกับ (day, period) เท่านั้น
pub async fn hide_instructor_from_slot_period(
    pool: &PgPool,
    slot_id: Uuid,
    instructor_id: Uuid,
    day_of_week: &str,
    period_id: Uuid,
) -> Result<(u64, Option<Uuid>), AppError> {
    let result = sqlx::query(
        "DELETE FROM timetable_entry_instructors
         WHERE instructor_id = $1
           AND entry_id IN (
               SELECT id FROM academic_timetable_entries
               WHERE activity_slot_id = $2
                 AND day_of_week = $3
                 AND period_id = $4
                 AND is_active = true
           )",
    )
    .bind(instructor_id)
    .bind(slot_id)
    .bind(day_of_week)
    .bind(period_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let semester_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT academic_semester_id FROM academic_timetable_entries
         WHERE activity_slot_id = $1 AND day_of_week = $2 AND period_id = $3 LIMIT 1",
    )
    .bind(slot_id)
    .bind(day_of_week)
    .bind(period_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    Ok((result.rows_affected(), semester_id))
}

/// คืน activity group ที่ user enrolled สำหรับ entry นี้ (สำหรับ student view)
pub async fn get_my_activity_for_entry(
    pool: &PgPool,
    user_id: Uuid,
    entry_id: Uuid,
) -> Result<Option<MyActivityForEntry>, AppError> {
    let slot_id: Option<Uuid> =
        sqlx::query_scalar("SELECT activity_slot_id FROM academic_timetable_entries WHERE id = $1")
            .bind(entry_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| AppError::InternalServerError("Query failed".to_string()))?
            .flatten();

    let slot_id = match slot_id {
        Some(id) => id,
        None => return Ok(None),
    };

    let group = sqlx::query_as::<_, (Uuid, String, Option<i32>, Option<String>)>(
        r#"
        SELECT ag.id, ag.name, ag.max_capacity,
               (SELECT concat(u.first_name, ' ', u.last_name)
                FROM activity_group_instructors agi
                JOIN users u ON agi.user_id = u.id
                WHERE agi.activity_group_id = ag.id
                LIMIT 1) AS instructor_name
        FROM activity_group_members agm
        JOIN activity_groups ag ON agm.activity_group_id = ag.id
        WHERE agm.student_id = $1 AND ag.slot_id = $2 AND ag.is_active = true
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(slot_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch activity for entry: {}", e);
        AppError::InternalServerError("Query failed".to_string())
    })?;

    match group {
        Some((id, name, max_capacity, instructor_name)) => {
            let member_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM activity_group_members WHERE activity_group_id = $1",
            )
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            let instructors: Vec<(Uuid, String)> = sqlx::query_as(
                r#"
                SELECT u.id, concat(u.first_name, ' ', u.last_name) AS name
                FROM activity_group_instructors agi
                JOIN users u ON agi.user_id = u.id
                WHERE agi.activity_group_id = $1
                "#,
            )
            .bind(id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            Ok(Some(MyActivityForEntry {
                enrolled: true,
                slot_id,
                group_id: Some(id),
                group_name: Some(name),
                max_capacity,
                member_count: Some(member_count),
                instructor_name,
                instructors: Some(
                    instructors
                        .into_iter()
                        .map(|(id, name)| MyActivityInstructor { id, name })
                        .collect(),
                ),
            }))
        }
        None => Ok(Some(MyActivityForEntry {
            enrolled: false,
            slot_id,
            group_id: None,
            group_name: None,
            max_capacity: None,
            member_count: None,
            instructor_name: None,
            instructors: None,
        })),
    }
}
