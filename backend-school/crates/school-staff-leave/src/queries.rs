use super::configuration::list_types;
use super::{models::*, policy::*, repository::*};
use crate::permission_codes_generated as codes;
use chrono::Utc;
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_workflow::process_models::*;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;
pub async fn options(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<LeavePersonalOptions, AppError> {
    let types = list_types(pool, actor, LeaveListQuery::default()).await?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.queries.options").await?;
    let personnel_type = category(
        &mut tx,
        actor.user_id,
        Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive(),
    )
    .await?;
    tx.commit().await?;
    Ok(LeavePersonalOptions {
        types,
        personnel_type,
    })
}
pub async fn routing_options(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<LeaveRoutingOptions, AppError> {
    require_configuration_manage(actor)?;

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.leave.queries.routing_options",
    )
    .await?;
    routing_options_on_connection(&mut connection, actor).await
}

pub async fn routing_options_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
) -> Result<LeaveRoutingOptions, AppError> {
    require_configuration_manage(actor)?;
    let users=DbSession::new(connection).query_as::<(Uuid,String)>("staff.leave.queries.routing_options_on_connection", "SELECT id,display_name FROM user_display_names WHERE user_type='staff' AND status='active' ORDER BY display_name,id LIMIT 500").fetch_all().await?.into_iter().map(|(id,display_name)|LeaveRoutingUser{id,display_name}).collect();
    let organizations: Vec<LeaveRoutingOrganization> = DbSession::new(connection)
        .query_as::<(Uuid, String)>(
            "staff.leave.queries.routing_options_on_connection",
            "SELECT id,name FROM organization_units WHERE is_active ORDER BY name,id LIMIT 500",
        )
        .fetch_all()
        .await?
        .into_iter()
        .map(|(id, name)| LeaveRoutingOrganization { id, name })
        .collect();
    // Positions are the provider's closed organization-role vocabulary.
    let positions = organizations
        .iter()
        .flat_map(|organization| {
            [
                ("director", "ผู้อำนวยการ"),
                ("deputy_director", "รองผู้อำนวยการ"),
                ("head", "หัวหน้า"),
                ("deputy_head", "รองหัวหน้า"),
                ("coordinator", "ผู้ประสานงาน"),
                ("member", "สมาชิก"),
            ]
            .map(|(code, name)| LeaveRoutingPosition {
                organization_unit_id: organization.id,
                code: code.into(),
                name: name.into(),
            })
        })
        .collect();
    Ok(LeaveRoutingOptions {
        users,
        organizations,
        positions,
    })
}
pub async fn detail(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<LeaveRequestDetail, AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "staff.leave.queries.detail").await?;
    let r = load(&mut tx, id).await?;
    require_read(&mut tx, actor, &r).await?;
    let editable_state = matches!(
        r.document.status,
        LeaveStatus::Draft | LeaveStatus::Returned
    );
    let travel = r.document.kind == LeaveKind::OfficialTravel;
    let policy = if editable_state && travel {
        None
    } else if editable_state {
        preview_policy(
            &mut tx,
            r.document.owner_id,
            r.document.type_id,
            r.document.start_date,
        )
        .await?
        .map(|(_, p)| p)
    } else {
        pinned_policy(&mut tx, r.policy_id).await?
    };
    let current_instance = if r.document.status == LeaveStatus::CancellationPending {
        r.instance_id
    } else {
        DbSession::new(&mut *tx).scalar("staff.leave.queries.detail", "SELECT id FROM process_instances WHERE source_module='staff_leave' AND source_type='staff_leave_request' AND source_id=$1").bind(id).fetch_optional().await?
    };
    let i = instance(&mut tx, current_instance).await?;
    let own =
        r.document.owner_id == actor.user_id && actor.has_permission(codes::STAFF_LEAVE_CREATE_OWN);
    let can_edit = own && editable_state;
    let can_act = actor.user_id != r.document.owner_id
        && actor.has_permission(codes::STAFF_LEAVE_UPDATE_ASSIGNED)
        && i.as_ref().is_some_and(|i| {
            i.status == ProcessStatus::Active
                && i.steps.get(i.current_step).is_some_and(|s| {
                    s.recipients
                        .iter()
                        .any(|p| p.user_id == actor.user_id && p.completed_at.is_none())
                })
        })
        && matches!(
            r.document.status,
            LeaveStatus::Submitted | LeaveStatus::CancellationPending
        );
    let coverage = super::coverage::detail_on_connection(&mut tx, actor, &r).await?;
    let companion_confirmations = super::companions::statuses(&mut tx, &r.document).await?;
    let companions_ready = companion_confirmations.iter().all(|s| s.confirmed);
    let coverage_ready = coverage.state == super::coverage_models::LeaveCoverageState::Confirmed
        || coverage.state == super::coverage_models::LeaveCoverageState::LegacyUnverified;
    let available_actions = if can_act {
        let step = i.as_ref().and_then(|i| i.steps.get(i.current_step));
        let action = if step.is_some_and(|s| s.definition.kind == ProcessStepKind::Review) {
            ProcessAction::Review
        } else {
            ProcessAction::Approve
        };
        let mut actions = vec![ProcessAction::Reject, ProcessAction::ReturnForRevision];
        if (coverage_ready && companions_ready)
            || r.document.status == LeaveStatus::CancellationPending
        {
            actions.insert(0, action)
        }
        actions
    } else {
        vec![]
    };
    let history=DbSession::new(&mut *tx).scalar::<Json<LeaveHistoryEntry>>("staff.leave.queries.detail", "SELECT entry FROM staff_leave_history WHERE request_id=$1 ORDER BY (entry->>'revision')::bigint,id").bind(id).fetch_all().await?.into_iter().map(|e|e.0).collect();
    let submission_blocking_reason = if can_edit {
        require_current_record(&r.document)
            .and_then(|()| {
                if travel {
                    return Ok(());
                }
                match &policy {
                    Some(p) => require_current_policy(r.document.kind, p),
                    None => Err(invalid("ไม่พบเกณฑ์ที่ใช้กับวันที่และประเภทบุคลากรนี้")),
                }
            })
            .err()
            .map(|e| match e {
                AppError::ValidationError(message) => message,
                _ => "กรุณาตรวจสอบช่วงคำขอและเกณฑ์ปัจจุบัน".into(),
            })
    } else {
        None
    };
    let result = LeaveRequestDetail {
        companion_confirmations,
        coverage,
        process_purpose: if r.document.status == LeaveStatus::CancellationPending {
            LeaveProcessPurpose::Cancellation
        } else {
            LeaveProcessPurpose::Request
        },
        can_edit,
        can_submit: can_edit
            && submission_blocking_reason.is_none()
            && (travel || policy.as_ref().is_some_and(|p| p.enabled)),
        submission_blocking_reason,
        can_act,
        can_cancel: own
            && matches!(
                r.document.status,
                LeaveStatus::Draft
                    | LeaveStatus::Returned
                    | LeaveStatus::Submitted
                    | LeaveStatus::Approved
            ),
        available_actions,
        request: r.document.0,
        policy,
        instance: i,
        history,
        evidence_file_ids: r.evidence_file_ids,
        content_hash: r.content_hash,
    };
    tx.commit().await?;
    Ok(result)
}
pub async fn list(
    pool: &PgPool,
    actor: &ActorContext,
    q: LeaveListQuery,
) -> Result<LeaveRequestPage, AppError> {
    require_leave_list_access(actor)?;
    validate_leave_page(q.limit.unwrap_or(30), q.offset.unwrap_or(0))?;
    let _preflight_status = serialized_leave_status(q.status)?;

    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "staff.leave.queries.list")
            .await?;
    list_on_connection(&mut connection, actor, q).await
}

