use school_administration::settings::{
    models::{SchoolAppearance, UpdateSchoolAppearanceRequest},
    services::appearance,
};
use school_errors::AppError;
use school_test_db::create_named_test_pool_with_max_connections;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const MIGRATION: &str = include_str!("../../../migrations/127_school_appearance.sql");

async fn predecessor(name: &str) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, 4).await;
    sqlx::raw_sql(
        "CREATE TABLE school_settings (id uuid PRIMARY KEY, logo_path text, \
         updated_at timestamptz NOT NULL DEFAULT now(), logo_file_id uuid)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn fixture(name: &str) -> (PgPool, Uuid) {
    let pool = predecessor(name).await;
    let logo = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO school_settings(id, logo_path, logo_file_id) VALUES($1, 'legacy-logo', $2)",
    )
    .bind(Uuid::new_v4())
    .bind(logo)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();
    (pool, logo)
}

fn request(color: &str, revision: i64) -> UpdateSchoolAppearanceRequest {
    serde_json::from_value(json!({
        "appearance":{"palette":"purple","font":"sarabun","density":"compact","primary":color},
        "expectedRevision":revision
    }))
    .unwrap()
}

#[tokio::test]
async fn appearance_migration_preserves_logo_defaults_and_persistence_is_tenant_local() {
    let (a, logo) = fixture("appearance_tenant_a").await;
    let (b, _) = fixture("appearance_tenant_b").await;
    let initial = appearance::get(&a).await.unwrap();
    assert_eq!(initial.appearance, SchoolAppearance::default());
    assert_eq!(initial.revision, 0);
    let saved = appearance::update(&a, request("#A1B2C3", 0)).await.unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.appearance.primary.as_deref(), Some("#a1b2c3"));
    assert_eq!(appearance::get(&a).await.unwrap(), saved);
    assert_eq!(appearance::get(&b).await.unwrap(), initial);
    let unchanged: (Option<Uuid>, Option<String>) =
        sqlx::query_as("SELECT logo_file_id, logo_path FROM school_settings")
            .fetch_one(&a)
            .await
            .unwrap();
    assert_eq!(unchanged, (Some(logo), Some("legacy-logo".into())));
    let before = appearance::get(&a).await.unwrap();
    assert!(matches!(
        appearance::update(&a, request("#000000", 0)).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(appearance::get(&a).await.unwrap(), before);
    assert!(appearance::update(&a, request("#123456", -1))
        .await
        .is_err());
    assert_eq!(appearance::get(&a).await.unwrap(), before);
    let reset = appearance::update(
        &a,
        UpdateSchoolAppearanceRequest {
            appearance: SchoolAppearance::default(),
            expected_revision: before.revision,
        },
    )
    .await
    .unwrap();
    assert_eq!(reset.revision, 2);
    assert_eq!(reset.appearance, SchoolAppearance::default());
    assert_eq!(appearance::get(&a).await.unwrap(), reset);
}

#[tokio::test]
async fn appearance_concurrent_admins_only_one_revision_wins() {
    let (pool, _) = fixture("appearance_concurrent").await;
    let (a, b) = tokio::join!(
        appearance::update(&pool, request("#111111", 0)),
        appearance::update(&pool, request("#222222", 0)),
    );
    let (winner, loser) = match (a, b) {
        (Ok(winner), Err(loser)) | (Err(loser), Ok(winner)) => (winner, loser),
        result => panic!("exactly one admin must win: {result:?}"),
    };
    assert!(matches!(loser, AppError::Conflict(_)));
    assert_eq!(winner.revision, 1);
    assert_eq!(appearance::get(&pool).await.unwrap(), winner);
}

#[tokio::test]
async fn appearance_database_rejects_invalid_configuration_and_duplicate_singletons() {
    let (pool, _) = fixture("appearance_constraints").await;
    for invalid in [
        json!(null),
        json!([]),
        json!("css"),
        json!({}),
        json!({"palette":"invalid","font":"kanit","density":"comfortable","primary":null}),
        json!({"palette":1,"font":"kanit","density":"comfortable","primary":null}),
        json!({"palette":"teal","font":"invalid","density":"comfortable","primary":null}),
        json!({"palette":"teal","font":"kanit","density":"invalid","primary":null}),
        json!({"palette":"teal","font":"kanit","density":"comfortable","primary":"red"}),
        json!({"palette":"teal","font":"kanit","density":"comfortable","primary":"#ABCDEF"}),
        json!({"palette":"teal","font":"kanit","density":"comfortable","primary":1}),
        json!({"palette":"teal","font":"kanit","density":"comfortable"}),
        json!({"palette":"teal","font":"kanit","density":"comfortable","primary":null,"mode":"dark"}),
    ] {
        let result = sqlx::query("UPDATE school_settings SET appearance=$1")
            .bind(invalid.clone())
            .execute(&pool)
            .await;
        assert!(result.is_err(), "must reject {invalid}");
        assert_eq!(appearance::get(&pool).await.unwrap().revision, 0);
    }
    assert!(
        sqlx::query("UPDATE school_settings SET appearance_revision=-1")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("INSERT INTO school_settings(id) VALUES($1)")
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .is_err());
    assert_eq!(
        appearance::get(&pool).await.unwrap().appearance,
        SchoolAppearance::default()
    );
}

#[tokio::test]
async fn appearance_missing_singleton_fails_without_inventing_a_saved_response() {
    let pool = predecessor("appearance_missing").await;
    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();
    assert!(matches!(
        appearance::get(&pool).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        appearance::update(&pool, request("#123456", 0)).await,
        Err(AppError::NotFound(_))
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM school_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn appearance_migration_refuses_duplicate_historical_settings_without_data_loss() {
    let pool = predecessor("appearance_duplicate_predecessor").await;
    for _ in 0..2 {
        sqlx::query("INSERT INTO school_settings(id,logo_file_id) VALUES($1,$2)")
            .bind(Uuid::new_v4())
            .bind(Uuid::new_v4())
            .execute(&pool)
            .await
            .unwrap();
    }
    let before: Vec<(Uuid, Option<Uuid>)> =
        sqlx::query_as("SELECT id,logo_file_id FROM school_settings ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(sqlx::raw_sql(MIGRATION).execute(&mut *tx).await.is_err());
    tx.rollback().await.unwrap();
    let after: Vec<(Uuid, Option<Uuid>)> =
        sqlx::query_as("SELECT id,logo_file_id FROM school_settings ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
}
