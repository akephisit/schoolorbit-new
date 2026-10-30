use school_authorization::ActorContext;
use school_authorization::{
    self as resource_access_policy, AcademicResourceAccess, AcademicResourceListFilter,
    AcademicResourcePermissions,
};
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;

pub fn can_manage_school(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL)
}
pub fn can_manage_group(actor: &ActorContext, assigned: bool) -> bool {
    can_manage_school(actor)
        || (assigned && actor.has_permission(codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED))
}
pub fn can_manage_configuration(actor: &ActorContext, coordinator: bool) -> bool {
    can_manage_school(actor)
        || (coordinator && actor.has_permission(codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED))
}
pub fn can_confirm(actor: &ActorContext, primary: bool) -> bool {
    primary && can_manage_group(actor, true)
}
pub fn can_lock(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL)
}
pub fn can_correct(actor: &ActorContext) -> bool {
    actor.has_permission(codes::ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL)
}
pub fn can_read_group(
    filter: &AcademicResourceListFilter,
    owner: Option<Uuid>,
    assigned: bool,
    subject_coordinator: bool,
) -> bool {
    resource_access_policy::academic_resource_access_for(
        filter,
        owner,
        assigned || subject_coordinator,
    ) != AcademicResourceAccess::None
}
pub async fn list_access(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<AcademicResourceListFilter, AppError> {
    let filter = resource_access_policy::resolve_academic_resource_list_filter(
        pool,
        actor,
        read_permissions(),
    )
    .await?;
    require_read_filter(filter)
}

pub async fn list_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
) -> Result<AcademicResourceListFilter, AppError> {
    let filter = resource_access_policy::resolve_academic_resource_list_filter_on_connection(
        connection,
        actor,
        read_permissions(),
    )
    .await?;
    require_read_filter(filter)
}

fn read_permissions() -> AcademicResourcePermissions {
    AcademicResourcePermissions {
        assigned: &[
            codes::ACADEMIC_LEARNER_EVALUATION_READ_ASSIGNED,
            codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED,
        ],
        organization_unit: &[codes::ACADEMIC_LEARNER_EVALUATION_READ_ORGANIZATION_UNIT],
        organization_tree: &[],
        school: &[
            codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL,
            codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL,
            codes::ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL,
            codes::ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL,
        ],
    }
}
fn require_read_filter(
    filter: AcademicResourceListFilter,
) -> Result<AcademicResourceListFilter, AppError> {
    if !filter.includes_school_owned
        && filter.assigned_actor_id.is_none()
        && filter.organization_unit_ids.is_empty()
    {
        return Err(AppError::Forbidden(
            "Learner evaluation access denied".into(),
        ));
    }
    Ok(filter)
}

/// A whole-term summary includes every subject. Require access to every contributing
/// group instead of leaking other teachers' evaluations through an assigned subject.
pub async fn require_student_summary_access(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
    student: Uuid,
) -> Result<(), AppError> {
    let preflight = resource_access_policy::academic_resource_list_filter_without_database(
        actor,
        read_permissions(),
    )
    .map(require_read_filter)
    .transpose()?;
    if preflight
        .as_ref()
        .is_some_and(|access| access.includes_school_owned)
    {
        return Ok(());
    }
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_assessment.policy.learner_evaluation.require_student_summary_access",
    )
    .await?;
    let access = match preflight {
        Some(access) => access,
        None => list_access_on_connection(&mut connection, actor).await?,
    };
    require_student_summary_with_access(&mut connection, actor, year, term, student, access).await
}

pub async fn require_student_summary_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
    student: Uuid,
) -> Result<(), AppError> {
    let access = list_access_on_connection(connection, actor).await?;
    require_student_summary_with_access(connection, actor, year, term, student, access).await
}