pub async fn list_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    q: LeaveListQuery,
) -> Result<LeaveRequestPage, AppError> {
    require_leave_list_access(actor)?;
    let limit = q.limit.unwrap_or(30);
    let offset = q.offset.unwrap_or(0);
    validate_leave_page(limit, offset)?;
    let kind = q.kind.map(LeaveKind::as_str);
    let status = serialized_leave_status(q.status)?;
    let mut items:Vec<_>=DbSession::new(connection).scalar::<Json<LeaveRequestSummary>>("staff.leave.queries.list_on_connection", "SELECT r.document FROM staff_leave_requests r WHERE ($1 OR ($2 AND r.owner_id=$3) OR ($4 AND EXISTS(SELECT 1 FROM process_participants p JOIN process_instances i ON i.id=p.instance_id WHERE p.user_id=$3 AND i.source_module='staff_leave' AND (i.source_id=r.id OR EXISTS(SELECT 1 FROM staff_leave_history h WHERE h.request_id=r.id AND h.entry->>'action'='cancellation_requested' AND i.source_id=h.id))))) AND ($5::text IS NULL OR r.document->>'kind'=$5) AND ($6::text IS NULL OR r.status=$6) ORDER BY r.created_at DESC,r.id DESC LIMIT $7 OFFSET $8").bind(actor.has_permission(codes::STAFF_LEAVE_READ_SCHOOL)).bind(actor.has_permission(codes::STAFF_LEAVE_READ_OWN)).bind(actor.user_id).bind(actor.has_permission(codes::STAFF_LEAVE_READ_ASSIGNED)).bind(kind).bind(status.as_ref().and_then(|s|s.as_str())).bind(limit+1).bind(offset).fetch_all().await?.into_iter().map(|r|r.0).collect();
    let has_more = items.len() > limit as usize;
    items.truncate(limit as usize);
    Ok(LeaveRequestPage { items, has_more })
}

fn require_leave_list_access(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::STAFF_LEAVE_READ_OWN,
        codes::STAFF_LEAVE_READ_ASSIGNED,
        codes::STAFF_LEAVE_READ_SCHOOL,
    ])?;
    Ok(())
}

