use serde::Serialize;
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffDashboardOverview {
    pub total_staff: i64,
    pub total_students: i64,
    pub active_homerooms: i64,
}

#[derive(Debug, FromRow)]
struct DashboardCountRow {
    total_staff: i64,
    total_students: i64,
    active_homerooms: i64,
}

fn dashboard_response_from_counts(row: DashboardCountRow) -> StaffDashboardOverview {
    StaffDashboardOverview {
        total_staff: row.total_staff,
        total_students: row.total_students,
        active_homerooms: row.active_homerooms,
    }
}

pub async fn ensure_active_staff_user(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let is_active_staff: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM users
            WHERE id = $1
              AND user_type = 'staff'
              AND status = 'active'
        )",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to verify active staff dashboard user: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบผู้ใช้งานได้".to_string())
    })?;

    if is_active_staff {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "แดชบอร์ดนี้สำหรับบุคลากรที่ใช้งานอยู่เท่านั้น".to_string(),
        ))
    }
}

pub async fn get_staff_dashboard(
    pool: &PgPool,
    academic_year_id: Uuid,
) -> Result<StaffDashboardOverview, AppError> {
    let row = sqlx::query_as::<_, DashboardCountRow>(
        r#"
        SELECT
            (SELECT COUNT(*) FROM users WHERE user_type = 'staff' AND status = 'active') AS total_staff,
            (SELECT COUNT(*) FROM users WHERE user_type = 'student' AND status = 'active') AS total_students,
            (SELECT COUNT(*)
             FROM homerooms
             WHERE academic_year_id = $1 AND is_active = true) AS active_homerooms
        "#,
    )
    .bind(academic_year_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load staff dashboard counts: {}", error);
        AppError::InternalServerError("ไม่สามารถโหลดภาพรวมโรงเรียนได้".to_string())
    })?;

    Ok(dashboard_response_from_counts(row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::academic::cutover_test_support::{
            apply_migrations_through, apply_phase_b_runtime_migrations,
            seed_academic_cutover_fixture, CutoverFixture,
        },
        test_helpers::create_named_test_pool,
    };

    #[test]
    fn dashboard_response_from_counts_maps_snake_case_row_to_camel_case_dto_fields() {
        let row = DashboardCountRow {
            total_staff: 84,
            total_students: 1248,
            active_homerooms: 42,
        };

        let response = dashboard_response_from_counts(row);

        assert_eq!(response.total_staff, 84);
        assert_eq!(response.total_students, 1248);
        assert_eq!(response.active_homerooms, 42);
    }

    #[tokio::test]
    async fn dashboard_counts_homerooms_only_in_the_selected_academic_year() {
        let pool = create_named_test_pool("staff_dashboard_selected_year").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_phase_b_runtime_migrations(&pool).await.unwrap();

        let selected_year_id: Uuid =
            sqlx::query_scalar("SELECT id FROM academic_years WHERE year = 2025")
                .fetch_one(&pool)
                .await
                .unwrap();
        let other_year_id: Uuid =
            sqlx::query_scalar("SELECT id FROM academic_years WHERE year = 2024")
                .fetch_one(&pool)
                .await
                .unwrap();

        let selected = get_staff_dashboard(&pool, selected_year_id).await.unwrap();
        let other = get_staff_dashboard(&pool, other_year_id).await.unwrap();

        assert_eq!(selected.active_homerooms, 2);
        assert_eq!(other.active_homerooms, 1);
    }
}
