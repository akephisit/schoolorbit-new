//! Directed assignments are a process mode; participants are visibility, never authority.
use crate::{
    process::{self, denied, invalid, stale},
    process_models::*,
    work,
};
use chrono::Utc;
use school_authorization::{staff_users_with_all_permissions_on_connection, ActorContext};
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

fn register_code(register: CorrespondenceRegister) -> &'static str {
    match register {
        CorrespondenceRegister::Incoming => "incoming",
        CorrespondenceRegister::Outgoing => "outgoing",
        CorrespondenceRegister::Internal => "internal",
    }
}
fn defaults(register: CorrespondenceRegister) -> CorrespondenceRoutingSettings {
    CorrespondenceRoutingSettings {
        register,
        revision: 0,
        eligible_decision_makers: vec![],
        default_first_recipient: None,
        default_purpose: DirectedPurpose::Acknowledge,
        default_require_review: false,
        default_reviewer: None,
        default_allow_onward_assignment: true,
        numbering: CorrespondenceNumberingSettings::default(),
        text_stamp_templates: crate::correspondence_settings::default_stamp_templates(),
    }
}
fn require_settings_read(actor: &ActorContext) -> Result<(), AppError> {
    if actor.has_any_permission(&[
        codes::CORRESPONDENCE_CREATE_OWN,
        codes::CORRESPONDENCE_UPDATE_ASSIGNED,
        codes::PROCESS_MANAGE_SCHOOL,
        codes::CORRESPONDENCE_MANAGE_SCHOOL,
        codes::CORRESPONDENCE_MANAGE_ORGANIZATION_UNIT,
    ]) {
        Ok(())
    } else {
        Err(denied())
    }
}
pub async fn settings(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<Vec<CorrespondenceRoutingSettings>, AppError> {
    require_settings_read(actor)?;
    let mut database_stage_0 =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "workflow.directed.settings")
            .await?;
    let rows: Vec<Json<CorrespondenceRoutingSettings>> = DbSession::new(&mut *database_stage_0)
        .scalar(
            "workflow.src.directed.settings",
            "SELECT definition FROM correspondence_routing_settings ORDER BY register",
        )
        .fetch_all()
        .await?;
    Ok([
        CorrespondenceRegister::Incoming,
        CorrespondenceRegister::Outgoing,
        CorrespondenceRegister::Internal,
    ]
    .into_iter()
    .map(|register| {
        rows.iter()
            .find(|r| r.register == register)
            .map_or_else(|| defaults(register), |r| r.0.clone())
    })
    .collect())
}
pub(crate) async fn settings_in(
    tx: &mut Transaction<'_, Postgres>,
    register: CorrespondenceRegister,
) -> Result<CorrespondenceRoutingSettings, AppError> {
    DbSession::new(&mut **tx)
        .query(
            "workflow.src.directed.settings_in",
            "SELECT pg_advisory_xact_lock_shared(hashtextextended($1,0))",
        )
        .bind(format!(
            "correspondence-settings:{}",
            register_code(register)
        ))
        .execute()
        .await?;
    Ok(DbSession::new(&mut **tx)
        .scalar::<Json<CorrespondenceRoutingSettings>>(
            "workflow.src.directed.settings_in",
            "SELECT definition FROM correspondence_routing_settings WHERE register=$1 FOR SHARE",
        )
        .bind(register_code(register))
        .fetch_optional()
        .await?
        .map_or_else(|| defaults(register), |s| s.0))
}
pub async fn save_settings(
    pool: &PgPool,
    actor: &ActorContext,
    request: SaveCorrespondenceRoutingSettingsRequest,
) -> Result<CorrespondenceRoutingSettings, AppError> {
    actor.require_permission(codes::PROCESS_MANAGE_SCHOOL)?;
    if let Some(numbering) = &request.numbering {
        crate::correspondence_settings::validate_numbering(numbering)?;
    }
    if let Some(templates) = &request.text_stamp_templates {
        crate::correspondence_settings::validate_templates(templates)?;
    }
    if request.eligible_decision_makers.len() > 100
        || request.default_purpose == DirectedPurpose::Decide
        || request.default_require_review != request.default_reviewer.is_some()
    {
        return Err(invalid("การตั้งค่าผู้พิจารณาหรือผู้ตรวจทานไม่ถูกต้อง"));
    }
    for selector in &request.eligible_decision_makers {
        process::validate_recipient(selector)?;
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "workflow.directed.save_settings")
            .await?;
    // Serialize creation and update, including the not-yet-existing profile.
    DbSession::new(&mut *tx)
        .query(
            "workflow.src.directed.save_settings",
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        )
        .bind(format!(
            "correspondence-settings:{}",
            register_code(request.register)
        ))
        .execute()
        .await?;
    let current = settings_in(&mut tx, request.register).await?;
    if process::receipt(
        &mut tx,
        actor.user_id,
        request.request_id,
        "correspondence_routing_settings",
        Uuid::nil(),
        &request,
    )
    .await?
    .is_some()
    {
        tx.commit().await?;
        return Ok(current);
    }
    if current.revision != request.expected_revision {
        return Err(stale());
    }
    let permitted = staff_users_with_all_permissions_on_connection(
        &mut tx,
        &[
            codes::CORRESPONDENCE_READ_ASSIGNED,
            codes::CORRESPONDENCE_UPDATE_ASSIGNED,
        ],
    )
    .await?;
    if let Some(reviewer) = &request.default_reviewer {
        resolve_one(&mut tx, reviewer, &permitted).await?;
    }
    let result = CorrespondenceRoutingSettings {
        register: request.register,
        revision: current.revision + 1,
        eligible_decision_makers: request.eligible_decision_makers,
        default_first_recipient: request.default_first_recipient,
        default_purpose: request.default_purpose,
        default_require_review: request.default_require_review,
        default_reviewer: request.default_reviewer,
        default_allow_onward_assignment: request.default_allow_onward_assignment,
        numbering: request.numbering.unwrap_or(current.numbering),
        text_stamp_templates: request
            .text_stamp_templates
            .unwrap_or(current.text_stamp_templates),
    };
    if let Some(first) = &result.default_first_recipient {
        let person = resolve_one(&mut tx, first, &permitted).await?;
        if !eligible_ids(&mut tx, &result, &permitted)
            .await?
            .contains(&person.user_id)
        {
            return Err(invalid("ผู้พิจารณาเริ่มต้นไม่อยู่ในรายชื่อที่กำหนด"));
        }
    }
    DbSession::new(&mut *tx).query("workflow.src.directed.save_settings", "INSERT INTO correspondence_routing_settings(register,revision,definition,updated_by) VALUES($1,$2,$3,$4) ON CONFLICT(register) DO UPDATE SET revision=excluded.revision,definition=excluded.definition,updated_by=excluded.updated_by,updated_at=now()")
        .bind(register_code(result.register)).bind(result.revision).bind(Json(&result)).bind(actor.user_id).execute().await?;
    tx.commit().await?;
    Ok(result)
}

