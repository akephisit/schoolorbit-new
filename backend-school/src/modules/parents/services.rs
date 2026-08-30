use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::core::models::AcademicContextOptions;
use crate::modules::academic::core::services::context as academic_context_service;
use crate::modules::academic::models::exam_schedule::PersonalExamScheduleRound;
use crate::modules::academic::models::timetable::TimetableEntry;
use crate::modules::academic::services::exam_schedule_service;
use crate::modules::academic::services::{timetable_service, timetable_version_service};
use crate::modules::calendar::models::{CalendarEventQuery, CalendarViewerEvent};
use crate::modules::students::models::{ParentDto, StudentDbRow, StudentProfile};
use crate::utils::field_encryption;

use super::models::{ChildDto, ParentDbRow, ParentProfile};

pub async fn get_own_parent_profile(
    pool: &PgPool,
    parent_id: Uuid,
    academic_year_id: Uuid,
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
    let children = list_children(pool, parent_id, academic_year_id).await?;

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
    academic_year_id: Uuid,
) -> Result<StudentProfile, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    ensure_parent_student_link(pool, parent_id, student_id).await?;

    let mut student_row = sqlx::query_as::<_, StudentDbRow>(
        r#"
        SELECT
            u.id, u.username, u.national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender, u.address, u.profile_image_file_id, u.status,
            si.student_id,
            CASE gl.level_type
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level,
            h.name as homeroom,
            placement.class_number as student_number,
            si.blood_type, si.allergies, si.medical_conditions
        FROM users u
        INNER JOIN student_info si ON u.id = si.user_id
        INNER JOIN student_academic_years student_year
          ON student_year.student_id = u.id
         AND student_year.academic_year_id = $2
        LEFT JOIN LATERAL (
            SELECT homeroom_id, class_number
            FROM homeroom_placements
            WHERE student_academic_year_id = student_year.id
            ORDER BY CASE status WHEN 'current' THEN 1 WHEN 'planned' THEN 2 ELSE 3 END,
                     start_date DESC, created_at DESC
            LIMIT 1
        ) placement ON true
        LEFT JOIN homerooms h ON placement.homeroom_id = h.id
        LEFT JOIN grade_levels gl ON student_year.grade_level_id = gl.id
        WHERE u.id = $1 AND u.status = 'active'
        "#,
    )
    .bind(student_id)
    .bind(academic_year_id)
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
    academic_term_id: Uuid,
    date: NaiveDate,
) -> Result<Vec<TimetableEntry>, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    ensure_parent_student_link(pool, parent_id, student_id).await?;

    let version = timetable_version_service::resolve_for_date(pool, academic_term_id, date).await?;
    timetable_service::list_student_entries(pool, version.id, academic_term_id, student_id).await
}

pub async fn get_child_academic_context_options(
    pool: &PgPool,
    parent_id: Uuid,
    student_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    ensure_parent_student_link(pool, parent_id, student_id).await?;
    academic_context_service::list_options_for_student(pool, student_id).await
}

pub async fn get_parent_academic_context_options(
    pool: &PgPool,
    parent_id: Uuid,
) -> Result<AcademicContextOptions, AppError> {
    ensure_parent_user(pool, parent_id).await?;
    academic_context_service::list_options_for_parent(pool, parent_id).await
}

