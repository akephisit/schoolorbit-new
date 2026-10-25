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
    let mut rejected = serde_json::to_value(subject_values(from, grade)).unwrap();
    rejected["effectiveUntil"] = json!("2040-12-31");
    assert!(serde_json::from_value::<CreateSubjectVersionRequest>(rejected.clone()).is_err());
    rejected["rowVersion"] = json!(first.row_version);
    assert!(serde_json::from_value::<UpdateSubjectVersionRequest>(rejected).is_err());
    let mut rejected = serde_json::to_value(activity_values(from, grade)).unwrap();
    rejected["effectiveUntil"] = Value::Null;
    assert!(serde_json::from_value::<CreateActivityVersionRequest>(rejected.clone()).is_err());
    rejected["rowVersion"] = json!(1);
    assert!(serde_json::from_value::<UpdateActivityVersionRequest>(rejected).is_err());
    // The former per-year name constraint conflicted with distinct canonical IDs.
    // Name sharing across distinct stable activities is valid. Start-only revisions
    // of one identity share dates, retaining unique revision numbers.
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
    let revision = catalog::create_activity_version(&pool, a, activity_values(from, grade))
        .await
        .unwrap();
    assert_eq!(revision.version_no, activity.version_no + 1);
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
        "(to_jsonb(v)-ARRAY['start_academic_year_id','effective_until']) || jsonb_build_object('migration_provenance',v.migration_provenance || jsonb_build_object('historicalCatalogContext',jsonb_build_object('migration',138,'startAcademicYearId',v.start_academic_year_id,'effectiveUntil',v.effective_until)))"
    } else {
        "to_jsonb(v)"
    };
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT jsonb_build_object('subjects',(SELECT jsonb_agg({projection} ORDER BY id) FROM subject_versions v),'activities',(SELECT jsonb_agg({projection} ORDER BY id) FROM activity_versions v))"))).fetch_one(pool).await.unwrap()
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
        "learning_offering_curriculum_sources",
    ] {
        let value:Value=sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COALESCE(jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text),'[]'::jsonb) FROM {table} r"))).fetch_one(pool).await.unwrap();
        result.insert(table.into(), value);
    }
    Value::Object(result)
}

async fn native_versions_before_138(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let year = Uuid::new_v4();
    sqlx::query("INSERT INTO academic_years(id,year,name,start_date,end_date,school_days,status) VALUES($1,2563,'Former metadata only','2020-01-01','2020-12-31','MON','planning')").bind(year).execute(pool).await.unwrap();
    let subject = stable_subject(pool, "YEAR-FREE-MIGRATION").await;
    let activity = stable_activity(pool).await;
    let version = Uuid::new_v4();
    let activity_version = Uuid::new_v4();
    // Historical schema137 requires the retired field. Preserve both native
    // published facts and native draft facts, including initially empty provenance.
    sqlx::query("INSERT INTO subject_versions(id,subject_id,version_no,code,name_th,credit,type,start_academic_year_id,effective_from,effective_until,status,published_at) VALUES($1,$2,1,'YEAR-FREE-MIGRATION','Native published',1,'ADDITIONAL',$3,'2020-01-01','2021-01-01','published',now())").bind(version).bind(subject).bind(year).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO activity_versions(id,activity_id,version_no,name,activity_type,periods_per_week,hours_per_week,scheduling_mode,is_active,grade_level_ids,start_academic_year_id,effective_from,effective_until,status) VALUES($1,$2,1,'Native draft','guidance',1,1,'synchronized',true,'[]'::jsonb,$3,'2020-01-01',NULL,'draft')").bind(activity_version).bind(activity).bind(year).execute(pool).await.unwrap();
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
    let predecessor_not_null_dependencies:i64=sqlx::query_scalar("SELECT count(*) FROM pg_constraint guard JOIN pg_depend dependency ON dependency.classid='pg_constraint'::regclass AND dependency.objid=guard.oid JOIN pg_attribute attribute ON attribute.attrelid=dependency.refobjid AND attribute.attnum=dependency.refobjsubid WHERE guard.contype='n' AND dependency.refclassid='pg_class'::regclass AND attribute.attrelid IN('subject_versions'::regclass,'activity_versions'::regclass) AND attribute.attname='start_academic_year_id'").fetch_one(&pool).await.unwrap();
    assert_eq!(
        predecessor_not_null_dependencies, 2,
        "the predecessor's required fields have intrinsic PostgreSQL 18 guards"
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
    let current = catalog::current_subject_revisions(
        &pool,
        &[catalog::get_subject_version(&pool, subject)
            .await
            .unwrap()
            .subject_id],
    )
    .await
    .unwrap();
    assert!(
        current.values().any(|version| version.id == subject),
        "former expiry is historical metadata only"
    );
    assert!(
        serde_json::to_value(catalog::get_subject_version(&pool, subject).await.unwrap())
            .unwrap()
            .get("effectiveUntil")
            .is_none()
    );
    let imported_activity: Uuid =
        sqlx::query_scalar("SELECT id FROM activity_versions WHERE status='published' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(sqlx::query(
        "UPDATE activity_versions SET migration_provenance='{}'::jsonb WHERE id=$1"
    )
    .bind(imported_activity)
    .execute(&pool)
    .await
    .is_err());
    assert!(sqlx::query("DELETE FROM activity_versions WHERE id=$1")
        .bind(imported_activity)
        .execute(&pool)
        .await
        .is_err());
    let audit:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE event_code='catalog.start_only_reconciled' AND payload->>'passed'='true'").fetch_one(&pool).await.unwrap();
    assert_eq!(audit, 1);
    let columns:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name IN('subject_versions','activity_versions') AND column_name IN('start_academic_year_id','effective_until')").fetch_one(&pool).await.unwrap();
    assert_eq!(columns, 0);
}

