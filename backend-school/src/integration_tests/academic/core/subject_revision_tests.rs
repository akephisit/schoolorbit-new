use super::*;
use school_academic_core::models::SubjectVersion;
use serde_json::Value;

fn content(version: &SubjectVersion) -> CreateSubjectVersionRequest {
    CreateSubjectVersionRequest {
        name_th: version.name_th.clone(),
        name_en: version.name_en.clone(),
        credit: version.credit.clone(),
        hours_per_semester: version.hours_per_semester,
        subject_type: version.subject_type.clone(),
        description: version.description.clone(),
        effective_from: version.effective_from,
        effective_until: version.effective_until,
        term_code: version.term_code.clone(),
        periods_per_week: version.periods_per_week,
        grade_level_ids: version.grade_level_ids.clone(),
    }
}

fn update(values: CreateSubjectVersionRequest, row_version: i64) -> UpdateSubjectVersionRequest {
    let mut json = serde_json::to_value(values).unwrap();
    json["rowVersion"] = json!(row_version);
    serde_json::from_value(json).unwrap()
}

async fn publish(pool: &PgPool, version: SubjectVersion) -> SubjectVersion {
    catalog::publish_subject_version(
        pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap()
}

async fn fixture(name: &str) -> PgPool {
    prepare_core_fixture_through(name, 131).await
}

async fn pinned_snapshot(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('offerings',(SELECT jsonb_agg(to_jsonb(d) ORDER BY learning_offering_id) FROM course_offering_details d),'requirements',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r),'publications',(SELECT jsonb_agg(to_jsonb(c) ORDER BY id) FROM curriculum_publication_courses c))")
        .fetch_one(pool).await.unwrap()
}

