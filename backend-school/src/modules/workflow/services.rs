use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::workflow::models::{
    WorkflowWindow, WorkflowWindowMetadata, WorkflowWindowStatus,
};
use crate::policies::workflow_access_policy::WorkflowWindowManageAccess;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WorkflowWindowSchedule {
    pub opens_at: Option<DateTime<Utc>>,
    pub due_at: Option<DateTime<Utc>>,
    pub closes_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WorkflowWindowScheduleError {
    OpensAfterDue,
    OpensAfterClose,
    DueAfterClose,
}

impl WorkflowWindowScheduleError {
    fn message(self) -> &'static str {
        match self {
            Self::OpensAfterDue => "เวลาเปิดต้องอยู่ก่อนหรือเท่ากับกำหนดส่ง",
            Self::OpensAfterClose => "เวลาเปิดต้องอยู่ก่อนหรือเท่ากับเวลาปิด",
            Self::DueAfterClose => "กำหนดส่งต้องอยู่ก่อนหรือเท่ากับเวลาปิด",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowWindowTimeState {
    Draft,
    Scheduled,
    Open,
    DueSoon,
    Overdue,
    Closed,
    Archived,
}

pub struct CreateWorkflowWindowInput {
    pub module_code: String,
    pub workflow_code: String,
    pub title: String,
    pub description: Option<String>,
    pub organization_unit_id: Option<Uuid>,
    pub managed_by_permission: String,
    pub schedule: WorkflowWindowSchedule,
    pub metadata: WorkflowWindowMetadata,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowWindowFilter {
    pub module_code: Option<String>,
    pub status: Option<WorkflowWindowStatus>,
}

pub fn validate_workflow_window_schedule(
    schedule: WorkflowWindowSchedule,
) -> Result<(), WorkflowWindowScheduleError> {
    if let (Some(opens_at), Some(due_at)) = (schedule.opens_at, schedule.due_at) {
        if opens_at > due_at {
            return Err(WorkflowWindowScheduleError::OpensAfterDue);
        }
    }

    if let (Some(opens_at), Some(closes_at)) = (schedule.opens_at, schedule.closes_at) {
        if opens_at > closes_at {
            return Err(WorkflowWindowScheduleError::OpensAfterClose);
        }
    }

    if let (Some(due_at), Some(closes_at)) = (schedule.due_at, schedule.closes_at) {
        if due_at > closes_at {
            return Err(WorkflowWindowScheduleError::DueAfterClose);
        }
    }

    Ok(())
}

pub fn workflow_window_time_state(
    status: WorkflowWindowStatus,
    opens_at: Option<DateTime<Utc>>,
    due_at: Option<DateTime<Utc>>,
    closes_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> WorkflowWindowTimeState {
    match status {
        WorkflowWindowStatus::Draft => return WorkflowWindowTimeState::Draft,
        WorkflowWindowStatus::Closed => return WorkflowWindowTimeState::Closed,
        WorkflowWindowStatus::Archived => return WorkflowWindowTimeState::Archived,
        WorkflowWindowStatus::Open => {}
    }

    if closes_at.is_some_and(|closes_at| now > closes_at) {
        return WorkflowWindowTimeState::Closed;
    }

    if opens_at.is_some_and(|opens_at| now < opens_at) {
        return WorkflowWindowTimeState::Scheduled;
    }

    if due_at.is_some_and(|due_at| now > due_at) {
        return WorkflowWindowTimeState::Overdue;
    }

    if due_at.is_some_and(|due_at| due_at - now <= chrono::Duration::hours(24)) {
        return WorkflowWindowTimeState::DueSoon;
    }

    WorkflowWindowTimeState::Open
}

pub fn can_transition_workflow_window_status(
    from: WorkflowWindowStatus,
    to: WorkflowWindowStatus,
) -> bool {
    use WorkflowWindowStatus::{Archived, Closed, Draft, Open};

    matches!(
        (from, to),
        (Draft, Draft)
            | (Draft, Open)
            | (Draft, Closed)
            | (Open, Open)
            | (Open, Closed)
            | (Closed, Closed)
            | (Closed, Open)
            | (Closed, Archived)
            | (Archived, Archived)
    )
}

pub async fn create_workflow_window(
    pool: &PgPool,
    input: CreateWorkflowWindowInput,
) -> Result<WorkflowWindow, AppError> {
    validate_workflow_window_schedule(input.schedule)
        .map_err(|error| AppError::ValidationError(error.message().to_string()))?;

    sqlx::query_as::<_, WorkflowWindow>(
        r#"
        INSERT INTO workflow_windows (
            module_code, workflow_code, title, description, organization_unit_id,
            managed_by_permission, opens_at, due_at, closes_at, metadata, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id, module_code, workflow_code, title, description, organization_unit_id,
                  managed_by_permission, opens_at, due_at, closes_at, status, metadata,
                  created_by, created_at, updated_at
        "#,
    )
    .bind(&input.module_code)
    .bind(&input.workflow_code)
    .bind(&input.title)
    .bind(&input.description)
    .bind(input.organization_unit_id)
    .bind(&input.managed_by_permission)
    .bind(input.schedule.opens_at)
    .bind(input.schedule.due_at)
    .bind(input.schedule.closes_at)
    .bind(Json(input.metadata))
    .bind(input.created_by)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to create workflow window: {}", error);
        AppError::InternalServerError("ไม่สามารถสร้างรอบงานได้".to_string())
    })
}

pub async fn list_manageable_workflow_windows(
    pool: &PgPool,
    access: WorkflowWindowManageAccess,
    filter: WorkflowWindowFilter,
) -> Result<Vec<WorkflowWindow>, AppError> {
    let WorkflowWindowManageAccess::Permissions(permissions) = &access else {
        return list_workflow_windows_query(pool, access, filter).await;
    };

    if permissions.is_empty() {
        return Ok(Vec::new());
    }

    list_workflow_windows_query(pool, access, filter).await
}

pub async fn close_workflow_window(pool: &PgPool, id: Uuid) -> Result<WorkflowWindow, AppError> {
    set_workflow_window_status(pool, id, WorkflowWindowStatus::Closed).await
}

pub async fn open_workflow_window(pool: &PgPool, id: Uuid) -> Result<WorkflowWindow, AppError> {
    set_workflow_window_status(pool, id, WorkflowWindowStatus::Open).await
}

pub async fn get_workflow_window(pool: &PgPool, id: Uuid) -> Result<WorkflowWindow, AppError> {
    sqlx::query_as::<_, WorkflowWindow>(
        r#"
        SELECT id, module_code, workflow_code, title, description, organization_unit_id,
               managed_by_permission, opens_at, due_at, closes_at, status, metadata,
               created_by, created_at, updated_at
        FROM workflow_windows
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get workflow window: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลรอบงานได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบงาน".to_string()))
}

pub async fn set_workflow_window_status(
    pool: &PgPool,
    id: Uuid,
    status: WorkflowWindowStatus,
) -> Result<WorkflowWindow, AppError> {
    let current_status =
        sqlx::query_scalar::<_, String>("SELECT status FROM workflow_windows WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|error| {
                tracing::error!("Failed to load workflow window status: {}", error);
                AppError::InternalServerError("ไม่สามารถตรวจสอบสถานะรอบงานได้".to_string())
            })?
            .ok_or_else(|| AppError::NotFound("ไม่พบรอบงาน".to_string()))?;

    let Some(current_status) = WorkflowWindowStatus::from_code(&current_status) else {
        return Err(AppError::InternalServerError(
            "สถานะรอบงานไม่ถูกต้อง".to_string(),
        ));
    };

    if !can_transition_workflow_window_status(current_status, status) {
        return Err(AppError::ValidationError(
            "ไม่สามารถเปลี่ยนสถานะรอบงานตามลำดับนี้ได้".to_string(),
        ));
    }

    sqlx::query_as::<_, WorkflowWindow>(
        r#"
        UPDATE workflow_windows
        SET status = $2
        WHERE id = $1
        RETURNING id, module_code, workflow_code, title, description, organization_unit_id,
                  managed_by_permission, opens_at, due_at, closes_at, status, metadata,
                  created_by, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(status.as_str())
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to update workflow window status: {}", error);
        AppError::InternalServerError("ไม่สามารถอัปเดตรอบงานได้".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบงาน".to_string()))
}

async fn list_workflow_windows_query(
    pool: &PgPool,
    access: WorkflowWindowManageAccess,
    filter: WorkflowWindowFilter,
) -> Result<Vec<WorkflowWindow>, AppError> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT id, module_code, workflow_code, title, description, organization_unit_id,
                managed_by_permission, opens_at, due_at, closes_at, status, metadata,
                created_by, created_at, updated_at
         FROM workflow_windows
         WHERE true",
    );

    match access {
        WorkflowWindowManageAccess::All => {}
        WorkflowWindowManageAccess::Permissions(permissions) => {
            query.push(" AND managed_by_permission = ANY(");
            query.push_bind(permissions);
            query.push(")");
        }
    }

    if let Some(module_code) = filter.module_code {
        query.push(" AND module_code = ");
        query.push_bind(module_code);
    }

    if let Some(status) = filter.status {
        query.push(" AND status = ");
        query.push_bind(status.as_str());
    }

    query.push(" ORDER BY COALESCE(due_at, closes_at, opens_at, created_at), created_at DESC");

    query
        .build_query_as::<WorkflowWindow>()
        .fetch_all(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to list workflow windows: {}", error);
            AppError::InternalServerError("ไม่สามารถดึงข้อมูลรอบงานได้".to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn schedule_validation_rejects_dates_out_of_order() {
        let now = Utc::now();
        let err = validate_workflow_window_schedule(WorkflowWindowSchedule {
            opens_at: Some(now + Duration::days(2)),
            due_at: Some(now + Duration::days(1)),
            closes_at: Some(now + Duration::days(3)),
        })
        .expect_err("open after due must be rejected");

        assert_eq!(err, WorkflowWindowScheduleError::OpensAfterDue);

        let err = validate_workflow_window_schedule(WorkflowWindowSchedule {
            opens_at: Some(now),
            due_at: Some(now + Duration::days(3)),
            closes_at: Some(now + Duration::days(2)),
        })
        .expect_err("due after close must be rejected");

        assert_eq!(err, WorkflowWindowScheduleError::DueAfterClose);
    }

    #[test]
    fn lifecycle_state_reflects_schedule_and_status() {
        let now = Utc::now();
        let opens_at = now + Duration::hours(1);
        let due_at = now + Duration::hours(36);
        let closes_at = now + Duration::hours(48);

        assert_eq!(
            workflow_window_time_state(
                WorkflowWindowStatus::Open,
                Some(opens_at),
                Some(due_at),
                Some(closes_at),
                now
            ),
            WorkflowWindowTimeState::Scheduled
        );

        assert_eq!(
            workflow_window_time_state(
                WorkflowWindowStatus::Open,
                Some(now - Duration::hours(1)),
                Some(now + Duration::hours(12)),
                Some(closes_at),
                now
            ),
            WorkflowWindowTimeState::DueSoon
        );

        assert_eq!(
            workflow_window_time_state(
                WorkflowWindowStatus::Open,
                Some(now - Duration::hours(48)),
                Some(now - Duration::hours(1)),
                Some(closes_at),
                now
            ),
            WorkflowWindowTimeState::Overdue
        );

        assert_eq!(
            workflow_window_time_state(
                WorkflowWindowStatus::Open,
                Some(now - Duration::hours(48)),
                Some(now - Duration::hours(24)),
                Some(now - Duration::hours(1)),
                now
            ),
            WorkflowWindowTimeState::Closed
        );
    }

    #[test]
    fn status_transition_rules_allow_reopen_but_not_archive_reopen() {
        assert!(can_transition_workflow_window_status(
            WorkflowWindowStatus::Draft,
            WorkflowWindowStatus::Open
        ));
        assert!(can_transition_workflow_window_status(
            WorkflowWindowStatus::Open,
            WorkflowWindowStatus::Closed
        ));
        assert!(can_transition_workflow_window_status(
            WorkflowWindowStatus::Closed,
            WorkflowWindowStatus::Open
        ));
        assert!(can_transition_workflow_window_status(
            WorkflowWindowStatus::Closed,
            WorkflowWindowStatus::Archived
        ));
        assert!(!can_transition_workflow_window_status(
            WorkflowWindowStatus::Archived,
            WorkflowWindowStatus::Open
        ));
    }
}