/// Resolve real current staff, using Authorization's effective role/unit/delegation owner.
/// Unit selection means its responsible lead; ambiguous units require an explicit position/person.
async fn resolve_many(
    tx: &mut Transaction<'_, Postgres>,
    selectors: &[ProcessRecipient],
    eligible: &[Uuid],
) -> Result<Vec<Vec<ProcessRecipientSnapshot>>, AppError> {
    for selector in selectors {
        process::validate_recipient(selector)?;
    }
    let rows:Vec<(i64,Uuid,String)>=DbSession::new(&mut **tx).query_as("workflow.src.directed.resolve_many", "WITH selectors AS (SELECT value,ordinality::bigint-1 AS idx FROM jsonb_array_elements($1::jsonb) WITH ORDINALITY) SELECT DISTINCT s.idx,u.id,u.display_name FROM selectors s JOIN user_display_names u ON u.id=ANY($2) AND ((s.value->>'kind'='user' AND u.id=(s.value->>'userId')::uuid) OR (s.value->>'kind' IN ('organization_unit','organization_position') AND EXISTS(SELECT 1 FROM organization_members m JOIN organization_units ou ON ou.id=m.organization_unit_id AND ou.is_active WHERE m.user_id=u.id AND m.organization_unit_id=(s.value->>'organizationUnitId')::uuid AND m.started_at<=CURRENT_DATE AND (m.ended_at IS NULL OR m.ended_at>CURRENT_DATE) AND ((s.value->>'kind'='organization_position' AND m.position_code=s.value->>'positionCode') OR (s.value->>'kind'='organization_unit' AND m.position_code IN ('director','deputy_director','head','deputy_head','coordinator')))))) ORDER BY s.idx,u.id")
        .bind(Json(selectors)).bind(eligible).fetch_all().await?;
    let mut result = vec![vec![]; selectors.len()];
    for (index, user_id, display_name) in rows {
        result[index as usize].push(ProcessRecipientSnapshot {
            user_id,
            display_name,
            completed_at: None,
            read_at: None,
            reassigned_from: None,
        });
    }
    for (index, people) in result.iter().enumerate() {
        if people.is_empty()
            || people.len() > 100
            || (selectors[index].kind == ProcessRecipientKind::OrganizationUnit
                && people.len() != 1)
        {
            return Err(invalid("ผู้รับไม่พร้อมใช้งาน ไม่มีสิทธิ์ดำเนินการ หรือหน่วยงานมีผู้รับผิดชอบหลายคน กรุณาเลือกบุคคลหรือตำแหน่งให้ชัดเจน"));
        }
    }
    Ok(result)
}
async fn resolve_one(
    tx: &mut Transaction<'_, Postgres>,
    selector: &ProcessRecipient,
    eligible: &[Uuid],
) -> Result<ProcessRecipientSnapshot, AppError> {
    let mut people = resolve_many(tx, std::slice::from_ref(selector), eligible)
        .await?
        .pop()
        .ok_or_else(|| invalid("ไม่พบผู้รับ"))?;
    if people.len() != 1 {
        return Err(invalid("กรุณาเลือกผู้พิจารณาหรือผู้ตรวจทานเพียงหนึ่งคน"));
    }
    people.pop().ok_or_else(|| invalid("ไม่พบผู้รับ"))
}
async fn eligible_ids(
    tx: &mut Transaction<'_, Postgres>,
    profile: &CorrespondenceRoutingSettings,
    permitted: &[Uuid],
) -> Result<BTreeSet<Uuid>, AppError> {
    if profile.eligible_decision_makers.is_empty() {
        return Ok(permitted.iter().copied().collect());
    }
    let mut ids = BTreeSet::new();
    // Unavailable configured positions do not hide unrelated eligible staff.
    let rows:Vec<Uuid>=DbSession::new(&mut **tx).scalar("workflow.src.directed.eligible_ids", "SELECT DISTINCT u.id FROM users u CROSS JOIN jsonb_array_elements($1::jsonb) s WHERE u.id=ANY($2) AND ((s->>'kind'='user' AND u.id=(s->>'userId')::uuid) OR (s->>'kind' IN ('organization_unit','organization_position') AND EXISTS(SELECT 1 FROM organization_members m JOIN organization_units ou ON ou.id=m.organization_unit_id AND ou.is_active WHERE m.user_id=u.id AND m.organization_unit_id=(s->>'organizationUnitId')::uuid AND m.started_at<=CURRENT_DATE AND (m.ended_at IS NULL OR m.ended_at>CURRENT_DATE) AND ((s->>'kind'='organization_unit' AND m.position_code IN ('director','deputy_director','head','deputy_head','coordinator')) OR (s->>'kind'='organization_position' AND m.position_code=s->>'positionCode'))))) ORDER BY u.id")
        .bind(Json(&profile.eligible_decision_makers)).bind(permitted).fetch_all().await?;
    ids.extend(rows);
    Ok(ids)
}
pub async fn selection_options(
    pool: &PgPool,
    register: CorrespondenceRegister,
    search: &str,
) -> Result<(Vec<CorrespondenceUserOption>, Vec<CorrespondenceUserOption>), AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "workflow.directed.selection_options")
            .await?;
    let profile = settings_in(&mut tx, register).await?;
    let permitted = staff_users_with_all_permissions_on_connection(
        &mut tx,
        &[
            codes::CORRESPONDENCE_READ_ASSIGNED,
            codes::CORRESPONDENCE_UPDATE_ASSIGNED,
        ],
    )
    .await?;
    let first = eligible_ids(&mut tx, &profile, &permitted).await?;
    let people:Vec<(Uuid,String)>=DbSession::new(&mut *tx).query_as("workflow.src.directed.selection_options", "SELECT id,display_name FROM user_display_names WHERE id=ANY($1) AND ($2='' OR display_name ILIKE '%'||$2||'%') ORDER BY first_name,last_name,id LIMIT 500").bind(permitted).bind(search).fetch_all().await?;
    let reviewers: Vec<_> = people
        .into_iter()
        .map(|(id, display_name)| CorrespondenceUserOption { id, display_name })
        .collect();
    let decision_makers = reviewers
        .iter()
        .filter(|p| first.contains(&p.id))
        .cloned()
        .collect();
    tx.commit().await?;
    Ok((decision_makers, reviewers))
}

