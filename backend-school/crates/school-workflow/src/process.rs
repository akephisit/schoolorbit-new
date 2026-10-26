//! Reusable sequential process provider. Source validation remains with its owner.
use crate::process_models::*;
use chrono::{Duration, Utc};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(crate) fn invalid(message: &str) -> AppError {
    AppError::ValidationError(message.into())
}
pub(crate) fn denied() -> AppError {
    AppError::Forbidden("ไม่มีสิทธิ์ดำเนินการรายการนี้".into())
}
pub(crate) fn stale() -> AppError {
    AppError::Conflict("ข้อมูลเปลี่ยนแล้ว กรุณาตรวจข้อมูลล่าสุดก่อนยืนยัน".into())
}
pub fn payload_hash<T: Serialize>(value: &T) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid("ข้อมูลไม่ถูกต้อง"))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
/// Claim before checking expected revision; same request with different semantics is rejected.
pub(crate) async fn receipt<T: Serialize>(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    request: Uuid,
    operation: &str,
    resource: Uuid,
    payload: &T,
) -> Result<Option<Uuid>, AppError> {
    let hash = payload_hash(&(operation, resource, payload))?;
    let inserted:Option<Uuid>=DbSession::new(&mut **tx).scalar("workflow.src.process.receipt", "INSERT INTO workflow_mutation_receipts(actor_id,request_id,operation,payload_hash,resource_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING RETURNING resource_id").bind(actor).bind(request).bind(operation).bind(&hash).bind(resource).fetch_optional().await?;
    if inserted.is_some() {
        return Ok(None);
    }
    let (old_op,old_hash,old_id):(String,String,Uuid)=DbSession::new(&mut **tx).query_as("workflow.src.process.receipt", "SELECT operation,payload_hash,resource_id FROM workflow_mutation_receipts WHERE actor_id=$1 AND request_id=$2").bind(actor).bind(request).fetch_one().await?;
    if old_op != operation || old_hash != hash {
        return Err(AppError::Conflict("รหัสคำขอถูกใช้กับข้อมูลอื่นแล้ว".into()));
    }
    Ok(Some(old_id))
}
/// Creation receipts hash a stable nil resource because the server allocates its UUID.
pub(crate) async fn creation_receipt<T: Serialize>(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    request: Uuid,
    operation: &str,
    id: Uuid,
    payload: &T,
) -> Result<Option<Uuid>, AppError> {
    let hash = payload_hash(&(operation, payload))?;
    let inserted:Option<Uuid>=DbSession::new(&mut **tx).scalar("workflow.src.process.creation_receipt", "INSERT INTO workflow_mutation_receipts(actor_id,request_id,operation,payload_hash,resource_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING RETURNING resource_id").bind(actor).bind(request).bind(operation).bind(&hash).bind(id).fetch_optional().await?;
    if inserted.is_some() {
        return Ok(None);
    }
    let old:(String,String,Uuid)=DbSession::new(&mut **tx).query_as("workflow.src.process.creation_receipt", "SELECT operation,payload_hash,resource_id FROM workflow_mutation_receipts WHERE actor_id=$1 AND request_id=$2").bind(actor).bind(request).fetch_one().await?;
    if old.0 != operation || old.1 != hash {
        return Err(AppError::Conflict("รหัสคำขอถูกใช้กับข้อมูลอื่นแล้ว".into()));
    }
    Ok(Some(old.2))
}
pub fn validate_recipient(value: &ProcessRecipient) -> Result<(), AppError> {
    let valid = match value.kind {
        ProcessRecipientKind::User => {
            value.user_id.is_some()
                && value.organization_unit_id.is_none()
                && value.position_code.is_none()
        }
        ProcessRecipientKind::OrganizationUnit => {
            value.user_id.is_none()
                && value.organization_unit_id.is_some()
                && value.position_code.is_none()
        }
        ProcessRecipientKind::OrganizationPosition => {
            value.user_id.is_none()
                && value.organization_unit_id.is_some()
                && matches!(
                    value.position_code.as_deref(),
                    Some(
                        "director"
                            | "deputy_director"
                            | "head"
                            | "deputy_head"
                            | "coordinator"
                            | "member"
                    )
                )
        }
    };
    if valid {
        Ok(())
    } else {
        Err(invalid("ผู้รับหรือรูปแบบตำแหน่งไม่ถูกต้อง"))
    }
}
pub fn validate_steps(steps: &[ProcessTemplateStep]) -> Result<(), AppError> {
    if steps.is_empty() || steps.len() > 30 {
        return Err(invalid("ต้องมีขั้นตอน 1–30 ขั้นตอน"));
    }
    for step in steps {
        if step.title.trim().is_empty()
            || step.title.chars().count() > 200
            || step.recipients.len() > 100
            || step.due_after_days.is_some_and(|n| !(0..=365).contains(&n))
        {
            return Err(invalid("ชื่อขั้นตอน ผู้รับ หรือกำหนดเวลาไม่ถูกต้อง"));
        }
        for recipient in &step.recipients {
            validate_recipient(recipient)?;
        }
    }
    Ok(())
}
pub async fn list_templates(pool: &PgPool) -> Result<Vec<ProcessTemplate>, AppError> {
    let mut database_stage_0 =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "workflow.process.list_templates")
            .await?;
    Ok(DbSession::new(&mut *database_stage_0).scalar::<Json<ProcessTemplate>>("workflow.src.process.list_templates",
        "SELECT definition FROM process_templates WHERE managed_source_module IS NULL ORDER BY code,id LIMIT 200",
    )
    .fetch_all()
    .await?
    .into_iter()
    .map(|r| r.0)
    .collect())
}
pub async fn save_template(
    pool: &PgPool,
    actor: &ActorContext,
    id: Option<Uuid>,
    request: SaveProcessTemplateRequest,
) -> Result<ProcessTemplate, AppError> {
    actor.require_permission(codes::PROCESS_MANAGE_SCHOOL)?;
    validate_steps(&request.steps)?;
    if request.title.trim().is_empty()
        || request.title.chars().count() > 200
        || request.code.is_empty()
        || request.code.len() > 100
        || !request
            .code
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    {
        return Err(invalid("ชื่อหรือรหัสแม่แบบไม่ถูกต้อง"));
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "workflow.process.save_template").await?;
    let template_id = id.unwrap_or_else(Uuid::now_v7);
    let replay = if id.is_some() {
        receipt(
            &mut tx,
            actor.user_id,
            request.request_id,
            "template_update",
            template_id,
            &request,
        )
        .await?
    } else {
        creation_receipt(
            &mut tx,
            actor.user_id,
            request.request_id,
            "template_create",
            template_id,
            &request,
        )
        .await?
    };
    if let Some(replay) = replay {
        let result: Json<ProcessTemplate> = DbSession::new(&mut *tx)
            .scalar(
                "workflow.src.process.save_template",
                "SELECT definition FROM process_templates WHERE id=$1",
            )
            .bind(replay)
            .fetch_one()
            .await?;
        tx.commit().await?;
        return Ok(result.0);
    }
    let version = if id.is_some() {
        let old: i64 =
            DbSession::new(&mut *tx).scalar("workflow.src.process.save_template", "SELECT version FROM process_templates WHERE id=$1 AND managed_source_module IS NULL FOR UPDATE")
                .bind(template_id)
                .fetch_optional()
                .await?
                .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบ".into()))?;
        if request.expected_revision != Some(old) {
            return Err(stale());
        }
        old + 1
    } else {
        1
    };
    let result = ProcessTemplate {
        id: template_id,
        code: request.code,
        title: request.title,
        version,
        steps: request.steps,
        active: request.active,
        created_at: Utc::now(),
    };
    DbSession::new(&mut *tx).query("workflow.src.process.save_template", "INSERT INTO process_templates(id,code,version,definition,created_by) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO UPDATE SET code=excluded.code,version=excluded.version,definition=excluded.definition").bind(template_id).bind(&result.code).bind(version).bind(Json(&result)).bind(actor.user_id).execute().await?;
    DbSession::new(&mut *tx).query("workflow.src.process.save_template", "INSERT INTO process_template_versions(template_id,version,definition,created_by) VALUES($1,$2,$3,$4)").bind(template_id).bind(version).bind(Json(&result)).bind(actor.user_id).execute().await?;
    tx.commit().await?;
    Ok(result)
}

