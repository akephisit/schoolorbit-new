use super::*;
use school_academic_core::models::{CatalogActivity, CatalogSubject, DeleteCatalogRequest};
use school_errors::AppError;
use std::time::Duration;

fn revision(row_version: i64) -> DeleteCatalogRequest {
    DeleteCatalogRequest { row_version }
}

async fn subject(pool: &PgPool, code: &str) -> CatalogSubject {
    catalog::create_subject(
        pool,
        CreateCatalogSubjectRequest {
            code: code.into(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap()
}

async fn activity(pool: &PgPool) -> CatalogActivity {
    catalog::create_activity(
        pool,
        CreateCatalogActivityRequest {
            activity_type: "guidance".into(),
        },
    )
    .await
    .unwrap()
}

async fn activity_values(pool: &PgPool) -> CreateActivityVersionRequest {
    let grade: Uuid = sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY id LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap();
    CreateActivityVersionRequest {
        name: format!("กิจกรรมในโครงสร้างโดยไม่มีชั่วโมงสอน {}", Uuid::now_v7()),
        description: None,
        hours_per_week: "0.00".into(),
        hours_per_term: Some("0.00".into()),
        scheduling_mode: "independent".into(),
        effective_from: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        term_code: None,
        grade_level_ids: vec![grade],
    }
}

#[tokio::test]
async fn catalog_deletion_empty_and_draft_rows_preserves_unrelated_history() {
    let pool = prepare_current_core_fixture("catalog_deletion_empty_draft").await;
    let before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM subject_versions WHERE status='published'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let empty_subject = subject(&pool, "DELETE-EMPTY").await;
    let empty_activity = activity(&pool).await;
    let scope = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let today = catalog::subject_catalog_today();
    let subjects = catalog::list_subject_overview(&pool, &scope, &scope, today)
        .await
        .unwrap();
    let item = subjects
        .items
        .iter()
        .find(|item| item.subject.id == empty_subject.id)
        .unwrap();
    assert!(item.can_delete);
    assert!(item.display_version.is_none());
    let read_only = catalog::list_subject_overview(&pool, &scope, &Default::default(), today)
        .await
        .unwrap();
    assert!(read_only.items.iter().all(|item| !item.can_delete));
    let activities = catalog::list_activity_overview(&pool, &scope, &scope, today)
        .await
        .unwrap();
    let item = activities
        .items
        .iter()
        .find(|item| item.activity.id == empty_activity.id)
        .unwrap();
    assert!(item.can_delete);
    assert!(item.display_version.is_none());
    let read_only = catalog::list_activity_overview(&pool, &scope, &Default::default(), today)
        .await
        .unwrap();
    assert!(read_only.items.iter().all(|item| !item.can_delete));
    catalog::delete_subject(&pool, empty_subject.id, revision(empty_subject.row_version))
        .await
        .unwrap();
    catalog::delete_activity(
        &pool,
        empty_activity.id,
        revision(empty_activity.row_version),
    )
    .await
    .unwrap();
    assert!(matches!(
        catalog::delete_subject(&pool, empty_subject.id, revision(1)).await,
        Err(AppError::NotFound(_))
    ));

    let draft_subject = subject(&pool, "DELETE-DRAFT").await;
    let grade: Uuid = sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    create_overview_subject_version(
        &pool,
        draft_subject.id,
        grade,
        "ฉบับร่างที่จะลบ",
        NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
        false,)
    .await;
    let version = catalog::list_subject_versions(&pool, draft_subject.id)
        .await
        .unwrap()
        .remove(0);
    let draft_activity = activity(&pool).await;
    let activity_version =
        catalog::create_activity_version(&pool, draft_activity.id, activity_values(&pool).await)
            .await
            .unwrap();
    let teacher = fixture_actor(&pool).await;
    sqlx::query("INSERT INTO subject_default_instructors(subject_id,instructor_id,role) VALUES($1,$2,'primary')")
        .bind(draft_subject.id).bind(teacher).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO activity_default_instructors(activity_id,instructor_id,role) VALUES($1,$2,'primary')")
        .bind(draft_activity.id).bind(teacher).execute(&pool).await.unwrap();
    catalog::delete_subject(&pool, draft_subject.id, revision(draft_subject.row_version))
        .await
        .unwrap();
    catalog::delete_activity(
        &pool,
        draft_activity.id,
        revision(draft_activity.row_version),
    )
    .await
    .unwrap();
    assert!(catalog::get_subject_version(&pool, version.id)
        .await
        .is_err());
    assert!(catalog::get_activity_version(&pool, activity_version.id)
        .await
        .is_err());
    let owned: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM subject_version_grade_levels WHERE subject_id=$1)+(SELECT count(*) FROM subject_default_instructors WHERE subject_id=$2)+(SELECT count(*) FROM activity_default_instructors WHERE activity_id=$3)")
        .bind(version.id).bind(draft_subject.id).bind(draft_activity.id).fetch_one(&pool).await.unwrap();
    assert_eq!(owned, 0);
    let after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM subject_versions WHERE status='published'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn catalog_deletion_refuses_stale_archived_published_and_referenced_rows() {
    let pool = prepare_current_core_fixture("catalog_deletion_refusal").await;
    let stale = subject(&pool, "DELETE-STALE").await;
    assert!(matches!(
        catalog::delete_subject(&pool, stale.id, revision(stale.row_version + 1)).await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        catalog::delete_subject(&pool, stale.id, revision(0)).await,
        Err(AppError::ValidationError(_))
    ));
    let archived = activity(&pool).await;
    sqlx::query("UPDATE activities SET archived_at=now() WHERE id=$1")
        .bind(archived.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        catalog::delete_activity(&pool, archived.id, revision(archived.row_version)).await,
        Err(AppError::Conflict(_))
    ));
    let published_subject: (Uuid, i64) = sqlx::query_as("SELECT s.id,s.row_version FROM subjects s JOIN subject_versions v ON v.subject_id=s.id WHERE v.status='published' ORDER BY s.id LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    assert!(matches!(
        catalog::delete_subject(&pool, published_subject.0, revision(published_subject.1)).await,
        Err(AppError::Conflict(_))
    ));
    let published_activity: (Uuid, i64) = sqlx::query_as("SELECT a.id,a.row_version FROM activities a JOIN activity_versions v ON v.activity_id=a.id WHERE v.status='published' ORDER BY a.id LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    assert!(matches!(
        catalog::delete_activity(&pool, published_activity.0, revision(published_activity.1)).await,
        Err(AppError::Conflict(_))
    ));
    // A stable reference must refuse even when the subject has no versions.
    sqlx::query("INSERT INTO subject_term_evaluation_configurations(subject_id,academic_term_id,academic_year_id,domain) SELECT $1,id,academic_year_id,'desirable_characteristic' FROM academic_terms ORDER BY id LIMIT 1")
        .bind(stale.id).execute(&pool).await.unwrap();
    assert!(matches!(
        catalog::delete_subject(&pool, stale.id, revision(stale.row_version)).await,
        Err(AppError::Conflict(_))
    ));
    // Source ledger rows have no catalog FK and still protect a referenced draft activity.
    let referenced = activity(&pool).await;
    let version =
        catalog::create_activity_version(&pool, referenced.id, activity_values(&pool).await)
            .await
            .unwrap();
    sqlx::query("INSERT INTO curriculum_requirement_sources SELECT gen_random_uuid(),'activity',curriculum_level_id,study_program_id,$1,grade_level_id,term_slot_id FROM curriculum_requirement_sources ORDER BY id LIMIT 1")
        .bind(version.id).execute(&pool).await.unwrap();
    assert!(matches!(
        catalog::delete_activity(&pool, referenced.id, revision(referenced.row_version)).await,
        Err(AppError::Conflict(_))
    ));
    assert!(catalog::get_activity_version(&pool, version.id)
        .await
        .is_ok());
    let historical = activity(&pool).await;
    let history =
        catalog::create_activity_version(&pool, historical.id, activity_values(&pool).await)
            .await
            .unwrap();
    sqlx::query("UPDATE activity_versions SET status='archived' WHERE id=$1")
        .bind(history.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        catalog::delete_activity(&pool, historical.id, revision(historical.row_version)).await,
        Err(AppError::Conflict(_))
    ));
    let timestamped = activity(&pool).await;
    let history =
        catalog::create_activity_version(&pool, timestamped.id, activity_values(&pool).await)
            .await
            .unwrap();
    sqlx::query("UPDATE activity_versions SET published_at=now() WHERE id=$1")
        .bind(history.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        catalog::delete_activity(&pool, timestamped.id, revision(timestamped.row_version)).await,
        Err(AppError::Conflict(_))
    ));
    let scope = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let subjects =
        catalog::list_subject_overview(&pool, &scope, &scope, catalog::subject_catalog_today())
            .await
            .unwrap();
    assert!(
        !subjects
            .items
            .iter()
            .find(|item| item.subject.id == stale.id)
            .unwrap()
            .can_delete
    );
    let activities =
        catalog::list_activity_overview(&pool, &scope, &scope, catalog::subject_catalog_today())
            .await
            .unwrap();
    for id in [
        archived.id,
        published_activity.0,
        referenced.id,
        historical.id,
        timestamped.id,
    ] {
        assert!(
            !activities
                .items
                .iter()
                .find(|item| item.activity.id == id)
                .unwrap()
                .can_delete
        );
    }
}

