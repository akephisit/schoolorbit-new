use super::*;
use school_academic_core::models::{CurriculumStructureWorkspace, StudyProgram, SubjectVersion};
use serde_json::Value;

struct Fixture {
    pool: PgPool,
    edition: Uuid,
    level: Uuid,
    program: StudyProgram,
    grade: Uuid,
    slot: Uuid,
    subject: Uuid,
    v1: SubjectVersion,
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

fn version_request(
    name: &str,
    grade: Uuid,
    from: NaiveDate,
    until: Option<NaiveDate>,
) -> CreateSubjectVersionRequest {
    CreateSubjectVersionRequest {
        name_th: name.into(),
        name_en: None,
        credit: "1.00".into(),
        hours_per_semester: Some(40),
        subject_type: "BASIC".into(),
        description: None,
        effective_from: from,
        effective_until: until,
        term_code: None,
        periods_per_week: Some(2),
        grade_level_ids: vec![grade],
    }
}

async fn fixture(name: &str) -> Fixture {
    let pool = prepare_core_fixture_through(name, 135).await;
    let grades: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type='secondary' AND year BETWEEN 1 AND 3 ORDER BY year")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(grades.len(), 3);
    let grade = grades[0];
    let edition = curriculum::create(
        &pool,
        CreateCurriculumRequest {
            name: "หลักสูตรทดสอบเวอร์ชันปัจจุบัน".into(),
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
            name_th: "มัธยมต้น".into(),
            grade_level_ids: grades,
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        &pool,
        level.id,
        CreateStudyProgramRequest {
            draft_id: resource_draft_id(&pool, edition.id).await,
            name_th: "แผนทดสอบ".into(),
        },
    )
    .await
    .unwrap();
    let workspace = curriculum_structure::get_workspace(&pool, level.id)
        .await
        .unwrap();
    let subject = catalog::create_subject(
        &pool,
        CreateCatalogSubjectRequest {
            code: "CUR-CURRENT".into(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    let v1 = publish(
        &pool,
        catalog::create_subject_version(
            &pool,
            subject.id,
            version_request("รายวิชารุ่นหนึ่ง", grade, catalog::subject_catalog_today(), None),
        )
        .await
        .unwrap(),
    )
    .await;
    Fixture {
        pool,
        edition: edition.id,
        level: level.id,
        program,
        grade,
        slot: workspace.term_slots[0].id,
        subject: subject.id,
        v1,
    }
}

fn row(f: &Fixture, version: Uuid) -> CurriculumStructureRequirementInput {
    CurriculumStructureRequirementInput {
        resource_kind: RequirementResourceKind::Course,
        catalog_version_id: version,
        grade_level_id: f.grade,
        term_slot_id: f.slot,
        requirement_kind: RequirementKind::Required,
        display_order: 1,
    }
}

async fn save(
    f: &Fixture,
    requirements: Vec<CurriculumStructureRequirementInput>,
) -> Result<CurriculumStructureWorkspace, school_errors::AppError> {
    let program = curriculum::get_program(&f.pool, f.program.id).await?;
    curriculum_structure::replace_program_structure(
        &f.pool,
        f.program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: resource_draft_id(&f.pool, f.program.id).await,
            requirements,
            row_version: program.row_version,
        },
    )
    .await
}

async fn revision(f: &Fixture, name: &str) -> SubjectVersion {
    publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            f.subject,
            version_request(name, f.grade, f.v1.effective_from, None),
        )
        .await
        .unwrap(),
    )
    .await
}

async fn snapshot(f: &Fixture) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('program',(SELECT to_jsonb(p) FROM study_programs p WHERE id=$1),'requirements',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r WHERE study_program_id=$1))").bind(f.program.id).fetch_one(&f.pool).await.unwrap()
}

async fn seed_existing(f: &Fixture, version: Uuid) {
    sqlx::query("INSERT INTO curriculum_course_requirements(id,curriculum_level_id,study_program_id,subject_version_id,grade_level_id,term_slot_id,requirement_kind,display_order) VALUES(gen_random_uuid(),$1,$2,$3,$4,$5,'required',1)").bind(f.level).bind(f.program.id).bind(version).bind(f.grade).bind(f.slot).execute(&f.pool).await.unwrap();
}

