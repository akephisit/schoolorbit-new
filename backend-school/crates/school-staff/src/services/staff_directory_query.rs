use crate::models::*;
use school_errors::AppError;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

pub(super) const CURRENT_SUBJECT_GROUPS: &str = "SELECT DISTINCT om.user_id,sg.id,sg.name_th AS name FROM organization_members om JOIN organization_units ou ON ou.id=om.organization_unit_id AND ou.is_active AND ou.unit_type='subject_group' JOIN subject_groups sg ON sg.id=ou.subject_group_id AND sg.is_active WHERE om.started_at <= CURRENT_DATE AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)";

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
        ("education_level", &filter.education_level),
    ] {
        if let Some(value) = value {
            if value == "unspecified" {
                query.push(" AND NOT EXISTS (SELECT 1 FROM staff_info info WHERE info.user_id=u.id AND info.").push(field).push(" IS NOT NULL)");
            } else {
                query.push(" AND EXISTS (SELECT 1 FROM staff_info info WHERE info.user_id=u.id AND info.").push(field).push("::text = ").push_bind(value.clone()).push(")");
            }
        }
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
    pool: &PgPool,
    user: Uuid,
) -> Result<Vec<StaffSubjectGroupSummary>, AppError> {
    let mut query = QueryBuilder::<Postgres>::new("SELECT id,name FROM (");
    query
        .push(CURRENT_SUBJECT_GROUPS)
        .push(") groups WHERE user_id=")
        .push_bind(user)
        .push(" ORDER BY name,id");
    Ok(query.build_query_as().fetch_all(pool).await?)
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
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT groups.user_id,groups.id AS subject_group_id,groups.name,sg.display_order FROM (",
    );
    query
        .push(CURRENT_SUBJECT_GROUPS)
        .push(") groups JOIN subject_groups sg ON sg.id=groups.id WHERE groups.user_id=ANY(")
        .push_bind(user_ids)
        .push(") ORDER BY sg.display_order NULLS LAST,groups.name,groups.id");
    Ok(query.build_query_as().fetch_all(pool).await?)
}
