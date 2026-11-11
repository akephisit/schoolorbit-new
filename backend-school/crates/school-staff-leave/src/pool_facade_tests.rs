use school_errors::AppError;
use uuid::Uuid;

async fn closed_pool() -> sqlx::PgPool {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    pool
}

#[tokio::test]
async fn pool_facade_preflight_leave_permissions_precede_checkout() {
    let pool = closed_pool().await;
    let actor = school_authorization::ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![],
    };
    assert!(matches!(
        crate::service::routing_options(&pool, &actor).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    assert!(matches!(
        crate::service::policies(&pool, &actor, Uuid::new_v4()).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    assert!(matches!(
        crate::service::list_types(&pool, &actor, Default::default()).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    assert!(matches!(
        crate::service::list(&pool, &actor, Default::default()).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
}
#[tokio::test]
async fn pool_facade_preflight_invalid_leave_page_and_range_precede_checkout() {
    use crate::models::{LeaveListQuery, LeaveUsageQuery};
    let pool = closed_pool().await;
    let actor = school_authorization::ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![
            school_permissions::registry::codes::STAFF_LEAVE_READ_OWN.into(),
            school_permissions::registry::codes::STAFF_LEAVE_DOWNLOAD_OWN.into(),
        ],
    };
    assert!(matches!(
        crate::service::list(
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
        crate::service::usage(
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
async fn pool_facade_exhausted_leave_paths_need_no_available_connection() {
    let pool =
        school_test_db::create_named_test_pool_with_max_connections("leave_facade_exhausted", 1)
            .await;
    let held = pool.acquire().await.unwrap();
    let actor = school_authorization::ActorContext {
        user_id: Uuid::new_v4(),
        permissions: vec![],
    };
    tokio::time::timeout(std::time::Duration::from_millis(250), async {
        assert!(matches!(
            crate::service::routing_options(&pool, &actor).await,
            Err(AppError::Forbidden(_))
        ));
        assert!(matches!(
            crate::service::policies(&pool, &actor, Uuid::new_v4()).await,
            Err(AppError::Forbidden(_))
        ));
    })
    .await
    .expect("Leave preflight must finish while the only connection is held");
    drop(held);
    pool.close().await;
}
