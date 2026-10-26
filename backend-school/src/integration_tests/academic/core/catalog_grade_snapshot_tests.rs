use super::*;

fn subject_values(grades: Vec<Uuid>) -> CreateSubjectVersionRequest {
    CreateSubjectVersionRequest {
        name_th: "รายวิชาทุกระดับชั้น".into(),
        name_en: None,
        credit: "1.00".into(),
        hours_per_semester: Some(40),
        subject_type: "BASIC".into(),
        description: None,
        effective_from: catalog::subject_catalog_today(),
        term_code: None,
        periods_per_week: Some(2),
        grade_level_ids: grades,
    }
}

fn activity_values(grades: Vec<Uuid>) -> CreateActivityVersionRequest {
    CreateActivityVersionRequest {
        name: "กิจกรรมทุกระดับชั้น".into(),
        description: None,
        hours_per_week: "1.00".into(),
        hours_per_term: Some("20.00".into()),
        scheduling_mode: "synchronized".into(),
        effective_from: catalog::subject_catalog_today(),
        term_code: None,
        grade_level_ids: grades,
    }
}

async fn identities(pool: &PgPool) -> (Uuid, Uuid) {
    let subject = catalog::create_subject(
        pool,
        CreateCatalogSubjectRequest {
            code: "GRADE-SNAPSHOT".into(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    let activity = catalog::create_activity(
        pool,
        CreateCatalogActivityRequest {
            activity_type: "guidance".into(),
        },
    )
    .await
    .unwrap();
    (subject.id, activity.id)
}

async fn active_grades(pool: &PgPool) -> Vec<Uuid> {
    sqlx::query_scalar("SELECT id FROM grade_levels WHERE is_active IS TRUE ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn add_grade(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO grade_levels(level_type,year,is_active) VALUES('secondary',99,true) RETURNING id")
        .fetch_one(pool).await.unwrap()
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
async fn catalog_grade_snapshot_create_update_and_publication_capture_save_time() {
    let pool = prepare_core_fixture("catalog_grade_snapshot_save").await;
    let (subject, activity) = identities(&pool).await;
    let grades = active_grades(&pool).await;
    let course = catalog::create_subject_version(&pool, subject, subject_values(vec![]))
        .await
        .unwrap();
    let event = catalog::create_activity_version(&pool, activity, activity_values(vec![]))
        .await
        .unwrap();
    assert_eq!(course.grade_level_ids, grades);
    assert_eq!(event.grade_level_ids, grades);
    let added = add_grade(&pool).await;
    let course = catalog::publish_subject_version(
        &pool,
        course.id,
        PublishVersionRequest {
            row_version: course.row_version,
        },
    )
    .await
    .unwrap();
    let event = catalog::publish_activity_version(
        &pool,
        event.id,
        PublishVersionRequest {
            row_version: event.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(course.grade_level_ids, grades);
    assert_eq!(event.grade_level_ids, grades);
    assert!(!course.grade_level_ids.contains(&added));

    let mut next_course = subject_values(vec![grades[0]]);
    next_course.name_th.push_str(" รุ่นใหม่");
    let course = catalog::create_subject_version(&pool, subject, next_course.clone())
        .await
        .unwrap();
    next_course.grade_level_ids.clear();
    let course = catalog::update_subject_version(
        &pool,
        course.id,
        subject_update(next_course, course.row_version),
    )
    .await
    .unwrap();
    let event = catalog::create_activity_version(&pool, activity, activity_values(vec![grades[0]]))
        .await
        .unwrap();
    let event = catalog::update_activity_version(
        &pool,
        event.id,
        activity_update(activity_values(vec![]), event.row_version),
    )
    .await
    .unwrap();
    assert_eq!(course.grade_level_ids, active_grades(&pool).await);
    assert_eq!(event.grade_level_ids, active_grades(&pool).await);
}

#[tokio::test]
async fn catalog_grade_snapshot_explicit_partial_inactive_and_unknown_ids() {
    let pool = prepare_core_fixture("catalog_grade_snapshot_explicit").await;
    let (subject, activity) = identities(&pool).await;
    let grade = active_grades(&pool).await[0];
    sqlx::query("UPDATE grade_levels SET is_active=false WHERE id=$1")
        .bind(grade)
        .execute(&pool)
        .await
        .unwrap();
    let course =
        catalog::create_subject_version(&pool, subject, subject_values(vec![grade, grade]))
            .await
            .unwrap();
    let event =
        catalog::create_activity_version(&pool, activity, activity_values(vec![grade, grade]))
            .await
            .unwrap();
    assert_eq!(course.grade_level_ids, vec![grade]);
    assert_eq!(event.grade_level_ids, vec![grade]);
    let course = catalog::publish_subject_version(
        &pool,
        course.id,
        PublishVersionRequest {
            row_version: course.row_version,
        },
    )
    .await
    .unwrap();
    let event = catalog::publish_activity_version(
        &pool,
        event.id,
        PublishVersionRequest {
            row_version: event.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(course.grade_level_ids, vec![grade]);
    assert_eq!(event.grade_level_ids, vec![grade]);
    assert!(matches!(
        catalog::create_subject_version(&pool, subject, subject_values(vec![Uuid::new_v4()])).await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    assert!(matches!(
        catalog::create_activity_version(&pool, activity, activity_values(vec![Uuid::new_v4()]))
            .await,
        Err(school_errors::AppError::ValidationError(_))
    ));
}

#[tokio::test]
async fn catalog_grade_snapshot_legacy_draft_publish_and_no_active_rollback() {
    let pool = prepare_core_fixture("catalog_grade_snapshot_legacy").await;
    let (subject, activity) = identities(&pool).await;
    let grades = active_grades(&pool).await;
    let course = catalog::create_subject_version(&pool, subject, subject_values(vec![grades[0]]))
        .await
        .unwrap();
    let event = catalog::create_activity_version(&pool, activity, activity_values(vec![grades[0]]))
        .await
        .unwrap();
    sqlx::query("DELETE FROM subject_version_grade_levels WHERE subject_id=$1")
        .bind(course.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE activity_versions SET grade_level_ids=NULL WHERE id=$1")
        .bind(event.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE grade_levels SET is_active=false")
        .execute(&pool)
        .await
        .unwrap();
    assert!(catalog::publish_subject_version(
        &pool,
        course.id,
        PublishVersionRequest {
            row_version: course.row_version
        }
    )
    .await
    .is_err());
    assert!(catalog::publish_activity_version(
        &pool,
        event.id,
        PublishVersionRequest {
            row_version: event.row_version
        }
    )
    .await
    .is_err());
    assert!(
        catalog::create_subject_version(&pool, subject, subject_values(vec![]))
            .await
            .is_err()
    );
    assert!(
        catalog::create_activity_version(&pool, activity, activity_values(vec![]))
            .await
            .is_err()
    );
    assert_eq!(
        catalog::get_subject_version(&pool, course.id)
            .await
            .unwrap()
            .row_version,
        course.row_version
    );
    assert_eq!(
        catalog::get_activity_version(&pool, event.id)
            .await
            .unwrap()
            .row_version,
        event.row_version
    );
    sqlx::query("UPDATE grade_levels SET is_active=true WHERE id=ANY($1)")
        .bind(&grades)
        .execute(&pool)
        .await
        .unwrap();
    let course = catalog::publish_subject_version(
        &pool,
        course.id,
        PublishVersionRequest {
            row_version: course.row_version,
        },
    )
    .await
    .unwrap();
    let event = catalog::publish_activity_version(
        &pool,
        event.id,
        PublishVersionRequest {
            row_version: event.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(course.grade_level_ids, grades);
    assert_eq!(event.grade_level_ids, grades);
}

#[tokio::test]
async fn catalog_grade_snapshot_duplicate_comparison_and_stale_save_are_atomic() {
    let pool = prepare_core_fixture("catalog_grade_snapshot_stale").await;
    let (subject, activity) = identities(&pool).await;
    let grades = active_grades(&pool).await;
    let course = catalog::create_subject_version(&pool, subject, subject_values(vec![]))
        .await
        .unwrap();
    let mut unordered = grades.clone();
    unordered.reverse();
    unordered.push(grades[0]);
    assert!(matches!(
        catalog::create_subject_version(&pool, subject, subject_values(unordered)).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let event = catalog::create_activity_version(&pool, activity, activity_values(vec![]))
        .await
        .unwrap();
    let updated = catalog::update_activity_version(
        &pool,
        event.id,
        activity_update(activity_values(vec![grades[0]]), event.row_version),
    )
    .await
    .unwrap();
    assert!(catalog::update_activity_version(
        &pool,
        event.id,
        activity_update(activity_values(vec![]), event.row_version)
    )
    .await
    .is_err());
    assert_eq!(
        catalog::get_activity_version(&pool, event.id)
            .await
            .unwrap()
            .grade_level_ids,
        vec![grades[0]]
    );
    assert_eq!(updated.row_version, event.row_version + 1);
    assert_eq!(
        catalog::get_subject_version(&pool, course.id)
            .await
            .unwrap()
            .grade_level_ids,
        grades
    );
}

#[tokio::test]
async fn catalog_grade_snapshot_waits_for_grade_deactivation_before_saving() {
    let pool = prepare_core_fixture("catalog_grade_snapshot_registry_race").await;
    let (subject, _) = identities(&pool).await;
    let removed = active_grades(&pool).await[0];
    let mut registry = pool.begin().await.unwrap();
    let registry_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *registry)
        .await
        .unwrap();
    sqlx::query("UPDATE grade_levels SET is_active=false WHERE id=$1")
        .bind(removed)
        .execute(&mut *registry)
        .await
        .unwrap();
    let save_pool = pool.clone();
    let save = tokio::spawn(async move {
        catalog::create_subject_version(&save_pool, subject, subject_values(vec![])).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(registry_pid)
            .fetch_one(&pool)
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("save must reach the locked grade row before registry commit");
    registry.commit().await.unwrap();
    let saved = save.await.unwrap().unwrap();
    assert!(!saved.grade_level_ids.contains(&removed));
    assert_eq!(saved.grade_level_ids, active_grades(&pool).await);
}