/// An owning domain validates its source and action within the caller's transaction.
/// There is deliberately no browser endpoint accepting arbitrary source identities.
#[async_trait::async_trait]
pub trait ProcessSourcePort: Send + Sync {
    fn may_reassign_others(&self, _actor: &ActorContext, _instance: &ProcessInstance) -> bool {
        false
    }

    async fn validate_existing(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        instance: &ProcessInstance,
    ) -> Result<(), AppError>;
    async fn validate_start(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        source: &ProcessSource,
    ) -> Result<(), AppError>;
    async fn validate_action(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        instance: &ProcessInstance,
        request: &ProcessActionRequest,
    ) -> Result<(), AppError>;
    async fn validate_delegated_authority(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        instance: &ProcessInstance,
    ) -> Result<(), AppError>;
    async fn completed(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        instance: &ProcessInstance,
    ) -> Result<(), AppError>;
}
#[derive(Clone, Serialize)]
pub struct StartProcessInput {
    pub request_id: Uuid,
    pub source: ProcessSource,
    pub template_id: Uuid,
    pub title: String,
    pub action_path: String,
    pub step_recipients: Vec<ProcessStepRecipients>,
    pub due_at: Option<chrono::DateTime<Utc>>,
}

async fn resolved_steps(
    tx: &mut Transaction<'_, Postgres>,
    template: &ProcessTemplate,
    overrides: &[ProcessStepRecipients],
) -> Result<Vec<ProcessRuntimeStep>, AppError> {
    let mut definitions = template.steps.clone();
    let mut due = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for value in overrides {
        if value.step_index >= definitions.len() || !seen.insert(value.step_index) {
            return Err(invalid("ขั้นตอนผู้รับซ้ำหรือไม่ถูกต้อง"));
        }
        definitions[value.step_index].recipients = value.recipients.clone();
        due.insert(value.step_index, value.due_at);
    }
    validate_steps(&definitions)?;
    let mut selectors = Vec::new();
    let mut ranges = Vec::new();
    for step in &definitions {
        if step.recipients.is_empty() {
            return Err(invalid("ต้องระบุผู้รับทุกขั้นตอนก่อนส่ง"));
        }
        let start = selectors.len();
        selectors.extend(step.recipients.clone());
        ranges.push(start..selectors.len());
    }
    let rows:Vec<(i64,Uuid,String)>=DbSession::new(&mut **tx).query_as("workflow.src.process.resolved_steps", "WITH selectors AS (SELECT value,ordinality::bigint-1 AS idx FROM jsonb_array_elements($1::jsonb) WITH ORDINALITY), matches AS (SELECT s.idx,u.id,u.display_name FROM selectors s JOIN user_display_names u ON u.status='active' AND u.user_type='staff' AND ((s.value->>'kind'='user' AND u.id=(s.value->>'userId')::uuid) OR (s.value->>'kind' IN ('organization_unit','organization_position') AND EXISTS (SELECT 1 FROM organization_members m JOIN organization_units ou ON ou.id=m.organization_unit_id AND ou.is_active WHERE m.user_id=u.id AND m.organization_unit_id=(s.value->>'organizationUnitId')::uuid AND m.started_at<=CURRENT_DATE AND (m.ended_at IS NULL OR m.ended_at>CURRENT_DATE) AND (s.value->>'kind'='organization_unit' OR m.position_code=s.value->>'positionCode'))))) SELECT DISTINCT idx,id,display_name FROM matches ORDER BY idx,id").bind(Json(&selectors)).fetch_all().await?;
    let mut resolved: BTreeMap<usize, Vec<(Uuid, String)>> = BTreeMap::new();
    for (index, id, name) in rows {
        resolved.entry(index as usize).or_default().push((id, name));
    }
    if (0..selectors.len()).any(|i| !resolved.contains_key(&i)) {
        return Err(invalid("มีผู้รับที่จำเป็นไม่พร้อมใช้งาน กรุณาเลือกผู้รับก่อนส่งใหม่"));
    }
    let mut result = Vec::new();
    for (index, definition) in definitions.into_iter().enumerate() {
        let mut people = BTreeMap::new();
        for selector in ranges[index].clone() {
            if let Some(users) = resolved.get(&selector) {
                for (id, name) in users {
                    people.insert(*id, name.clone());
                }
            }
        }
        if people.len() > 500 {
            return Err(invalid("ผู้รับต่อขั้นตอนเกิน 500 คน"));
        }
        let deadline = due.get(&index).copied().flatten();
        result.push(ProcessRuntimeStep {
            step_index: index,
            definition,
            recipients: people
                .into_iter()
                .map(|(user_id, display_name)| ProcessRecipientSnapshot {
                    user_id,
                    display_name,
                    completed_at: None,
                    read_at: None,
                    reassigned_from: None,
                })
                .collect(),
            due_at: deadline,
            work_item_id: None,
            completed_at: None,
        });
    }
    Ok(result)
}
pub(crate) async fn start_core_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_port: &dyn ProcessSourcePort,
    input: StartProcessInput,
) -> Result<ProcessInstance, AppError> {
    source_port.validate_start(tx, actor, &input.source).await?;
    if input.source.module_code.is_empty()
        || input.source.resource_type.is_empty()
        || !input.action_path.starts_with("/staff/")
        || input.action_path.starts_with("//")
        || input.title.trim().is_empty()
    {
        return Err(invalid("ข้อมูลแหล่งงานไม่ถูกต้อง"));
    }
    let existing:Option<Json<ProcessInstance>>=DbSession::new(&mut **tx).scalar("workflow.src.process.start_core_in_transaction", "SELECT definition FROM process_instances WHERE source_module=$1 AND source_type=$2 AND source_id=$3 FOR UPDATE").bind(&input.source.module_code).bind(&input.source.resource_type).bind(input.source.resource_id).fetch_optional().await?;
    let template: Json<ProcessTemplate> = DbSession::new(&mut **tx)
        .scalar(
            "workflow.src.process.start_core_in_transaction",
            "SELECT definition FROM process_templates WHERE id=$1 FOR SHARE",
        )
        .bind(input.template_id)
        .fetch_optional()
        .await?
        .ok_or_else(|| invalid("ไม่พบแม่แบบ"))?;
    if !template.active {
        return Err(invalid("แม่แบบถูกปิดใช้งาน"));
    }
    let mut instance = if let Some(old) = existing {
        let mut old = old.0;
        if old.status != ProcessStatus::RevisionRequested {
            return Err(stale());
        }
        old.steps = resolved_steps(tx, &template, &input.step_recipients).await?;
        old.template_id = Some(template.id);
        old.template_version = Some(template.version);
        old.directed = None;
        old.status = ProcessStatus::Active;
        old.current_step = 0;
        old.revision += 1;
        old.due_at = input.due_at;
        old
    } else {
        ProcessInstance {
            id: Uuid::now_v7(),
            source: input.source,
            title: input.title,
            action_path: input.action_path,
            template_id: Some(template.id),
            template_version: Some(template.version),
            revision: 1,
            status: ProcessStatus::Active,
            current_step: 0,
            steps: resolved_steps(tx, &template, &input.step_recipients).await?,
            created_by: actor.user_id,
            created_at: Utc::now(),
            due_at: input.due_at,
            directed: None,
        }
    };
    DbSession::new(&mut **tx).query("workflow.src.process.start_core_in_transaction", "INSERT INTO process_instances(id,source_module,source_type,source_id,template_id,template_version,revision,definition,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(id) DO UPDATE SET template_id=excluded.template_id,template_version=excluded.template_version,revision=excluded.revision,definition=excluded.definition").bind(instance.id).bind(&instance.source.module_code).bind(&instance.source.resource_type).bind(instance.source.resource_id).bind(instance.template_id).bind(instance.template_version).bind(instance.revision).bind(Json(&instance)).bind(actor.user_id).execute().await?;
    let users: Vec<Uuid> = instance
        .steps
        .iter()
        .flat_map(|s| s.recipients.iter().map(|r| r.user_id))
        .collect();
    DbSession::new(&mut **tx).query("workflow.src.process.start_core_in_transaction", "INSERT INTO process_participants(instance_id,user_id) SELECT $1,unnest($2::uuid[]) ON CONFLICT DO NOTHING").bind(instance.id).bind(users).execute().await?;
    activate_step(tx, &mut instance).await?;
    store(tx, &instance).await?;
    Ok(instance)
}
pub(crate) async fn store(
    tx: &mut Transaction<'_, Postgres>,
    instance: &ProcessInstance,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx).query("workflow.src.process.store", "INSERT INTO process_instance_revisions(instance_id,revision,definition) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(instance.id).bind(instance.revision).bind(Json(instance)).execute().await?;
    DbSession::new(&mut **tx)
        .query(
            "workflow.src.process.store",
            "UPDATE process_instances SET definition=$2,revision=$3 WHERE id=$1",
        )
        .bind(instance.id)
        .bind(Json(instance))
        .bind(instance.revision)
        .execute()
        .await?;
    Ok(())
}

