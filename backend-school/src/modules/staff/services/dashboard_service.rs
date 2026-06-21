use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StaffDashboardOverview {
    pub total_staff: i64,
    pub total_students: i64,
    pub active_classrooms: i64,
}

#[derive(Debug, FromRow)]
struct DashboardCountRow {
    total_staff: i64,
    total_students: i64,
    active_classrooms: i64,
}

fn dashboard_response_from_counts(row: DashboardCountRow) -> StaffDashboardOverview {
    StaffDashboardOverview {
        total_staff: row.total_staff,
        total_students: row.total_students,
        active_classrooms: row.active_classrooms,
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

pub async fn get_staff_dashboard(pool: &PgPool) -> Result<StaffDashboardOverview, AppError> {
    let row = sqlx::query_as::<_, DashboardCountRow>(
        r#"
        SELECT
            (SELECT COUNT(*) FROM users WHERE user_type = 'staff' AND status = 'active') AS total_staff,
            (SELECT COUNT(*) FROM users WHERE user_type = 'student' AND status = 'active') AS total_students,
            (SELECT COUNT(*) FROM class_rooms WHERE is_active = true) AS active_classrooms
        "#,
    )
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

    #[test]
    fn dashboard_response_from_counts_maps_snake_case_row_to_camel_case_dto_fields() {
        let row = DashboardCountRow {
            total_staff: 84,
            total_students: 1248,
            active_classrooms: 42,
        };

        let response = dashboard_response_from_counts(row);

        assert_eq!(response.total_staff, 84);
        assert_eq!(response.total_students, 1248);
        assert_eq!(response.active_classrooms, 42);
    }
}
