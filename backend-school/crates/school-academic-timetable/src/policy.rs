use school_database::DbSession;
use sqlx::PgPool;
use uuid::Uuid;

use school_authorization::ActorContext;
use school_authorization::{self as resource_access_policy, AcademicResourcePermissions};
use school_errors::AppError;
use school_permissions::registry::codes;

mod visibility;
pub(crate) use visibility::project_visibility;
pub use visibility::{project_workspace, recompute_workspace, TimetableVisibility};

const READ_ASSIGNED_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_TIMETABLE_READ_ASSIGNED,
    codes::ACADEMIC_TIMETABLE_MANAGE_ASSIGNED,
];
const READ_UNIT_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_TIMETABLE_READ_ORGANIZATION_UNIT,
    codes::ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_UNIT,
];
const READ_TREE_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_TIMETABLE_READ_ORGANIZATION_TREE,
    codes::ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_TREE,
];
const READ_SCHOOL_PERMISSIONS: &[&str] = &[
    codes::ACADEMIC_TIMETABLE_READ_SCHOOL,
    codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
    codes::ACADEMIC_TIMETABLE_PUBLISH_SCHOOL,
];
const MANAGE_ASSIGNED_PERMISSIONS: &[&str] = &[codes::ACADEMIC_TIMETABLE_MANAGE_ASSIGNED];
const MANAGE_UNIT_PERMISSIONS: &[&str] = &[codes::ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_UNIT];
const MANAGE_TREE_PERMISSIONS: &[&str] = &[codes::ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_TREE];
const MANAGE_SCHOOL_PERMISSIONS: &[&str] = &[codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimetableAction {
    Read,
    Manage,
    Publish,
    AlignDeliverySource,
}

enum ScopedTimetableAction {
    Read,
    Manage,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TimetableResourceSet {
    pub timetable_version_ids: Vec<Uuid>,
    pub learning_offering_ids: Vec<Uuid>,
    pub learning_group_ids: Vec<Uuid>,
    pub homeroom_ids: Vec<Uuid>,
    pub teacher_ids: Vec<Uuid>,
    pub room_ids: Vec<Uuid>,
    pub requires_school_scope: bool,
}

impl TimetableResourceSet {
    pub fn include_block(&mut self, block: &crate::models::timetable_block::TimetableBlock) {
        self.timetable_version_ids.push(block.timetable_version_id);
        self.learning_offering_ids
            .extend(block.learning_offering_id);
        self.learning_group_ids
            .extend(block.groups.iter().map(|group| group.learning_group_id));
        self.homeroom_ids
            .extend(block.homerooms.iter().map(|target| target.homeroom_id));
        self.teacher_ids
            .extend(block.teachers.iter().map(|target| target.teacher_id));
        self.teacher_ids.extend(
            block
                .groups
                .iter()
                .flat_map(|group| group.instructors.iter())
                .map(|teacher| teacher.teacher_id),
        );
        self.room_ids.extend(
            block
                .groups
                .iter()
                .filter_map(|group| group.room_id)
                .chain(block.homerooms.iter().filter_map(|target| target.room_id)),
        );
        self.requires_school_scope |=
            block.block_kind == crate::models::timetable_block::TimetableBlockKind::Structural;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TimetableAccessFilter {
    pub assigned_actor_id: Option<Uuid>,
    pub organization_unit_ids: Vec<Uuid>,
    pub organization_tree_unit_ids: Vec<Uuid>,
    pub includes_school_owned: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TimetableResourceTarget {
    owner_organization_unit_ids: Vec<Uuid>,
    actor_is_assigned: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct TimetableResourceTargetRow {
    resource_kind: String,
    resource_id: Uuid,
    owner_organization_unit_ids: Vec<Uuid>,
    actor_is_assigned: bool,
    permission_neutral: bool,
}

pub async fn timetable_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: TimetableAction,
) -> Result<TimetableAccessFilter, AppError> {
    let Some(action) = scoped_action(actor, action)? else {
        return Ok(TimetableAccessFilter {
            includes_school_owned: true,
            ..Default::default()
        });
    };
    let resolved = resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        timetable_permissions(action),
    )
    .await?;
    Ok(timetable_filter(resolved))
}

pub async fn timetable_list_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    action: TimetableAction,
) -> Result<TimetableAccessFilter, AppError> {
    let Some(action) = scoped_action(actor, action)? else {
        return Ok(TimetableAccessFilter {
            includes_school_owned: true,
            ..Default::default()
        });
    };

    let resolved = resource_access_policy::resolve_academic_resource_list_filter_on_connection(
        connection,
        actor,
        timetable_permissions(action),
    )
    .await?;
    Ok(timetable_filter(resolved))
}

fn scoped_action(
    actor: &ActorContext,
    action: TimetableAction,
) -> Result<Option<ScopedTimetableAction>, AppError> {
    let action = match action {
        TimetableAction::AlignDeliverySource => {
            actor.require_any_permission(&[
                codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
                codes::ACADEMIC_TIMETABLE_PUBLISH_SCHOOL,
            ])?;
            return Ok(None);
        }
        TimetableAction::Publish => {
            actor.require_permission(codes::ACADEMIC_TIMETABLE_PUBLISH_SCHOOL)?;
            return Ok(None);
        }
        TimetableAction::Read => ScopedTimetableAction::Read,
        TimetableAction::Manage => ScopedTimetableAction::Manage,
    };

    Ok(Some(action))
}

fn timetable_filter(
    resolved: school_authorization::AcademicResourceListFilter,
) -> TimetableAccessFilter {
    TimetableAccessFilter {
        assigned_actor_id: resolved.assigned_actor_id,
        organization_unit_ids: resolved.organization_unit_ids,
        organization_tree_unit_ids: resolved.organization_tree_unit_ids,
        includes_school_owned: resolved.includes_school_owned,
    }
}

pub async fn require_timetable_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: TimetableAction,
) -> Result<TimetableAccessFilter, AppError> {
    require_list_filter(timetable_list_access(pool, actor, action).await?)
}

pub async fn require_timetable_list_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    action: TimetableAction,
) -> Result<TimetableAccessFilter, AppError> {
    let filter = timetable_list_access_on_connection(connection, actor, action).await?;
    require_list_filter(filter)
}

fn require_list_filter(filter: TimetableAccessFilter) -> Result<TimetableAccessFilter, AppError> {
    if filter.includes_school_owned
        || filter.assigned_actor_id.is_some()
        || !filter.organization_unit_ids.is_empty()
        || !filter.organization_tree_unit_ids.is_empty()
    {
        Ok(filter)
    } else {
        Err(AppError::Forbidden(
            "ไม่มีสิทธิ์เข้าถึงทรัพยากรตารางสอน".to_string(),
        ))
    }
}

pub async fn require_timetable_resources(
    pool: &PgPool,
    actor: &ActorContext,
    action: TimetableAction,
    resources: &TimetableResourceSet,
) -> Result<(), AppError> {
    let preflight = match scoped_action(actor, action)? {
        None => Some(TimetableAccessFilter {
            includes_school_owned: true,
            ..Default::default()
        }),
        Some(scoped) => resource_access_policy::academic_resource_list_filter_without_database(
            actor,
            timetable_permissions(scoped),
        )
        .map(timetable_filter),
    }
    .map(require_list_filter)
    .transpose()?;
    if let Some(filter) = &preflight {
        if !needs_resource_lookup(filter, resources)? {
            return Ok(());
        }
    }
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_timetable.policy.require_timetable_resources",
    )
    .await?;
    let filter = match preflight {
        Some(filter) => filter,
        None => require_timetable_list_access_on_connection(&mut connection, actor, action).await?,
    };
    require_resources_with_filter(&mut connection, actor, resources, filter).await
}

