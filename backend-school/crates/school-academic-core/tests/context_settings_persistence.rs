use school_academic_core::{
    models::*,
    services::{context, context_settings, years_terms},
};
use school_authorization::ActorContext;
use school_permissions::registry::codes;
use uuid::Uuid;

async fn fixture(name: &str) -> (sqlx::PgPool, ActorContext, AcademicYear) {
    let pool = school_test_db::create_named_test_pool(name).await;
    school_test_db::run_test_migrations(&pool).await;
    let actor = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type) VALUES($1,$2,'!','Context','Fixture','staff')")
        .bind(actor).bind(actor.to_string()).execute(&pool).await.unwrap();
    let actor = ActorContext {
        user_id: actor,
        permissions: vec![
            codes::ACADEMIC_YEAR_MANAGE_SCHOOL.into(),
            codes::ACADEMIC_TERM_MANAGE_SCHOOL.into(),
        ],
    };
    let year = years_terms::create_year(
        &pool,
        actor.user_id,
        CreateAcademicYearRequest {
            year: 2901,
            custom_name: None,
            school_days: vec!["MON".into()],
        },
    )
    .await
    .unwrap();
    (pool, actor, year)
}

fn command(
    version: i64,
    year: Option<Uuid>,
    changes: Vec<AcademicContextVisibilityInput>,
) -> UpdateAcademicContextSettingsRequest {
    UpdateAcademicContextSettingsRequest {
        row_version: version,
        default_academic_year_id: year,
        default_academic_term_id: None,
        years: changes,
        terms: vec![],
    }
}

#[tokio::test]
async fn visibility_defaults_are_atomic_revision_protected_and_independent_of_lifecycle() {
    let (pool, actor, year) = fixture("context_visibility_defaults").await;
    assert!(!year.is_visible);
    let options = context::list_options(&pool).await.unwrap();
    assert!(!options.years.iter().any(|row| row.id == year.id));
    let hidden_default =
        context_settings::update(&pool, &actor, command(1, Some(year.id), vec![])).await;
    assert!(matches!(
        hidden_default,
        Err(school_errors::AppError::ValidationError(_))
    ));
    let accepted = context_settings::update(
        &pool,
        &actor,
        command(
            1,
            Some(year.id),
            vec![AcademicContextVisibilityInput {
                id: year.id,
                is_visible: true,
            }],
        ),
    )
    .await
    .unwrap();
    assert_eq!(accepted.settings.row_version, 2);
    assert_eq!(accepted.years[0].row_version, year.row_version + 1);
    assert_eq!(accepted.years[0].status, AcademicYearStatus::Planning);
    assert_eq!(
        accepted.context_options.default_academic_year_id,
        Some(year.id)
    );
    assert!(matches!(
        context_settings::update(&pool, &actor, command(1, None, vec![])).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert!(context_settings::update(
        &pool,
        &actor,
        command(
            2,
            Some(year.id),
            vec![AcademicContextVisibilityInput {
                id: year.id,
                is_visible: false
            }]
        )
    )
    .await
    .is_err());
    assert!(
        years_terms::get_year(&pool, year.id)
            .await
            .unwrap()
            .is_visible
    );
    let hidden = context_settings::update(
        &pool,
        &actor,
        command(
            2,
            None,
            vec![AcademicContextVisibilityInput {
                id: year.id,
                is_visible: false,
            }],
        ),
    )
    .await
    .unwrap();
    assert_eq!(hidden.settings.default_academic_year_id, None);
    assert!(!hidden.years[0].is_visible);
    let date_columns:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND ((table_name='academic_years' AND column_name IN('start_date','end_date')) OR(table_name='academic_terms' AND column_name IN('start_date','planned_end_date','closed_on')))").fetch_one(&pool).await.unwrap();
    assert_eq!(date_columns, 0);
}

#[tokio::test]
async fn concurrent_context_commands_accept_one_revision_and_deny_partial_permissions() {
    let (pool, actor, year) = fixture("context_visibility_concurrency").await;
    let missing = ActorContext {
        user_id: actor.user_id,
        permissions: vec![codes::ACADEMIC_YEAR_MANAGE_SCHOOL.into()],
    };
    assert!(matches!(
        context_settings::update(&pool, &missing, command(1, None, vec![])).await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    let a = command(
        1,
        Some(year.id),
        vec![AcademicContextVisibilityInput {
            id: year.id,
            is_visible: true,
        }],
    );
    let b = command(1, None, vec![]);
    let (a, b) = tokio::join!(
        context_settings::update(&pool, &actor, a),
        context_settings::update(&pool, &actor, b)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(
        matches!(a, Err(school_errors::AppError::Conflict(_)))
            || matches!(b, Err(school_errors::AppError::Conflict(_)))
    );
}