/// Source-owned viewing may project read participation; it never grants decision authority.
pub(crate) async fn mark_read_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    instance: &mut ProcessInstance,
    assignment: Option<Uuid>,
) -> Result<bool, AppError> {
    if !actor.has_permission(codes::CORRESPONDENCE_READ_ASSIGNED) {
        return Ok(false);
    }
    let at = Utc::now();
    let mut changed = false;
    let mut items = Vec::new();
    if let Some(graph) = &mut instance.directed {
        for a in &mut graph.assignments {
            if assignment.is_some_and(|id| id != a.id) {
                continue;
            }
            let active = instance.status == ProcessStatus::Active;
            let terminal_copy = matches!(
                instance.status,
                ProcessStatus::Completed | ProcessStatus::Closed
            ) && !a.required;
            if a.status == DirectedAssignmentStatus::Pending
                && a.recipient.user_id == actor.user_id
                && (active || terminal_copy)
            {
                if a.recipient.read_at.is_none() {
                    a.recipient.read_at = Some(at);
                    changed = true;
                }
                items.push((a.work_item_id, a.recipient.read_at.unwrap_or(at)));
            }
            if active && a.status == DirectedAssignmentStatus::WaitingReview {
                if let Some(reviewer) = a.reviewer.as_mut().filter(|r| r.user_id == actor.user_id) {
                    if reviewer.read_at.is_none() {
                        reviewer.read_at = Some(at);
                        changed = true;
                    }
                    items.push((a.review_work_item_id, reviewer.read_at.unwrap_or(at)));
                }
            }
        }
    } else if instance.status == ProcessStatus::Active && assignment.is_none() {
        if let Some(step) = instance.steps.get_mut(instance.current_step) {
            for recipient in step
                .recipients
                .iter_mut()
                .filter(|r| r.user_id == actor.user_id && r.completed_at.is_none())
            {
                if recipient.read_at.is_none() {
                    recipient.read_at = Some(at);
                    changed = true;
                }
                items.push((step.work_item_id, recipient.read_at.unwrap_or(at)));
            }
        }
    }
    let mut work_changed = false;
    for (item, read_at) in items {
        work_changed |=
            crate::work::mark_process_read_in_transaction(tx, item, actor.user_id, read_at).await?;
    }
    if changed {
        instance.revision += 1;
        store(tx, instance).await?;
    }
    Ok(changed || work_changed)
}
pub async fn completion_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    source: &ProcessSource,
) -> Result<Option<ProcessStatus>, AppError> {
    Ok(DbSession::new(&mut **tx).scalar::<Json<ProcessInstance>>("workflow.src.process.completion_in_transaction", "SELECT definition FROM process_instances WHERE source_module=$1 AND source_type=$2 AND source_id=$3").bind(&source.module_code).bind(&source.resource_type).bind(source.resource_id).fetch_optional().await?.map(|r|r.0.status))
}
pub(crate) async fn load_locked(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<ProcessInstance, AppError> {
    Ok(DbSession::new(&mut **tx)
        .scalar::<Json<ProcessInstance>>(
            "workflow.src.process.load_locked",
            "SELECT definition FROM process_instances WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกระบวนงาน".into()))?
        .0)
}
async fn activate_step(
    tx: &mut Transaction<'_, Postgres>,
    instance: &mut ProcessInstance,
) -> Result<(), AppError> {
    let step = &mut instance.steps[instance.current_step];
    if step.due_at.is_none() {
        step.due_at = step
            .definition
            .due_after_days
            .map(|days| Utc::now() + Duration::days(i64::from(days)));
    }
    let window = Uuid::now_v7();
    let item = Uuid::now_v7();
    DbSession::new(&mut **tx).query("workflow.src.process.activate_step", "INSERT INTO workflow_windows(id,module_code,workflow_code,title,managed_by_permission,status,created_by,due_at) VALUES($1,$2,'process_step',$3,$4,'open',$5,$6)").bind(window).bind(&instance.source.module_code).bind(&step.definition.title).bind(codes::PROCESS_MANAGE_SCHOOL).bind(instance.created_by).bind(step.due_at.or(instance.due_at)).execute().await?;
    crate::work::create_work_item_in_transaction(
        tx,
        item,
        window,
        &instance.source.module_code,
        &instance.source.resource_type,
        instance.source.resource_id,
        &format!("{} · {}", instance.title, step.definition.title),
        &instance.action_path,
        instance.created_by,
        &step
            .recipients
            .iter()
            .map(|r| r.user_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    step.work_item_id = Some(item);
    let jobs: Vec<_> = step
        .recipients
        .iter()
        .map(|r| school_notifications::dispatch::NewNotification {
            id: Uuid::now_v7(),
            notification_id: Uuid::now_v7(),
            recipient_id: r.user_id,
            source: school_notifications::dispatch::NotificationSource::Process {
                source_id: Uuid::now_v7(),
            },
            idempotency_key: format!(
                "process:{}:{}:{}:{}",
                instance.id, instance.revision, instance.current_step, r.user_id
            ),
            title: instance.title.clone(),
            message: step.definition.title.clone(),
            type_: school_notifications::publisher::NotificationType::Info,
            presentation: school_notifications::dispatch::Presentation::AlertOnce,
            link: Some(instance.action_path.clone()),
        })
        .collect();
    for job in &jobs {
        if let school_notifications::dispatch::NotificationSource::Process { source_id } =
            job.source
        {
            DbSession::new(&mut **tx).query("workflow.src.process.activate_step", "INSERT INTO process_notification_sources(id,instance_id,recipient_id,work_item_id) VALUES($1,$2,$3,$4)").bind(source_id).bind(instance.id).bind(job.recipient_id).bind(item).execute().await?;
        }
    }
    school_notifications::dispatch::create_in_transaction(tx, &jobs).await?;
    Ok(())
}
pub fn step_satisfied(step: &ProcessRuntimeStep) -> bool {
    match step.definition.completion_policy {
        ProcessCompletionPolicy::All => {
            !step.recipients.is_empty() && step.recipients.iter().all(|r| r.completed_at.is_some())
        }
        ProcessCompletionPolicy::Any => step.recipients.iter().any(|r| r.completed_at.is_some()),
    }
}
pub fn action_matches(kind: ProcessStepKind, action: ProcessAction) -> bool {
    matches!(
        (kind, action),
        (ProcessStepKind::Acknowledge, ProcessAction::Acknowledge)
            | (ProcessStepKind::Review, ProcessAction::Review)
            | (ProcessStepKind::Approve, ProcessAction::Approve)
            | (ProcessStepKind::Execute, ProcessAction::Execute)
            | (ProcessStepKind::Sign, ProcessAction::Sign)
    )
}
pub(crate) async fn action_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_port: &dyn ProcessSourcePort,
    instance: &mut ProcessInstance,
    request: &ProcessActionRequest,
) -> Result<(), AppError> {
    source_port
        .validate_action(tx, actor, instance, request)
        .await?;
    if instance.revision != request.expected_revision {
        return Err(stale());
    }
    if request.action == ProcessAction::Close {
        if instance.created_by != actor.user_id
            || !matches!(
                instance.status,
                ProcessStatus::Completed | ProcessStatus::Rejected
            )
        {
            return Err(denied());
        }
        instance.status = ProcessStatus::Closed;
        instance.revision += 1;
        append_event(tx, actor, instance, request).await?;
        store(tx, instance).await?;
        return Ok(());
    }
    if instance.directed.is_some() {
        crate::directed::action(
            tx,
            actor,
            instance,
            request,
            source_port.may_reassign_others(actor, instance),
        )
        .await?;
        append_event(tx, actor, instance, request).await?;
        store(tx, instance).await?;
        return Ok(());
    }
    if request.assignment_id.is_some()
        || request.routing.is_some()
        || request.action == ProcessAction::Route
    {
        return Err(invalid("การส่งต่อแบบสั่งการใช้ได้เฉพาะกระบวนงานสั่งการ"));
    }
    if instance.status != ProcessStatus::Active {
        return Err(stale());
    }
    if instance.steps[instance.current_step]
        .definition
        .requires_delegated_authority
        && !matches!(
            request.action,
            ProcessAction::Read | ProcessAction::Report | ProcessAction::Reassign
        )
    {
        source_port
            .validate_delegated_authority(tx, actor, instance)
            .await?;
    }
    let from = if request.action == ProcessAction::Reassign {
        request.reassign_from_user_id.unwrap_or(actor.user_id)
    } else {
        actor.user_id
    };
    if from != actor.user_id && !source_port.may_reassign_others(actor, instance) {
        return Err(denied());
    }
    let step = &mut instance.steps[instance.current_step];
    let pos = step
        .recipients
        .iter()
        .position(|r| r.user_id == from && r.completed_at.is_none())
        .ok_or_else(denied)?;
    if request.action == ProcessAction::Read {
        step.recipients[pos].read_at.get_or_insert_with(Utc::now);
        crate::work::mark_process_assignment_in_transaction(
            tx,
            step.work_item_id,
            actor.user_id,
            false,
        )
        .await?;
    } else if request.action == ProcessAction::Reassign {
        if request
            .comment
            .as_deref()
            .is_none_or(|c| c.trim().is_empty())
        {
            return Err(invalid("กรุณาระบุเหตุผลการส่งต่อผู้รับงาน"));
        }
        let target = request
            .reassign_user_id
            .ok_or_else(|| invalid("กรุณาระบุผู้รับใหม่"))?;
        if step
            .recipients
            .iter()
            .any(|r| r.user_id == target && r.completed_at.is_none())
        {
            return Err(invalid("ผู้รับใหม่มีอยู่ในขั้นตอนนี้แล้ว"));
        }
        let name:String=DbSession::new(&mut **tx).scalar("workflow.src.process.action_in_transaction", "SELECT display_name FROM user_display_names WHERE id=$1 AND status='active' AND user_type='staff' FOR SHARE").bind(target).fetch_optional().await?.ok_or_else(||invalid("ผู้รับใหม่ไม่พร้อมใช้งาน"))?;
        // Keep the original recipient in immutable history; the replacement links back to it.
        step.recipients[pos].completed_at = Some(Utc::now());
        step.recipients.push(ProcessRecipientSnapshot {
            user_id: target,
            display_name: name,
            completed_at: None,
            read_at: None,
            reassigned_from: Some(from),
        });
        if let Some(item) = step.work_item_id {
            DbSession::new(&mut **tx).query("workflow.src.process.action_in_transaction", "UPDATE process_notification_sources SET cancelled_at=now() WHERE work_item_id=$1 AND recipient_id=$2 AND cancelled_at IS NULL").bind(item).bind(from).execute().await?;
            DbSession::new(&mut **tx).query("workflow.src.process.action_in_transaction", "UPDATE work_item_assignees SET status='dismissed' WHERE work_item_id=$1 AND user_id=$2").bind(item).bind(from).execute().await?;
            DbSession::new(&mut **tx).query("workflow.src.process.action_in_transaction", "INSERT INTO work_item_assignees(id,work_item_id,assignee_type,user_id) VALUES($1,$2,'user',$3) ON CONFLICT(work_item_id,user_id) WHERE assignee_type='user' DO UPDATE SET status='assigned',read_at=NULL,submitted_at=NULL").bind(Uuid::now_v7()).bind(item).bind(target).execute().await?;
        }
        DbSession::new(&mut **tx).query("workflow.src.process.action_in_transaction", "INSERT INTO process_participants(instance_id,user_id) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(instance.id).bind(target).execute().await?;
    } else if matches!(
        request.action,
        ProcessAction::Reject | ProcessAction::ReturnForRevision
    ) {
        if request
            .comment
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
        {
            return Err(invalid("กรุณาระบุเหตุผล"));
        }
        instance.status = if request.action == ProcessAction::Reject {
            ProcessStatus::Rejected
        } else {
            ProcessStatus::RevisionRequested
        };
        crate::work::close_process_item_in_transaction(tx, step.work_item_id).await?;
    } else if request.action == ProcessAction::Report {
        if request
            .comment
            .as_deref()
            .is_none_or(|s| s.trim().is_empty())
        {
            return Err(invalid("กรุณาระบุรายงาน"));
        }
    } else {
        if !action_matches(step.definition.kind, request.action) {
            return Err(invalid("การดำเนินการไม่ตรงกับขั้นตอน"));
        }
        if step.definition.requires_comment
            && request
                .comment
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
        {
            return Err(invalid("ขั้นตอนนี้ต้องมีความเห็น"));
        }
        if step.definition.requires_evidence && request.evidence_file_ids.is_empty() {
            return Err(invalid("ขั้นตอนนี้ต้องมีหลักฐาน"));
        }
        if (step.definition.requires_signature || step.definition.kind == ProcessStepKind::Sign)
            && request.signature.is_none()
        {
            return Err(invalid("ขั้นตอนนี้ต้องลงนาม"));
        }

        step.recipients[pos].completed_at = Some(Utc::now());
        crate::work::mark_process_assignment_in_transaction(
            tx,
            step.work_item_id,
            actor.user_id,
            true,
        )
        .await?;
        if step_satisfied(step) {
            step.completed_at = Some(Utc::now());
            crate::work::close_process_item_in_transaction(tx, step.work_item_id).await?;
            if instance.current_step + 1 == instance.steps.len() {
                instance.status = ProcessStatus::Completed;
                source_port.completed(tx, instance).await?;
            } else {
                instance.current_step += 1;
                activate_step(tx, instance).await?;
            }
        }
    }
    instance.revision += 1;
    if request.action == ProcessAction::Reassign {
        if let Some(user) = request.reassign_user_id {
            notify_in_transaction(
                tx,
                instance,
                user,
                "มีงานส่งต่อให้คุณ",
                "reassign",
                instance.steps[instance.current_step].work_item_id,
            )
            .await?;
        }
    }
    if matches!(
        instance.status,
        ProcessStatus::RevisionRequested | ProcessStatus::Rejected | ProcessStatus::Completed
    ) {
        notify_in_transaction(
            tx,
            instance,
            instance.created_by,
            match instance.status {
                ProcessStatus::RevisionRequested => "รายการส่งกลับเพื่อแก้ไข",
                ProcessStatus::Rejected => "รายการถูกปฏิเสธ",
                _ => "กระบวนงานเสร็จสิ้น",
            },
            "outcome",
            None,
        )
        .await?;
    }
    append_event(tx, actor, instance, request).await?;
    store(tx, instance).await?;
    Ok(())
}
pub async fn authorize_notification_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    user: Uuid,
) -> Result<bool, AppError> {
    // Central materialization calls this inside its transaction before locking its job.
    // Share the engine's instance guard so a transition cannot invalidate this decision mid-commit.
    let identity: Option<(Uuid, Uuid)> = DbSession::new(&mut *connection)
        .query_as(
            "workflow.src.process.authorize_notification_on_connection",
            "SELECT instance_id,recipient_id FROM process_notification_sources WHERE id=$1",
        )
        .bind(id)
        .fetch_optional()
        .await?;
    let Some((instance, recipient)) = identity else {
        return Ok(false);
    };
    if recipient != user {
        return Err(AppError::Conflict(
            "notification_source_identity_conflict".into(),
        ));
    }
    let present: Option<Uuid> = DbSession::new(&mut *connection)
        .scalar(
            "workflow.src.process.authorize_notification_on_connection",
            "SELECT id FROM process_instances WHERE id=$1 FOR SHARE",
        )
        .bind(instance)
        .fetch_optional()
        .await?;
    if present.is_none() {
        return Ok(false);
    }
    Ok(DbSession::new(connection).scalar("workflow.src.process.authorize_notification_on_connection", "SELECT EXISTS(SELECT 1 FROM process_notification_sources s JOIN process_instances i ON i.id=s.instance_id WHERE s.id=$1 AND s.recipient_id=$2 AND s.cancelled_at IS NULL AND (s.work_item_id IS NULL OR EXISTS(SELECT 1 FROM work_items w JOIN work_item_assignees a ON a.work_item_id=w.id WHERE w.id=s.work_item_id AND w.status='active' AND a.assignee_type='user' AND a.user_id=s.recipient_id AND a.status IN ('assigned','read'))))").bind(id).bind(user).fetch_one().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recipient_shapes_are_closed() {
        assert!(validate_recipient(&ProcessRecipient {
            kind: ProcessRecipientKind::User,
            user_id: None,
            organization_unit_id: None,
            position_code: None
        })
        .is_err());
        assert!(validate_recipient(&ProcessRecipient {
            kind: ProcessRecipientKind::OrganizationPosition,
            user_id: None,
            organization_unit_id: Some(Uuid::new_v4()),
            position_code: Some("administrator".into())
        })
        .is_err());
    }
    #[test]
    fn action_types_cannot_skip_steps() {
        assert!(action_matches(
            ProcessStepKind::Approve,
            ProcessAction::Approve
        ));
        assert!(!action_matches(
            ProcessStepKind::Review,
            ProcessAction::Approve
        ));
        assert!(!action_matches(
            ProcessStepKind::Acknowledge,
            ProcessAction::Read
        ));
    }
    #[test]
    fn hashes_bind_every_payload_field() {
        assert_ne!(
            payload_hash(&("approve", 1)).unwrap(),
            payload_hash(&("approve", 2)).unwrap()
        );
    }
}

/// Replay-safe provider entry point; source guards and projections share the caller transaction.
pub async fn start_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_port: &dyn ProcessSourcePort,
    input: StartProcessInput,
) -> Result<ProcessInstance, AppError> {
    if receipt(
        tx,
        actor.user_id,
        input.request_id,
        "process_start",
        input.source.resource_id,
        &input,
    )
    .await?
    .is_some()
    {
        let existing=DbSession::new(&mut **tx).scalar::<Json<ProcessInstance>>("workflow.src.process.start_in_transaction", "SELECT definition FROM process_instances WHERE source_module=$1 AND source_type=$2 AND source_id=$3 FOR UPDATE").bind(&input.source.module_code).bind(&input.source.resource_type).bind(input.source.resource_id).fetch_one().await?.0;
        source_port.validate_existing(tx, actor, &existing).await?;
        return Ok(existing);
    }
    start_core_in_transaction(tx, actor, source_port, input).await
}
pub async fn act_source_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_port: &dyn ProcessSourcePort,
    id: Uuid,
    request: &ProcessActionRequest,
) -> Result<ProcessInstance, AppError> {
    let replay = receipt(
        tx,
        actor.user_id,
        request.request_id,
        "process_action",
        id,
        request,
    )
    .await?
    .is_some();
    let mut instance = load_locked(tx, id).await?;
    if replay {
        source_port.validate_existing(tx, actor, &instance).await?;
        return Ok(instance);
    }
    action_in_transaction(tx, actor, source_port, &mut instance, request).await?;
    Ok(instance)
}

