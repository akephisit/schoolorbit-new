//! Scope predicates are unions. Organization relationships are resolved by Authorization.
use crate::{
    models::*,
    repository::{self, ProjectRecord},
};
use school_authorization::{
    accessible_exact_units_for_permission_on_connection,
    accessible_tree_units_for_permission_on_connection,
    resource_access_policy::{
        accessible_organization_unit_ids_on_connection, UserResourceListAccess,
    },
    ActorContext,
};
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;
#[derive(Debug, Clone, Default)]
pub(crate) struct Scope {
    pub school: bool,
    pub own: bool,
    pub assigned: bool,
    pub units: Vec<Uuid>,
}
impl Scope {
    pub fn any(&self) -> bool {
        self.school || self.own || self.assigned || !self.units.is_empty()
    }
}
pub(crate) async fn read_scope(
    pool: &PgPool,
    a: &ActorContext,
    export: bool,
) -> Result<Scope, AppError> {
    let (s, unit, tree) = read_scope_seed(a, export);
    if !s.school && (a.has_permission(unit) || a.has_permission(tree)) {
        let mut connection =
            school_tenancy::TenantPool::observed_checkout_pool(pool, "budget.policy.read_scope")
                .await?;
        return read_scope_on_connection(&mut connection, a, export).await;
    }
    require_scope(s)
}
fn read_scope_seed(a: &ActorContext, export: bool) -> (Scope, &'static str, &'static str) {
    let (school, own, assigned, unit, tree) = if export {
        (
            codes::BUDGET_DOWNLOAD_SCHOOL,
            codes::BUDGET_DOWNLOAD_OWN,
            None,
            codes::BUDGET_DOWNLOAD_ORGANIZATION_UNIT,
            codes::BUDGET_DOWNLOAD_ORGANIZATION_TREE,
        )
    } else {
        (
            codes::BUDGET_READ_SCHOOL,
            codes::BUDGET_READ_OWN,
            Some(codes::BUDGET_READ_ASSIGNED),
            codes::BUDGET_READ_ORGANIZATION_UNIT,
            codes::BUDGET_READ_ORGANIZATION_TREE,
        )
    };
    let s = Scope {
        school: a.has_permission(school),
        own: a.has_permission(own),
        assigned: assigned.is_some_and(|p| a.has_permission(p)),
        units: vec![],
    };
    (s, unit, tree)
}
fn require_scope(s: Scope) -> Result<Scope, AppError> {
    if !s.any() {
        return Err(repository::denied());
    }
    Ok(s)
}
pub(crate) async fn read_scope_on_connection(
    connection: &mut sqlx::PgConnection,
    a: &ActorContext,
    export: bool,
) -> Result<Scope, AppError> {
    let (mut s, unit, tree) = read_scope_seed(a, export);
    if !s.school {
        if a.has_permission(unit) {
            s.units.extend(
                accessible_exact_units_for_permission_on_connection(connection, a.user_id, unit)
                    .await?,
            );
        }
        if a.has_permission(tree) {
            s.units.extend(
                accessible_tree_units_for_permission_on_connection(connection, a.user_id, tree)
                    .await?,
            );
        }
        s.units.sort();
        s.units.dedup();
    }
    if !s.any() {
        return Err(repository::denied());
    }
    Ok(s)
}