#[tokio::test]
async fn curriculum_current_subject_options_keep_history_and_reject_no_current_fallback() {
    let f = fixture("curriculum_current_subject_options").await;
    let grades:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type='secondary' AND year BETWEEN 1 AND 3 ORDER BY year").fetch_all(&f.pool).await.unwrap();
    let mut widened = version_request(&f.v1.name_th, f.grade, f.v1.effective_from, None);
    widened.grade_level_ids = grades.clone();
    let v2 = publish(
        &f.pool,
        catalog::create_subject_version(&f.pool, f.subject, widened)
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(v2.effective_from, f.v1.effective_from);
    assert_eq!(v2.name_th, f.v1.name_th);
    assert_eq!(f.v1.grade_level_ids, vec![f.grade]);
    assert_eq!(v2.grade_level_ids.len(), 3);

    let today = catalog::subject_catalog_today();
    let draft = catalog::create_subject_version(
        &f.pool,
        f.subject,
        version_request("ยังไม่เผยแพร่", f.grade, today, None),
    )
    .await
    .unwrap();
    let unavailable = catalog::create_subject(
        &f.pool,
        CreateCatalogSubjectRequest {
            code: "NO-CURRENT".into(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    let expired = publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            unavailable.id,
            version_request("หมดอายุวันนี้", f.grade, today.pred_opt().unwrap(), Some(today)),
        )
        .await
        .unwrap(),
    )
    .await;
    let future = publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            unavailable.id,
            version_request("เริ่มพรุ่งนี้", f.grade, today.succ_opt().unwrap(), None),
        )
        .await
        .unwrap(),
    )
    .await;
    let filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let options = workspaces::curriculum_management_options(&f.pool, f.level, &filter)
        .await
        .unwrap();
    for version in [&f.v1, &v2, &expired, &future] {
        let option = options
            .catalog_versions
            .iter()
            .find(|option| option.id == version.id)
            .unwrap();
        assert_eq!(option.catalog_id, version.subject_id);
        let mut expected_grades = version.grade_level_ids.clone();
        expected_grades.sort();
        assert_eq!(option.grade_level_ids, expected_grades);
        assert_eq!(
            option.section,
            school_academic_core::models::CurriculumDocumentSection::BasicCourse
        );
        assert_eq!(option.selectable_for_new_requirement, version.id == v2.id);
    }
    assert!(!options
        .catalog_versions
        .iter()
        .any(|option| option.id == draft.id));
    let before = snapshot(&f).await;
    for version in [&f.v1, &expired, &future, &draft] {
        assert!(matches!(
            save(&f, vec![row(&f, version.id)]).await,
            Err(school_errors::AppError::Conflict(_))
        ));
        assert_eq!(before, snapshot(&f).await);
    }
    assert!(matches!(
        workspaces::curriculum_management_options(
            &f.pool,
            f.level,
            &AcademicResourceListFilter::default()
        )
        .await,
        Err(school_errors::AppError::Forbidden(_))
    ));
    assert!(matches!(
        curriculum_structure::replace_program_structure(
            &f.pool,
            Uuid::new_v4(),
            ReplaceCurriculumStructureRequest {
                draft_id: resource_draft_id(&f.pool, f.edition).await,
                requirements: vec![row(&f, v2.id)],
                row_version: 1
            }
        )
        .await,
        Err(school_errors::AppError::NotFound(_))
    ));
    let other_tenant =
        school_test_db::create_named_test_pool("curriculum_current_subject_other_tenant").await;
    apply_migrations_through(&other_tenant, 135).await.unwrap();
    assert!(matches!(
        curriculum_structure::replace_program_structure(
            &other_tenant,
            f.program.id,
            ReplaceCurriculumStructureRequest {
                draft_id: resource_draft_id(&f.pool, f.program.id).await,
                requirements: vec![row(&f, v2.id)],
                row_version: f.program.row_version
            }
        )
        .await,
        Err(school_errors::AppError::NotFound(_))
    ));
    assert_eq!(before, snapshot(&f).await);
    let requirements = grades
        .iter()
        .map(|grade| {
            let mut value = row(&f, v2.id);
            value.grade_level_id = *grade;
            value
        })
        .collect();
    let saved = save(&f, requirements).await.unwrap();
    assert_eq!(saved.requirements.len(), 3);
    assert!(saved
        .requirements
        .iter()
        .all(|item| item.catalog_version_id == v2.id
            && item.catalog_id == f.subject
            && item.version_no == 2
            && !item.has_newer_current_version));
    assert_eq!(
        saved
            .requirements
            .iter()
            .map(|item| item.grade_level.id)
            .collect::<std::collections::HashSet<_>>(),
        grades.into_iter().collect()
    );
}

