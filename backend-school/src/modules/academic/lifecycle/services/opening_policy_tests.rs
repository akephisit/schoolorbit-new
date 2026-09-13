use super::*;
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::{
        core::services_tests::prepare_core_fixture, cutover_test_support::apply_migrations_through,
        lifecycle::models::*,
    },
    permissions::registry::codes,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str) -> (PgPool, ActorContext) {
    let pool = prepare_core_fixture(name).await;
    apply_migrations_through(&pool, 77).await.unwrap();
    let user_id: Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    (
        pool,
        ActorContext {
            user_id,
            permissions: vec![
                codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into(),
                codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL.into(),
            ],
        },
    )
}

fn changes(row_version: i64) -> UpdateOpeningPolicyInput {
    UpdateOpeningPolicyInput {
        row_version,
        require_homeroom_placements: true,
        require_published_offerings: true,
        require_published_timetable: false,
    }
}

#[tokio::test]
async fn opening_policy_defaults_do_not_require_later_term_work_and_updates_need_exact_permissions()
{
    let (pool, manager) = fixture("opening_policy_defaults").await;
    let reader = ActorContext {
        user_id: manager.user_id,
        permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
    };
    let initial = get_opening_policy(&pool, &reader).await.unwrap();
    assert_eq!(initial.row_version, 1);
    assert!(!initial.require_homeroom_placements);
    assert!(!initial.require_published_offerings);
    assert!(!initial.require_published_timetable);
    for permissions in [
        vec![],
        vec![codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL.into()],
        vec![codes::ACADEMIC_LIFECYCLE_ACTIVATE_SCHOOL.into()],
    ] {
        let actor = ActorContext {
            user_id: manager.user_id,
            permissions,
        };
        assert!(matches!(
            get_opening_policy(&pool, &actor).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            update_opening_policy(&pool, &actor, changes(1)).await,
            Err(AppError::Forbidden(_))
        ));
    }
    assert!(matches!(
        update_opening_policy(&pool, &reader, changes(1)).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        update_opening_policy(&pool, &manager, changes(0)).await,
        Err(AppError::ValidationError(_))
    ));
    let updated = update_opening_policy(&pool, &manager, changes(1))
        .await
        .unwrap();
    assert_eq!(updated.row_version, 2);
    assert!(updated.require_homeroom_placements);
    assert!(updated.require_published_offerings);
    assert!(!updated.require_published_timetable);
    assert_eq!(get_opening_policy(&pool, &reader).await.unwrap(), updated);
    assert!(matches!(
        update_opening_policy(&pool, &manager, changes(1)).await,
        Err(AppError::Conflict(_))
    ));
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE event_code='academic.opening_policy.updated'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 1);
}

#[tokio::test]
async fn opening_policy_audit_failure_rolls_back_and_concurrent_edit_has_one_winner() {
    let (pool, actor) = fixture("opening_policy_atomic").await;
    let before = get_opening_policy(&pool, &actor).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION fail_opening_policy_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'E2E_OPENING_POLICY_AUDIT'; END $$; CREATE TRIGGER fail_opening_policy_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_opening_policy_audit();")
        .execute(&pool).await.unwrap();
    assert!(update_opening_policy(&pool, &actor, changes(1))
        .await
        .is_err());
    assert_eq!(get_opening_policy(&pool, &actor).await.unwrap(), before);
    sqlx::query("DROP TRIGGER fail_opening_policy_audit ON academic_audit_events")
        .execute(&pool)
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        update_opening_policy(&pool, &actor, changes(1)),
        update_opening_policy(&pool, &actor, changes(1))
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(matches!(
        (&first, &second),
        (Ok(_), Err(AppError::Conflict(_))) | (Err(AppError::Conflict(_)), Ok(_))
    ));
    assert_eq!(
        get_opening_policy(&pool, &actor).await.unwrap().row_version,
        2
    );
}