pub(crate) async fn update_scope(pool: &PgPool, a: &ActorContext) -> Result<Scope, AppError> {
    let s = update_scope_seed(a);
    if !s.school
        && a.has_any_permission(&[
            codes::BUDGET_UPDATE_ORGANIZATION_UNIT,
            codes::BUDGET_UPDATE_ORGANIZATION_TREE,
        ])
    {
        let mut connection =
            school_tenancy::TenantPool::observed_checkout_pool(pool, "budget.policy.update_scope")
                .await?;
        return update_scope_on_connection(&mut connection, a).await;
    }
    Ok(s)
}
fn update_scope_seed(a: &ActorContext) -> Scope {
    let s = Scope {
        school: a.has_permission(codes::BUDGET_UPDATE_SCHOOL),
        own: a.has_permission(codes::BUDGET_UPDATE_OWN),
        assigned: false,
        units: vec![],
    };
    s
}
pub(crate) async fn update_scope_on_connection(
    connection: &mut sqlx::PgConnection,
    a: &ActorContext,
) -> Result<Scope, AppError> {
    let mut s = update_scope_seed(a);
    if !s.school {
        if a.has_permission(codes::BUDGET_UPDATE_ORGANIZATION_UNIT) {
            s.units.extend(
                accessible_exact_units_for_permission_on_connection(
                    connection,
                    a.user_id,
                    codes::BUDGET_UPDATE_ORGANIZATION_UNIT,
                )
                .await?,
            );
        }
        if a.has_permission(codes::BUDGET_UPDATE_ORGANIZATION_TREE) {
            s.units.extend(
                accessible_tree_units_for_permission_on_connection(
                    connection,
                    a.user_id,
                    codes::BUDGET_UPDATE_ORGANIZATION_TREE,
                )
                .await?,
            );
        }
        s.units.sort();
        s.units.dedup();
    }
    Ok(s)
}

pub(crate) fn direct(s: &Scope, actor: Uuid, owner: Uuid, unit: Uuid) -> bool {
    s.school || (s.own && actor == owner) || s.units.contains(&unit)
}