#[tokio::test]
async fn catalog_year_independence_migration_refuses_dependency_and_retries_atomically() {
    for (index,create,drop) in [
        (0,"CREATE VIEW unexpected_catalog_year_consumer AS SELECT start_academic_year_id FROM subject_versions","DROP VIEW unexpected_catalog_year_consumer RESTRICT"),
        (1,"ALTER TABLE subject_versions ADD CONSTRAINT unexpected_catalog_end_check CHECK (effective_until IS NULL OR effective_until > '1900-01-01')","ALTER TABLE subject_versions DROP CONSTRAINT unexpected_catalog_end_check"),
        (2,"CREATE INDEX unexpected_activity_year_index ON activity_versions(start_academic_year_id)","DROP INDEX unexpected_activity_year_index RESTRICT"),
    ] {
        let pool=prepare_core_fixture_through(&format!("catalog_year_independence_dependency_{index}"),137).await;
        native_versions_before_138(&pool).await;
        let before=versions(&pool,false).await;let references=pins(&pool).await;
        sqlx::query(create).execute(&pool).await.unwrap();
        let before_guards:Value=sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(tgname,tgenabled::text) ORDER BY tgname) FROM pg_trigger WHERE tgrelid IN('subject_versions'::regclass,'activity_versions'::regclass)").fetch_one(&pool).await.unwrap();
        let error=apply_migrations_through(&pool,138).await.unwrap_err();
        assert!(error.to_string().contains("CATALOG_138_DEPENDENCY_UNSUPPORTED"),"{error}");
        assert_eq!(before,versions(&pool,false).await);assert_eq!(references,pins(&pool).await);
        let after_guards:Value=sqlx::query_scalar("SELECT jsonb_agg(jsonb_build_array(tgname,tgenabled::text) ORDER BY tgname) FROM pg_trigger WHERE tgrelid IN('subject_versions'::regclass,'activity_versions'::regclass)").fetch_one(&pool).await.unwrap();
        assert_eq!(before_guards,after_guards);
        let applied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version=138)").fetch_one(&pool).await.unwrap();assert!(!applied);
        sqlx::query(drop).execute(&pool).await.unwrap();
        apply_migrations_through(&pool,138).await.unwrap();
    }
}

#[tokio::test]
async fn catalog_year_independence_migration_refuses_unknown_provenance_and_guard_state() {
    for (index,statement,code) in [
        (0,"UPDATE activity_versions SET migration_provenance='[]'::jsonb WHERE status='draft'","CATALOG_138_PROVENANCE_UNMAPPABLE"),
        (1,"UPDATE activity_versions SET migration_provenance='{\"historicalCatalogContext\":{}}'::jsonb WHERE status='draft'","CATALOG_138_PROVENANCE_UNMAPPABLE"),
        (2,"ALTER TABLE subject_versions DISABLE TRIGGER update_subjects_updated_at","CATALOG_138_GUARD_STATE_INVALID"),
    ] {
        let pool=prepare_core_fixture_through(&format!("catalog_year_independence_refusal_{index}"),137).await;
        native_versions_before_138(&pool).await;
        sqlx::query(statement).execute(&pool).await.unwrap();let before=versions(&pool,false).await;
        let error=apply_migrations_through(&pool,138).await.unwrap_err();assert!(error.to_string().contains(code),"{error}");
        assert_eq!(before,versions(&pool,false).await);
    }
}

