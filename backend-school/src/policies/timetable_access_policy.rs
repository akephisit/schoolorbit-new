use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::{
    self, AcademicResourceListFilter, AcademicResourcePermissions,
};

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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TimetableAccessFilter {
    pub assigned_actor_id: Option<Uuid>,
    pub organization_unit_ids: Vec<Uuid>,
    pub organization_tree_unit_ids: Vec<Uuid>,
    pub includes_school_owned: bool,
}

impl TimetableAccessFilter {
    pub fn as_academic_resource_filter(&self) -> AcademicResourceListFilter {
        AcademicResourceListFilter {
            assigned_actor_id: self.assigned_actor_id,
            organization_unit_ids: self.organization_unit_ids.clone(),
            organization_tree_unit_ids: self.organization_tree_unit_ids.clone(),
            includes_school_owned: self.includes_school_owned,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TimetableResourceTarget {
    owner_organization_unit_ids: Vec<Uuid>,
    actor_is_assigned: bool,
}

#[derive(sqlx::FromRow)]
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
    if action == TimetableAction::Publish {
        actor.require_permission(codes::ACADEMIC_TIMETABLE_PUBLISH_SCHOOL)?;
        return Ok(TimetableAccessFilter {
            includes_school_owned: true,
            ..TimetableAccessFilter::default()
        });
    }

    let resolved = resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        timetable_permissions(action),
    )
    .await?;
    Ok(TimetableAccessFilter {
        assigned_actor_id: resolved.assigned_actor_id,
        organization_unit_ids: resolved.organization_unit_ids,
        organization_tree_unit_ids: resolved.organization_tree_unit_ids,
        includes_school_owned: resolved.includes_school_owned,
    })
}

pub async fn require_timetable_list_access(
    pool: &PgPool,
    actor: &ActorContext,
    action: TimetableAction,
) -> Result<TimetableAccessFilter, AppError> {
    let filter = timetable_list_access(pool, actor, action).await?;
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
    let filter = require_timetable_list_access(pool, actor, action).await?;
    if resources.requires_school_scope && !filter.includes_school_owned {
        return Err(AppError::Forbidden(
            "รายการตารางสอนนี้ต้องใช้สิทธิ์ระดับโรงเรียน".to_string(),
        ));
    }
    if filter.includes_school_owned {
        return Ok(());
    }

    let version_ids = normalized_ids(&resources.timetable_version_ids);
    let offering_ids = normalized_ids(&resources.learning_offering_ids);
    let group_ids = normalized_ids(&resources.learning_group_ids);
    let homeroom_ids = normalized_ids(&resources.homeroom_ids);
    let teacher_ids = normalized_ids(&resources.teacher_ids);
    let room_ids = normalized_ids(&resources.room_ids);
    let expected_count = version_ids.len()
        + offering_ids.len()
        + group_ids.len()
        + homeroom_ids.len()
        + teacher_ids.len()
        + room_ids.len();
    if expected_count == 0 {
        return Ok(());
    }

    let targets = sqlx::query_as::<_, TimetableResourceTargetRow>(
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
    .fetch_all(pool)
    .await?;

    if targets.len() != expected_count {
        return Err(AppError::NotFound(
            "ไม่พบทรัพยากรตารางสอนบางรายการ".to_string(),
        ));
    }
    if targets.iter().any(|target| {
        let _identity = (&target.resource_kind, target.resource_id);
        !target.permission_neutral
            && !timetable_target_allowed(
                &filter,
                &TimetableResourceTarget {
                    owner_organization_unit_ids: target.owner_organization_unit_ids.clone(),
                    actor_is_assigned: target.actor_is_assigned,
                },
            )
    }) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์เข้าถึงทรัพยากรตารางสอนบางรายการ".to_string(),
        ));
    }
    Ok(())
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

fn timetable_permissions(action: TimetableAction) -> AcademicResourcePermissions {
    match action {
        TimetableAction::Read => AcademicResourcePermissions {
            assigned: READ_ASSIGNED_PERMISSIONS,
            organization_unit: READ_UNIT_PERMISSIONS,
            organization_tree: READ_TREE_PERMISSIONS,
            school: READ_SCHOOL_PERMISSIONS,
        },
        TimetableAction::Manage => AcademicResourcePermissions {
            assigned: MANAGE_ASSIGNED_PERMISSIONS,
            organization_unit: MANAGE_UNIT_PERMISSIONS,
            organization_tree: MANAGE_TREE_PERMISSIONS,
            school: MANAGE_SCHOOL_PERMISSIONS,
        },
        TimetableAction::Publish => unreachable!("publish is checked before scope resolution"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::permission::ActorContext;
    use crate::permissions::registry::codes;
    use crate::test_helpers::create_named_test_pool;
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
