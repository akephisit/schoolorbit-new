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
    assert!(
        context_settings::update(
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
        .is_err()
    );
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

#[tokio::test]
async fn term_defaults_require_visible_matching_parents_and_hidden_parents_filter_children() {
    let (pool, actor, year) = fixture("context_visible_term_parent").await;
    let bell:Uuid=sqlx::query_scalar("INSERT INTO bell_schedules(academic_year_id,code,name,status) VALUES($1,'CONTEXT','Context bell','published') RETURNING id").bind(year.id).fetch_one(&pool).await.unwrap();
    let term = years_terms::create_term(
        &pool,
        actor.user_id,
        CreateAcademicTermRequest {
            academic_year_id: year.id,
            term_type: AcademicTermType::Regular,
            custom_name: None,
            included_in_year_result: true,
            blocks_year_closure: true,
            bell_schedule_id: bell,
        },
    )
    .await
    .unwrap();
    assert!(!term.is_visible);
    let selected = context_settings::update(
        &pool,
        &actor,
        UpdateAcademicContextSettingsRequest {
            row_version: 1,
            default_academic_year_id: Some(year.id),
            default_academic_term_id: Some(term.id),
            years: vec![AcademicContextVisibilityInput {
                id: year.id,
                is_visible: true,
            }],
            terms: vec![AcademicContextVisibilityInput {
                id: term.id,
                is_visible: true,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        selected.context_options.default_academic_term_id,
        Some(term.id)
    );
    assert_eq!(selected.terms[0].status, AcademicTermStatus::Planning);
    let invalid = context_settings::update(
        &pool,
        &actor,
        UpdateAcademicContextSettingsRequest {
            row_version: 2,
            default_academic_year_id: Some(year.id),
            default_academic_term_id: Some(term.id),
            years: vec![AcademicContextVisibilityInput {
                id: year.id,
                is_visible: false,
            }],
            terms: vec![],
        },
    )
    .await;
    assert!(invalid.is_err());
    let hidden = context_settings::update(
        &pool,
        &actor,
        UpdateAcademicContextSettingsRequest {
            row_version: 2,
            default_academic_year_id: None,
            default_academic_term_id: None,
            years: vec![AcademicContextVisibilityInput {
                id: year.id,
                is_visible: false,
            }],
            terms: vec![],
        },
    )
    .await
    .unwrap();
    assert!(
        !hidden
            .context_options
            .terms
            .iter()
            .any(|row| row.id == term.id)
    );
    assert!(
        years_terms::get_term(&pool, term.id)
            .await
            .unwrap()
            .is_visible,
        "own flag survives hidden parent"
    );
    let other = years_terms::create_year(
        &pool,
        actor.user_id,
        CreateAcademicYearRequest {
            year: 2902,
            custom_name: None,
            school_days: vec!["MON".into()],
        },
    )
    .await
    .unwrap();
    let mismatch = context_settings::update(
        &pool,
        &actor,
        UpdateAcademicContextSettingsRequest {
            row_version: 3,
            default_academic_year_id: Some(other.id),
            default_academic_term_id: Some(term.id),
            years: vec![
                AcademicContextVisibilityInput {
                    id: year.id,
                    is_visible: true,
                },
                AcademicContextVisibilityInput {
                    id: other.id,
                    is_visible: true,
                },
            ],
            terms: vec![],
        },
    )
    .await;
    assert!(mismatch.is_err());
    assert!(
        !years_terms::get_year(&pool, other.id)
            .await
            .unwrap()
            .is_visible
    );
    assert!(sqlx::query("UPDATE academic_context_settings SET default_academic_year_id=$1,default_academic_term_id=$2 WHERE singleton").bind(year.id).bind(term.id).execute(&pool).await.is_err(),"deferred database guard rejects hidden default parent");
    let selected = context_settings::update(
        &pool,
        &actor,
        UpdateAcademicContextSettingsRequest {
            row_version: 3,
            default_academic_year_id: Some(year.id),
            default_academic_term_id: Some(term.id),
            years: vec![AcademicContextVisibilityInput {
                id: year.id,
                is_visible: true,
            }],
            terms: vec![],
        },
    )
    .await
    .unwrap();
    assert!(
        context_settings::update(
            &pool,
            &actor,
            UpdateAcademicContextSettingsRequest {
                row_version: selected.settings.row_version,
                default_academic_year_id: Some(year.id),
                default_academic_term_id: Some(term.id),
                years: vec![],
                terms: vec![AcademicContextVisibilityInput {
                    id: term.id,
                    is_visible: false
                }],
            }
        )
        .await
        .is_err()
    );
    let cleared = context_settings::update(
        &pool,
        &actor,
        UpdateAcademicContextSettingsRequest {
            row_version: selected.settings.row_version,
            default_academic_year_id: Some(year.id),
            default_academic_term_id: None,
            years: vec![],
            terms: vec![AcademicContextVisibilityInput {
                id: term.id,
                is_visible: false,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(cleared.settings.default_academic_year_id, Some(year.id));
    assert_eq!(cleared.settings.default_academic_term_id, None);
}

#[test]
fn year_and_term_commands_reject_every_retired_boundary_key() {
    fn rejects<T: serde::de::DeserializeOwned>(base: serde_json::Value) {
        assert!(serde_json::from_value::<T>(base.clone()).is_ok());
        for key in ["startDate", "endDate", "plannedEndDate", "closedOn"] {
            for value in [serde_json::Value::Null, serde_json::json!("2025-01-01")] {
                let mut input = base.clone();
                input.as_object_mut().unwrap().insert(key.into(), value);
                assert!(
                    serde_json::from_value::<T>(input).is_err(),
                    "retired {key} must fail closed even when null"
                );
            }
        }
    }
    let year = serde_json::json!({"year":2901,"customName":null,"schoolDays":["MON"]});
    rejects::<CreateAcademicYearRequest>(year.clone());
    let mut update = year;
    update["rowVersion"] = serde_json::json!(1);
    rejects::<UpdateAcademicYearRequest>(update);
    let term = serde_json::json!({"termType":"regular","customName":null,"includedInYearResult":true,"blocksYearClosure":true,"bellScheduleId":Uuid::new_v4()});
    let mut create = term.clone();
    create["academicYearId"] = serde_json::json!(Uuid::new_v4());
    rejects::<CreateAcademicTermRequest>(create);
    let mut update = term;
    update["rowVersion"] = serde_json::json!(1);
    rejects::<UpdateAcademicTermRequest>(update);
}