#[tokio::test]
async fn catalog_year_independence_activity_revisions_serialize_and_select_start_then_revision() {
    let pool = prepare_current_core_fixture("catalog_start_only_activity_revisions").await;
    let grade: Uuid = sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let stable = stable_activity(&pool).await;
    let day = NaiveDate::from_ymd_opt(2040, 1, 1).unwrap();
    let values = activity_values(day, grade);
    let (left, right) = tokio::join!(
        catalog::create_activity_version(&pool, stable, values.clone()),
        catalog::create_activity_version(&pool, stable, values.clone())
    );
    let mut revisions = vec![left.unwrap(), right.unwrap()];
    revisions.sort_by_key(|v| v.version_no);
    assert_eq!((revisions[0].version_no, revisions[1].version_no), (1, 2));
    for version in &revisions {
        catalog::publish_activity_version(
            &pool,
            version.id,
            PublishVersionRequest {
                row_version: version.row_version,
            },
        )
        .await
        .unwrap();
    }
    let pinned = serde_json::to_value(
        catalog::get_activity_version(&pool, revisions[0].id)
            .await
            .unwrap(),
    )
    .unwrap();
    let mut future = values.clone();
    future.effective_from = day.succ_opt().unwrap();
    let mut future_revisions = Vec::new();
    for _ in 0..2 {
        let draft = catalog::create_activity_version(&pool, stable, future.clone())
            .await
            .unwrap();
        future_revisions.push(
            catalog::publish_activity_version(
                &pool,
                draft.id,
                PublishVersionRequest {
                    row_version: draft.row_version,
                },
            )
            .await
            .unwrap(),
        );
    }
    let scope = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    for (as_of, expected, state) in [
        (
            day.pred_opt().unwrap(),
            revisions[1].id,
            CatalogDisplayState::Upcoming,
        ),
        (day, revisions[1].id, CatalogDisplayState::Current),
        (
            day.succ_opt().unwrap(),
            future_revisions[1].id,
            CatalogDisplayState::Current,
        ),
    ] {
        let overview = catalog::list_activity_overview(&pool, &scope, &scope, as_of)
            .await
            .unwrap();
        let item = overview
            .items
            .iter()
            .find(|item| item.activity.id == stable)
            .unwrap();
        assert_eq!(item.display_state, state);
        assert_eq!(item.display_version.as_ref().unwrap().id, expected);
    }
    let future_only = stable_activity(&pool).await;
    for _ in 0..2 {
        let draft = catalog::create_activity_version(&pool, future_only, future.clone())
            .await
            .unwrap();
        catalog::publish_activity_version(
            &pool,
            draft.id,
            PublishVersionRequest {
                row_version: draft.row_version,
            },
        )
        .await
        .unwrap();
    }
    let overview = catalog::list_activity_overview(&pool, &scope, &scope, day)
        .await
        .unwrap();
    let item = overview
        .items
        .iter()
        .find(|item| item.activity.id == future_only)
        .unwrap();
    assert_eq!(item.display_state, CatalogDisplayState::Upcoming);
    assert_eq!(item.display_version.as_ref().unwrap().version_no, 2);
    assert_eq!(
        pinned,
        serde_json::to_value(
            catalog::get_activity_version(&pool, revisions[0].id)
                .await
                .unwrap()
        )
        .unwrap()
    );
    let draft = catalog::create_activity_version(&pool, stable, values.clone())
        .await
        .unwrap();
    let changed = catalog::update_activity_version(
        &pool,
        draft.id,
        activity_update(values.clone(), draft.row_version),
    )
    .await
    .unwrap();
    assert!(matches!(
        catalog::update_activity_version(
            &pool,
            draft.id,
            activity_update(values, draft.row_version)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(changed.row_version, draft.row_version + 1);
}