pub(crate) async fn notify_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    instance: &ProcessInstance,
    user: Uuid,
    message: &str,
    identity: &str,
    work_item: Option<Uuid>,
) -> Result<(), AppError> {
    let source_id = Uuid::now_v7();
    DbSession::new(&mut **tx).query("workflow.src.process.notify_in_transaction",
        "INSERT INTO process_notification_sources(id,instance_id,recipient_id,work_item_id) VALUES($1,$2,$3,$4)",
    )
    .bind(source_id)
    .bind(instance.id)
    .bind(user)
    .bind(work_item)
    .execute()
    .await?;
    school_notifications::dispatch::create_in_transaction(
        tx,
        &[school_notifications::dispatch::NewNotification {
            id: Uuid::now_v7(),
            notification_id: Uuid::now_v7(),
            recipient_id: user,
            source: school_notifications::dispatch::NotificationSource::Process { source_id },
            idempotency_key: format!(
                "process:{}:{}:{}:{}",
                instance.id, instance.revision, identity, user
            ),
            title: instance.title.clone(),
            message: message.into(),
            type_: school_notifications::publisher::NotificationType::Info,
            presentation: school_notifications::dispatch::Presentation::AlertOnce,
            link: Some(instance.action_path.clone()),
        }],
    )
    .await?;
    Ok(())
}

