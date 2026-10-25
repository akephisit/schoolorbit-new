use super::*;
use school_errors::AppError;
use serde_json::Value;

fn subject_values(from: NaiveDate, grade: Uuid) -> CreateSubjectVersionRequest {
    CreateSubjectVersionRequest {
        name_th: "รายวิชาส่วนกลาง".into(),
        name_en: None,
        credit: "1.00".into(),
        hours_per_semester: Some(40),
        subject_type: "ADDITIONAL".into(),
        description: None,
        effective_from: from,
        effective_until: None,
        term_code: None,
        periods_per_week: Some(2),
        grade_level_ids: vec![grade],
    }
}

fn activity_values(from: NaiveDate, grade: Uuid) -> CreateActivityVersionRequest {
    CreateActivityVersionRequest {
        name: "กิจกรรมส่วนกลาง".into(),
        description: None,
        hours_per_week: "1.00".into(),
        hours_per_term: Some("20.00".into()),
        scheduling_mode: "synchronized".into(),
        effective_from: from,
        effective_until: None,
        term_code: None,
        grade_level_ids: vec![grade],
    }
}

async fn stable_subject(pool: &PgPool, code: &str) -> Uuid {
    catalog::create_subject(
        pool,
        CreateCatalogSubjectRequest {
            code: code.into(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap()
    .id
}

async fn stable_activity(pool: &PgPool) -> Uuid {
    catalog::create_activity(
        pool,
        CreateCatalogActivityRequest {
            activity_type: "guidance".into(),
        },
    )
    .await
    .unwrap()
    .id
}

fn subject_update(
    values: CreateSubjectVersionRequest,
    revision: i64,
) -> UpdateSubjectVersionRequest {
    let mut value = serde_json::to_value(values).unwrap();
    value["rowVersion"] = json!(revision);
    serde_json::from_value(value).unwrap()
}

fn activity_update(
    values: CreateActivityVersionRequest,
    revision: i64,
) -> UpdateActivityVersionRequest {
    let mut value = serde_json::to_value(values).unwrap();
    value["rowVersion"] = json!(revision);
    serde_json::from_value(value).unwrap()
}

#[tokio::test]
async fn catalog_year_independence_create_update_publish_without_any_academic_year() {
    let pool = school_test_db::create_named_test_pool("catalog_year_independence_empty").await;
    apply_migrations_through(&pool, 138).await.unwrap();
    let years: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_years")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(years, 0);
    let grade: Uuid = sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let from = NaiveDate::from_ymd_opt(2040, 1, 1).unwrap();
    let subject = stable_subject(&pool, "YEAR-FREE-EMPTY").await;
    let draft = catalog::create_subject_version(&pool, subject, subject_values(from, grade))
        .await
        .unwrap();
    assert!(!draft.migrated);
    let mut values = subject_values(from, grade);
    values.effective_from = from + chrono::Duration::days(1);
    let draft =
        catalog::update_subject_version(&pool, draft.id, subject_update(values, draft.row_version))
            .await
            .unwrap();
    let published = catalog::publish_subject_version(
        &pool,
        draft.id,
        PublishVersionRequest {
            row_version: draft.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(published.effective_from, from + chrono::Duration::days(1));
    let activity = stable_activity(&pool).await;
    let draft = catalog::create_activity_version(&pool, activity, activity_values(from, grade))
        .await
        .unwrap();
    assert!(!draft.migrated);
    let mut values = activity_values(from, grade);
    values.effective_from = from + chrono::Duration::days(1);
    let draft = catalog::update_activity_version(
        &pool,
        draft.id,
        activity_update(values, draft.row_version),
    )
    .await
    .unwrap();
    let published = catalog::publish_activity_version(
        &pool,
        draft.id,
        PublishVersionRequest {
            row_version: draft.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(published.effective_from, from + chrono::Duration::days(1));
    assert_eq!(
        0,
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM academic_years")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn catalog_year_independence_outside_and_overlapping_years_keep_domain_guards() {
    let pool = prepare_current_core_fixture("catalog_year_independence_dates").await;
    let grade: Uuid = sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let from = NaiveDate::from_ymd_opt(2040, 1, 1).unwrap();
    let subject = stable_subject(&pool, "YEAR-FREE-DATES").await;
    let first = catalog::create_subject_version(&pool, subject, subject_values(from, grade))
        .await
        .unwrap();
    let mut values = subject_values(from, grade);
    values.name_th = "ฉบับแก้ไขวันเดียวกัน".into();
    assert!(
        catalog::create_subject_version(&pool, subject, values.clone())
            .await
            .is_ok()
    );
    assert!(matches!(
        catalog::create_subject_version(&pool, subject, values).await,
        Err(AppError::Conflict(_))
    ));
    let mut invalid = subject_values(from, grade);
    invalid.effective_until = Some(from);
    assert!(matches!(
        catalog::update_subject_version(
            &pool,
            first.id,
            subject_update(invalid, first.row_version)
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    // The former per-year name constraint conflicted with distinct canonical IDs.
    // Name sharing across different stable activities is valid; their own ranges cannot overlap.
    let a = stable_activity(&pool).await;
    let b = stable_activity(&pool).await;
    let activity = catalog::create_activity_version(&pool, a, activity_values(from, grade))
        .await
        .unwrap();
    assert!(
        catalog::create_activity_version(&pool, b, activity_values(from, grade))
            .await
            .is_ok()
    );
    assert!(
        catalog::create_activity_version(&pool, a, activity_values(from, grade))
            .await
            .is_err()
    );
    let published = catalog::publish_activity_version(
        &pool,
        activity.id,
        PublishVersionRequest {
            row_version: activity.row_version,
        },
    )
    .await
    .unwrap();
    assert!(catalog::update_activity_version(
        &pool,
        published.id,
        activity_update(activity_values(from, grade), published.row_version)
    )
    .await
    .is_err());
    for (index, year) in [2599, 2600].into_iter().enumerate() {
        sqlx::query("INSERT INTO academic_years(id,year,name,start_date,end_date,school_days,status) VALUES($1,$2,'Overlapping fixture','2040-01-01','2040-12-31','MON','planning')")
            .bind(Uuid::new_v4()).bind(year).execute(&pool).await.unwrap();
        let stable = stable_subject(&pool, &format!("YEAR-FREE-OVERLAP-{index}")).await;
        assert!(
            catalog::create_subject_version(&pool, stable, subject_values(from, grade))
                .await
                .is_ok()
        );
        let stable = stable_activity(&pool).await;
        assert!(
            catalog::create_activity_version(&pool, stable, activity_values(from, grade))
                .await
                .is_ok()
        );
    }
}

async fn versions(pool: &PgPool, normalized: bool) -> Value {
    let projection = if normalized {
        "(to_jsonb(v)-'start_academic_year_id') || jsonb_build_object('migration_provenance',v.migration_provenance || jsonb_build_object('historicalYearBinding',jsonb_build_object('migration',138,'startAcademicYearId',v.start_academic_year_id)))"
    } else {
        "to_jsonb(v)"
    };
    sqlx::query_scalar(&format!("SELECT jsonb_build_object('subjects',(SELECT jsonb_agg({projection} ORDER BY id) FROM subject_versions v),'activities',(SELECT jsonb_agg({projection} ORDER BY id) FROM activity_versions v))")).fetch_one(pool).await.unwrap()
}

async fn pins(pool: &PgPool) -> Value {
    let mut result = serde_json::Map::new();
    for table in [
        "subjects",
        "activities",
        "subject_version_grade_levels",
        "subject_default_instructors",
        "activity_default_instructors",
        "curriculum_course_requirements",
        "curriculum_activity_requirements",
        "curriculum_publication_courses",
        "curriculum_publication_activities",
        "curriculum_requirement_sources",
        "course_offering_details",
        "activity_offering_details",
        "course_assessment_plans",
        "academic_delivery_versions",
        "learning_delivery_apply_runs",
        "academic_term_change_sets",
        "academic_timetable_versions",
        "offering_curriculum_sources",
    ] {
        let value:Value=sqlx::query_scalar(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text),'[]'::jsonb) FROM {table} r")).fetch_one(pool).await.unwrap();
        result.insert(table.into(), value);
    }
    Value::Object(result)
}

async fn native_versions_before_138(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let year = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_years(id,year,name,start_date,end_date,school_days,status) VALUES($1,2598,'Former metadata only','2039-01-01','2039-12-31','MON','planning')").bind(year).execute(pool).await.unwrap();
    let subject = stable_subject(pool, "YEAR-FREE-MIGRATION").await;
    let activity = stable_activity(pool).await;
    let version = Uuid::new_v4();
    let activity_version = Uuid::new_v4();
    // Historical schema137 requires the retired field. Preserve both native
    // published facts and native draft facts, including initially empty provenance.
    sqlx::query("INSERT INTO subject_versions(id,subject_id,version_no,code,name_th,credit,type,start_academic_year_id,effective_from,status,published_at) VALUES($1,$2,1,'YEAR-FREE-MIGRATION','Native published',1,'ADDITIONAL',$3,'2039-01-01','published',now())").bind(version).bind(subject).bind(year).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO activity_versions(id,activity_id,version_no,name,activity_type,periods_per_week,hours_per_week,scheduling_mode,is_active,grade_level_ids,start_academic_year_id,effective_from,status) VALUES($1,$2,1,'Native draft','guidance',1,1,'synchronized',true,'[]'::jsonb,$3,'2039-01-01','draft')").bind(activity_version).bind(activity).bind(year).execute(pool).await.unwrap();
    (version, activity_version, year)
}

#[tokio::test]
async fn catalog_year_independence_migration_preserves_all_facts_pins_and_migrated_meaning() {
    let pool = prepare_core_fixture_through("catalog_year_independence_preserve", 137).await;
    let (subject, activity, year) = native_versions_before_138(&pool).await;
    let imported: Uuid = sqlx::query_scalar(
        "SELECT id FROM subject_versions WHERE migration_provenance ? 'migration' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        !catalog::get_subject_version(&pool, subject)
            .await
            .unwrap()
            .migrated
    );
    assert!(
        !catalog::get_activity_version(&pool, activity)
            .await
            .unwrap()
            .migrated
    );
    assert!(
        catalog::get_subject_version(&pool, imported)
            .await
            .unwrap()
            .migrated
    );
    let expected = versions(&pool, true).await;
    let references = pins(&pool).await;
    apply_migrations_through(&pool, 138).await.unwrap();
    assert_eq!(expected, versions(&pool, false).await);
    assert_eq!(references, pins(&pool).await);
    assert!(
        !catalog::get_subject_version(&pool, subject)
            .await
            .unwrap()
            .migrated
    );
    assert!(
        !catalog::get_activity_version(&pool, activity)
            .await
            .unwrap()
            .migrated
    );
    assert!(
        catalog::get_subject_version(&pool, imported)
            .await
            .unwrap()
            .migrated
    );
    apply_migrations_through(&pool, 138).await.unwrap();
    assert_eq!(expected, versions(&pool, false).await);
    assert!(
        sqlx::query("UPDATE subject_versions SET name_th='Forbidden' WHERE id=$1")
            .bind(subject)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("DELETE FROM subject_versions WHERE id=$1")
        .bind(subject)
        .execute(&pool)
        .await
        .is_err());
    assert!(sqlx::query(
        "UPDATE subject_versions SET migration_provenance='{}'::jsonb WHERE id=$1"
    )
    .bind(subject)
    .execute(&pool)
    .await
    .is_err());
    sqlx::query("DELETE FROM academic_years WHERE id=$1")
        .bind(year)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(expected, versions(&pool, false).await);
    let audit:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE event_code='catalog.year_binding_removed.reconciled' AND payload->>'passed'='true'").fetch_one(&pool).await.unwrap();
    assert_eq!(audit, 1);
    let columns:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name IN('subject_versions','activity_versions') AND column_name='start_academic_year_id'").fetch_one(&pool).await.unwrap();
    assert_eq!(columns, 0);
}

#[tokio::test]
async fn catalog_year_independence_migration_refuses_dependency_and_retries_atomically() {
    let pool = prepare_core_fixture_through("catalog_year_independence_dependency", 137).await;
    native_versions_before_138(&pool).await;
    let before = versions(&pool, false).await;
    let references = pins(&pool).await;
    sqlx::query("CREATE VIEW unexpected_catalog_year_consumer AS SELECT start_academic_year_id FROM subject_versions").execute(&pool).await.unwrap();
    assert!(apply_migrations_through(&pool, 138).await.is_err());
    assert_eq!(before, versions(&pool, false).await);
    assert_eq!(references, pins(&pool).await);
    let applied: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version=138)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!applied);
    sqlx::query("DROP VIEW unexpected_catalog_year_consumer RESTRICT")
        .execute(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 138).await.unwrap();
}

#[tokio::test]
async fn catalog_year_independence_migration_refuses_unknown_provenance_and_guard_state() {
    for (index,statement,code) in [
        (0,"UPDATE activity_versions SET migration_provenance='[]'::jsonb WHERE status='draft'","CATALOG_138_PROVENANCE_UNMAPPABLE"),
        (1,"UPDATE activity_versions SET migration_provenance='{\"historicalYearBinding\":{}}'::jsonb WHERE status='draft'","CATALOG_138_PROVENANCE_UNMAPPABLE"),
        (2,"ALTER TABLE subject_versions DISABLE TRIGGER update_subjects_updated_at","CATALOG_138_GUARD_STATE_INVALID"),
    ] {
        let pool=prepare_core_fixture_through(&format!("catalog_year_independence_refusal_{index}"),137).await;
        native_versions_before_138(&pool).await;
        sqlx::query(statement).execute(&pool).await.unwrap();let before=versions(&pool,false).await;
        let error=apply_migrations_through(&pool,138).await.unwrap_err();assert!(error.to_string().contains(code),"{error}");
        assert_eq!(before,versions(&pool,false).await);
    }
}