async fn require_student_summary_with_access(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
    student: Uuid,
    access: AcademicResourceListFilter,
) -> Result<(), AppError> {
    if access.includes_school_owned {
        return Ok(());
    }
    let scopes:Vec<(Option<Uuid>,bool,bool)>=DbSession::new(connection).query_as("academic_assessment.policy.learner_evaluation.require_student_summary_access_on_connection", r#"WITH relevant AS (
        SELECT m.learning_group_id FROM learning_group_students m WHERE m.student_academic_year_id=$1 AND m.academic_term_id=$2 AND m.academic_year_id=$3 AND m.membership_status='active'
        UNION SELECT learning_group_id FROM subject_term_student_evaluations WHERE student_academic_year_id=$1 AND academic_term_id=$2 AND academic_year_id=$3)
        SELECT o.owning_organization_unit_id,EXISTS(SELECT 1 FROM learning_group_teachers teacher WHERE teacher.learning_group_id=g.id AND teacher.teacher_id=$4 AND teacher.starts_on<=(now() AT TIME ZONE 'Asia/Bangkok')::date AND (teacher.ends_on IS NULL OR teacher.ends_on>=(now() AT TIME ZONE 'Asia/Bangkok')::date)),
        EXISTS(SELECT 1 FROM course_assessment_plans p JOIN course_offering_details coordinated ON coordinated.learning_offering_id=p.learning_offering_id WHERE coordinated.subject_id=d.subject_id AND p.academic_term_id=g.academic_term_id AND p.academic_year_id=g.academic_year_id AND p.assessment_coordinator_id=$4)
        FROM relevant r JOIN learning_groups g ON g.id=r.learning_group_id JOIN learning_offerings o ON o.id=g.learning_offering_id JOIN course_offering_details d ON d.learning_offering_id=o.id JOIN academic_terms t ON t.id=g.academic_term_id"#).bind(student).bind(term).bind(year).bind(actor.user_id).fetch_all().await?;
    if scopes.is_empty()
        || scopes.iter().any(|(owner, assigned, coordinator)| {
            !can_read_group(&access, *owner, *assigned, *coordinator)
        })
    {
        return Err(AppError::Forbidden(
            "Access to all contributing subjects is required for a term summary".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_unit_and_assigned_access_are_unioned() {
        let unit = Uuid::new_v4();
        let filter = AcademicResourceListFilter {
            assigned_actor_id: Some(Uuid::new_v4()),
            organization_unit_ids: vec![unit],
            ..Default::default()
        };
        assert!(can_read_group(&filter, Some(unit), false, false));
        assert!(can_read_group(&filter, None, true, false));
        assert!(can_read_group(&filter, None, false, true));
        assert!(!can_read_group(&filter, Some(Uuid::new_v4()), false, false));
        let unit_only = AcademicResourceListFilter {
            assigned_actor_id: None,
            ..filter
        };
        assert!(!can_read_group(&unit_only, None, false, true));
    }
    #[test]
    fn management_does_not_replace_current_primary_or_lock_permission() {
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL.into()],
        };
        assert!(can_manage_group(&actor, false));
        assert!(!can_confirm(&actor, false));
        assert!(!can_lock(&actor));
        let teacher = ActorContext {
            permissions: vec![codes::ACADEMIC_LEARNER_EVALUATION_MANAGE_ASSIGNED.into()],
            ..actor
        };
        assert!(!can_manage_configuration(&teacher, false));
        assert!(can_manage_configuration(&teacher, true));
        assert!(can_confirm(&teacher, true));
    }
    #[tokio::test]
    async fn list_access_preserves_database_free_school_grants_and_denials() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(50))
            .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
            .unwrap();
        let granted = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL.into()],
        };
        let filter = list_access(&pool, &granted).await.unwrap();
        assert!(filter.includes_school_owned);
        require_student_summary_access(
            &pool,
            &granted,
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        let denied = ActorContext {
            user_id: granted.user_id,
            permissions: vec![],
        };
        assert!(matches!(
            list_access(&pool, &denied).await,
            Err(AppError::Forbidden(_))
        ));
    }
}