pub(crate) async fn start(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    record: &CorrespondenceSummary,
    request: &DirectedSendCorrespondenceRequest,
) -> Result<ProcessInstance, AppError> {
    let profile = settings_in(tx, record.register).await?;
    let permitted = staff_users_with_all_permissions_on_connection(
        &mut **tx,
        &[
            codes::CORRESPONDENCE_READ_ASSIGNED,
            codes::CORRESPONDENCE_UPDATE_ASSIGNED,
        ],
    )
    .await?;
    let recipient = resolve_one(tx, &request.first_recipient, &permitted).await?;
    if !eligible_ids(tx, &profile, &permitted)
        .await?
        .contains(&recipient.user_id)
    {
        return Err(invalid("ผู้พิจารณาคนแรกไม่อยู่ในรายชื่อที่โรงเรียนกำหนด"));
    }
    let existing:Option<Json<ProcessInstance>>=DbSession::new(&mut **tx).scalar("workflow.src.directed.start", "SELECT definition FROM process_instances WHERE source_module='correspondence' AND source_type='correspondence' AND source_id=$1 FOR UPDATE").bind(record.id).fetch_optional().await?;
    if existing
        .as_ref()
        .is_some_and(|i| i.status != ProcessStatus::RevisionRequested)
    {
        return Err(stale());
    }
    let mut instance = ProcessInstance {
        id: existing.as_ref().map_or_else(Uuid::now_v7, |i| i.id),
        source: ProcessSource {
            module_code: "correspondence".into(),
            resource_type: "correspondence".into(),
            resource_id: record.id,
        },
        title: record.title.clone(),
        action_path: format!("/staff/correspondence/{}", record.id),
        template_id: None,
        template_version: None,
        revision: existing.as_ref().map_or(1, |i| i.revision + 1),
        status: ProcessStatus::Active,
        current_step: 0,
        steps: vec![],
        created_by: actor.user_id,
        created_at: existing.as_ref().map_or_else(Utc::now, |i| i.created_at),
        due_at: request.due_at,
        directed: Some(DirectedProcess {
            assignments: vec![DirectedAssignment {
                id: Uuid::now_v7(),
                assigned_by: actor.user_id,
                created_at: Utc::now(),
                parent_id: None,
                selector: request.first_recipient.clone(),
                recipient,
                purpose: DirectedPurpose::Decide,
                status: DirectedAssignmentStatus::Pending,
                required: true,
                allow_onward_assignment: true,
                reviewer: None,
                due_at: request.due_at,
                work_item_id: None,
                review_work_item_id: None,
                completed_at: None,
            }],
        }),
    };
    DbSession::new(&mut **tx).query("workflow.src.directed.start", "INSERT INTO process_instances(id,source_module,source_type,source_id,template_id,template_version,revision,definition,created_by) VALUES($1,'correspondence','correspondence',$2,NULL,NULL,$3,$4,$5) ON CONFLICT(id) DO UPDATE SET template_id=NULL,template_version=NULL,revision=excluded.revision,definition=excluded.definition")
        .bind(instance.id).bind(record.id).bind(instance.revision).bind(Json(&instance)).bind(actor.user_id).execute().await?;
    activate(tx, &mut instance, 0, false).await?;
    process::store(tx, &instance).await?;
    Ok(instance)
}