#[tokio::test]
async fn catalog_deletion_serializes_with_activity_publication() {
    let pool = prepare_current_core_fixture("catalog_deletion_publish_race").await;
    let mut values = activity_values(&pool).await;
    values.hours_per_week = "1.00".into();
    values.hours_per_term = Some("20.00".into());
    let stable = activity(&pool).await;
    let draft = catalog::create_activity_version(&pool, stable.id, values.clone())
        .await
        .unwrap();
    let mut gate = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM activities WHERE id=$1 FOR UPDATE")
        .bind(stable.id)
        .execute(&mut *gate)
        .await
        .unwrap();
    let publish_pool = pool.clone();
    let mut publish = tokio::spawn(async move {
        catalog::publish_activity_version(
            &publish_pool,
            draft.id,
            PublishVersionRequest {
                row_version: draft.row_version,
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut publish)
            .await
            .is_err()
    );
    let delete_pool = pool.clone();
    let mut delete = tokio::spawn(async move {
        catalog::delete_activity(&delete_pool, stable.id, revision(stable.row_version)).await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut delete)
            .await
            .is_err()
    );
    gate.commit().await.unwrap();
    let published = publish.await.unwrap().unwrap();
    assert_eq!(published.status, VersionStatus::Published);
    assert!(matches!(delete.await.unwrap(), Err(AppError::Conflict(_))));
    assert!(catalog::get_activity_version(&pool, published.id)
        .await
        .is_ok());
    // Deletion wins the parent lock before publication can take a child-row lock.
    // The old child-first UPDATE/trigger path could deadlock in this ordering.
    values.name.push_str(" delete-first");
    let stable = activity(&pool).await;
    let draft = catalog::create_activity_version(&pool, stable.id, values.clone())
        .await
        .unwrap();
    let mut gate = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM activities WHERE id=$1 FOR UPDATE")
        .bind(stable.id)
        .execute(&mut *gate)
        .await
        .unwrap();
    let delete_pool = pool.clone();
    let mut delete = tokio::spawn(async move {
        catalog::delete_activity(&delete_pool, stable.id, revision(stable.row_version)).await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut delete)
            .await
            .is_err()
    );
    let publish_pool = pool.clone();
    let mut publish = tokio::spawn(async move {
        catalog::publish_activity_version(
            &publish_pool,
            draft.id,
            PublishVersionRequest {
                row_version: draft.row_version,
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut publish)
            .await
            .is_err()
    );
    gate.commit().await.unwrap();
    delete.await.unwrap().unwrap();
    assert!(matches!(publish.await.unwrap(), Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn catalog_deletion_serializes_with_activity_creation_and_allocation() {
    let pool = prepare_current_core_fixture("catalog_deletion_create_race").await;
    let activity = activity(&pool).await;
    let values = activity_values(&pool).await;
    let mut gate = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM activities WHERE id=$1 FOR UPDATE")
        .bind(activity.id)
        .execute(&mut *gate)
        .await
        .unwrap();
    let delete_pool = pool.clone();
    let mut delete = tokio::spawn(async move {
        catalog::delete_activity(&delete_pool, activity.id, revision(activity.row_version)).await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut delete)
            .await
            .is_err()
    );
    let create_pool = pool.clone();
    let mut create = tokio::spawn(async move {
        catalog::create_activity_version(&create_pool, activity.id, values).await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut create)
            .await
            .is_err()
    );
    gate.commit().await.unwrap();
    delete.await.unwrap().unwrap();
    assert!(matches!(create.await.unwrap(), Err(AppError::NotFound(_))));
    let orphans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM activity_versions WHERE activity_id=$1")
            .bind(activity.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(orphans, 0);
}

#[tokio::test]
async fn catalog_zero_hour_activity_publishes_and_survives_curriculum_publication() {
    let pool = prepare_current_core_fixture("catalog_zero_hour_curriculum").await;
    let activity = activity(&pool).await;
    let values = activity_values(&pool).await;
    let grade = values.grade_level_ids[0];
    for invalid in ["-1", "", "00", "0.000"] {
        let mut invalid_values = values.clone();
        invalid_values.hours_per_week = invalid.into();
        assert!(matches!(
            catalog::create_activity_version(&pool, activity.id, invalid_values).await,
            Err(AppError::ValidationError(_))
        ));
    }
    let draft = catalog::create_activity_version(&pool, activity.id, values)
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
    let edition = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรกิจกรรมศูนย์ชั่วโมง".into(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let level = curriculum::create_level(
        &pool,
        edition.id,
        CreateCurriculumLevelRequest {
            draft_id: resource_draft_id(&pool, edition.id).await,
            name_th: "ระดับกิจกรรม".into(),
            grade_level_ids: vec![grade],
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        &pool,
        level.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, level.id).await,
            name_th: "แผนกิจกรรม".into(),
        },
    )
    .await
    .unwrap();
    let workspace = curriculum_structure::get_workspace(&pool, level.id)
        .await
        .unwrap();
    let workspace = curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&pool, program.id).await,
            requirements: vec![CurriculumStructureRequirementInput {
                resource_kind: RequirementResourceKind::Activity,
                catalog_version_id: published.id,
                grade_level_id: grade,
                term_slot_id: workspace.term_slots[0].id,
                requirement_kind: RequirementKind::Required,
                display_order: 1,
            }],
            row_version: program.row_version,
        },
    )
    .await
    .unwrap();
    let row = workspace
        .requirements
        .iter()
        .find(|row| row.catalog_version_id == published.id)
        .unwrap();
    assert_eq!(row.metrics.weekly_value.as_deref(), Some("0.00"));
    assert_eq!(row.metrics.total_hours.as_deref(), Some("0.00"));
    assert!(workspace.validation.blockers.is_empty());
    publish_level_fixture(
        &pool,
        level.id,
        PublishVersionRequest {
            row_version: workspace.row_version,
        },
    )
    .await
    .unwrap();
    let frozen = curriculum_structure::get_workspace(&pool, level.id)
        .await
        .unwrap();
    let row = frozen
        .requirements
        .iter()
        .find(|row| row.catalog_version_id == published.id)
        .unwrap();
    assert_eq!(row.metrics.weekly_value.as_deref(), Some("0.00"));
    assert_eq!(row.metrics.total_hours.as_deref(), Some("0.00"));
}