/// Read-only replay recognition avoids expensive source rendering and stale checks on a retry.
/// Acceptance still claims/rechecks the same receipt atomically in its transaction.
pub(crate) async fn replay_exists<T: Serialize>(
    connection: &mut sqlx::PgConnection,
    actor: Uuid,
    request: Uuid,
    operation: &str,
    resource: Uuid,
    payload: &T,
    creation: bool,
) -> Result<bool, AppError> {
    let stored:Option<(String,String)>=DbSession::new(&mut *connection).query_as("workflow.src.process.replay_exists", "SELECT operation,payload_hash FROM workflow_mutation_receipts WHERE actor_id=$1 AND request_id=$2").bind(actor).bind(request).fetch_optional().await?;
    let Some((old_operation, old_hash)) = stored else {
        return Ok(false);
    };
    let hash = if creation {
        payload_hash(&(operation, payload))?
    } else {
        payload_hash(&(operation, resource, payload))?
    };
    if old_operation != operation || old_hash != hash {
        return Err(AppError::Conflict("รหัสคำขอถูกใช้กับข้อมูลอื่นแล้ว".into()));
    }
    Ok(true)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessActionEvent {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub actor_id: Uuid,
    pub request_id: Uuid,
    pub revision: i64,
    pub action: ProcessActionRequest,
    pub created_at: chrono::DateTime<Utc>,
}
async fn append_event(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    instance: &ProcessInstance,
    request: &ProcessActionRequest,
) -> Result<(), AppError> {
    let event = ProcessActionEvent {
        id: Uuid::now_v7(),
        instance_id: instance.id,
        actor_id: actor.user_id,
        request_id: request.request_id,
        revision: instance.revision,
        action: request.clone(),
        created_at: Utc::now(),
    };
    DbSession::new(&mut **tx).query("workflow.src.process.append_event", "INSERT INTO process_action_events(id,instance_id,actor_id,request_id,revision,event) VALUES($1,$2,$3,$4,$5,$6)").bind(event.id).bind(instance.id).bind(actor.user_id).bind(request.request_id).bind(instance.revision).bind(Json(&event)).execute().await?;
    Ok(())
}
/// The source owner authorizes access before reading its process history in its transaction.
pub async fn history_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    source: &ProcessSource,
) -> Result<Vec<ProcessActionEvent>, AppError> {
    Ok(DbSession::new(&mut **tx).scalar::<Json<ProcessActionEvent>>("workflow.src.process.history_in_transaction", "SELECT e.event FROM process_action_events e JOIN process_instances i ON i.id=e.instance_id WHERE i.source_module=$1 AND i.source_type=$2 AND i.source_id=$3 ORDER BY e.revision,e.id").bind(&source.module_code).bind(&source.resource_type).bind(source.resource_id).fetch_all().await?.into_iter().map(|e|e.0).collect())
}