pub async fn get_child_exam_schedule(
    pool: &PgPool,
    parent_id: Uuid,
    student_id: Uuid,
    academic_term_id: Uuid,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    exam_schedule_service::list_child_published_exam_schedule(
        pool,
        parent_id,
        student_id,
        academic_term_id,
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

async fn list_children(
    pool: &PgPool,
    parent_id: Uuid,
    academic_year_id: Uuid,
) -> Result<Vec<ChildDto>, AppError> {
    sqlx::query_as::<_, ChildDto>(
        r#"
        SELECT
            u.id, u.first_name, u.last_name, u.profile_image_file_id,
            si.student_id,
            CASE gl.level_type
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level,
            h.name as homeroom,
            sp.relationship
        FROM student_parents sp
        INNER JOIN users u ON sp.student_user_id = u.id
        LEFT JOIN student_info si ON u.id = si.user_id
        INNER JOIN student_academic_years student_year
          ON student_year.student_id = u.id
         AND student_year.academic_year_id = $2
        LEFT JOIN LATERAL (
            SELECT homeroom_id
            FROM homeroom_placements
            WHERE student_academic_year_id = student_year.id
            ORDER BY CASE status WHEN 'current' THEN 1 WHEN 'planned' THEN 2 ELSE 3 END,
                     start_date DESC, created_at DESC
            LIMIT 1
        ) placement ON true
        LEFT JOIN homerooms h ON placement.homeroom_id = h.id
        LEFT JOIN grade_levels gl ON student_year.grade_level_id = gl.id
        WHERE sp.parent_user_id = $1 AND u.status = 'active'
        ORDER BY u.first_name ASC
        "#,
    )
    .bind(parent_id)
    .bind(academic_year_id)
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
    use crate::{
        modules::academic::cutover_test_support::{
            apply_migrations_through, apply_phase_b_runtime_migrations,
            seed_academic_cutover_fixture, CutoverFixture,
        },
        test_helpers::{create_named_test_pool, create_test_user},
    };

    #[tokio::test]
    async fn parent_profile_lists_child_in_the_caller_selected_academic_year() {
        let pool = create_named_test_pool("parent_profile_selected_year").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_phase_b_runtime_migrations(&pool).await.unwrap();

        let parent_id = create_test_user(&pool, "parent-year@example.test", "test-password")
            .await
            .unwrap();
        sqlx::query("UPDATE users SET user_type = 'parent' WHERE id = $1")
            .bind(parent_id)
            .execute(&pool)
            .await
            .unwrap();
        let student_id = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
        sqlx::query(
            "INSERT INTO student_parents (
                 student_user_id, parent_user_id, relationship, is_primary
             ) VALUES ($1, $2, 'parent', true)",
        )
        .bind(student_id)
        .bind(parent_id)
        .execute(&pool)
        .await
        .unwrap();
        let year_2025_id: Uuid =
            sqlx::query_scalar("SELECT id FROM academic_years WHERE year = 2025")
                .fetch_one(&pool)
                .await
                .unwrap();
        let year_2026_id: Uuid =
            sqlx::query_scalar("SELECT id FROM academic_years WHERE year = 2026")
                .fetch_one(&pool)
                .await
                .unwrap();

        let current = get_own_parent_profile(&pool, parent_id, year_2025_id)
            .await
            .unwrap();
        let planned = get_own_parent_profile(&pool, parent_id, year_2026_id)
            .await
            .unwrap();

        assert_eq!(
            current.children[0].homeroom.as_deref(),
            Some("ม.1/1 ปี 2025")
        );
        assert_eq!(
            planned.children[0].homeroom.as_deref(),
            Some("ม.1/1 ปี 2026")
        );
    }

    #[tokio::test]
    async fn parent_academic_context_contains_only_linked_student_years() {
        let pool = create_named_test_pool("parent_academic_context").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_phase_b_runtime_migrations(&pool).await.unwrap();
        apply_migrations_through(&pool, 52).await.unwrap();

        let parent_id = create_test_user(&pool, "parent-context@example.test", "test-password")
            .await
            .unwrap();
        sqlx::query("UPDATE users SET user_type = 'parent' WHERE id = $1")
            .bind(parent_id)
            .execute(&pool)
            .await
            .unwrap();
        let student_id = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
        sqlx::query(
            "INSERT INTO student_parents (
                 student_user_id, parent_user_id, relationship, is_primary
             ) VALUES ($1, $2, 'parent', true)",
        )
        .bind(student_id)
        .bind(parent_id)
        .execute(&pool)
        .await
        .unwrap();

        let options = get_parent_academic_context_options(&pool, parent_id)
            .await
            .unwrap();
        let expected_years: Vec<i32> = sqlx::query_scalar(
            r#"
            SELECT year.year
            FROM student_academic_years student_year
            JOIN academic_years year ON year.id = student_year.academic_year_id
            WHERE student_year.student_id = $1
            ORDER BY year.year DESC
            "#,
        )
        .bind(student_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            options
                .years
                .iter()
                .map(|year| year.year)
                .collect::<Vec<_>>(),
            expected_years
        );
        assert!(options.terms.iter().all(|term| options
            .years
            .iter()
            .any(|year| year.id == term.academic_year_id)));

        let staff_id = create_test_user(&pool, "staff-context@example.test", "test-password")
            .await
            .unwrap();
        assert!(matches!(
            get_parent_academic_context_options(&pool, staff_id).await,
            Err(AppError::Forbidden(_))
        ));
    }

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