#[tokio::test]
async fn curriculum_current_subject_preserves_old_rows_and_allows_explicit_replacement() {
    let f = fixture("curriculum_current_subject_preserved").await;
    save(&f, vec![row(&f, f.v1.id)]).await.unwrap();
    let v2 = revision(&f, "รายวิชารุ่นสอง").await;
    let mut old = row(&f, f.v1.id);
    old.display_order = 9;
    old.requirement_kind = RequirementKind::Elective;
    let saved = save(&f, vec![old.clone()]).await.unwrap();
    let retained = &saved.requirements[0];
    assert_eq!(retained.catalog_version_id, f.v1.id);
    assert_eq!(retained.catalog_id, f.subject);
    assert_eq!(retained.version_no, 1);
    assert!(retained.has_newer_current_version);
    assert_eq!(retained.display_order, 9);
    assert_eq!(retained.requirement_kind, RequirementKind::Elective);
    let before = snapshot(&f).await;
    assert!(matches!(
        save(&f, vec![old, row(&f, v2.id)]).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert_eq!(before, snapshot(&f).await);
    let replaced = save(&f, vec![row(&f, v2.id)]).await.unwrap();
    assert_eq!(replaced.requirements.len(), 1);
    assert_eq!(replaced.requirements[0].catalog_version_id, v2.id);
    assert_eq!(replaced.requirements[0].catalog_id, f.subject);
    assert_eq!(replaced.requirements[0].version_no, 2);
    assert!(!replaced.requirements[0].has_newer_current_version);
}

#[tokio::test]
async fn curriculum_current_subject_grandfathers_duplicate_buckets_and_future_existing_tuples() {
    let f = fixture("curriculum_current_subject_legacy").await;
    save(&f, vec![row(&f, f.v1.id)]).await.unwrap();
    let v2 = revision(&f, "รายวิชารุ่นสอง").await;
    seed_existing(&f, v2.id).await;
    let mut old = row(&f, f.v1.id);
    old.display_order = 8;
    save(&f, vec![old.clone(), row(&f, v2.id)]).await.unwrap();
    let v3 = revision(&f, "รายวิชารุ่นสาม").await;
    let before = snapshot(&f).await;
    assert!(matches!(
        save(&f, vec![old, row(&f, v3.id)]).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert_eq!(before, snapshot(&f).await);
    save(&f, vec![row(&f, v3.id)]).await.unwrap();
    let future = publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            f.subject,
            version_request(
                "ฉบับอนาคต",
                f.grade,
                catalog::subject_catalog_today().succ_opt().unwrap(),
                None,
            ),
        )
        .await
        .unwrap(),
    )
    .await;
    let today = catalog::subject_catalog_today();
    let expired = publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            f.subject,
            version_request("ฉบับหมดอายุ", f.grade, today.pred_opt().unwrap(), Some(today)),
        )
        .await
        .unwrap(),
    )
    .await;
    // Existing/copied unavailable tuples are retained; changing term makes a new tuple.
    for version_id in [future.id, expired.id] {
        sqlx::query("DELETE FROM curriculum_course_requirements WHERE study_program_id=$1")
            .bind(f.program.id)
            .execute(&f.pool)
            .await
            .unwrap();
        seed_existing(&f, version_id).await;
        let mut retained = row(&f, version_id);
        retained.display_order = 4;
        save(&f, vec![retained.clone()]).await.unwrap();
        let workspace = curriculum_structure::get_workspace(&f.pool, f.level)
            .await
            .unwrap();
        retained.term_slot_id = workspace.term_slots[1].id;
        assert!(matches!(
            save(&f, vec![retained]).await,
            Err(school_errors::AppError::Conflict(_))
        ));
    }
}

