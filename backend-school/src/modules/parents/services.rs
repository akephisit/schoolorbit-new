use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable::TimetableEntry;
use crate::modules::academic::services::timetable_service::{self, TimetableFilter};
use crate::modules::calendar::models::{CalendarEventQuery, CalendarViewerEvent};
use crate::modules::students::models::{ParentDto, StudentDbRow, StudentProfile};
use crate::utils::field_encryption;

use super::models::{ChildDto, ParentDbRow, ParentProfile};

pub async fn get_own_parent_profile(
    pool: &PgPool,
    parent_id: Uuid,
) -> Result<ParentProfile, AppError> {
    ensure_parent_user(pool, parent_id).await?;

    let mut parent = sqlx::query_as::<_, ParentDbRow>(
        r#"
        SELECT
            id, username, first_name, last_name, title, phone, email, national_id
        FROM users
        WHERE id = $1 AND status = 'active'
        "#,
    )
    .bind(parent_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get parent profile: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?
    .ok_or(AppError::NotFound("Parent not found".to_string()))?;

    decrypt_parent_fields(&mut parent);
    let children = list_children(pool, parent_id).await?;

    Ok(ParentProfile {
        id: parent.id,
        username: parent.username,
        first_name: parent.first_name,
        last_name: parent.last_name,
        title: parent.title,
        phone: parent.phone,
        email: parent.email,
        national_id: parent.national_id,
        children,
    })
}

pub async fn get_child_profile(
    pool: &PgPool,
    parent_id: Uuid,
    student_id: Uuid,
) -> Result<StudentProfile, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    ensure_parent_student_link(pool, parent_id, student_id).await?;

    let mut student_row = sqlx::query_as::<_, StudentDbRow>(
        r#"
        SELECT
            u.id, u.username, u.national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender, u.address, u.profile_image_url, u.status,
            si.student_id,
            CASE gl.level_type
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level,
            c.name as class_room,
            sce.class_number as student_number,
            si.blood_type, si.allergies, si.medical_conditions
        FROM users u
        INNER JOIN student_info si ON u.id = si.user_id
        LEFT JOIN student_class_enrollments sce ON u.id = sce.student_id AND sce.status = 'active'
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE u.id = $1 AND u.status = 'active'
        "#,
    )
    .bind(student_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get child profile: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูลนักเรียน".to_string())
    })?
    .ok_or(AppError::NotFound("Student not found".to_string()))?;

    decrypt_child_student_fields(&mut student_row);
    let parents = list_student_parents(pool, student_id).await?;

    Ok(StudentProfile {
        info: student_row,
        parents,
    })
}

pub async fn get_child_timetable(
    pool: &PgPool,
    parent_id: Uuid,
    student_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<TimetableEntry>, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    ensure_parent_student_link(pool, parent_id, student_id).await?;

    timetable_service::list_entries(
        pool,
        TimetableFilter {
            student_id: Some(student_id),
            academic_semester_id,
            ..Default::default()
        },
    )
    .await
}

pub async fn get_child_calendar_events(
    pool: &PgPool,
    parent_id: Uuid,
    student_id: Uuid,
    query: CalendarEventQuery,
) -> Result<Vec<CalendarViewerEvent>, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    ensure_parent_student_link(pool, parent_id, student_id).await?;

    crate::modules::calendar::services::list_child_events(pool, parent_id, student_id, query).await
}

async fn ensure_parent_user(pool: &PgPool, parent_id: Uuid) -> Result<(), AppError> {
    let user_type: Option<String> = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(parent_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to load parent user type: {}", e);
            AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้ใช้ได้".to_string())
        })?;

    parent_user_access(user_type.as_deref())
}

async fn ensure_parent_student_link(
    pool: &PgPool,
    parent_id: Uuid,
    student_id: Uuid,
) -> Result<(), AppError> {
    let is_linked: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM student_parents
            JOIN users u ON u.id = student_parents.student_user_id
            WHERE student_parents.parent_user_id = $1
              AND student_parents.student_user_id = $2
              AND u.user_type = 'student'
              AND u.status = 'active'
        )
        "#,
    )
    .bind(parent_id)
    .bind(student_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Parent-child link check failed: {}", e);
        AppError::InternalServerError("ตรวจสอบสิทธิ์ผิดพลาด".to_string())
    })?;

    if !is_linked {
        return Err(AppError::Forbidden(
            "คุณไม่มีสิทธิ์เข้าถึงข้อมูลนักเรียนคนนี้".to_string(),
        ));
    }

    Ok(())
}

async fn list_children(pool: &PgPool, parent_id: Uuid) -> Result<Vec<ChildDto>, AppError> {
    sqlx::query_as::<_, ChildDto>(
        r#"
        SELECT
            u.id, u.first_name, u.last_name, u.profile_image_url,
            si.student_id,
            CASE gl.level_type
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level,
            c.name as class_room,
            sp.relationship
        FROM student_parents sp
        INNER JOIN users u ON sp.student_user_id = u.id
        LEFT JOIN student_info si ON u.id = si.user_id
        LEFT JOIN student_class_enrollments sce ON u.id = sce.student_id AND sce.status = 'active'
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE sp.parent_user_id = $1 AND u.status = 'active'
        ORDER BY u.first_name ASC
        "#,
    )
    .bind(parent_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list parent children: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลบุตรหลานได้".to_string())
    })
}

async fn list_student_parents(pool: &PgPool, student_id: Uuid) -> Result<Vec<ParentDto>, AppError> {
    sqlx::query_as::<_, ParentDto>(
        r#"
        SELECT
            u.id, u.username, u.first_name, u.last_name, u.phone,
            sp.relationship, sp.is_primary
        FROM student_parents sp
        INNER JOIN users u ON sp.parent_user_id = u.id
        WHERE sp.student_user_id = $1
        "#,
    )
    .bind(student_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list child parents: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้ปกครองได้".to_string())
    })
}

fn decrypt_parent_fields(parent: &mut ParentDbRow) {
    if let Some(national_id) = parent.national_id.clone() {
        match field_encryption::decrypt(&national_id) {
            Ok(decrypted) => parent.national_id = Some(decrypted),
            Err(error) => tracing::error!("Failed to decrypt parent national_id: {}", error),
        }
    }
}

fn decrypt_child_student_fields(student: &mut StudentDbRow) {
    if let Some(national_id) = student.national_id.clone() {
        match field_encryption::decrypt(&national_id) {
            Ok(decrypted) => student.national_id = Some(decrypted),
            Err(error) => tracing::error!("Failed to decrypt child national_id: {}", error),
        }
    }
}

fn parent_user_access(user_type: Option<&str>) -> Result<(), AppError> {
    match user_type {
        Some("parent") => Ok(()),
        Some(_) => Err(AppError::Forbidden("เฉพาะผู้ปกครองเท่านั้น".to_string())),
        None => Err(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_user_access_allows_parent_users() {
        assert!(parent_user_access(Some("parent")).is_ok());
    }

    #[test]
    fn parent_user_access_rejects_non_parent_users() {
        assert!(matches!(
            parent_user_access(Some("staff")),
            Err(AppError::Forbidden(message)) if message == "เฉพาะผู้ปกครองเท่านั้น"
        ));
    }

    #[test]
    fn parent_user_access_treats_missing_user_as_auth_error() {
        assert!(matches!(
            parent_user_access(None),
            Err(AppError::AuthError(message)) if message == "กรุณาเข้าสู่ระบบ"
        ));
    }
}
