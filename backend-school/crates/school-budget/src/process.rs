//! Budget authorizes a source; Workflow owns frozen routing, decisions, work and notification dispatch.
use crate::{
    models::*,
    money::{invalid, text},
    repository, settings,
};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_workflow::{
    process::{self, ProcessSourcePort, StartProcessInput},
    process_models::*,
};
use sqlx::{types::Json, Postgres, Transaction};
use uuid::Uuid;
pub(crate) struct Source {
    pub resource_type: &'static str,
    pub resource_id: Uuid,
    pub owner_id: Uuid,
    pub actor_id: Uuid,
}
impl Source {
    pub fn identity(&self) -> ProcessSource {
        ProcessSource {
            module_code: "budget".into(),
            resource_type: self.resource_type.into(),
            resource_id: self.resource_id,
        }
    }
}
#[async_trait::async_trait]
impl ProcessSourcePort for Source {
    async fn validate_existing(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        a: &ActorContext,
        i: &ProcessInstance,
    ) -> Result<(), AppError> {
        if a.user_id != self.actor_id
            || i.source.module_code != "budget"
            || i.source.resource_type != self.resource_type
            || i.source.resource_id != self.resource_id
        {
            return Err(repository::denied());
        }
        Ok(())
    }
    async fn validate_start(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        a: &ActorContext,
        s: &ProcessSource,
    ) -> Result<(), AppError> {
        if a.user_id != self.actor_id
            || s.module_code != "budget"
            || s.resource_type != self.resource_type
            || s.resource_id != self.resource_id
        {
            return Err(repository::denied());
        }
        Ok(())
    }
    async fn validate_action(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        a: &ActorContext,
        i: &ProcessInstance,
        r: &ProcessActionRequest,
    ) -> Result<(), AppError> {
        self.validate_existing(tx, a, i).await?;
        a.require_permission(codes::BUDGET_APPROVE_ASSIGNED)?;
        if a.user_id == self.owner_id
            || a.user_id == i.created_by
            || !matches!(
                r.action,
                ProcessAction::Review
                    | ProcessAction::Approve
                    | ProcessAction::Reject
                    | ProcessAction::ReturnForRevision
            )
            || r.signature.is_some()
            || !r.annotations.is_empty()
            || !r.evidence_file_ids.is_empty()
            || r.routing.is_some()
            || r.reassign_user_id.is_some()
            || r.assignment_id.is_some()
        {
            return Err(repository::denied());
        }
        let step = i.steps.get(i.current_step).ok_or_else(repository::stale)?;
        if !step
            .recipients
            .iter()
            .any(|p| p.user_id == a.user_id && p.completed_at.is_none())
            || matches!(r.action, ProcessAction::Approve)
                && step.definition.kind != ProcessStepKind::Approve
            || matches!(r.action, ProcessAction::Review)
                && step.definition.kind != ProcessStepKind::Review
        {
            return Err(repository::denied());
        }
        Ok(())
    }
    async fn validate_delegated_authority(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        _a: &ActorContext,
        _i: &ProcessInstance,
    ) -> Result<(), AppError> {
        Err(invalid("เส้นทางงบประมาณไม่รองรับการมอบอำนาจ"))
    }
    async fn completed(
        &self,
        _tx: &mut Transaction<'_, Postgres>,
        _i: &ProcessInstance,
    ) -> Result<(), AppError> {
        Ok(())
    }
}
pub(crate) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    id: Option<Uuid>,
) -> Result<Option<ProcessInstance>, AppError> {
    if let Some(id) = id {
        Ok(DbSession::new(&mut **tx)
            .scalar::<Json<ProcessInstance>>(
                "budget.src.process.load",
                "SELECT definition FROM process_instances WHERE id=$1",
            )
            .bind(id)
            .fetch_optional()
            .await?
            .map(|x| x.0))
    } else {
        Ok(None)
    }
}
pub(crate) async fn start(
    tx: &mut Transaction<'_, Postgres>,
    a: &ActorContext,
    s: &Source,
    key: Uuid,
    template: Option<Uuid>,
    title: String,
    path: String,
) -> Result<ProcessInstance, AppError> {
    let id = template.ok_or_else(|| invalid("กรุณาตั้งค่าเส้นทางอนุมัติงบประมาณก่อนส่ง"))?;
    let managed: Option<(Option<String>, Json<ProcessTemplate>)> = DbSession::new(&mut **tx)
        .query_as(
            "budget.src.process.start",
            "SELECT managed_source_module,definition FROM process_templates WHERE id=$1 FOR SHARE",
        )
        .bind(id)
        .fetch_optional()
        .await?;
    let (module, t) = managed.ok_or_else(|| invalid("ไม่พบเส้นทางอนุมัติ"))?;
    if module.as_deref() != Some("budget") {
        return Err(invalid("เส้นทางไม่ได้เป็นของระบบงบประมาณ"));
    }
    settings::validate_steps(&t.steps)?;
    let i = process::start_in_transaction(
        tx,
        a,
        s,
        StartProcessInput {
            request_id: key,
            source: s.identity(),
            template_id: id,
            title,
            action_path: path,
            step_recipients: vec![],
            due_at: None,
        },
    )
    .await?;
    if i.steps.iter().any(|step| {
        step.recipients
            .iter()
            .any(|r| r.user_id == s.owner_id || r.user_id == a.user_id || r.user_id == i.created_by)
    }) {
        return Err(invalid("ผู้ขอหรือผู้ส่งไม่สามารถตรวจหรืออนุมัติรายการของตนเอง"));
    }
    let recipients = i
        .steps
        .iter()
        .flat_map(|s| s.recipients.iter().map(|r| r.user_id))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let units:Vec<Uuid>=match s.resource_type {
     "budget_project_plan"=>DbSession::new(&mut **tx).scalar("budget.src.process.start", "SELECT p.organization_unit_id FROM budget_project_attempts a JOIN budget_projects p ON p.id=a.project_id WHERE a.id=$1").bind(s.resource_id).fetch_all().await?,
     "budget_request"=>DbSession::new(&mut **tx).scalar("budget.src.process.start", "SELECT p.organization_unit_id FROM budget_requests r JOIN budget_projects p ON p.id=r.project_id WHERE r.id=$1").bind(s.resource_id).fetch_all().await?,
     "budget_adjustment"=>DbSession::new(&mut **tx).scalar("budget.src.process.start", "SELECT DISTINCT p.organization_unit_id FROM budget_adjustments a JOIN budget_projects p ON p.id=a.from_project_id OR p.id=a.to_project_id WHERE a.id=$1").bind(s.resource_id).fetch_all().await?,
     _=>return Err(repository::denied()),
    };
    let target = school_authorization::resource_access_policy::ResourceAccessTarget {
        owner_user_id: Some(s.owner_id),
        assigned_user_ids: recipients.clone(),
        organization_unit_ids: units,
    };
    let eligible=school_authorization::resource_access_policy::eligible_active_staff_for_resource_on_connection(&mut **tx,&recipients,&[codes::BUDGET_APPROVE_ASSIGNED],school_authorization::resource_access_policy::ResourceAccessPermissions{own:Some(codes::BUDGET_READ_OWN),assigned:Some(codes::BUDGET_READ_ASSIGNED),school:Some(codes::BUDGET_READ_SCHOOL),organization_unit:Some(codes::BUDGET_READ_ORGANIZATION_UNIT),organization_tree:Some(codes::BUDGET_READ_ORGANIZATION_TREE)},&target).await?;
    if recipients.iter().any(|id| !eligible.contains(id)) {
        return Err(invalid(
            "ผู้รับทุกขั้นตอนต้องมีสิทธิ์พิจารณาและเข้าถึงงบประมาณรายการนี้ กรุณาปรับเส้นทางหรือสิทธิ์ก่อนส่ง",
        ));
    }
    Ok(i)
}
pub(crate) async fn act(
    tx: &mut Transaction<'_, Postgres>,
    a: &ActorContext,
    s: &Source,
    id: Option<Uuid>,
    input: &BudgetActionRequest,
) -> Result<ProcessInstance, AppError> {
    if !matches!(
        input.action,
        ProcessAction::Review
            | ProcessAction::Approve
            | ProcessAction::Reject
            | ProcessAction::ReturnForRevision
    ) {
        return Err(invalid("การดำเนินการไม่รองรับในงบประมาณ"));
    }
    if let Some(c) = &input.comment {
        text(c, 4000)?;
    }
    if matches!(
        input.action,
        ProcessAction::Reject | ProcessAction::ReturnForRevision
    ) && input.comment.as_ref().is_none_or(|s| s.trim().is_empty())
    {
        return Err(invalid("กรุณาระบุเหตุผลที่คืนหรือปฏิเสธ"));
    }
    process::act_source_in_transaction(
        tx,
        a,
        s,
        id.ok_or_else(|| invalid("ไม่พบกระบวนงานงบประมาณ"))?,
        &ProcessActionRequest {
            request_id: input.request_id,
            expected_revision: input.expected_process_revision,
            action: input.action,
            annotations: vec![],
            comment: input.comment.clone(),
            evidence_file_ids: vec![],
            signature: None,
            reassign_user_id: None,
            reassign_from_user_id: None,
            assignment_id: None,
            routing: None,
        },
    )
    .await
}
pub(crate) fn status(i: &ProcessInstance) -> BudgetStatus {
    match i.status {
        ProcessStatus::Active => BudgetStatus::Submitted,
        ProcessStatus::RevisionRequested => BudgetStatus::Returned,
        ProcessStatus::Completed => BudgetStatus::Approved,
        ProcessStatus::Rejected => BudgetStatus::Rejected,
        ProcessStatus::Closed => BudgetStatus::Closed,
    }
}
pub(crate) fn history_action(action: ProcessAction, status: BudgetStatus) -> &'static str {
    match action {
        ProcessAction::Review => "reviewed",
        ProcessAction::Approve => "approved",
        ProcessAction::Reject => "rejected",
        ProcessAction::ReturnForRevision => "returned",
        _ => status.as_str(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_completed_workflow_approves_source() {
        assert!(!BudgetStatus::Submitted.editable());
        assert!(BudgetStatus::Returned.editable());
        let source = Source {
            resource_type: "budget_request",
            resource_id: Uuid::now_v7(),
            owner_id: Uuid::now_v7(),
            actor_id: Uuid::now_v7(),
        };
        assert_eq!(source.identity().module_code, "budget");
    }
}
