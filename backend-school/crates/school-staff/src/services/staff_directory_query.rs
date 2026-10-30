use crate::models::*;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

pub(super) const CURRENT_SUBJECT_GROUPS: &str = "SELECT DISTINCT om.user_id,sg.id,sg.name_th AS name FROM organization_members om JOIN organization_units ou ON ou.id=om.organization_unit_id AND ou.is_active AND ou.unit_type='subject_group' JOIN subject_groups sg ON sg.id=ou.subject_group_id AND sg.is_active WHERE om.started_at <= CURRENT_DATE AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)";

/// Bounded private portrait references; authorization matches the Staff profile/Files reader.
pub async fn collaboration_portraits_on_connection(
    connection: &mut sqlx::PgConnection,
    viewer: &school_authorization::ActorContext,
    user_ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, AppError> {
    use school_authorization::resource_access_policy::UserResourceListAccess;
    if user_ids.len() > 128 { return Err(AppError::ValidationError("Too many collaboration portrait targets".into())); }
    if user_ids.is_empty() { return Ok(Vec::new()); }
    let mut result = DbSession::new(connection).query_as::<(Uuid, Uuid)>(
        "staff.services.staff_directory_query.collaboration_portraits.self",
        "SELECT u.id,u.profile_image_file_id FROM users u JOIN files file ON file.id=u.profile_image_file_id AND file.owner_user_id=u.id AND file.purpose_code='profile_image' AND file.lifecycle_status='ready' WHERE u.id=$1 AND u.id=ANY($2) AND u.user_type='staff' AND u.status='active'",
    ).bind(viewer.user_id).bind(user_ids).fetch_all().await?;
    let access = match crate::staff_access_policy::resolve_staff_profile_list_access(viewer) {
        Ok(UserResourceListAccess::School) => StaffListAccess::School,
        Ok(UserResourceListAccess::OrganizationUnit(id)) => StaffListAccess::OrganizationUnit(id),
        Ok(UserResourceListAccess::OrganizationTree(id)) => StaffListAccess::OrganizationTree(id),
        Ok(UserResourceListAccess::Own(_)) | Ok(UserResourceListAccess::Assigned(_)) | Err(AppError::Forbidden(_)) => return Ok(result),
        Err(error) => return Err(error),
    };
    let mut query = QueryBuilder::<Postgres>::new("SELECT u.id,u.profile_image_file_id FROM users u JOIN files file ON file.id=u.profile_image_file_id AND file.owner_user_id=u.id AND file.purpose_code='profile_image' AND file.lifecycle_status='ready' WHERE u.user_type='staff' AND u.status='active' AND u.id=ANY(");
    query.push_bind(user_ids).push(")");
    push_staff_access_filter(&mut query, access);
    result.extend(DbSession::new(connection).prepared_query_as(
        "staff.services.staff_directory_query.collaboration_portraits.scoped", query.build_query_as::<(Uuid, Uuid)>(),
    ).fetch_all().await?);
    result.sort_unstable(); result.dedup();
    Ok(result)
}

pub(super) fn validate_directory_filters(filter: &StaffListFilter) -> Result<(), AppError> {
    let invalid = || AppError::BadRequest("ตัวกรองบุคลากรไม่ถูกต้อง".into());
    for (value, missing) in [
        (&filter.job_position_id, "unspecified"),
        (&filter.subject_group_id, "unassigned"),
    ] {
        if let Some(value) = value {
            if value != missing && Uuid::parse_str(value).is_err() {
                return Err(invalid());
            }
        }
    }
    if let Some(value) = &filter.academic_rank {
        if value != "unspecified" && !StaffAcademicRank::ALL.iter().any(|v| v.as_str() == value) {
            return Err(invalid());
        }
    }
    if let Some(value) = &filter.education_level {
        if value != "unspecified" && !StaffEducationLevel::ALL.iter().any(|v| v.as_str() == value) {
            return Err(invalid());
        }
    }
    if let Some(value) = &filter.status {
        if ![
            "all",
            "active",
            "inactive",
            "suspended",
            "resigned",
            "retired",
        ]
        .contains(&value.as_str())
        {
            return Err(invalid());
        }
    }
    Ok(())
}
pub(super) fn push_personnel_filters(query: &mut QueryBuilder<Postgres>, filter: &StaffListFilter) {
    for (field, value) in [
        ("job_position_id", &filter.job_position_id),
        ("academic_rank", &filter.academic_rank),
    ] {
        if let Some(value) = value {
            if value == "unspecified" {
                query.push(" AND NOT EXISTS (SELECT 1 FROM staff_info info WHERE info.user_id=u.id AND info.").push(field).push(" IS NOT NULL)");
            } else {
                query.push(" AND EXISTS (SELECT 1 FROM staff_info info WHERE info.user_id=u.id AND info.").push(field).push("::text = ").push_bind(value.clone()).push(")");
            }
        }
    }
    if let Some(level) = &filter.education_level {
        query.push(if level == "unspecified" { " AND NOT EXISTS(SELECT 1 FROM staff_highest_education highest WHERE highest.staff_id=u.id" } else { " AND EXISTS(SELECT 1 FROM staff_highest_education highest WHERE highest.staff_id=u.id" });
        if level != "unspecified" {
            query
                .push(" AND highest.education_level=")
                .push_bind(level.clone());
        }
        query.push(")");
    }
    if let Some(group) = &filter.subject_group_id {
        query
            .push(if group == "unassigned" {
                " AND NOT EXISTS (SELECT 1 FROM ("
            } else {
                " AND EXISTS (SELECT 1 FROM ("
            })
            .push(CURRENT_SUBJECT_GROUPS)
            .push(") groups WHERE groups.user_id=u.id");
        if group != "unassigned" {
            query
                .push(" AND groups.id::text = ")
                .push_bind(group.clone());
        }
        query.push(")");
    }
}
pub(super) async fn read_subject_groups(
    connection: &mut sqlx::PgConnection,
    user: Uuid,
) -> Result<Vec<StaffSubjectGroupSummary>, AppError> {
    let mut query = QueryBuilder::<Postgres>::new("SELECT id,name FROM (");
    query
        .push(CURRENT_SUBJECT_GROUPS)
        .push(") groups WHERE user_id=")
        .push_bind(user)
        .push(" ORDER BY name,id");
    Ok(DbSession::new(connection)
        .prepared_query_as(
            "staff.services.staff_directory_query.read_subject_groups",
            query.build_query_as(),
        )
        .fetch_all()
        .await?)
}

pub(super) fn push_staff_access_filter(
    query: &mut QueryBuilder<Postgres>,
    access: StaffListAccess,
) {
    match access {
        StaffListAccess::School => {}
        StaffListAccess::Own(actor_user_id) | StaffListAccess::Assigned(actor_user_id) => {
            query.push(" AND u.id = ").push_bind(actor_user_id);
        }
        StaffListAccess::OrganizationUnit(actor_user_id) => {
            query
                .push(
                    r#" AND EXISTS (
                        SELECT 1
                        FROM organization_members actor_member
                        JOIN organization_units active_actor_unit
                          ON active_actor_unit.id = actor_member.organization_unit_id
                         AND active_actor_unit.is_active = true
                        JOIN organization_members target_member
                          ON target_member.organization_unit_id = actor_member.organization_unit_id
                        WHERE actor_member.user_id = "#,
                )
                .push_bind(actor_user_id)
                .push(
                    r#" AND target_member.user_id = u.id
                        AND actor_member.started_at <= CURRENT_DATE
                        AND (actor_member.ended_at IS NULL OR actor_member.ended_at > CURRENT_DATE)
                        AND target_member.started_at <= CURRENT_DATE
                        AND (target_member.ended_at IS NULL OR target_member.ended_at > CURRENT_DATE)
                    )"#,
                );
        }
        StaffListAccess::OrganizationTree(actor_user_id) => {
            query
                .push(
                    r#" AND EXISTS (
                        WITH RECURSIVE actor_roots AS (
                            SELECT actor_member.organization_unit_id
                            FROM organization_members actor_member
                            JOIN organization_units active_root
                              ON active_root.id = actor_member.organization_unit_id
                             AND active_root.is_active = true
                            WHERE actor_member.user_id = "#,
                )
                .push_bind(actor_user_id)
                .push(
                    r#"
                              AND actor_member.started_at <= CURRENT_DATE
                        AND (actor_member.ended_at IS NULL OR actor_member.ended_at > CURRENT_DATE)
                        ),
                        organization_tree AS (
                            SELECT organization_unit_id
                            FROM actor_roots
                            UNION
                            SELECT child.id
                            FROM organization_units child
                            JOIN organization_tree parent_tree
                              ON child.parent_unit_id = parent_tree.organization_unit_id
                            WHERE child.is_active = true
                        )
                        SELECT 1
                        FROM organization_members target_member
                        WHERE target_member.user_id = u.id
                          AND target_member.organization_unit_id IN (
                              SELECT organization_unit_id FROM organization_tree
                          )
                          AND target_member.started_at <= CURRENT_DATE
                        AND (target_member.ended_at IS NULL OR target_member.ended_at > CURRENT_DATE)
                    )"#,
                );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_filter_validation_rejects_unknown_and_accepts_missing() {
        let mut filter = StaffListFilter::default();
        filter.academic_rank = Some("unspecified".into());
        assert!(validate_directory_filters(&filter).is_ok());
        filter.academic_rank = Some("typo".into());
        assert!(validate_directory_filters(&filter).is_err());
    }
}

/// Current affiliations for an already-authorized, bounded set of users.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StaffSubjectGroupAssignment {
    pub user_id: Uuid,
    pub subject_group_id: Uuid,
    pub name: String,
    pub display_order: Option<i32>,
}

