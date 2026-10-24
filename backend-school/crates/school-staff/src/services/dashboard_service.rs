use school_database::DbSession;
use serde::Serialize;
use sqlx::{FromRow, PgPool};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

use school_errors::AppError;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
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
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.services.dashboard_service.ensure_active_staff_user",
    )
    .await?;
    ensure_active_staff_user_on_connection(&mut connection, user_id).await
}

pub async fn ensure_active_staff_user_on_connection(
    connection: &mut sqlx::PgConnection,
    user_id: Uuid,
) -> Result<(), AppError> {
    let is_active_staff: bool = DbSession::new(connection)
        .scalar(
            "staff.services.dashboard_service.ensure_active_staff_user_on_connection",
            "SELECT EXISTS(
            SELECT 1
            FROM users
            WHERE id = $1
              AND user_type = 'staff'
              AND status = 'active'
        )",
        )
        .bind(user_id)
        .fetch_one()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to verify active staff dashboard user: {}",
                school_database::error_class(&error)
            );
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
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.services.dashboard_service.get_staff_dashboard",
    )
    .await?;
    get_staff_dashboard_on_connection(&mut connection, academic_year_id).await
}

pub async fn get_staff_dashboard_on_connection(
    connection: &mut sqlx::PgConnection,
    academic_year_id: Uuid,
) -> Result<StaffDashboardOverview, AppError> {
    let row = DbSession::new(connection).query_as::<DashboardCountRow>("staff.services.dashboard_service.get_staff_dashboard_on_connection",
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
    .fetch_one()
    .await
    .map_err(|error| {
        tracing::error!("Failed to load staff dashboard counts: {}", school_database::error_class(&error));
        AppError::InternalServerError("ไม่สามารถโหลดภาพรวมโรงเรียนได้".to_string())
    })?;

    Ok(dashboard_response_from_counts(row))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