/// Source-owned immutable template creation. The caller authorizes its domain settings.
pub async fn create_source_template_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
    module_code: &str,
    title: &str,
    steps: Vec<ProcessTemplateStep>,
) -> Result<(), AppError> {
    validate_steps(&steps)?;
    let template = ProcessTemplate {
        id,
        code: format!("source_{}", id.simple()),
        title: title.into(),
        version: 1,
        steps,
        active: true,
        created_at: Utc::now(),
    };
    DbSession::new(&mut **tx).query("workflow.src.process.create_source_template_in_transaction", "INSERT INTO process_templates(id,code,version,definition,created_by,managed_source_module) VALUES($1,$2,1,$3,$4,$5)").bind(id).bind(&template.code).bind(Json(&template)).bind(actor).bind(module_code).execute().await?;
    DbSession::new(&mut **tx).query("workflow.src.process.create_source_template_in_transaction", "INSERT INTO process_template_versions(template_id,version,definition,created_by) VALUES($1,1,$2,$3)").bind(id).bind(Json(&template)).bind(actor).execute().await?;
    Ok(())
}
/// Withdraw an active source under its owning domain policy and the existing owner identity.
/// All assignment, notification validity and process history writes share the caller transaction.
pub async fn withdraw_source_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_port: &dyn ProcessSourcePort,
    id: Uuid,
    request_id: Uuid,
    expected_revision: i64,
    reason: &str,
) -> Result<ProcessInstance, AppError> {
    let mut instance = load_locked(tx, id).await?;
    source_port.validate_existing(tx, actor, &instance).await?;
    if instance.created_by != actor.user_id {
        return Err(denied());
    }
    if instance.revision != expected_revision
        || !matches!(
            instance.status,
            ProcessStatus::Active | ProcessStatus::RevisionRequested
        )
    {
        return Err(stale());
    }
    if reason.trim().is_empty() {
        return Err(invalid("กรุณาระบุเหตุผลการยกเลิก"));
    }
    for step in &instance.steps {
        crate::work::close_process_item_in_transaction(tx, step.work_item_id).await?;
    }
    DbSession::new(&mut **tx).query("workflow.src.process.withdraw_source_in_transaction", "UPDATE process_notification_sources SET cancelled_at=now() WHERE instance_id=$1 AND cancelled_at IS NULL").bind(id).execute().await?;
    instance.status = ProcessStatus::Closed;
    instance.revision += 1;
    let action = ProcessActionRequest {
        request_id,
        expected_revision,
        action: ProcessAction::Close,
        annotations: vec![],
        comment: Some(reason.into()),
        evidence_file_ids: vec![],
        signature: None,
        reassign_user_id: None,
        reassign_from_user_id: None,
        assignment_id: None,
        routing: None,
    };
    append_event(tx, actor, &instance, &action).await?;
    store(tx, &instance).await?;
    Ok(instance)
}
