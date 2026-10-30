use super::*;
use school_academic_core::{
    models as core_models,
    services::{curriculum, curriculum_structure},
};
use school_academic_delivery::models::{CurriculumPreparationProposal, CurriculumPreviewAction};
use school_academic_delivery::services::versions;
use serde_json::Value;

struct PreparationFixture {
    pool: PgPool,
    context: RuntimeContext,
    offering_id: Uuid,
    group_id: Uuid,
    initial_revision: super::super::models::AcademicTermChangeSet,
    program_b: Uuid,
    room_b: Uuid,
}

async fn fixture(name: &str, generated: bool) -> PreparationFixture {
    fixture_with_programs(name, generated, false).await
}

async fn fixture_with_programs(name: &str, generated: bool, include_b: bool) -> PreparationFixture {
    let pool = prepare_delivery_runtime_fixture(name).await;
    let mut context = planning_runtime_context(&pool).await;
    let version = zero_workload_subject(&pool, &context).await;
    context.subject_version_id = version.id;
    let edition = curriculum::create(
        &pool,
        core_models::CreateCurriculumRequest {
            name: "หลักสูตรทดสอบเป้าหมายรายการเดิม".into(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let token = edition.draft_id.unwrap();
    let level = curriculum::create_level(
        &pool,
        edition.id,
        core_models::CreateCurriculumLevelRequest {
            draft_id: token,
            name_th: "ระดับทดสอบ".into(),
            grade_level_ids: vec![context.grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let slot = curriculum_structure::get_workspace(&pool, level.id)
        .await
        .unwrap()
        .term_slots[0]
        .id;
    let mut programs = Vec::new();
    for label in ["แผน A", "แผน B"] {
        let program = curriculum::create_program(
            &pool,
            level.id,
            core_models::CreateStudyProgramRequest {
                draft_id: token,
                name_th: label.into(),
            },
        )
        .await
        .unwrap();
        curriculum_structure::replace_program_structure(
            &pool,
            program.id,
            core_models::ReplaceCurriculumStructureRequest {
                draft_id: token,
                row_version: program.row_version,
                requirements: vec![core_models::CurriculumStructureRequirementInput {
                    resource_kind: core_models::RequirementResourceKind::Course,
                    catalog_version_id: version.id,
                    grade_level_id: context.grade_level_id,
                    term_slot_id: slot,
                    requirement_kind: core_models::RequirementKind::Required,
                    display_order: 1,
                }],
            },
        )
        .await
        .unwrap();
        programs.push(program.id);
    }
    let edition = curriculum::get(&pool, edition.id).await.unwrap();
    curriculum::publish(
        &pool,
        edition.id,
        core_models::PublishCurriculumRequest {
            draft_id: token,
            change_note: "ทดสอบ A และ B ใช้รายวิชาฉบับเดียวกัน".into(),
            row_version: edition.row_version,
        },
        context.teacher_id,
    )
    .await
    .unwrap();
    let mut rooms = Vec::new();
    for (index, program) in programs.iter().enumerate() {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,capacity,is_active)
            VALUES($1,$2,$2,$3,$4,$5,40,true)")
            .bind(id).bind(format!("PREP-TARGET-{index}")).bind(context.year_id)
            .bind(context.grade_level_id).bind(program).execute(&pool).await.unwrap();
        rooms.push(id);
    }
    context.study_program_id = programs[0];
    context.homeroom_id = rooms[0];
    let initial_revision =
        create_runtime_change_set(&pool, context.teacher_id, context.term_id, 1, name).await;
    let offering_id;
    let group_id;
    if generated {
        let preview = offerings::preview_from_curriculum(
            &pool,
            PreviewCurriculumOfferingsRequest {
            starts_on: chrono::Utc::now().with_timezone(&chrono_tz::Asia::Bangkok).date_naive(),
                delivery_version_id: Some(initial_revision.target_delivery_version_id),
                academic_term_id: context.term_id,
                study_program_ids: if include_b {
                    programs.clone()
                } else {
                    vec![context.study_program_id]
                },
            },
        )
        .await
        .unwrap();
        let applied = offerings::apply_from_curriculum(
            &pool,
            context.teacher_id,
            ApplyCurriculumOfferingsRequest {
            starts_on: chrono::Utc::now().with_timezone(&chrono_tz::Asia::Bangkok).date_naive(),
                academic_term_id: context.term_id,
                study_program_ids: if include_b {
                    programs.clone()
                } else {
                    vec![context.study_program_id]
                },
                delivery_version_id: Some(initial_revision.target_delivery_version_id),
                source_hash: preview.source_hash.clone(),
                choices: default_preparation_choices(&preview),
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .unwrap();
        offering_id = applied.offering_ids[0];
        group_id = applied.group_ids[0];
    } else {
        let offering = offerings::create_for_delivery(
            &pool,
            context.teacher_id,
            Some(initial_revision.target_delivery_version_id),
            course_request(&context),
        )
        .await
        .unwrap();
        offering_id = offering.id;
        let group = groups::create(
            &pool,
            context.teacher_id,
            offering_id,
            CreateLearningGroupRequest {
                name: "กลุ่ม A ที่จัดเอง".into(),
                description: None,
                capacity: None,
                preferred_room_ids: vec![],
            },
        )
        .await
        .unwrap();
        let group = groups::replace_homerooms(
            &pool,
            context.teacher_id,
            group.id,
            ReplaceLearningGroupHomeroomsRequest {
                row_version: group.row_version,
                homeroom_ids: vec![context.homeroom_id],
            },
        )
        .await
        .unwrap();
        group_id = group.id;
    }
    let group = groups::get(&pool, group_id).await.unwrap();
    groups::replace_teachers(
        &pool,
        context.teacher_id,
        group_id,
        ReplaceLearningGroupTeachersRequest {
            row_version: group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    PreparationFixture {
        pool,
        context,
        offering_id,
        group_id,
        initial_revision,
        program_b: programs[1],
        room_b: rooms[1],
    }
}

async fn preview(f: &PreparationFixture, program: Uuid) -> CurriculumOfferingPreview {
    offerings::preview_from_curriculum(
        &f.pool,
        PreviewCurriculumOfferingsRequest {
            starts_on: chrono::Utc::now().with_timezone(&chrono_tz::Asia::Bangkok).date_naive(),
            delivery_version_id: None,
            academic_term_id: f.context.term_id,
            study_program_ids: vec![program],
        },
    )
    .await
    .unwrap()
}

fn proposal(preview: &CurriculumOfferingPreview) -> &CurriculumPreparationProposal {
    assert_eq!(preview.proposals.len(), 1);
    &preview.proposals[0]
}

fn request(
    f: &PreparationFixture,
    program: Uuid,
    preview: &CurriculumOfferingPreview,
    action: PreparationAction,
) -> ApplyCurriculumOfferingsRequest {
    ApplyCurriculumOfferingsRequest {
        starts_on: preview.starts_on,
        academic_term_id: f.context.term_id,
        study_program_ids: vec![program],
        delivery_version_id: None,
        source_hash: preview.source_hash.clone(),
        idempotency_key: Uuid::new_v4(),
        choices: vec![CurriculumPreparationChoice {
            proposal_id: proposal(preview).proposal_id.clone(),
            action,
            groups: if action == PreparationAction::Apply {
                proposal(preview).default_groups.clone()
            } else {
                vec![]
            },
        }],
    }
}

async fn facts(f: &PreparationFixture) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object(
        'offerings',(SELECT jsonb_agg(to_jsonb(o) ORDER BY id) FROM learning_offerings o),
        'targets',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_offering_targets t),
        'groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g),
        'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t),
        'students',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learning_group_students s),
        'versions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY id) FROM academic_delivery_versions v),
        'revisions',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM academic_term_change_sets r),
        'receipts',(SELECT jsonb_agg(to_jsonb(r) ORDER BY idempotency_key) FROM learning_delivery_apply_runs r),
        'auditCount',(SELECT count(*) FROM academic_audit_events))")
        .fetch_one(&f.pool).await.unwrap()
}

async fn publish(f: &PreparationFixture) {
    let offering = offerings::get(&f.pool, f.offering_id).await.unwrap();
    offerings::publish(
        &f.pool,
        f.context.teacher_id,
        f.offering_id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let preview = publication_preview(&f.pool, f.initial_revision.id)
        .await
        .unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|x| x.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        &f.pool,
        f.context.teacher_id,
        f.initial_revision.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: vec![],
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
}

async fn covered_published(name: &str, grade_program: bool) {
    let f = fixture(name, true).await;
    if grade_program {
        let offering = offerings::get(&f.pool, f.offering_id).await.unwrap();
        offerings::update(
            &f.pool,
            f.context.teacher_id,
            f.offering_id,
            UpdateLearningOfferingRequest {
                row_version: offering.row_version,
                targets: vec![OfferingTargetInput {
                    target_kind: OfferingTargetKind::GradeProgram,
                    homeroom_id: None,
                    grade_level_id: f.context.grade_level_id,
                    study_program_id: f.context.study_program_id,
                }],
            },
        )
        .await
        .unwrap();
    }
    publish(&f).await;
    let before = facts(&f).await;
    let snapshot_before: String =
        sqlx::query_scalar("SELECT snapshot::text FROM academic_delivery_versions WHERE id=$1")
            .bind(f.initial_revision.target_delivery_version_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    for action in [PreparationAction::Apply, PreparationAction::DeferGroups] {
        // Preview and apply share the selected draft context; the published source stays immutable.
        let selected = create_runtime_change_set(
            &f.pool,
            f.context.teacher_id,
            f.context.term_id,
            2,
            &format!("{name}-{action:?}"),
        )
        .await;
        let preview = offerings::preview_from_curriculum(
            &f.pool,
            PreviewCurriculumOfferingsRequest {
            starts_on: chrono::Utc::now().with_timezone(&chrono_tz::Asia::Bangkok).date_naive(),
                academic_term_id: f.context.term_id,
                study_program_ids: vec![f.context.study_program_id],
                delivery_version_id: Some(selected.target_delivery_version_id),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            proposal(&preview).offering_action,
            CurriculumPreviewAction::Retain
        );
        assert!(proposal(&preview).conflicts.is_empty());
        let mut apply = request(&f, f.context.study_program_id, &preview, action);
        apply.delivery_version_id = Some(selected.target_delivery_version_id);
        let result = offerings::apply_from_curriculum(&f.pool, f.context.teacher_id, apply)
            .await
            .unwrap();
        assert_eq!(result.offering_ids, vec![f.offering_id]);
        assert_eq!(result.created_offering_count, 0);
        assert_eq!(result.created_group_count, 0);
        if action == PreparationAction::Apply {
            assert_eq!(result.group_ids, vec![f.group_id]);
        }
    }
    let after = facts(&f).await;
    for key in ["offerings", "targets", "groups", "teachers", "students"] {
        assert_eq!(before[key], after[key], "{key}");
    }
    let snapshot_after: String =
        sqlx::query_scalar("SELECT snapshot::text FROM academic_delivery_versions WHERE id=$1")
            .bind(f.initial_revision.target_delivery_version_id)
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(snapshot_before, snapshot_after);
    assert!(
        versions::get_version(&f.pool, f.initial_revision.target_delivery_version_id)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn preparation_published_homeroom_coverage_retains_without_target_writes() {
    covered_published("prep_published_homeroom_coverage", false).await;
}

#[tokio::test]
async fn preparation_published_grade_program_coverage_retains_without_target_writes() {
    covered_published("prep_published_grade_program_coverage", true).await;
}

#[tokio::test]
async fn preparation_published_extension_is_conflict_even_when_groups_are_deferred() {
    let f = fixture("prep_published_extension", true).await;
    publish(&f).await;
    let preview = preview(&f, f.program_b).await;
    assert_eq!(proposal(&preview).existing_offering_id, Some(f.offering_id));
    assert_eq!(
        proposal(&preview).offering_action,
        CurriculumPreviewAction::Conflict
    );
    assert!(proposal(&preview)
        .conflicts
        .iter()
        .any(|x| x.code == "published_offering_target_extension"));
    for action in [PreparationAction::Apply, PreparationAction::DeferGroups] {
        let before = facts(&f).await;
        let result = offerings::apply_from_curriculum(
            &f.pool,
            f.context.teacher_id,
            request(&f, f.program_b, &preview, action),
        )
        .await;
        assert!(matches!(result, Err(AppError::Conflict(_))), "{result:?}");
        assert_eq!(before, facts(&f).await);
    }
}

#[tokio::test]
async fn preparation_draft_extension_preserves_manual_a_and_creates_b_groups() {
    let f = fixture("prep_draft_manual_extension", false).await;
    let before = groups::get(&f.pool, f.group_id).await.unwrap();
    let preview = preview(&f, f.program_b).await;
    assert_eq!(
        proposal(&preview).offering_action,
        CurriculumPreviewAction::Retain
    );
    assert!(proposal(&preview).conflicts.is_empty());
    let result = offerings::apply_from_curriculum(
        &f.pool,
        f.context.teacher_id,
        request(&f, f.program_b, &preview, PreparationAction::Apply),
    )
    .await
    .unwrap();
    assert_eq!(result.offering_ids, vec![f.offering_id]);
    assert_eq!(result.created_group_count, 1);
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(groups::get(&f.pool, f.group_id).await.unwrap()).unwrap()
    );
    let targets = offerings::get(&f.pool, f.offering_id)
        .await
        .unwrap()
        .targets;
    assert!(targets
        .iter()
        .any(|target| target.homeroom_id == Some(f.room_b)));
}

#[tokio::test]
async fn preparation_draft_generated_group_mismatch_keeps_defer_groups_policy() {
    let f = fixture("prep_draft_generated_extension", true).await;
    let preview = preview(&f, f.program_b).await;
    assert_eq!(
        proposal(&preview).offering_action,
        CurriculumPreviewAction::Retain
    );
    assert!(proposal(&preview)
        .conflicts
        .iter()
        .any(|x| x.code == "generated_group_target_mismatch"));
    let old_groups =
        serde_json::to_value(groups::list(&f.pool, f.offering_id).await.unwrap()).unwrap();
    let result = offerings::apply_from_curriculum(
        &f.pool,
        f.context.teacher_id,
        request(&f, f.program_b, &preview, PreparationAction::DeferGroups),
    )
    .await
    .unwrap();
    assert_eq!(result.created_group_count, 0);
    assert_eq!(
        old_groups,
        serde_json::to_value(groups::list(&f.pool, f.offering_id).await.unwrap()).unwrap()
    );
    assert!(offerings::get(&f.pool, f.offering_id)
        .await
        .unwrap()
        .targets
        .iter()
        .any(|x| x.homeroom_id == Some(f.room_b)));
}

#[tokio::test]
async fn preparation_late_publication_rejects_a_previous_draft_extension_preview() {
    let f = fixture("prep_late_publication_extension", false).await;
    let old = preview(&f, f.program_b).await;
    assert_eq!(
        proposal(&old).offering_action,
        CurriculumPreviewAction::Retain
    );
    publish(&f).await;
    let before = facts(&f).await;
    let result = offerings::apply_from_curriculum(
        &f.pool,
        f.context.teacher_id,
        request(&f, f.program_b, &old, PreparationAction::DeferGroups),
    )
    .await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(before, facts(&f).await);
}

#[tokio::test]
async fn delivery_room_scope_reopen_reviews_current_rooms_and_retains_valid_generated_identity() {
    let mut f = fixture_with_programs("room_scope_reopen", true, true).await;
    let initial = versions::get_version(&f.pool, f.initial_revision.target_delivery_version_id)
        .await
        .unwrap();
    let opened = initial
        .snapshot
        .offerings
        .iter()
        .find(|o| o.id == f.offering_id)
        .unwrap();
    let retained = opened
        .groups
        .iter()
        .find(|g| g.homeroom_ids.contains(&f.room_b))
        .unwrap()
        .clone();
    // Both groups need their own existing teacher configuration before publication.
    for selected in &opened.groups {
        let group = groups::get(&f.pool, selected.id).await.unwrap();
        groups::replace_teachers(
            &f.pool,
            f.context.teacher_id,
            group.id,
            ReplaceLearningGroupTeachersRequest {
                row_version: group.row_version,
                teachers: vec![TeacherAssignmentInput {
                    teacher_id: f.context.teacher_id,
                    role: LearningTeacherRole::Primary,
                }],
            },
        )
        .await
        .unwrap();
    }
    publish(&f).await;
    super::room_scope_tests::seed_retained_grade(&f.pool, &f.context, retained.id).await;
    f.initial_revision = create_runtime_change_set(
        &f.pool,
        f.context.teacher_id,
        f.context.term_id,
        2,
        "scope-reopen-selected",
    )
    .await;
    let before_history = super::room_scope_tests::retained_grade_facts(&f.pool, retained.id).await;
    let before=sqlx::query_scalar::<_,Value>("SELECT jsonb_build_object('group',(SELECT to_jsonb(g) FROM learning_groups g WHERE id=$1),'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t WHERE learning_group_id=$1))").bind(retained.id).fetch_one(&f.pool).await.unwrap();
    let target = versions::get_version(&f.pool, f.initial_revision.target_delivery_version_id)
        .await
        .unwrap();
    let current = change_sets::get_change_set(&f.pool, f.initial_revision.id)
        .await
        .unwrap();
    change_sets::remove_draft_offering(
        &f.pool,
        f.context.teacher_id,
        current.id,
        f.offering_id,
        school_academic_delivery::models::RemoveDeliveryDraftResourceRequest {
            change_set_row_version: current.row_version,
            target_delivery_version_row_version: target.row_version,
        },
    )
    .await
    .unwrap();
    let preview = offerings::preview_from_curriculum(
        &f.pool,
        PreviewCurriculumOfferingsRequest {
            starts_on: chrono::Utc::now().with_timezone(&chrono_tz::Asia::Bangkok).date_naive(),
            academic_term_id: f.context.term_id,
            study_program_ids: vec![f.program_b],
            delivery_version_id: Some(target.id),
        },
    )
    .await
    .unwrap();
    assert_eq!(proposal(&preview).target_homeroom_ids, vec![f.room_b]);
    assert!(
        proposal(&preview).conflicts.is_empty(),
        "{:?}",
        proposal(&preview).conflicts
    );
    let mut apply = request(&f, f.program_b, &preview, PreparationAction::Apply);
    apply.delivery_version_id = Some(target.id);
    let applied = offerings::apply_from_curriculum(&f.pool, f.context.teacher_id, apply.clone())
        .await
        .unwrap();
    assert_eq!(applied.offering_ids, vec![f.offering_id]);
    assert_eq!(applied.group_ids, vec![retained.id]);
    let after = versions::get_version(&f.pool, target.id).await.unwrap();
    let scoped = after
        .snapshot
        .offerings
        .iter()
        .find(|o| o.id == f.offering_id)
        .unwrap();
    assert_eq!(scoped.homeroom_ids, vec![f.room_b]);
    assert_eq!(scoped.groups.len(), 1);
    assert_eq!(scoped.groups[0].id, retained.id);
    let after_facts=sqlx::query_scalar::<_,Value>("SELECT jsonb_build_object('group',(SELECT to_jsonb(g) FROM learning_groups g WHERE id=$1),'teachers',(SELECT jsonb_agg(to_jsonb(t) ORDER BY id) FROM learning_group_teachers t WHERE learning_group_id=$1))").bind(retained.id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(before, after_facts);
    assert_eq!(
        before_history,
        super::room_scope_tests::retained_grade_facts(&f.pool, retained.id).await
    );
    let replay_facts = facts(&f).await;
    let replay = offerings::apply_from_curriculum(&f.pool, f.context.teacher_id, apply)
        .await
        .unwrap();
    assert_eq!(replay.offering_ids, applied.offering_ids);
    assert_eq!(replay.group_ids, applied.group_ids);
    assert_eq!(
        replay_facts,
        facts(&f).await,
        "same receipt must not reopen obsolete rooms or mutate counters"
    );
    let stale = offerings::apply_from_curriculum(
        &f.pool,
        f.context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            idempotency_key: Uuid::new_v4(),
            ..request(&f, f.program_b, &preview, PreparationAction::Apply)
        },
    )
    .await
    .expect_err("unscoped apply cannot consume selected-revision preview hash");
    assert!(matches!(stale, AppError::Conflict(_)));
    assert_eq!(replay_facts, facts(&f).await);
}