async fn activate(
    tx: &mut Transaction<'_, Postgres>,
    instance: &mut ProcessInstance,
    index: usize,
    review: bool,
) -> Result<(), AppError> {
    let assignment = instance
        .directed
        .as_ref()
        .and_then(|g| g.assignments.get(index))
        .ok_or_else(|| invalid("ไม่พบงานที่มอบหมาย"))?;
    let recipient = if review {
        assignment
            .reviewer
            .as_ref()
            .ok_or_else(|| invalid("ไม่มีผู้ตรวจทาน"))?
    } else {
        &assignment.recipient
    };
    let user = recipient.user_id;
    let title = if review {
        "ตรวจทานผลการดำเนินการ"
    } else {
        match assignment.purpose {
            DirectedPurpose::Decide => "พิจารณาและสั่งการ",
            DirectedPurpose::Acknowledge => {
                if assignment.required {
                    "รับทราบหนังสือ"
                } else {
                    "สำเนาเพื่อทราบ"
                }
            }
            DirectedPurpose::Execute => "ดำเนินการตามสั่งการ",
        }
    };
    if !assignment.required && !review {
        DbSession::new(&mut **tx).query("workflow.src.directed.activate", "INSERT INTO process_participants(instance_id,user_id) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(instance.id).bind(user).execute().await?;
        let identity = format!("directed:{}:copy", assignment.id);
        process::notify_in_transaction(tx, instance, user, title, &identity, None).await?;
        return Ok(());
    }
    let window = Uuid::now_v7();
    let item = Uuid::now_v7();
    DbSession::new(&mut **tx).query("workflow.src.directed.activate", "INSERT INTO workflow_windows(id,module_code,workflow_code,title,managed_by_permission,status,created_by,due_at) VALUES($1,'correspondence','process_directed',$2,$3,'open',$4,$5)").bind(window).bind(title).bind(codes::PROCESS_MANAGE_SCHOOL).bind(instance.created_by).bind(assignment.due_at.or(instance.due_at)).execute().await?;
    work::create_work_item_in_transaction(
        tx,
        item,
        window,
        &instance.source.module_code,
        &instance.source.resource_type,
        instance.source.resource_id,
        &format!("{} · {title}", instance.title),
        &instance.action_path,
        instance.created_by,
        &[user],
    )
    .await?;
    DbSession::new(&mut **tx).query("workflow.src.directed.activate", "INSERT INTO process_participants(instance_id,user_id) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(instance.id).bind(user).execute().await?;
    let identity = format!(
        "directed:{}:{}",
        assignment.id,
        if review { "review" } else { "assignment" }
    );
    process::notify_in_transaction(tx, instance, user, title, &identity, Some(item)).await?;
    let assignment = &mut instance
        .directed
        .as_mut()
        .ok_or_else(|| invalid("ไม่พบกระบวนงาน"))?
        .assignments[index];
    if review {
        assignment.review_work_item_id = Some(item);
    } else {
        assignment.work_item_id = Some(item);
    }
    Ok(())
}

pub fn assignment_actions(
    actor: &ActorContext,
    instance: &ProcessInstance,
    manager_reassign: bool,
) -> Vec<DirectedAssignmentActions> {
    let Some(graph) = &instance.directed else {
        return vec![];
    };
    let active = instance.status == ProcessStatus::Active;
    let terminal_copy = matches!(
        instance.status,
        ProcessStatus::Completed | ProcessStatus::Closed
    );
    graph
        .assignments
        .iter()
        .filter_map(|a| {
            let mut actions = vec![];
            let pending = a.status == DirectedAssignmentStatus::Pending
                && a.recipient.user_id == actor.user_id;
            let reviewing = a.status == DirectedAssignmentStatus::WaitingReview
                && a.reviewer
                    .as_ref()
                    .is_some_and(|r| r.user_id == actor.user_id);
            if (active && (pending || reviewing)) || (terminal_copy && pending && !a.required) {
                if actor.has_permission(codes::CORRESPONDENCE_READ_ASSIGNED) {
                    actions.push(ProcessAction::Read);
                }
                if actor.has_permission(codes::CORRESPONDENCE_UPDATE_ASSIGNED) {
                    if reviewing {
                        actions.extend([
                            ProcessAction::Review,
                            ProcessAction::Report,
                            ProcessAction::Reject,
                            ProcessAction::ReturnForRevision,
                        ]);
                    } else {
                        if a.required {
                            actions.extend([
                                ProcessAction::Report,
                                ProcessAction::Reject,
                                ProcessAction::ReturnForRevision,
                            ]);
                            if a.allow_onward_assignment {
                                actions.push(ProcessAction::Route);
                            }
                        }
                        actions.push(match a.purpose {
                            DirectedPurpose::Acknowledge => ProcessAction::Acknowledge,
                            DirectedPurpose::Execute => ProcessAction::Execute,
                            DirectedPurpose::Decide => ProcessAction::Review,
                        });
                    }
                }
            }
            if active
                && manager_reassign
                && a.required
                && matches!(
                    a.status,
                    DirectedAssignmentStatus::Pending | DirectedAssignmentStatus::WaitingReview
                )
            {
                actions.push(ProcessAction::Reassign);
            }
            if actions.is_empty() {
                None
            } else {
                Some(DirectedAssignmentActions {
                    assignment_id: a.id,
                    actions,
                })
            }
        })
        .collect()
}
fn graph_mut(instance: &mut ProcessInstance) -> Result<&mut DirectedProcess, AppError> {
    instance
        .directed
        .as_mut()
        .ok_or_else(|| invalid("ไม่ใช่กระบวนงานแบบสั่งการ"))
}
fn validate_routing(request: &DirectedRoutingRequest) -> Result<(), AppError> {
    if request.recipients.is_empty()
        || request.recipients.len() > 100
        || !request.recipients.iter().any(|r| r.required)
        || request.require_review != request.reviewer.is_some()
    {
        return Err(invalid("กรุณาระบุผู้รับงานที่จำเป็นและผู้ตรวจทานให้ถูกต้อง"));
    }
    for target in &request.recipients {
        process::validate_recipient(&target.recipient)?;
        if target.purpose == DirectedPurpose::Decide
            || (!target.required
                && (target.purpose != DirectedPurpose::Acknowledge
                    || target.allow_onward_assignment))
        {
            return Err(invalid("สำเนาใช้เพื่อรับทราบเท่านั้นและไม่มีสิทธิ์มอบหมายต่อ"));
        }
    }
    Ok(())
}
async fn route(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    instance: &mut ProcessInstance,
    index: usize,
    routing: &DirectedRoutingRequest,
) -> Result<(), AppError> {
    validate_routing(routing)?;
    let graph = instance
        .directed
        .as_ref()
        .ok_or_else(|| invalid("ไม่พบกระบวนงาน"))?;
    if graph.assignments.len() + routing.recipients.len() > 500 {
        return Err(invalid("การมอบหมายเกิน 500 รายการ"));
    }
    let parent = &graph.assignments[index];
    let parent_id = parent.id;
    let mut forbidden = BTreeSet::from([actor.user_id]);
    let mut ancestor = parent.parent_id;
    let mut depth = 0;
    while let Some(id) = ancestor {
        let item = graph
            .assignments
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| invalid("เส้นทางมอบหมายไม่ถูกต้อง"))?;
        forbidden.insert(item.recipient.user_id);
        if let Some(reviewer) = &item.reviewer {
            forbidden.insert(reviewer.user_id);
        }
        ancestor = item.parent_id;
        depth += 1;
        if depth >= 30 {
            return Err(invalid("มอบหมายต่อได้ไม่เกิน 30 ระดับ"));
        }
    }
    let permitted = staff_users_with_all_permissions_on_connection(
        &mut **tx,
        &[
            codes::CORRESPONDENCE_READ_ASSIGNED,
            codes::CORRESPONDENCE_UPDATE_ASSIGNED,
        ],
    )
    .await?;
    let mut selectors: Vec<_> = routing
        .recipients
        .iter()
        .map(|t| t.recipient.clone())
        .collect();
    if let Some(reviewer) = &routing.reviewer {
        selectors.push(reviewer.clone());
    }
    let mut resolved = resolve_many(tx, &selectors, &permitted).await?;
    let reviewer = if routing.reviewer.is_some() {
        let mut people = resolved.pop().ok_or_else(|| invalid("ไม่พบผู้ตรวจทาน"))?;
        if people.len() != 1 {
            return Err(invalid("กรุณาเลือกผู้ตรวจทานหนึ่งคน"));
        }
        people.pop()
    } else {
        None
    };
    let mut children = vec![];
    let mut seen = BTreeSet::new();
    for (target, people) in routing.recipients.iter().zip(resolved) {
        for recipient in people {
            if forbidden.contains(&recipient.user_id)
                || !seen.insert(recipient.user_id)
                || reviewer
                    .as_ref()
                    .is_some_and(|r| r.user_id == recipient.user_id)
            {
                return Err(invalid(
                    "ผู้รับซ้ำ ส่งกลับถึงผู้มอบหมายเดิม หรือผู้รับเป็นผู้ตรวจทานของงานเดียวกัน",
                ));
            }
            children.push(DirectedAssignment {
                id: Uuid::now_v7(),
                assigned_by: actor.user_id,
                created_at: Utc::now(),
                parent_id: Some(parent_id),
                selector: target.recipient.clone(),
                recipient,
                purpose: target.purpose,
                status: DirectedAssignmentStatus::Pending,
                required: target.required,
                allow_onward_assignment: target.allow_onward_assignment,
                reviewer: None,
                due_at: target.due_at,
                work_item_id: None,
                review_work_item_id: None,
                completed_at: None,
            });
        }
    }
    if graph_mut(instance)?.assignments.len() + children.len() > 500 {
        return Err(invalid("การมอบหมายเกิน 500 รายการ"));
    }
    let old_item = graph_mut(instance)?.assignments[index].work_item_id;
    work::mark_process_assignment_in_transaction(tx, old_item, actor.user_id, true).await?;
    work::close_process_item_in_transaction(tx, old_item).await?;
    let graph = graph_mut(instance)?;
    // The review belongs to this parent's immutable routing decision. Descendants cannot replace it.
    let reviewer_id = reviewer.as_ref().map(|r| r.user_id);
    graph.assignments[index].reviewer = reviewer;
    graph.assignments[index].status = DirectedAssignmentStatus::WaitingChildren;
    let start = graph.assignments.len();
    graph.assignments.extend(children);
    if let Some(user) = reviewer_id {
        DbSession::new(&mut **tx).query("workflow.src.directed.route", "INSERT INTO process_participants(instance_id,user_id) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(instance.id).bind(user).execute().await?;
    }
    for child in start..graph_mut(instance)?.assignments.len() {
        activate(tx, instance, child, false).await?;
    }
    Ok(())
}
/// Propagate only required child completion. Parent review is activated once, when ready.
async fn advance(
    tx: &mut Transaction<'_, Postgres>,
    instance: &mut ProcessInstance,
) -> Result<(), AppError> {
    loop {
        let ready = {
            let graph = instance
                .directed
                .as_ref()
                .ok_or_else(|| invalid("ไม่พบกระบวนงาน"))?;
            graph.assignments.iter().enumerate().find_map(|(index, a)| {
                (a.status == DirectedAssignmentStatus::WaitingChildren
                    && graph
                        .assignments
                        .iter()
                        .filter(|child| child.parent_id == Some(a.id) && child.required)
                        .all(|child| child.status == DirectedAssignmentStatus::Completed))
                .then_some(index)
            })
        };
        let Some(index) = ready else {
            break;
        };
        if graph_mut(instance)?.assignments[index].reviewer.is_some() {
            graph_mut(instance)?.assignments[index].status =
                DirectedAssignmentStatus::WaitingReview;
            activate(tx, instance, index, true).await?;
        } else {
            let a = &mut graph_mut(instance)?.assignments[index];
            a.status = DirectedAssignmentStatus::Completed;
            a.completed_at = Some(Utc::now());
            a.recipient.completed_at = a.completed_at;
        }
    }
    if graph_mut(instance)?
        .assignments
        .iter()
        .filter(|a| a.parent_id.is_none() && a.required)
        .all(|a| a.status == DirectedAssignmentStatus::Completed)
    {
        instance.status = ProcessStatus::Completed;
        // Copies remain in history but do not leave dangling central work after overall completion.
        let items: Vec<_> = graph_mut(instance)?
            .assignments
            .iter()
            .filter(|a| !a.required)
            .filter_map(|a| a.work_item_id)
            .collect();
        for item in items {
            work::close_process_item_in_transaction(tx, Some(item)).await?;
        }
    }
    Ok(())
}
fn descendants(graph: &DirectedProcess, id: Uuid) -> BTreeSet<Uuid> {
    let mut ids = BTreeSet::from([id]);
    loop {
        let old = ids.len();
        for a in &graph.assignments {
            if a.parent_id.is_some_and(|p| ids.contains(&p)) {
                ids.insert(a.id);
            }
        }
        if ids.len() == old {
            break;
        }
    }
    ids.remove(&id);
    ids
}
async fn reassign(
    tx: &mut Transaction<'_, Postgres>,
    instance: &mut ProcessInstance,
    index: usize,
    request: &ProcessActionRequest,
) -> Result<(), AppError> {
    if request
        .comment
        .as_deref()
        .is_none_or(|c| c.trim().is_empty())
        || request.reassign_from_user_id.is_some()
    {
        return Err(invalid("กรุณาระบุเหตุผลการเปลี่ยนผู้รับ โดยระบบจะระบุผู้รับเดิมจากงาน"));
    }
    let target = request
        .reassign_user_id
        .ok_or_else(|| invalid("กรุณาระบุผู้รับใหม่"))?;
    let permitted = staff_users_with_all_permissions_on_connection(
        &mut **tx,
        &[
            codes::CORRESPONDENCE_READ_ASSIGNED,
            codes::CORRESPONDENCE_UPDATE_ASSIGNED,
        ],
    )
    .await?;
    let selector = ProcessRecipient {
        kind: ProcessRecipientKind::User,
        user_id: Some(target),
        organization_unit_id: None,
        position_code: None,
    };
    let mut replacement = resolve_one(tx, &selector, &permitted).await?;
    let graph = instance
        .directed
        .as_ref()
        .ok_or_else(|| invalid("ไม่พบกระบวนงาน"))?;
    let a = &graph.assignments[index];
    let reviewing = a.status == DirectedAssignmentStatus::WaitingReview;
    let old = if reviewing {
        a.reviewer.as_ref().ok_or_else(denied)?.user_id
    } else {
        a.recipient.user_id
    };
    if target == old {
        return Err(invalid("ผู้รับใหม่ต้องต่างจากผู้รับเดิม"));
    }
    if reviewing {
        let children = descendants(graph, a.id);
        if graph
            .assignments
            .iter()
            .any(|child| children.contains(&child.id) && child.recipient.user_id == target)
        {
            return Err(invalid("ผู้ตรวจทานไม่สามารถตรวจผลการดำเนินการของตนเอง"));
        }
    } else {
        let mut parent = a.parent_id;
        while let Some(id) = parent {
            let ancestor = graph
                .assignments
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| invalid("เส้นทางมอบหมายไม่ถูกต้อง"))?;
            if ancestor.recipient.user_id == target
                || ancestor
                    .reviewer
                    .as_ref()
                    .is_some_and(|r| r.user_id == target)
            {
                return Err(invalid("ผู้รับใหม่เป็นผู้มอบหมายเดิมหรือผู้ตรวจทานของงาน"));
            }
            parent = ancestor.parent_id;
        }
        if a.parent_id.is_none() && a.purpose == DirectedPurpose::Decide {
            let register: String = DbSession::new(&mut **tx)
                .scalar(
                    "workflow.src.directed.reassign",
                    "SELECT register_kind FROM correspondence_records WHERE id=$1",
                )
                .bind(instance.source.resource_id)
                .fetch_one()
                .await?;
            let register = match register.as_str() {
                "incoming" => CorrespondenceRegister::Incoming,
                "outgoing" => CorrespondenceRegister::Outgoing,
                "internal" => CorrespondenceRegister::Internal,
                _ => return Err(invalid("ทะเบียนหนังสือไม่ถูกต้อง")),
            };
            let profile = settings_in(tx, register).await?;
            if !eligible_ids(tx, &profile, &permitted)
                .await?
                .contains(&target)
            {
                return Err(invalid("ผู้พิจารณาคนแรกไม่อยู่ในรายชื่อที่โรงเรียนกำหนด"));
            }
        }
    }
    let item = if reviewing {
        a.review_work_item_id
    } else {
        a.work_item_id
    };
    DbSession::new(&mut **tx).query("workflow.src.directed.reassign", "UPDATE process_notification_sources SET cancelled_at=now() WHERE work_item_id=$1 AND recipient_id=$2 AND cancelled_at IS NULL").bind(item).bind(old).execute().await?;
    DbSession::new(&mut **tx).query("workflow.src.directed.reassign",
        "UPDATE work_item_assignees SET status='dismissed' WHERE work_item_id=$1 AND user_id=$2",
    )
    .bind(item)
    .bind(old)
    .execute()
    .await?;
    work::close_process_item_in_transaction(tx, item).await?;
    replacement.reassigned_from = Some(old);
    let a = &mut graph_mut(instance)?.assignments[index];
    if reviewing {
        a.reviewer = Some(replacement);
        a.review_work_item_id = None;
    } else {
        a.recipient = replacement;
        a.selector = selector;
        a.work_item_id = None;
    }
    activate(tx, instance, index, reviewing).await?;
    Ok(())
}

pub(crate) async fn action(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    instance: &mut ProcessInstance,
    request: &ProcessActionRequest,
    manager_reassign: bool,
) -> Result<(), AppError> {
    if instance.revision != request.expected_revision {
        return Err(stale());
    }
    let terminal_copy = matches!(
        instance.status,
        ProcessStatus::Completed | ProcessStatus::Closed
    );
    if instance.status != ProcessStatus::Active && !terminal_copy {
        return Err(stale());
    }
    let id = request
        .assignment_id
        .ok_or_else(|| invalid("กรุณาระบุงานที่ต้องการดำเนินการ"))?;
    if !assignment_actions(actor, instance, manager_reassign)
        .iter()
        .any(|a| a.assignment_id == id && a.actions.contains(&request.action))
    {
        return Err(denied());
    }
    if (request.action == ProcessAction::Route) != request.routing.is_some() {
        return Err(invalid("ข้อมูลส่งต่อไม่ตรงกับการดำเนินการ"));
    }
    let index = graph_mut(instance)?
        .assignments
        .iter()
        .position(|a| a.id == id)
        .ok_or_else(denied)?;
    if terminal_copy
        && (request.comment.is_some()
            || !request.annotations.is_empty()
            || request.signature.is_some()
            || !request.evidence_file_ids.is_empty()
            || request.reassign_user_id.is_some()
            || request.reassign_from_user_id.is_some())
    {
        return Err(invalid("สำเนาของเรื่องที่จบแล้วรับเฉพาะการอ่านหรือรับทราบ"));
    }
    let reviewing =
        graph_mut(instance)?.assignments[index].status == DirectedAssignmentStatus::WaitingReview;
    let item = if reviewing {
        graph_mut(instance)?.assignments[index].review_work_item_id
    } else {
        graph_mut(instance)?.assignments[index].work_item_id
    };
    match request.action {
        ProcessAction::Reassign => reassign(tx, instance, index, request).await?,
        ProcessAction::Read => {
            let a = &mut graph_mut(instance)?.assignments[index];
            let recipient = if reviewing {
                a.reviewer.as_mut().ok_or_else(denied)?
            } else {
                &mut a.recipient
            };
            recipient.read_at.get_or_insert_with(Utc::now);
            work::mark_process_assignment_in_transaction(tx, item, actor.user_id, false).await?;
        }
        ProcessAction::Route => {
            if request
                .comment
                .as_deref()
                .is_none_or(|c| c.trim().is_empty())
            {
                return Err(invalid("กรุณาระบุข้อความสั่งการ"));
            }
            route(
                tx,
                actor,
                instance,
                index,
                request
                    .routing
                    .as_ref()
                    .ok_or_else(|| invalid("กรุณาระบุผู้รับ"))?,
            )
            .await?;
        }
        ProcessAction::Report => {
            if request
                .comment
                .as_deref()
                .is_none_or(|c| c.trim().is_empty())
            {
                return Err(invalid("กรุณาระบุรายงาน"));
            }
        }
        ProcessAction::Reject | ProcessAction::ReturnForRevision => {
            if request
                .comment
                .as_deref()
                .is_none_or(|c| c.trim().is_empty())
            {
                return Err(invalid("กรุณาระบุเหตุผล"));
            }
            instance.status = if request.action == ProcessAction::Reject {
                ProcessStatus::Rejected
            } else {
                ProcessStatus::RevisionRequested
            };
            let items: Vec<_> = graph_mut(instance)?
                .assignments
                .iter()
                .flat_map(|a| [a.work_item_id, a.review_work_item_id])
                .flatten()
                .collect();
            for item in items {
                work::close_process_item_in_transaction(tx, Some(item)).await?;
            }
        }
        ProcessAction::Acknowledge | ProcessAction::Execute | ProcessAction::Review => {
            if request.action == ProcessAction::Review
                && !reviewing
                && request
                    .comment
                    .as_deref()
                    .is_none_or(|c| c.trim().is_empty())
            {
                return Err(invalid("กรุณาระบุผลการพิจารณา"));
            }
            let a = &mut graph_mut(instance)?.assignments[index];
            a.status = DirectedAssignmentStatus::Completed;
            a.completed_at = Some(Utc::now());
            a.recipient.completed_at = a.completed_at;
            if reviewing {
                if let Some(r) = a.reviewer.as_mut() {
                    r.completed_at = a.completed_at;
                }
            }
            work::mark_process_assignment_in_transaction(tx, item, actor.user_id, true).await?;
            work::close_process_item_in_transaction(tx, item).await?;
            if !terminal_copy {
                advance(tx, instance).await?;
            }
        }
        _ => return Err(denied()),
    }
    instance.revision += 1;
    if !terminal_copy
        && matches!(
            instance.status,
            ProcessStatus::Completed | ProcessStatus::Rejected | ProcessStatus::RevisionRequested
        )
    {
        process::notify_in_transaction(
            tx,
            instance,
            instance.created_by,
            "มีผลการดำเนินการหนังสือ",
            "outcome",
            None,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_cannot_acquire_routing_authority() {
        let request = DirectedRoutingRequest {
            recipients: vec![DirectedRouteRecipient {
                recipient: ProcessRecipient {
                    kind: ProcessRecipientKind::User,
                    user_id: Some(Uuid::now_v7()),
                    organization_unit_id: None,
                    position_code: None,
                },
                purpose: DirectedPurpose::Execute,
                required: false,
                allow_onward_assignment: true,
                due_at: None,
            }],
            require_review: false,
            reviewer: None,
        };
        assert!(validate_routing(&request).is_err());
    }
    #[test]
    fn absent_settings_have_no_hardcoded_hierarchy_or_return() {
        let profile = defaults(CorrespondenceRegister::Incoming);
        assert!(profile.eligible_decision_makers.is_empty());
        assert!(!profile.default_require_review);
        assert!(profile.default_reviewer.is_none());
    }
}