pub(crate) async fn assigned_project_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: Uuid,
    id: Uuid,
) -> Result<bool, AppError> {
    Ok(DbSession::new(&mut *connection)
        .scalar(
            "budget.src.policy.assigned_project_on_connection",
            ASSIGNED_PROJECT_SQL,
        )
        .bind(actor)
        .bind(id)
        .fetch_one()
        .await?)
}
pub(crate) async fn require_read(
    connection: &mut sqlx::PgConnection,
    a: &ActorContext,
    r: &ProjectRecord,
) -> Result<(), AppError> {
    let s = read_scope_on_connection(connection, a, false).await?;
    if direct(
        &s,
        a.user_id,
        r.owner_id,
        r.document.project.organization_unit_id,
    ) || (s.assigned && assigned_project_on_connection(connection, a.user_id, r.id).await?)
    {
        Ok(())
    } else {
        Err(repository::denied())
    }
}
pub(crate) async fn require_update(
    connection: &mut sqlx::PgConnection,
    a: &ActorContext,
    r: &ProjectRecord,
) -> Result<(), AppError> {
    let s = update_scope_on_connection(connection, a).await?;
    if direct(
        &s,
        a.user_id,
        r.owner_id,
        r.document.project.organization_unit_id,
    ) {
        Ok(())
    } else {
        Err(repository::denied())
    }
}
pub(crate) fn require_create_permission(a: &ActorContext) -> Result<(), AppError> {
    a.require_permission(codes::BUDGET_CREATE_OWN)
}
pub(crate) async fn require_create_unit(
    connection: &mut sqlx::PgConnection,
    a: &ActorContext,
    unit: Uuid,
) -> Result<(), AppError> {
    require_create_permission(a)?;
    let s = update_scope_on_connection(connection, a).await?;
    let member = accessible_organization_unit_ids_on_connection(
        connection,
        UserResourceListAccess::OrganizationUnit(a.user_id),
    )
    .await?
    .unwrap_or_default();
    if s.school || s.units.contains(&unit) || member.contains(&unit) {
        Ok(())
    } else {
        Err(repository::denied())
    }
}
pub(crate) fn can_act(
    a: &ActorContext,
    instance: Option<&school_workflow::process_models::ProcessInstance>,
    owner: Uuid,
) -> bool {
    a.user_id != owner
        && a.has_permission(codes::BUDGET_APPROVE_ASSIGNED)
        && instance.is_some_and(|i| {
            a.user_id != i.created_by
                && i.status == school_workflow::process_models::ProcessStatus::Active
                && i.steps.get(i.current_step).is_some_and(|s| {
                    s.recipients
                        .iter()
                        .any(|r| r.user_id == a.user_id && r.completed_at.is_none())
                })
        })
}
pub(crate) fn capabilities(
    a: &ActorContext,
    editable: bool,
    act: bool,
    accepted: bool,
) -> BudgetCapabilities {
    BudgetCapabilities {
        can_edit: editable,
        can_submit: editable,
        can_act: act,
        can_amend: editable && accepted,
        can_create_request: accepted && a.has_permission(codes::BUDGET_CREATE_OWN),
        can_pay: a.has_permission(codes::BUDGET_PAYMENT_EXECUTE_SCHOOL),
        can_close: a.has_permission(codes::BUDGET_PAYMENT_EXECUTE_SCHOOL),
        can_adjust: a.has_permission(codes::BUDGET_ADJUSTMENT_CREATE_SCHOOL),
        can_manage_settings: a.has_permission(codes::BUDGET_MANAGE_SCHOOL),
        can_export: a.has_any_permission(&[
            codes::BUDGET_DOWNLOAD_OWN,
            codes::BUDGET_DOWNLOAD_ORGANIZATION_UNIT,
            codes::BUDGET_DOWNLOAD_ORGANIZATION_TREE,
            codes::BUDGET_DOWNLOAD_SCHOOL,
        ]),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn scope_preflight_keeps_direct_grants_and_denials_database_free() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        for permission in [
            codes::BUDGET_READ_SCHOOL,
            codes::BUDGET_READ_OWN,
            codes::BUDGET_READ_ASSIGNED,
        ] {
            let actor = ActorContext {
                user_id: Uuid::new_v4(),
                permissions: vec![permission.into()],
            };
            assert!(read_scope(&pool, &actor, false).await.unwrap().any());
        }
        let denied = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![],
        };
        assert!(matches!(
            read_scope(&pool, &denied, false).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            read_scope(&pool, &denied, true).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(!update_scope(&pool, &denied).await.unwrap().any());
        let own = ActorContext {
            permissions: vec![codes::BUDGET_UPDATE_OWN.into()],
            ..denied
        };
        assert!(update_scope(&pool, &own).await.unwrap().own);
    }
    #[test]
    fn independent_scopes_form_a_union() {
        let actor = Uuid::now_v7();
        let unit = Uuid::now_v7();
        let s = Scope {
            own: true,
            units: vec![unit],
            ..Default::default()
        };
        assert!(direct(&s, actor, actor, Uuid::now_v7()));
        assert!(direct(&s, actor, Uuid::now_v7(), unit));
        assert!(!direct(&s, actor, Uuid::now_v7(), Uuid::now_v7()));
    }
}

const ASSIGNED_PROJECT_SQL:&str="SELECT EXISTS(SELECT 1 FROM process_participants pp JOIN process_instances i ON i.id=pp.instance_id JOIN budget_projects p ON p.id=$2 WHERE pp.user_id=$1 AND i.source_module='budget' AND ((i.source_type='budget_project_plan' AND EXISTS(SELECT 1 FROM budget_project_attempts attempt WHERE attempt.project_id=p.id AND attempt.id=i.source_id)) OR (i.source_type='budget_request' AND EXISTS(SELECT 1 FROM budget_requests r WHERE r.project_id=p.id AND r.id=i.source_id)) OR (i.source_type='budget_adjustment' AND EXISTS(SELECT 1 FROM budget_adjustments a WHERE (a.from_project_id=p.id OR a.to_project_id=p.id) AND a.id=i.source_id))))";

pub(crate) async fn require_read_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    a: &ActorContext,
    r: &ProjectRecord,
    s: &Scope,
) -> Result<(), AppError> {
    if direct(
        s,
        a.user_id,
        r.owner_id,
        r.document.project.organization_unit_id,
    ) {
        return Ok(());
    }
    if s.assigned
        && DbSession::new(&mut **tx)
            .scalar::<bool>(
                "budget.src.policy.require_read_in_transaction",
                ASSIGNED_PROJECT_SQL,
            )
            .bind(a.user_id)
            .bind(r.id)
            .fetch_one()
            .await?
    {
        return Ok(());
    }
    Err(repository::denied())
}
pub(crate) fn require_update_with_scope(
    a: &ActorContext,
    r: &ProjectRecord,
    s: &Scope,
) -> Result<(), AppError> {
    if direct(
        s,
        a.user_id,
        r.owner_id,
        r.document.project.organization_unit_id,
    ) {
        Ok(())
    } else {
        Err(repository::denied())
    }
}