pub async fn require_timetable_resources_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    action: TimetableAction,
    resources: &TimetableResourceSet,
) -> Result<(), AppError> {
    let filter = require_timetable_list_access_on_connection(connection, actor, action).await?;
    require_resources_with_filter(connection, actor, resources, filter).await
}

fn needs_resource_lookup(
    filter: &TimetableAccessFilter,
    resources: &TimetableResourceSet,
) -> Result<bool, AppError> {
    if resources.requires_school_scope && !filter.includes_school_owned {
        return Err(AppError::Forbidden(
            "รายการตารางสอนนี้ต้องใช้สิทธิ์ระดับโรงเรียน".to_string(),
        ));
    }
    if filter.includes_school_owned {
        return Ok(false);
    }

    Ok(!resources.timetable_version_ids.is_empty()
        || !resources.learning_offering_ids.is_empty()
        || !resources.learning_group_ids.is_empty()
        || !resources.homeroom_ids.is_empty()
        || !resources.teacher_ids.is_empty()
        || !resources.room_ids.is_empty())
}

async fn require_resources_with_filter(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    resources: &TimetableResourceSet,
    filter: TimetableAccessFilter,
) -> Result<(), AppError> {
    if !needs_resource_lookup(&filter, resources)? {
        return Ok(());
    }
    let snapshot = resource_access_snapshot_on_connection(connection, actor, resources).await?;
    snapshot.require_resources(resources, &filter)
}