#[tokio::test]
async fn curriculum_current_subject_copied_v1_remains_pinned_and_activities_keep_eligibility() {
    let mut f = fixture("curriculum_current_subject_copy").await;
    save(&f, vec![row(&f, f.v1.id)]).await.unwrap();
    let edition = curriculum::get(&f.pool, f.edition).await.unwrap();
    curriculum::publish(
        &f.pool,
        f.edition,
        PublishCurriculumRequest {
            draft_id: edition.draft_id.unwrap(),
            change_note: "เผยแพร่ต้นฉบับ".into(),
            row_version: edition.row_version,
        },
        fixture_actor(&f.pool).await,
    )
    .await
    .unwrap();
    let publication_id = curriculum::get(&f.pool, f.edition)
        .await
        .unwrap()
        .current_publication_id;
    let published_before = school_academic_core::services::curriculum_publications::read_workspace(
        &f.pool,
        f.level,
        &school_academic_core::models::CurriculumViewQuery {
            publication_id,
            draft_id: None,
        },
    )
    .await
    .unwrap();
    assert!(!published_before.requirements[0].has_newer_current_version);
    let pinned_group = catalog::subject_groups_for_versions(&f.pool, &[f.v1.id])
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        published_before.requirements[0].subject_group_id,
        Some(pinned_group.subject_group_id)
    );
    assert_eq!(
        published_before.requirements[0].subject_group_order,
        pinned_group.display_order
    );
    revision(&f, "รายวิชารุ่นสอง").await;
    let persisted_before_read = snapshot(&f).await;
    let published_after = school_academic_core::services::curriculum_publications::read_workspace(
        &f.pool,
        f.level,
        &school_academic_core::models::CurriculumViewQuery {
            publication_id,
            draft_id: None,
        },
    )
    .await
    .unwrap();
    assert!(published_after.requirements[0].has_newer_current_version);
    let mut expected = serde_json::to_value(&published_before).unwrap();
    expected["requirements"][0]["hasNewerCurrentVersion"] = json!(true);
    assert_eq!(expected, serde_json::to_value(&published_after).unwrap());
    assert_eq!(persisted_before_read, snapshot(&f).await);
    let destination = curriculum::create(
        &f.pool,
        CreateCurriculumRequest {
            name: "หลักสูตรสำเนา".into(),
            revision_year: 2570,
            description: None,
        },
    )
    .await
    .unwrap();
    let level = curriculum::create_level(
        &f.pool,
        destination.id,
        CreateCurriculumLevelRequest {
            draft_id: destination.draft_id.unwrap(),
            name_th: "มัธยมต้น".into(),
            grade_level_ids: vec![f.grade],
            description: None,
        },
    )
    .await
    .unwrap();
    let source = curriculum::get_program(&f.pool, f.program.id)
        .await
        .unwrap();
    let copied = curriculum::copy_program(
        &f.pool,
        level.id,
        CopyStudyProgramRequest {
            draft_id: destination.draft_id.unwrap(),
            source_program_id: source.id,
            source_row_version: source.row_version,
            destination_row_version: level.row_version,
            name_th: None,
        },
    )
    .await
    .unwrap();
    f.program = copied;
    f.level = level.id;
    f.edition = destination.id;
    let copied_workspace = curriculum_structure::get_workspace(&f.pool, f.level)
        .await
        .unwrap();
    let retained = &copied_workspace.requirements[0];
    f.slot = retained.term_slot_id;
    assert_eq!(retained.catalog_version_id, f.v1.id);
    assert_eq!(retained.catalog_id, f.subject);
    assert_eq!(retained.version_no, 1);
    assert!(retained.has_newer_current_version);
    save(&f, vec![row(&f, f.v1.id)]).await.unwrap();
    let activity = catalog::create_activity(
        &f.pool,
        CreateCatalogActivityRequest {
            activity_type: "guidance".into(),
        },
    )
    .await
    .unwrap();
    let version = catalog::create_activity_version(
        &f.pool,
        activity.id,
        CreateActivityVersionRequest {
            name: "กิจกรรมอนาคต".into(),
            description: None,
            hours_per_week: "1".into(),
            hours_per_term: Some("20".into()),
            scheduling_mode: "synchronized".into(),
            effective_from: catalog::subject_catalog_today().succ_opt().unwrap(),
            effective_until: None,
            term_code: None,
            grade_level_ids: vec![f.grade],
        },
    )
    .await
    .unwrap();
    let activity_version = catalog::publish_activity_version(
        &f.pool,
        version.id,
        PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap();
    let options = workspaces::curriculum_management_options(
        &f.pool,
        f.level,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let option = options
        .catalog_versions
        .iter()
        .find(|option| option.id == activity_version.id)
        .unwrap();
    assert_eq!(option.catalog_id, activity.id);
    assert_eq!(option.grade_level_ids, vec![f.grade]);
    assert_eq!(
        option.section,
        school_academic_core::models::CurriculumDocumentSection::StudentDevelopment
    );
    assert!(option.selectable_for_new_requirement);
    let mut activity_row = row(&f, activity_version.id);
    activity_row.resource_kind = RequirementResourceKind::Activity;
    let workspace = save(&f, vec![row(&f, f.v1.id), activity_row])
        .await
        .unwrap();
    let activity_row = workspace
        .requirements
        .iter()
        .find(|row| row.resource_kind == RequirementResourceKind::Activity)
        .unwrap();
    assert_eq!(activity_row.subject_group_id, None);
    assert_eq!(activity_row.subject_group_order, None);
    assert_eq!(activity_row.catalog_id, activity.id);
    assert_eq!(activity_row.version_no, 1);
    assert!(!activity_row.has_newer_current_version);
}

#[tokio::test]
async fn curriculum_current_subject_save_waits_for_publication_lock_and_rolls_back_stale_selection()
{
    let f = fixture("curriculum_current_subject_race").await;
    let draft = catalog::create_subject_version(
        &f.pool,
        f.subject,
        version_request("รายวิชารุ่นสอง", f.grade, f.v1.effective_from, None),
    )
    .await
    .unwrap();
    let mut publisher = f.pool.begin().await.unwrap();
    catalog::lock_subjects_for_revision_selection(&mut publisher, &[f.subject])
        .await
        .unwrap();
    let before = snapshot(&f).await;
    let save_future = save(&f, vec![row(&f, f.v1.id)]);
    tokio::pin!(save_future);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut save_future)
            .await
            .is_err(),
        "save must await the stable subject lock"
    );
    sqlx::query("UPDATE subject_versions SET status='published',published_at=now(),row_version=row_version+1 WHERE id=$1").bind(draft.id).execute(&mut *publisher).await.unwrap();
    publisher.commit().await.unwrap();
    assert!(matches!(
        save_future.await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert_eq!(before, snapshot(&f).await);
}

#[tokio::test]
async fn curriculum_current_subject_warning_ignores_unavailable_revisions_and_preserves_pins() {
    let f = fixture("curriculum_current_subject_warning").await;
    let initial = save(&f, vec![row(&f, f.v1.id)]).await.unwrap();
    assert!(!initial.requirements[0].has_newer_current_version);
    let today = catalog::subject_catalog_today();
    // A newer draft, future publication, or expired publication is not current.
    catalog::create_subject_version(
        &f.pool,
        f.subject,
        version_request("ร่างใหม่", f.grade, today, None),
    )
    .await
    .unwrap();
    let future = publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            f.subject,
            version_request("อนาคตใหม่", f.grade, today.succ_opt().unwrap(), None),
        )
        .await
        .unwrap(),
    )
    .await;
    let expired = publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            f.subject,
            version_request("หมดอายุใหม่", f.grade, today.pred_opt().unwrap(), Some(today)),
        )
        .await
        .unwrap(),
    )
    .await;
    let current = catalog::current_subject_revisions(&f.pool, &[f.subject])
        .await
        .unwrap();
    assert_eq!(current[&f.subject].id, f.v1.id);
    assert_eq!(current[&f.subject].version_no, 1);
    let before = snapshot(&f).await;
    let workspace = curriculum_structure::get_workspace(&f.pool, f.level)
        .await
        .unwrap();
    assert!(!workspace.requirements[0].has_newer_current_version);
    assert_eq!(workspace.requirements[0].catalog_version_id, f.v1.id);
    assert_eq!(before, snapshot(&f).await);
    // Legacy future pin has a higher revision number than the selected current version.
    seed_existing(&f, future.id).await;
    let before = snapshot(&f).await;
    let workspace = curriculum_structure::get_workspace(&f.pool, f.level)
        .await
        .unwrap();
    assert!(workspace
        .requirements
        .iter()
        .all(|item| !item.has_newer_current_version));
    assert_eq!(before, snapshot(&f).await);

    let unavailable = catalog::create_subject(
        &f.pool,
        CreateCatalogSubjectRequest {
            code: "WARNING-NO-CURRENT".into(),
            subject_group_id: DEFAULT_SUBJECT_GROUP_ID,
        },
    )
    .await
    .unwrap();
    let old = publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            unavailable.id,
            version_request("หมดอายุ", f.grade, today.pred_opt().unwrap(), Some(today)),
        )
        .await
        .unwrap(),
    )
    .await;
    publish(
        &f.pool,
        catalog::create_subject_version(
            &f.pool,
            unavailable.id,
            version_request("อนาคต", f.grade, today.succ_opt().unwrap(), None),
        )
        .await
        .unwrap(),
    )
    .await;
    catalog::create_subject_version(
        &f.pool,
        unavailable.id,
        version_request("ร่าง", f.grade, today, None),
    )
    .await
    .unwrap();
    seed_existing(&f, old.id).await;
    assert!(
        !catalog::current_subject_revisions(&f.pool, &[unavailable.id])
            .await
            .unwrap()
            .contains_key(&unavailable.id)
    );
    let before = snapshot(&f).await;
    let workspace = curriculum_structure::get_workspace(&f.pool, f.level)
        .await
        .unwrap();
    assert_eq!(workspace.requirements.len(), 3);
    assert!(workspace
        .requirements
        .iter()
        .all(|item| !item.has_newer_current_version));
    assert_eq!(before, snapshot(&f).await);
    assert!(expired.version_no > f.v1.version_no);
}