async fn catalog_snapshot(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('subjects',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM subjects s),'versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM subject_versions v),'grades',(SELECT jsonb_agg(to_jsonb(g) ORDER BY subject_id,grade_level_id) FROM subject_version_grade_levels g),'activities',(SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM activity_versions a))")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn subject_revision_migration_preserves_history_and_pinned_references() {
    let pool = prepare_core_fixture_through("subject_revision_migration", 130).await;
    for index in 0..27 {
        catalog::create_subject(
            &pool,
            CreateCatalogSubjectRequest {
                code: format!("UNRELATED-{index}"),
                subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
            },
        )
        .await
        .unwrap();
    }
    let before = catalog_snapshot(&pool).await;
    let pins = pinned_snapshot(&pool).await;
    apply_migrations_through(&pool, 131).await.unwrap();
    assert_eq!(before, catalog_snapshot(&pool).await);
    assert_eq!(pins, pinned_snapshot(&pool).await);
    let old_id:Uuid = sqlx::query_scalar("SELECT d.subject_version_id FROM course_offering_details d JOIN curriculum_course_requirements r ON r.subject_version_id=d.subject_version_id LIMIT 1").fetch_one(&pool).await.unwrap();
    let old = catalog::get_subject_version(&pool, old_id).await.unwrap();
    let old_json = serde_json::to_value(&old).unwrap();
    let mut values = content(&old);
    let grades:Vec<Uuid> = sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type='secondary' AND year BETWEEN 1 AND 3 ORDER BY year").fetch_all(&pool).await.unwrap();
    assert_eq!(grades.len(), 3);
    values.grade_level_ids = grades.clone();
    assert_ne!(old.grade_level_ids, grades);
    let revised = catalog::create_subject_version(&pool, old.subject_id, values)
        .await
        .unwrap();
    assert!(revised.version_no > old.version_no);
    assert_eq!(revised.name_th, old.name_th);
    assert_eq!(revised.effective_from, old.effective_from);
    let revised = publish(&pool, revised).await;
    assert_eq!(revised.grade_level_ids.len(), 3);
    assert_eq!(
        old_json,
        serde_json::to_value(catalog::get_subject_version(&pool, old.id).await.unwrap()).unwrap()
    );
    assert_eq!(pins, pinned_snapshot(&pool).await);
    let unrelated: i64 =
        sqlx::query_scalar("SELECT count(*) FROM subjects WHERE code LIKE 'UNRELATED-%'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unrelated, 27);
    assert!(
        sqlx::query("UPDATE subject_versions SET name_th='changed' WHERE id=$1")
            .bind(old.id)
            .execute(&pool)
            .await
            .unwrap_err()
            .to_string()
            .contains("PUBLISHED_VERSION_IMMUTABLE")
    );
    assert!(sqlx::query("DELETE FROM subject_versions WHERE id=$1")
        .bind(old.id)
        .execute(&pool)
        .await
        .unwrap_err()
        .to_string()
        .contains("PUBLISHED_VERSION_IMMUTABLE"));
    let activity_overlap = sqlx::query("INSERT INTO activity_versions SELECT (jsonb_populate_record(NULL::activity_versions,to_jsonb(v)||jsonb_build_object('id',gen_random_uuid(),'version_no',version_no+10,'status','draft'))).* FROM activity_versions v ORDER BY id LIMIT 1").execute(&pool).await.unwrap_err();
    assert!(
        activity_overlap
            .to_string()
            .contains("ACTIVITY_VERSION_RANGE_OVERLAP"),
        "{activity_overlap}"
    );
    assert!(sqlx::query("INSERT INTO subject_versions SELECT (jsonb_populate_record(NULL::subject_versions,to_jsonb(v)||jsonb_build_object('id',gen_random_uuid(),'version_no',version_no+100,'status','draft','effective_until',effective_from))).* FROM subject_versions v ORDER BY id LIMIT 1").execute(&pool).await.unwrap_err().to_string().contains("subject_versions_effective_range_check"));
}

async fn new_subject(pool: &PgPool, code: &str) -> (Uuid, CreateSubjectVersionRequest) {
    let subject = catalog::create_subject(
        pool,
        CreateCatalogSubjectRequest {
            code: code.into(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    let grades=sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type='secondary' AND year BETWEEN 1 AND 3 ORDER BY year").fetch_all(pool).await.unwrap();
    (
        subject.id,
        CreateSubjectVersionRequest {
            name_th: "รายวิชาเดิม".into(),
            name_en: Some("Subject".into()),
            credit: "1".into(),
            hours_per_semester: Some(40),
            subject_type: "ADDITIONAL".into(),
            description: None,
            effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            effective_until: None,
            term_code: None,
            periods_per_week: Some(2),
            grade_level_ids: grades,
        },
    )
}

#[tokio::test]
async fn subject_revision_duplicates_normalize_grades_and_decimal_and_allow_restoration() {
    let pool = fixture("subject_revision_duplicates").await;
    let (id, a) = new_subject(&pool, "REV-DUP").await;
    let first = publish(
        &pool,
        catalog::create_subject_version(&pool, id, a.clone())
            .await
            .unwrap(),
    )
    .await;
    let mut same = a.clone();
    same.credit = "1.00".into();
    same.grade_level_ids.reverse();
    same.grade_level_ids.push(same.grade_level_ids[0]);
    assert!(matches!(
        catalog::create_subject_version(&pool, id, same).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let mut b = a.clone();
    b.description = Some("แก้ไข".into());
    let second = catalog::create_subject_version(&pool, id, b.clone())
        .await
        .unwrap();
    assert!(matches!(
        catalog::create_subject_version(&pool, id, b.clone()).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert!(matches!(
        catalog::update_subject_version(&pool, second.id, update(a.clone(), second.row_version))
            .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    publish(&pool, second).await;
    let restored = publish(
        &pool,
        catalog::create_subject_version(&pool, id, a.clone())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(restored.version_no, first.version_no + 2);
    let mut changed = a.clone();
    changed.name_th = "ชื่อใหม่".into();
    let draft = catalog::create_subject_version(&pool, id, changed)
        .await
        .unwrap();
    // Another legacy draft with duplicate content must not be publishable.
    let legacy:Uuid=sqlx::query_scalar("INSERT INTO subject_versions SELECT (jsonb_populate_record(NULL::subject_versions,to_jsonb(v)||jsonb_build_object('id',gen_random_uuid(),'version_no',version_no+10))).* FROM subject_versions v WHERE id=$1 RETURNING id").bind(draft.id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO subject_version_grade_levels SELECT $1,grade_level_id FROM subject_version_grade_levels WHERE subject_id=$2").bind(legacy).bind(draft.id).execute(&pool).await.unwrap();
    assert!(matches!(
        catalog::publish_subject_version(
            &pool,
            draft.id,
            PublishVersionRequest {
                row_version: draft.row_version
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn subject_revision_concurrent_creates_and_updates_keep_one_semantic_draft() {
    let pool = fixture("subject_revision_concurrency").await;
    let (id, a) = new_subject(&pool, "REV-RACE").await;
    let (left, right) = tokio::join!(
        catalog::create_subject_version(&pool, id, a.clone()),
        catalog::create_subject_version(&pool, id, a.clone())
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let original = left.or(right).unwrap();
    let mut b = a.clone();
    b.name_th = "ชื่อสอง".into();
    let other = catalog::create_subject_version(&pool, id, b).await.unwrap();
    let mut c = a.clone();
    c.name_th = "ชื่อร่วม".into();
    let (left, right) = tokio::join!(
        catalog::update_subject_version(
            &pool,
            original.id,
            update(c.clone(), original.row_version)
        ),
        catalog::update_subject_version(&pool, other.id, update(c, other.row_version))
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let successful = left.or(right).unwrap();
    let (left, right) = tokio::join!(
        catalog::publish_subject_version(
            &pool,
            successful.id,
            PublishVersionRequest {
                row_version: successful.row_version
            }
        ),
        catalog::publish_subject_version(
            &pool,
            successful.id,
            PublishVersionRequest {
                row_version: successful.row_version
            }
        )
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let (left, right) = tokio::join!(
        catalog::publish_subject_version(
            &pool,
            original.id,
            PublishVersionRequest {
                row_version: original.row_version
            }
        ),
        catalog::update_subject_version(&pool, original.id, update(a, original.row_version))
    );
    assert!(left.is_err() || right.is_err());
}

#[tokio::test]
async fn subject_revision_current_future_and_exclusive_end_selection() {
    let pool = fixture("subject_revision_selection").await;
    let (id, mut a) = new_subject(&pool, "REV-SELECT").await;
    a.effective_until = Some(NaiveDate::from_ymd_opt(2026, 8, 1).unwrap());
    let first = publish(
        &pool,
        catalog::create_subject_version(&pool, id, a.clone())
            .await
            .unwrap(),
    )
    .await;
    a.description = Some("รุ่นล่าสุด".into());
    let second = publish(
        &pool,
        catalog::create_subject_version(&pool, id, a.clone())
            .await
            .unwrap(),
    )
    .await;
    a.effective_from = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    a.effective_until = None;
    publish(
        &pool,
        catalog::create_subject_version(&pool, id, a.clone())
            .await
            .unwrap(),
    )
    .await;
    a.description = Some("รุ่นอนาคตล่าสุด".into());
    let future = publish(
        &pool,
        catalog::create_subject_version(&pool, id, a.clone())
            .await
            .unwrap(),
    )
    .await;
    let scope = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    for (date, expected, state) in [
        (
            NaiveDate::from_ymd_opt(2026, 7, 31).unwrap(),
            second.id,
            CatalogDisplayState::Current,
        ),
        (
            NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            future.id,
            CatalogDisplayState::Upcoming,
        ),
        (
            NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            future.id,
            CatalogDisplayState::Current,
        ),
    ] {
        let overview = catalog::list_subject_overview(&pool, &scope, &scope, date)
            .await
            .unwrap();
        let item = overview
            .items
            .iter()
            .find(|item| item.subject.id == id)
            .unwrap();
        assert_eq!(item.display_state, state);
        assert_eq!(item.display_version.as_ref().unwrap().id, expected);
    }
    assert_ne!(first.id, second.id);
}

#[tokio::test]
async fn subject_revision_each_meaningful_field_can_create_a_revision() {
    let pool = fixture("subject_revision_fields").await;
    let (id, a) = new_subject(&pool, "REV-FIELDS").await;
    publish(
        &pool,
        catalog::create_subject_version(&pool, id, a.clone())
            .await
            .unwrap(),
    )
    .await;
    let mut variants = Vec::new();
    for (field, value) in [
        ("nameTh", json!("ชื่อใหม่")),
        ("nameEn", json!("New name")),
        ("credit", json!("1.25")),
        ("hoursPerSemester", json!(60)),
        ("subjectType", json!("BASIC")),
        ("description", json!("คำอธิบาย")),
        ("effectiveFrom", json!("2026-06-01")),
        ("effectiveUntil", json!("2026-12-01")),
        ("termCode", json!("2")),
        ("periodsPerWeek", json!(3)),
        ("gradeLevelIds", json!([a.grade_level_ids[0]])),
    ] {
        let mut value_object = serde_json::to_value(&a).unwrap();
        value_object[field] = value;
        variants.push(serde_json::from_value::<CreateSubjectVersionRequest>(value_object).unwrap());
    }
    for value in variants {
        catalog::create_subject_version(&pool, id, value)
            .await
            .unwrap();
    }
}