#[derive(Debug, Clone, Default)]
pub struct TimetableResourceAccessSnapshot {
    targets: std::collections::BTreeMap<(String, Uuid), TimetableResourceTargetRow>,
}

impl TimetableResourceAccessSnapshot {
    pub fn require_resources(
        &self,
        resources: &TimetableResourceSet,
        filter: &TimetableAccessFilter,
    ) -> Result<(), AppError> {
        if !needs_resource_lookup(filter, resources)? {
            return Ok(());
        }
        for (kind, ids) in [
            ("version", &resources.timetable_version_ids),
            ("offering", &resources.learning_offering_ids),
            ("group", &resources.learning_group_ids),
            ("homeroom", &resources.homeroom_ids),
            ("teacher", &resources.teacher_ids),
            ("room", &resources.room_ids),
        ] {
            for id in normalized_ids(ids) {
                let target = self
                    .targets
                    .get(&(kind.into(), id))
                    .ok_or_else(|| AppError::NotFound("ไม่พบทรัพยากรตารางสอนบางรายการ".into()))?;
                if !target.permission_neutral
                    && !timetable_target_allowed(
                        filter,
                        &TimetableResourceTarget {
                            owner_organization_unit_ids: target.owner_organization_unit_ids.clone(),
                            actor_is_assigned: target.actor_is_assigned,
                        },
                    )
                {
                    return Err(AppError::Forbidden(
                        "ไม่มีสิทธิ์เข้าถึงทรัพยากรตารางสอนบางรายการ".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

pub async fn resource_access_snapshot_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    resources: &TimetableResourceSet,
) -> Result<TimetableResourceAccessSnapshot, AppError> {
    let version_ids = normalized_ids(&resources.timetable_version_ids);
    let offering_ids = normalized_ids(&resources.learning_offering_ids);
    let group_ids = normalized_ids(&resources.learning_group_ids);
    let homeroom_ids = normalized_ids(&resources.homeroom_ids);
    let teacher_ids = normalized_ids(&resources.teacher_ids);
    let room_ids = normalized_ids(&resources.room_ids);
    let targets = DbSession::new(connection).query_as::<TimetableResourceTargetRow>("academic_timetable.policy.require_timetable_resources_on_connection",
        r#"
        SELECT 'version'::text AS resource_kind,
               version.id AS resource_id,
               ARRAY[]::uuid[] AS owner_organization_unit_ids,
               false AS actor_is_assigned,
               true AS permission_neutral
        FROM academic_timetable_versions version
        WHERE version.id = ANY($1)
        UNION ALL
        SELECT 'offering', offering.id,
               ARRAY_REMOVE(ARRAY[offering.owning_organization_unit_id], NULL)::uuid[],
               EXISTS (
                   SELECT 1
                   FROM learning_groups learning_group
                   JOIN learning_group_teachers teacher
                     ON teacher.learning_group_id = learning_group.id
                   WHERE learning_group.learning_offering_id = offering.id
                     AND teacher.teacher_id = $7
               ),
               false
        FROM learning_offerings offering
        WHERE offering.id = ANY($2)
        UNION ALL
        SELECT 'group', learning_group.id,
               ARRAY_REMOVE(ARRAY[offering.owning_organization_unit_id], NULL)::uuid[],
               EXISTS (
                   SELECT 1
                   FROM learning_group_teachers teacher
                   WHERE teacher.learning_group_id = learning_group.id
                     AND teacher.teacher_id = $7
               ),
               false
        FROM learning_groups learning_group
        JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
        WHERE learning_group.id = ANY($3)
        UNION ALL
        SELECT 'homeroom', homeroom.id,
               COALESCE((
                   SELECT array_agg(DISTINCT member.organization_unit_id ORDER BY member.organization_unit_id)
                   FROM homeroom_advisors advisor
                   JOIN organization_members member ON member.user_id = advisor.user_id
                   WHERE advisor.homeroom_id = homeroom.id
                     AND member.ended_at IS NULL
               ), ARRAY[]::uuid[]),
               EXISTS (
                   SELECT 1 FROM homeroom_advisors advisor
                   WHERE advisor.homeroom_id = homeroom.id AND advisor.user_id = $7
               ),
               false
        FROM homerooms homeroom
        WHERE homeroom.id = ANY($4)
        UNION ALL
        SELECT 'teacher', teacher.id,
               COALESCE((
                   SELECT array_agg(DISTINCT member.organization_unit_id ORDER BY member.organization_unit_id)
                   FROM organization_members member
                   WHERE member.user_id = teacher.id AND member.ended_at IS NULL
               ), ARRAY[]::uuid[]),
               teacher.id = $7,
               false
        FROM users teacher
        WHERE teacher.id = ANY($5)
        UNION ALL
        SELECT 'room', room.id, ARRAY[]::uuid[], false, true
        FROM rooms room
        WHERE room.id = ANY($6)
        "#,
    )
    .bind(&version_ids)
    .bind(&offering_ids)
    .bind(&group_ids)
    .bind(&homeroom_ids)
    .bind(&teacher_ids)
    .bind(&room_ids)
    .bind(actor.user_id)
    .fetch_all()
    .await?;

    Ok(TimetableResourceAccessSnapshot {
        targets: targets
            .into_iter()
            .map(|target| ((target.resource_kind.clone(), target.resource_id), target))
            .collect(),
    })
}

fn timetable_target_allowed(
    filter: &TimetableAccessFilter,
    target: &TimetableResourceTarget,
) -> bool {
    filter.includes_school_owned
        || (target.actor_is_assigned && filter.assigned_actor_id.is_some())
        || target.owner_organization_unit_ids.iter().any(|owner_id| {
            filter.organization_unit_ids.contains(owner_id)
                || filter.organization_tree_unit_ids.contains(owner_id)
        })
}

fn normalized_ids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut normalized = ids.to_vec();
    normalized.sort_unstable();
    normalized.dedup();
    normalized
}

fn timetable_permissions(action: ScopedTimetableAction) -> AcademicResourcePermissions {
    match action {
        ScopedTimetableAction::Read => AcademicResourcePermissions {
            assigned: READ_ASSIGNED_PERMISSIONS,
            organization_unit: READ_UNIT_PERMISSIONS,
            organization_tree: READ_TREE_PERMISSIONS,
            school: READ_SCHOOL_PERMISSIONS,
        },
        ScopedTimetableAction::Manage => AcademicResourcePermissions {
            assigned: MANAGE_ASSIGNED_PERMISSIONS,
            organization_unit: MANAGE_UNIT_PERMISSIONS,
            organization_tree: MANAGE_TREE_PERMISSIONS,
            school: MANAGE_SCHOOL_PERMISSIONS,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use school_authorization::ActorContext;
    use school_permissions::registry::codes;
    use school_test_db::create_named_test_pool;
    use uuid::Uuid;

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    #[tokio::test]
    async fn list_and_empty_resource_grants_and_denials_do_not_require_a_database() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        for permission in [
            codes::ACADEMIC_TIMETABLE_READ_SCHOOL,
            codes::ACADEMIC_TIMETABLE_READ_ASSIGNED,
        ] {
            let granted = actor(&[permission]);
            assert!(
                require_timetable_list_access(&pool, &granted, TimetableAction::Read)
                    .await
                    .is_ok()
            );
            assert!(require_timetable_resources(
                &pool,
                &granted,
                TimetableAction::Read,
                &TimetableResourceSet::default()
            )
            .await
            .is_ok());
        }
        let denied = actor(&[]);
        assert!(matches!(
            require_timetable_list_access(&pool, &denied, TimetableAction::Read).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            require_timetable_resources(
                &pool,
                &denied,
                TimetableAction::Read,
                &TimetableResourceSet::default()
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
    }

    #[tokio::test]
    async fn alignment_accepts_only_school_manage_or_publish_without_granting_other_actions() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        for permission in [
            codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL,
            codes::ACADEMIC_TIMETABLE_PUBLISH_SCHOOL,
        ] {
            let actor = actor(&[permission]);
            let filter = timetable_list_access(&pool, &actor, TimetableAction::AlignDeliverySource)
                .await
                .unwrap();
            assert!(filter.includes_school_owned);
            let unrelated = if permission == codes::ACADEMIC_TIMETABLE_PUBLISH_SCHOOL {
                TimetableAction::Manage
            } else {
                TimetableAction::Publish
            };
            assert!(matches!(
                require_timetable_resources(
                    &pool,
                    &actor,
                    unrelated,
                    &TimetableResourceSet {
                        requires_school_scope: true,
                        ..Default::default()
                    }
                )
                .await,
                Err(AppError::Forbidden(_))
            ));
        }
        for permissions in [
            vec![],
            vec![codes::ACADEMIC_TIMETABLE_READ_SCHOOL],
            vec![codes::ACADEMIC_TIMETABLE_MANAGE_ASSIGNED],
            vec![codes::ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_UNIT],
            vec![codes::ACADEMIC_TIMETABLE_MANAGE_ORGANIZATION_TREE],
        ] {
            assert!(matches!(
                timetable_list_access(
                    &pool,
                    &actor(&permissions),
                    TimetableAction::AlignDeliverySource
                )
                .await,
                Err(AppError::Forbidden(_))
            ));
        }
    }

    #[tokio::test]
    async fn publish_requires_school_publish_permission_without_resolving_resource_scopes() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        for permissions in [
            vec![],
            vec![codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL],
            vec![codes::ACADEMIC_TIMETABLE_MANAGE_ASSIGNED],
        ] {
            assert!(matches!(
                timetable_list_access(&pool, &actor(&permissions), TimetableAction::Publish).await,
                Err(AppError::Forbidden(_))
            ));
        }
        let access = timetable_list_access(
            &pool,
            &actor(&[codes::ACADEMIC_TIMETABLE_PUBLISH_SCHOOL]),
            TimetableAction::Publish,
        )
        .await
        .unwrap();
        assert_eq!(
            access,
            TimetableAccessFilter {
                includes_school_owned: true,
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn timetable_policy_separates_read_manage_and_school_publish() {
        let pool = create_named_test_pool("timetable_access_policy_actions").await;
        let school_target = TimetableResourceSet {
            requires_school_scope: true,
            ..TimetableResourceSet::default()
        };

        require_timetable_resources(
            &pool,
            &actor(&[codes::ACADEMIC_TIMETABLE_READ_SCHOOL]),
            TimetableAction::Read,
            &school_target,
        )
        .await
        .expect("school reader must read a school-wide timetable resource");
        assert!(require_timetable_resources(
            &pool,
            &actor(&[codes::ACADEMIC_TIMETABLE_READ_SCHOOL]),
            TimetableAction::Manage,
            &school_target,
        )
        .await
        .is_err());
        assert!(require_timetable_resources(
            &pool,
            &actor(&[codes::ACADEMIC_TIMETABLE_MANAGE_SCHOOL]),
            TimetableAction::Publish,
            &school_target,
        )
        .await
        .is_err());
        require_timetable_resources(
            &pool,
            &actor(&[codes::ACADEMIC_TIMETABLE_PUBLISH_SCHOOL]),
            TimetableAction::Publish,
            &school_target,
        )
        .await
        .expect("publish must require its dedicated school permission");
    }

    #[test]
    fn timetable_target_authorization_unions_assignment_exact_unit_and_tree() {
        let actor_id = Uuid::new_v4();
        let exact_unit_id = Uuid::new_v4();
        let tree_unit_id = Uuid::new_v4();
        let filter = TimetableAccessFilter {
            assigned_actor_id: Some(actor_id),
            organization_unit_ids: vec![exact_unit_id],
            organization_tree_unit_ids: vec![tree_unit_id],
            includes_school_owned: false,
        };

        assert!(timetable_target_allowed(
            &filter,
            &TimetableResourceTarget {
                owner_organization_unit_ids: Vec::new(),
                actor_is_assigned: true,
            }
        ));
        assert!(timetable_target_allowed(
            &filter,
            &TimetableResourceTarget {
                owner_organization_unit_ids: vec![exact_unit_id],
                actor_is_assigned: false,
            }
        ));
        assert!(timetable_target_allowed(
            &filter,
            &TimetableResourceTarget {
                owner_organization_unit_ids: vec![tree_unit_id],
                actor_is_assigned: false,
            }
        ));
        assert!(!timetable_target_allowed(
            &filter,
            &TimetableResourceTarget {
                owner_organization_unit_ids: vec![Uuid::new_v4()],
                actor_is_assigned: false,
            }
        ));
    }
}
