use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::workflow::models::WorkflowWindowStatus;
use crate::modules::workflow::services::{workflow_window_time_state, WorkflowWindowTimeState};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemLifecycleStatus {
    Active,
    Closed,
    Cancelled,
    Archived,
}

impl WorkItemLifecycleStatus {
    fn from_code(status: &str) -> Option<Self> {
        match status {
            "active" => Some(Self::Active),
            "closed" => Some(Self::Closed),
            "cancelled" => Some(Self::Cancelled),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemAssigneeType {
    User,
    OrganizationUnit,
    OrganizationPosition,
}

impl WorkItemAssigneeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::OrganizationUnit => "organization_unit",
            Self::OrganizationPosition => "organization_position",
        }
    }

    fn from_code(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "organization_unit" => Some(Self::OrganizationUnit),
            "organization_position" => Some(Self::OrganizationPosition),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemAssigneeStatus {
    Assigned,
    Read,
    Submitted,
    Dismissed,
}

impl WorkItemAssigneeStatus {
    fn from_code(value: &str) -> Option<Self> {
        match value {
            "assigned" => Some(Self::Assigned),
            "read" => Some(Self::Read),
            "submitted" => Some(Self::Submitted),
            "dismissed" => Some(Self::Dismissed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemState {
    Scheduled,
    Open,
    DueSoon,
    Overdue,
    Submitted,
    Closed,
    Archived,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkItemMetadata {
    #[serde(default)]
    pub tags: Vec<String>,
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkItemAssigneeTargetInput {
    pub assignee_type: WorkItemAssigneeType,
    pub user_id: Option<Uuid>,
    pub organization_unit_id: Option<Uuid>,
    pub position_code: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkItemAssigneeTarget {
    pub assignee_type: WorkItemAssigneeType,
    pub user_id: Option<Uuid>,
    pub organization_unit_id: Option<Uuid>,
    pub position_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateWorkItemInput {
    pub workflow_window_id: Uuid,
    pub module_code: String,
    pub source_resource_type: String,
    pub source_resource_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub action_path: String,
    pub required_permission: Option<String>,
    pub metadata: WorkItemMetadata,
    pub assignees: Vec<WorkItemAssigneeTargetInput>,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkItemFilter {
    pub module_code: Option<String>,
    pub state: Option<WorkItemState>,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkItemCounts {
    pub open: i64,
    pub due_soon: i64,
    pub overdue: i64,
    pub submitted: i64,
    pub closed: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkItem {
    pub id: Uuid,
    pub workflow_window_id: Uuid,
    pub module_code: String,
    pub workflow_code: String,
    pub source_resource_type: String,
    pub source_resource_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub action_path: String,
    pub required_permission: Option<String>,
    pub item_status: WorkItemLifecycleStatus,
    pub assignee_id: Uuid,
    pub assignee_type: WorkItemAssigneeType,
    pub assignee_status: WorkItemAssigneeStatus,
    pub state: WorkItemState,
    pub opens_at: Option<DateTime<Utc>>,
    pub due_at: Option<DateTime<Utc>>,
    pub closes_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub metadata: WorkItemMetadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct WorkItemAssignmentRow {
    id: Uuid,
    workflow_window_id: Uuid,
    module_code: String,
    workflow_code: String,
    source_resource_type: String,
    source_resource_id: Option<Uuid>,
    title: String,
    description: Option<String>,
    action_path: String,
    required_permission: Option<String>,
    item_status: String,
    metadata: Json<WorkItemMetadata>,
    assignee_id: Uuid,
    assignee_type: String,
    assignee_status: String,
    opens_at: Option<DateTime<Utc>>,
    due_at: Option<DateTime<Utc>>,
    closes_at: Option<DateTime<Utc>>,
    window_status: String,
    read_at: Option<DateTime<Utc>>,
    submitted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub fn derive_work_item_state(
    assignee_status: WorkItemAssigneeStatus,
    window_state: WorkflowWindowTimeState,
    item_status: WorkItemLifecycleStatus,
) -> WorkItemState {
    if assignee_status == WorkItemAssigneeStatus::Submitted {
        return WorkItemState::Submitted;
    }

    if assignee_status == WorkItemAssigneeStatus::Dismissed {
        return WorkItemState::Closed;
    }

    match item_status {
        WorkItemLifecycleStatus::Archived => return WorkItemState::Archived,
        WorkItemLifecycleStatus::Closed | WorkItemLifecycleStatus::Cancelled => {
            return WorkItemState::Closed;
        }
        WorkItemLifecycleStatus::Active => {}
    }

    match window_state {
        WorkflowWindowTimeState::Draft | WorkflowWindowTimeState::Scheduled => {
            WorkItemState::Scheduled
        }
        WorkflowWindowTimeState::Open => WorkItemState::Open,
        WorkflowWindowTimeState::DueSoon => WorkItemState::DueSoon,
        WorkflowWindowTimeState::Overdue => WorkItemState::Overdue,
        WorkflowWindowTimeState::Closed => WorkItemState::Closed,
        WorkflowWindowTimeState::Archived => WorkItemState::Archived,
    }
}

pub fn work_item_counts_from_states(states: &[WorkItemState]) -> WorkItemCounts {
    let mut counts = WorkItemCounts {
        open: 0,
        due_soon: 0,
        overdue: 0,
        submitted: 0,
        closed: 0,
        total: states.len() as i64,
    };

    for state in states {
        match state {
            WorkItemState::Scheduled | WorkItemState::Open => counts.open += 1,
            WorkItemState::DueSoon => counts.due_soon += 1,
            WorkItemState::Overdue => counts.overdue += 1,
            WorkItemState::Submitted => counts.submitted += 1,
            WorkItemState::Closed | WorkItemState::Archived => counts.closed += 1,
        }
    }

    counts
}

pub fn normalize_assignee_target(
    target: WorkItemAssigneeTargetInput,
) -> Result<WorkItemAssigneeTarget, AppError> {
    let position_code = target
        .position_code
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let valid = match target.assignee_type {
        WorkItemAssigneeType::User => {
            target.user_id.is_some()
                && target.organization_unit_id.is_none()
                && position_code.is_none()
        }
        WorkItemAssigneeType::OrganizationUnit => {
            target.user_id.is_none()
                && target.organization_unit_id.is_some()
                && position_code.is_none()
        }
        WorkItemAssigneeType::OrganizationPosition => {
            target.user_id.is_none()
                && target.organization_unit_id.is_some()
                && position_code.is_some()
        }
    };

    if !valid {
        return Err(AppError::ValidationError(
            "รูปแบบผู้รับมอบหมายงานไม่ถูกต้อง".to_string(),
        ));
    }

    Ok(WorkItemAssigneeTarget {
        assignee_type: target.assignee_type,
        user_id: target.user_id,
        organization_unit_id: target.organization_unit_id,
        position_code,
    })
}

pub async fn create_work_item(pool: &PgPool, input: CreateWorkItemInput) -> Result<Uuid, AppError> {
    if input.assignees.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องระบุผู้รับมอบหมายงานอย่างน้อยหนึ่งรายการ".to_string(),
        ));
    }

    let assignees = input
        .assignees
        .into_iter()
        .map(normalize_assignee_target)
        .collect::<Result<Vec<_>, _>>()?;

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!("Failed to begin work item transaction: {}", error);
        AppError::InternalServerError("ไม่สามารถสร้างงานได้".to_string())
    })?;

    let item_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO work_items (
            workflow_window_id, module_code, source_resource_type, source_resource_id,
            title, description, action_path, required_permission, metadata, created_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(input.workflow_window_id)
    .bind(&input.module_code)
    .bind(&input.source_resource_type)
    .bind(input.source_resource_id)
    .bind(&input.title)
    .bind(&input.description)
    .bind(&input.action_path)
    .bind(&input.required_permission)
    .bind(Json(input.metadata))
    .bind(input.created_by)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to insert work item: {}", error);
        AppError::InternalServerError("ไม่สามารถสร้างงานได้".to_string())
    })?;

    for assignee in assignees {
        sqlx::query(
            r#"
            INSERT INTO work_item_assignees (
                work_item_id, assignee_type, user_id, organization_unit_id, position_code
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(item_id)
        .bind(assignee.assignee_type.as_str())
        .bind(assignee.user_id)
        .bind(assignee.organization_unit_id)
        .bind(&assignee.position_code)
        .execute(&mut *tx)
        .await
        .map_err(|error| {
            tracing::error!("Failed to insert work item assignee: {}", error);
            AppError::InternalServerError("ไม่สามารถมอบหมายงานได้".to_string())
        })?;
    }

    tx.commit().await.map_err(|error| {
        tracing::error!("Failed to commit work item transaction: {}", error);
        AppError::InternalServerError("ไม่สามารถสร้างงานได้".to_string())
    })?;

    Ok(item_id)
}

pub async fn list_my_work_items(
    pool: &PgPool,
    user_id: Uuid,
    filter: WorkItemFilter,
) -> Result<Vec<WorkItem>, AppError> {
    let rows = query_my_work_items(pool, user_id, &filter).await?;
    let mut items = map_work_item_rows(rows, Utc::now())?;
    if let Some(state) = filter.state {
        items.retain(|item| item.state == state);
    }
    Ok(items)
}

pub async fn get_my_work_counts(pool: &PgPool, user_id: Uuid) -> Result<WorkItemCounts, AppError> {
    let items = list_my_work_items(pool, user_id, WorkItemFilter::default()).await?;
    let states = items.iter().map(|item| item.state).collect::<Vec<_>>();
    Ok(work_item_counts_from_states(&states))
}

fn map_work_item_rows(
    rows: Vec<WorkItemAssignmentRow>,
    now: DateTime<Utc>,
) -> Result<Vec<WorkItem>, AppError> {
    rows.into_iter()
        .map(|row| {
            let item_status = WorkItemLifecycleStatus::from_code(&row.item_status)
                .ok_or_else(|| AppError::InternalServerError("สถานะงานไม่ถูกต้อง".to_string()))?;
            let assignee_type =
                WorkItemAssigneeType::from_code(&row.assignee_type).ok_or_else(|| {
                    AppError::InternalServerError("ประเภทผู้รับมอบหมายงานไม่ถูกต้อง".to_string())
                })?;
            let assignee_status = WorkItemAssigneeStatus::from_code(&row.assignee_status)
                .ok_or_else(|| {
                    AppError::InternalServerError("สถานะผู้รับมอบหมายงานไม่ถูกต้อง".to_string())
                })?;
            let window_status = WorkflowWindowStatus::from_code(&row.window_status)
                .ok_or_else(|| AppError::InternalServerError("สถานะรอบงานไม่ถูกต้อง".to_string()))?;
            let window_state = workflow_window_time_state(
                window_status,
                row.opens_at,
                row.due_at,
                row.closes_at,
                now,
            );
            let state = derive_work_item_state(assignee_status, window_state, item_status);

            Ok(WorkItem {
                id: row.id,
                workflow_window_id: row.workflow_window_id,
                module_code: row.module_code,
                workflow_code: row.workflow_code,
                source_resource_type: row.source_resource_type,
                source_resource_id: row.source_resource_id,
                title: row.title,
                description: row.description,
                action_path: row.action_path,
                required_permission: row.required_permission,
                item_status,
                assignee_id: row.assignee_id,
                assignee_type,
                assignee_status,
                state,
                opens_at: row.opens_at,
                due_at: row.due_at,
                closes_at: row.closes_at,
                read_at: row.read_at,
                submitted_at: row.submitted_at,
                metadata: row.metadata.0,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
        .collect()
}

async fn query_my_work_items(
    pool: &PgPool,
    user_id: Uuid,
    filter: &WorkItemFilter,
) -> Result<Vec<WorkItemAssignmentRow>, AppError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        WITH matched_work AS (
            SELECT
                wi.id,
                wi.workflow_window_id,
                wi.module_code,
                ww.workflow_code,
                wi.source_resource_type,
                wi.source_resource_id,
                wi.title,
                wi.description,
                wi.action_path,
                wi.required_permission,
                wi.status AS item_status,
                wi.metadata,
                wia.id AS assignee_id,
                wia.assignee_type,
                wia.status AS assignee_status,
                ww.opens_at,
                ww.due_at,
                ww.closes_at,
                ww.status AS window_status,
                wia.read_at,
                wia.submitted_at,
                wi.created_at,
                wi.updated_at,
                CASE wia.assignee_type
                    WHEN 'user' THEN 1
                    WHEN 'organization_position' THEN 2
                    ELSE 3
                END AS match_rank
            FROM work_items wi
            JOIN workflow_windows ww ON ww.id = wi.workflow_window_id
            JOIN work_item_assignees wia ON wia.work_item_id = wi.id
            LEFT JOIN organization_members om
                ON om.user_id = "#,
    );
    query.push_bind(user_id);
    query.push(
        r#"
               AND om.ended_at IS NULL
               AND om.organization_unit_id = wia.organization_unit_id
            WHERE wi.status <> 'archived'
              AND (
                (wia.assignee_type = 'user' AND wia.user_id = "#,
    );
    query.push_bind(user_id);
    query.push(
        r#")
                OR (wia.assignee_type = 'organization_unit' AND om.id IS NOT NULL)
                OR (
                    wia.assignee_type = 'organization_position'
                    AND om.id IS NOT NULL
                    AND om.position_code = wia.position_code
                )
              )
        "#,
    );

    if let Some(module_code) = &filter.module_code {
        query.push(" AND wi.module_code = ");
        query.push_bind(module_code);
    }

    query.push(
        r#"
        )
        SELECT DISTINCT ON (id)
            id, workflow_window_id, module_code, workflow_code, source_resource_type,
            source_resource_id, title, description, action_path, required_permission,
            item_status, metadata, assignee_id, assignee_type, assignee_status,
            opens_at, due_at, closes_at, window_status, read_at, submitted_at,
            created_at, updated_at
        FROM matched_work
        ORDER BY id, match_rank, COALESCE(due_at, closes_at, opens_at, created_at), created_at DESC
        "#,
    );

    let rows = query
        .build_query_as::<WorkItemAssignmentRow>()
        .fetch_all(pool)
        .await
        .map_err(|error| {
            tracing::error!("Failed to list my work items: {}", error);
            AppError::InternalServerError("ไม่สามารถดึงรายการงานได้".to_string())
        })?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_state_prefers_submitted_assignee_over_window_state() {
        assert_eq!(
            derive_work_item_state(
                WorkItemAssigneeStatus::Submitted,
                WorkflowWindowTimeState::Closed,
                WorkItemLifecycleStatus::Active
            ),
            WorkItemState::Submitted
        );
    }

    #[test]
    fn work_item_state_uses_window_time_state_for_active_assignments() {
        assert_eq!(
            derive_work_item_state(
                WorkItemAssigneeStatus::Assigned,
                WorkflowWindowTimeState::DueSoon,
                WorkItemLifecycleStatus::Active
            ),
            WorkItemState::DueSoon
        );
        assert_eq!(
            derive_work_item_state(
                WorkItemAssigneeStatus::Read,
                WorkflowWindowTimeState::Overdue,
                WorkItemLifecycleStatus::Active
            ),
            WorkItemState::Overdue
        );
        assert_eq!(
            derive_work_item_state(
                WorkItemAssigneeStatus::Assigned,
                WorkflowWindowTimeState::Closed,
                WorkItemLifecycleStatus::Active
            ),
            WorkItemState::Closed
        );
    }

    #[test]
    fn work_item_counts_group_derived_states() {
        let counts = work_item_counts_from_states(&[
            WorkItemState::Open,
            WorkItemState::DueSoon,
            WorkItemState::DueSoon,
            WorkItemState::Overdue,
            WorkItemState::Submitted,
            WorkItemState::Closed,
        ]);

        assert_eq!(counts.open, 1);
        assert_eq!(counts.due_soon, 2);
        assert_eq!(counts.overdue, 1);
        assert_eq!(counts.submitted, 1);
        assert_eq!(counts.closed, 1);
        assert_eq!(counts.total, 6);
    }

    #[test]
    fn normalize_assignee_target_rejects_invalid_shapes() {
        assert!(normalize_assignee_target(WorkItemAssigneeTargetInput {
            assignee_type: WorkItemAssigneeType::User,
            user_id: None,
            organization_unit_id: None,
            position_code: None,
        })
        .is_err());

        assert!(normalize_assignee_target(WorkItemAssigneeTargetInput {
            assignee_type: WorkItemAssigneeType::OrganizationPosition,
            user_id: None,
            organization_unit_id: Some(uuid::Uuid::new_v4()),
            position_code: None,
        })
        .is_err());

        assert!(normalize_assignee_target(WorkItemAssigneeTargetInput {
            assignee_type: WorkItemAssigneeType::OrganizationUnit,
            user_id: None,
            organization_unit_id: Some(uuid::Uuid::new_v4()),
            position_code: Some("head".to_string()),
        })
        .is_err());
    }
}