pub async fn subject_groups_for_users(
    pool: &PgPool,
    user_ids: &[Uuid],
) -> Result<Vec<StaffSubjectGroupAssignment>, AppError> {
    if user_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.services.staff_directory_query.subject_groups_for_users",
    )
    .await?;
    subject_groups_for_users_on_connection(&mut connection, user_ids).await
}

pub async fn subject_groups_for_users_on_connection(
    connection: &mut sqlx::PgConnection,
    user_ids: &[Uuid],
) -> Result<Vec<StaffSubjectGroupAssignment>, AppError> {
    if user_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT groups.user_id,groups.id AS subject_group_id,groups.name,sg.display_order FROM (",
    );
    query
        .push(CURRENT_SUBJECT_GROUPS)
        .push(") groups JOIN subject_groups sg ON sg.id=groups.id WHERE groups.user_id=ANY(")
        .push_bind(user_ids)
        .push(") ORDER BY sg.display_order NULLS LAST,groups.name,groups.id");
    Ok(DbSession::new(connection)
        .prepared_query_as(
            "staff.services.staff_directory_query.subject_groups_for_users_on_connection",
            query.build_query_as(),
        )
        .fetch_all()
        .await?)
}

#[cfg(test)]
mod pool_facade_tests {
    use super::*;
    async fn closed_pool() -> sqlx::PgPool {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        pool
    }