pub async fn usage(
    pool: &PgPool,
    actor: &ActorContext,
    q: LeaveUsageQuery,
) -> Result<LeaveUsageReport, AppError> {
    validate_leave_usage_request(actor, &q)?;

    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "staff.leave.queries.usage")
            .await?;
    usage_on_connection(&mut connection, actor, q).await
}

pub async fn usage_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    q: LeaveUsageQuery,
) -> Result<LeaveUsageReport, AppError> {
    validate_leave_usage_request(actor, &q)?;
    let kind = q.kind.map(LeaveKind::as_str);
    #[derive(sqlx::FromRow)]
    struct UsageRow {
        document: Json<LeaveUsageItem>,
        compatible: bool,
    }
    let rows:Vec<UsageRow>=DbSession::new(connection).query_as("staff.leave.queries.usage_on_connection",r#"
      WITH ledger AS (
        SELECT r.owner_id,r.type_id,r.quota_year_start,r.document->>'ownerName' display_name,r.document->>'typeName' type_name,r.status,
          CASE WHEN r.document->>'unit' IN ('day','half_day') THEN 'day' ELSE 'minute' END canonical_unit,
          r.document->>'unit' IN ('day','half_day','minute') AND p.document->>'unit' IN ('day','half_day','minute') valid_unit,
          r.amount::numeric*CASE WHEN r.document->>'unit'='day' THEN 2 ELSE 1 END amount_scaled,
          p.document policy,p.version,p.id policy_id
        FROM staff_leave_requests r JOIN staff_leave_policies p ON p.id=r.policy_id
        WHERE r.quota_year_start IS NOT NULL AND ($1 OR r.owner_id=$2) AND (r.document->>'startDate')::date BETWEEN $3 AND $4
          AND ($5::text IS NULL OR r.document->>'kind'=$5) AND r.status IN ('submitted','approved','cancellation_pending')
      ), grouped AS (
        SELECT owner_id,type_id,quota_year_start,max(display_name) display_name,max(type_name) type_name,
          max(canonical_unit) unit,bool_and(valid_unit) AND count(DISTINCT canonical_unit)=1 compatible,
          coalesce(sum(amount_scaled) FILTER(WHERE status IN ('approved','cancellation_pending')),0)::text approved,
          coalesce(sum(amount_scaled) FILTER(WHERE status='submitted'),0)::text reserved,
          (array_agg(policy ORDER BY version DESC))[1] policy,(array_agg(policy_id ORDER BY version DESC))[1] policy_id
        FROM ledger GROUP BY owner_id,type_id,quota_year_start
      ) SELECT jsonb_build_object('userId',owner_id,'displayName',display_name,'typeId',type_id,'typeName',type_name,
          'unit',unit,'amountScale',CASE WHEN unit='day' THEN 2 ELSE 1 END,'quotaYearStart',quota_year_start,
          'approvedAmount',approved,'reservedAmount',reserved,'annualLimit',CASE WHEN policy->>'annualLimit' IS NULL THEN NULL
            ELSE ((policy->>'annualLimit')::numeric*CASE WHEN policy->>'unit'='day' THEN 2 ELSE 1 END)::text END,
          'policyVersionId',policy_id) document,compatible FROM grouped
      ORDER BY owner_id,type_id,quota_year_start LIMIT 5001
    "#).bind(actor.has_permission(codes::STAFF_LEAVE_DOWNLOAD_SCHOOL)).bind(actor.user_id).bind(q.from).bind(q.to).bind(kind).fetch_all().await?;
    if rows.iter().any(|r| !r.compatible) {
        return Err(invalid(
            "หน่วยในบัญชีสิทธิ์เดิมไม่เข้ากัน กรุณาให้ผู้ดูแลตรวจสอบโดยไม่แปลงวันเป็นนาที",
        ));
    }
    let items = rows.into_iter().map(|r| r.document.0).collect::<Vec<_>>();
    if items.len() > 5000 {
        return Err(invalid("รายงานมีรายการมากเกินไป กรุณาลดช่วงวันที่"));
    }
    Ok(LeaveUsageReport {
        items,
        from: q.from,
        to: q.to,
    })
}

fn validate_leave_usage_request(actor: &ActorContext, q: &LeaveUsageQuery) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::STAFF_LEAVE_DOWNLOAD_OWN,
        codes::STAFF_LEAVE_DOWNLOAD_SCHOOL,
    ])?;
    if q.to < q.from || (q.to - q.from).num_days() > 1096 {
        return Err(invalid("ช่วงรายงานต้องไม่เกินสามปี"));
    }
    Ok(())
}

fn validate_leave_page(limit: i64, offset: i64) -> Result<(), AppError> {
    if !(1..=100).contains(&limit) || !(0..=10000).contains(&offset) {
        return Err(invalid("ขนาดหน้าหรือจุดเริ่มต้นไม่ถูกต้อง"));
    }
    Ok(())
}

fn serialized_leave_status(
    status: Option<LeaveStatus>,
) -> Result<Option<serde_json::Value>, AppError> {
    status
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| invalid("สถานะไม่ถูกต้อง"))
}