#[tokio::test]
async fn curriculum_credit_category_unknown_published_pin_stays_unknown_after_new_revision() {
    use school_academic_core::models::{
        CurriculumCourseCreditCategory, CurriculumDocumentSection, CurriculumViewQuery,
    };
    use school_academic_core::services::curriculum_publications;
    let f = fixture("curriculum_credit_category_unknown_pin").await;
    let mut unknown = version_request(
        "รายวิชาพื้นฐานชื่อนี้ไม่กำหนดประเภท",
        f.grade,
        f.v1.effective_from,
        None,
    );
    unknown.subject_type = "LEGACY-UNKNOWN".into();
    let unknown = publish(
        &f.pool,
        catalog::create_subject_version(&f.pool, f.subject, unknown)
            .await
            .unwrap(),
    )
    .await;
    let saved = save(&f, vec![row(&f, unknown.id)]).await.unwrap();
    assert_eq!(saved.requirements[0].credit_category, None);
    assert_eq!(
        saved.requirements[0].section,
        CurriculumDocumentSection::BasicCourse
    );
    let edition = curriculum::get(&f.pool, f.edition).await.unwrap();
    let published = curriculum::publish(
        &f.pool,
        f.edition,
        PublishCurriculumRequest {
            draft_id: edition.draft_id.unwrap(),
            row_version: edition.row_version,
            change_note: "Preserve unknown catalog classification".into(),
        },
        fixture_actor(&f.pool).await,
    )
    .await
    .unwrap();
    let newer = revision(&f, "ชื่อเดียวกันไม่เปลี่ยนประเภทเก่า").await;
    let pinned = curriculum_publications::read_workspace(
        &f.pool,
        f.level,
        &CurriculumViewQuery {
            publication_id: published.current_publication_id,
            draft_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(pinned.requirements[0].catalog_version_id, unknown.id);
    assert_eq!(pinned.requirements[0].credit_category, None);
    let options = workspaces::curriculum_management_options(
        &f.pool,
        f.level,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        options
            .catalog_versions
            .iter()
            .find(|option| option.id == unknown.id)
            .unwrap()
            .credit_category,
        None
    );
    assert_eq!(
        options
            .catalog_versions
            .iter()
            .find(|option| option.id == newer.id)
            .unwrap()
            .credit_category,
        Some(CurriculumCourseCreditCategory::Basic)
    );
    assert!(options
        .catalog_versions
        .iter()
        .filter(|option| option.resource_kind == RequirementResourceKind::Activity)
        .all(|option| option.credit_category.is_none()));
}