    #[tokio::test]
    async fn pool_facade_preflight_empty_staff_affiliations_need_no_database() {
        let pool = closed_pool().await;
        assert!(subject_groups_for_users(&pool, &[])
            .await
            .unwrap()
            .is_empty());
    }
    #[tokio::test]
    async fn pool_facade_preflight_leave_permissions_precede_checkout() {
        let pool = closed_pool().await;
        let actor = school_authorization::ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![],
        };
        assert!(matches!(
            crate::leave::service::routing_options(&pool, &actor).await,
            Err(school_errors::AppError::Forbidden(_))
        ));
        assert!(matches!(
            crate::leave::service::policies(&pool, &actor, Uuid::new_v4()).await,
            Err(school_errors::AppError::Forbidden(_))
        ));
        assert!(matches!(
            crate::leave::service::list_types(&pool, &actor, Default::default()).await,
            Err(school_errors::AppError::Forbidden(_))
        ));
        assert!(matches!(
            crate::leave::service::list(&pool, &actor, Default::default()).await,
            Err(school_errors::AppError::Forbidden(_))
        ));
    }
    #[tokio::test]
    async fn pool_facade_preflight_invalid_leave_page_and_range_precede_checkout() {
        use crate::leave::models::{LeaveListQuery, LeaveUsageQuery};
        let pool = closed_pool().await;
        let actor = school_authorization::ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![
                school_permissions::registry::codes::STAFF_LEAVE_READ_OWN.into(),
                school_permissions::registry::codes::STAFF_LEAVE_DOWNLOAD_OWN.into(),
            ],
        };
        assert!(matches!(
            crate::leave::service::list(
                &pool,
                &actor,
                LeaveListQuery {
                    limit: Some(0),
                    ..Default::default()
                }
            )
            .await,
            Err(school_errors::AppError::ValidationError(_))
        ));
        let from = chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap();
        assert!(matches!(
            crate::leave::service::usage(
                &pool,
                &actor,
                LeaveUsageQuery {
                    from,
                    to: from - chrono::Duration::days(1),
                    kind: None
                }
            )
            .await,
            Err(school_errors::AppError::ValidationError(_))
        ));
    }
    #[tokio::test]
    async fn pool_facade_preflight_invalid_career_page_precedes_checkout() {
        let pool = closed_pool().await;
        let query = crate::career::StaffCareerHistoryQuery {
            cursor: None,
            page_size: Some(0),
        };
        assert!(matches!(
            super::super::staff_career_service::list_staff_career_history(
                &pool,
                Uuid::new_v4(),
                query
            )
            .await,
            Err(school_errors::AppError::BadRequest(_))
        ));
    }
    #[tokio::test]
    async fn pool_facade_preflight_license_resource_denial_precedes_checkout() {
        let pool = closed_pool().await;
        let actor = school_authorization::ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![],
        };
        assert!(matches!(
            super::super::staff_license_service::authorize_upload(&pool, &actor, None).await,
            Err(school_errors::AppError::Forbidden(_))
        ));
    }
    #[tokio::test]
    async fn pool_facade_exhausted_staff_paths_need_no_available_connection() {
        let pool = school_test_db::create_named_test_pool_with_max_connections(
            "staff_facade_exhausted",
            1,
        )
        .await;
        let held = pool.acquire().await.unwrap();
        let actor = school_authorization::ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![],
        };
        tokio::time::timeout(std::time::Duration::from_millis(250), async {
            assert!(subject_groups_for_users(&pool, &[])
                .await
                .unwrap()
                .is_empty());
            assert!(matches!(
                crate::leave::service::routing_options(&pool, &actor).await,
                Err(AppError::Forbidden(_))
            ));
            assert!(matches!(
                crate::leave::service::policies(&pool, &actor, Uuid::new_v4()).await,
                Err(AppError::Forbidden(_))
            ));
            assert!(matches!(
                super::super::staff_license_service::authorize_upload(&pool, &actor, None).await,
                Err(AppError::Forbidden(_))
            ));
            let query = crate::career::StaffCareerHistoryQuery {
                cursor: None,
                page_size: Some(0),
            };
            assert!(matches!(
                super::super::staff_career_service::list_staff_career_history(
                    &pool,
                    Uuid::new_v4(),
                    query
                )
                .await,
                Err(AppError::BadRequest(_))
            ));
        })
        .await
        .expect("pure facade paths must finish while the only connection is held");
        drop(held);
        pool.close().await;
    }
}
