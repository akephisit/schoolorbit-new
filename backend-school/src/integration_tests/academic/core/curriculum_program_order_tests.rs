use super::*;
use school_academic_core::models::{
    CurriculumViewQuery, OpenCurriculumDraftRequest, ReplaceStudyProgramOrderRequest,
    StudyProgramOrderResult,
};
use school_academic_core::services::curriculum_publications as publications;
use serde_json::Value;

// Historical migration fixtures must not call the evolving program DTO/service
// before its required schema exists. This explicit projection works at 132/133.
pub(super) async fn historical_publication_fixture(pool: &PgPool, code: &str) -> (Uuid, Uuid) {
    let (subject, grade): (Uuid, Uuid) = sqlx::query_as("SELECT r.subject_version_id,r.grade_level_id FROM curriculum_course_requirements r JOIN subject_versions v ON v.id=r.subject_version_id WHERE v.periods_per_week>0 AND v.hours_per_semester>0 ORDER BY r.id LIMIT 1")
        .fetch_one(pool).await.unwrap();
    let edition = Uuid::now_v7();
    let level = Uuid::now_v7();
    let program = Uuid::now_v7();
    let slot = Uuid::now_v7();
    sqlx::query("INSERT INTO curriculum_editions(id,name,revision_year) VALUES($1,$2,2569)")
        .bind(edition)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO curriculum_levels(id,edition_id,code,name_th,grade_level_ids,is_active) VALUES($1,$2,$3,$3,$4,true)")
        .bind(level).bind(edition).bind(code).bind(sqlx::types::Json(vec![grade])).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status) VALUES($1,$2,'HIST-P1','Historical plan','draft')")
        .bind(program).bind(level).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO curriculum_term_slots(id,curriculum_level_id,sequence,term_type,type_occurrence,name) VALUES($1,$2,1,'regular',1,'Term1')")
        .bind(slot).bind(level).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO curriculum_course_requirements(id,curriculum_level_id,study_program_id,subject_version_id,grade_level_id,term_slot_id,requirement_kind,display_order) VALUES(gen_random_uuid(),$1,$2,$3,$4,$5,'required',1)")
        .bind(level).bind(program).bind(subject).bind(grade).bind(slot).execute(pool).await.unwrap();
    sqlx::query("UPDATE study_programs SET status='published' WHERE id=$1")
        .bind(program)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("SELECT capture_curriculum_publication($1,$2,'Historical SQL fixture')")
        .bind(edition)
        .bind(fixture_actor(pool).await)
        .execute(pool)
        .await
        .unwrap();
    (edition, level)
}

pub(super) async fn historical_workspace_evidence(
    pool: &PgPool,
    level: Uuid,
    publication: Option<Uuid>,
) -> Value {
    let (programs, courses, activities, slots, condition) = if publication.is_some() {
        (
            "curriculum_publication_programs",
            "curriculum_publication_courses",
            "curriculum_publication_activities",
            "curriculum_publication_slots",
            " AND publication_id=$2",
        )
    } else {
        (
            "study_programs",
            "curriculum_course_requirements",
            "curriculum_activity_requirements",
            "curriculum_term_slots",
            " AND $2::uuid IS NULL",
        )
    };
    let query = format!("SELECT jsonb_build_object('programs',(SELECT jsonb_agg(to_jsonb(r)-'is_default'-'display_order' ORDER BY id) FROM {programs} r WHERE curriculum_level_id=$1{condition}),'courses',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM {courses} r WHERE curriculum_level_id=$1{condition}),'activities',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM {activities} r WHERE curriculum_level_id=$1{condition}),'slots',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM {slots} r WHERE curriculum_level_id=$1{condition}))");
    sqlx::query_scalar(sqlx::AssertSqlSafe(query))
        .bind(level)
        .bind(publication)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn fixture(name: &str, count: usize) -> (PgPool, Uuid, Uuid, Uuid) {
    let pool = prepare_current_core_fixture(name).await;
    let (edition, level) = publication_fixture(&pool, name).await;
    let released = curriculum::get(&pool, edition).await.unwrap();
    let opened = publications::open_draft(
        &pool,
        edition,
        OpenCurriculumDraftRequest {
            row_version: released.row_version,
        },
    )
    .await
    .unwrap();
    let token = opened.draft_id.unwrap();
    let source = publications::read_programs(&pool, level, &CurriculumViewQuery::default())
        .await
        .unwrap()
        .remove(0);
    for index in 1..count {
        curriculum::copy_program(
            &pool,
            level,
            CopyStudyProgramRequest {
                draft_id: token,
                source_program_id: source.id,
                source_row_version: source.row_version,
                destination_row_version: curriculum::get_level(&pool, level)
                    .await
                    .unwrap()
                    .row_version,
                name_th: Some(format!("Plan {}", count - index)),
            },
        )
        .await
        .unwrap();
    }
    (pool, edition, level, token)
}

async fn reorder(
    pool: &PgPool,
    level: Uuid,
    token: Uuid,
    version: i64,
    ids: Vec<Uuid>,
) -> Result<StudyProgramOrderResult, school_errors::AppError> {
    curriculum::replace_program_order(
        pool,
        level,
        ReplaceStudyProgramOrderRequest {
            draft_id: token,
            row_version: version,
            study_program_ids: ids,
        },
    )
    .await
}
fn ids(programs: &[school_academic_core::models::StudyProgram]) -> Vec<Uuid> {
    programs.iter().map(|program| program.id).collect()
}
async fn program_evidence(pool: &PgPool, level: Uuid) -> Value {
    sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM study_programs p WHERE curriculum_level_id=$1").bind(level).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn curriculum_program_order_migration_preserves_133_graph_history_and_view_grants() {
    let pool = prepare_core_fixture_through("program_order_134_upgrade", 133).await;
    let (edition, level) = historical_publication_fixture(&pool, "ORDER-HISTORY").await;
    let released = curriculum::get(&pool, edition).await.unwrap();
    let publication = released.current_publication_id.unwrap();
    let opened = publications::open_draft(
        &pool,
        edition,
        OpenCurriculumDraftRequest {
            row_version: released.row_version,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE study_programs SET name_th='Deliberate draft difference',row_version=row_version+1,updated_at=now() WHERE curriculum_level_id=$1").bind(level).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status) VALUES(gen_random_uuid(),$1,'AAA','Second draft plan','draft')").bind(level).execute(&pool).await.unwrap();
    let draft_before = historical_workspace_evidence(&pool, level, None).await;
    let publication_before = historical_workspace_evidence(&pool, level, Some(publication)).await;
    let actual = publication_actual_evidence(&pool).await;
    let history = serde_json::to_value(
        publications::list_publications(&pool, edition)
            .await
            .unwrap(),
    )
    .unwrap();
    sqlx::query("GRANT SELECT ON published_study_programs TO PUBLIC")
        .execute(&pool)
        .await
        .unwrap();
    let view:(i64,Option<String>)=sqlx::query_as("SELECT oid::bigint,relacl::text FROM pg_class WHERE oid='published_study_programs'::regclass").fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 134).await.unwrap();
    apply_migrations_through(&pool, 134).await.unwrap();
    assert_eq!(
        draft_before,
        historical_workspace_evidence(&pool, level, None).await
    );
    assert_eq!(
        publication_before,
        historical_workspace_evidence(&pool, level, Some(publication)).await
    );
    assert_eq!(actual, publication_actual_evidence(&pool).await);
    assert_eq!(
        history,
        serde_json::to_value(
            publications::list_publications(&pool, edition)
                .await
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(view,sqlx::query_as::<_,(i64,Option<String>)>("SELECT oid::bigint,relacl::text FROM pg_class WHERE oid='published_study_programs'::regclass").fetch_one(&pool).await.unwrap());
    let zeros:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM study_programs WHERE display_order<>0) AND NOT EXISTS(SELECT 1 FROM curriculum_publication_programs WHERE display_order<>0)").fetch_one(&pool).await.unwrap();
    assert!(zeros);
    let workspace = curriculum_structure::get_workspace(&pool, level)
        .await
        .unwrap();
    assert_eq!(workspace.programs[0].code, "AAA");
    assert!(workspace
        .programs
        .iter()
        .all(|program| program.display_order == 0));
    let before = program_evidence(&pool, level).await;
    let result = reorder(
        &pool,
        level,
        opened.draft_id.unwrap(),
        workspace.row_version,
        ids(&workspace.programs),
    )
    .await
    .unwrap();
    assert_eq!(result.row_version, workspace.row_version);
    assert_eq!(before, program_evidence(&pool, level).await);
    assert!(sqlx::query(
        "UPDATE curriculum_publication_programs SET display_order=9 WHERE publication_id=$1"
    )
    .bind(publication)
    .execute(&pool)
    .await
    .is_err());
    assert!(
        sqlx::query("UPDATE study_programs SET display_order=-1 WHERE curriculum_level_id=$1")
            .bind(level)
            .execute(&pool)
            .await
            .is_err()
    );
    let audits:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE event_code='curriculum.program_order_added.reconciled' AND payload->>'passed'='true'").fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 1);
}

#[tokio::test]
async fn curriculum_program_order_clean_baseline_and_retry() {
    let pool = school_test_db::create_named_test_pool("program_order_clean_baseline").await;
    apply_migrations_through(&pool, 134).await.unwrap();
    apply_migrations_through(&pool, 134).await.unwrap();
    assert!(
        sqlx::query("SELECT display_order FROM published_study_programs LIMIT 1")
            .fetch_optional(&pool)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn curriculum_program_order_noop_changed_rows_archived_guards_and_append() {
    let (pool, _, level, token) = fixture("program_order_noop_archived", 3).await;
    let programs = curriculum::list_programs(&pool, level).await.unwrap();
    assert_eq!(
        programs.iter().map(|p| p.display_order).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    sqlx::query("UPDATE study_programs SET status='archived' WHERE id=$1")
        .bind(programs[0].id)
        .execute(&pool)
        .await
        .unwrap();
    let before = curriculum::list_programs(&pool, level).await.unwrap();
    let level_before = curriculum::get_level(&pool, level).await.unwrap();
    let raw_before = program_evidence(&pool, level).await;
    let noop = reorder(&pool, level, token, level_before.row_version, ids(&before))
        .await
        .unwrap();
    assert_eq!(noop.row_version, level_before.row_version);
    assert_eq!(raw_before, program_evidence(&pool, level).await);
    assert_eq!(
        noop.previous_program_row_versions
            .iter()
            .map(|p| (p.id, p.row_version))
            .collect::<Vec<_>>(),
        before
            .iter()
            .map(|p| (p.id, p.row_version))
            .collect::<Vec<_>>()
    );
    let mut reversed = ids(&before);
    reversed.reverse();
    let changed = reorder(
        &pool,
        level,
        token,
        level_before.row_version,
        reversed.clone(),
    )
    .await
    .unwrap();
    assert_eq!(ids(&changed.programs), reversed);
    assert_eq!(changed.row_version, level_before.row_version + 1);
    assert_eq!(
        changed
            .previous_program_row_versions
            .iter()
            .map(|p| (p.id, p.row_version))
            .collect::<Vec<_>>(),
        before
            .iter()
            .map(|p| (p.id, p.row_version))
            .collect::<Vec<_>>()
    );
    for old in &before {
        let new = changed.programs.iter().find(|p| p.id == old.id).unwrap();
        let mut old_business = serde_json::to_value(old).unwrap();
        let mut new_business = serde_json::to_value(new).unwrap();
        for key in ["displayOrder", "rowVersion", "updatedAt"] {
            old_business.as_object_mut().unwrap().remove(key);
            new_business.as_object_mut().unwrap().remove(key);
        }
        assert_eq!(old_business, new_business);
        assert_eq!(
            new.row_version,
            old.row_version + i64::from(new.display_order != old.display_order)
        );
        if new.display_order == old.display_order {
            assert_eq!(new.updated_at, old.updated_at);
        }
    }
    let archived = changed
        .programs
        .iter()
        .find(|p| p.id == before[0].id)
        .unwrap();
    assert!(matches!(
        curriculum::update_program(
            &pool,
            archived.id,
            UpdateStudyProgramRequest {
                draft_id: token,
                name_th: "Forbidden rename".into(),
                row_version: archived.row_version
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let added = curriculum::create_program(
        &pool,
        level,
        CreateStudyProgramRequest {
            draft_id: token,
            name_th: "Appended".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(added.display_order, 4);
    let source = publications::read_programs(&pool, level, &CurriculumViewQuery::default())
        .await
        .unwrap()
        .remove(0);
    let copied = curriculum::copy_program(
        &pool,
        level,
        CopyStudyProgramRequest {
            draft_id: token,
            source_program_id: source.id,
            source_row_version: source.row_version,
            destination_row_version: changed.row_version,
            name_th: Some("Copied last".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(copied.display_order, 5);
}

#[tokio::test]
async fn curriculum_program_order_rejects_invalid_stale_foreign_and_oversized_sets() {
    let (pool, edition, level, token) = fixture("program_order_invalid", 2).await;
    let programs = curriculum::list_programs(&pool, level).await.unwrap();
    let version = curriculum::get_level(&pool, level)
        .await
        .unwrap()
        .row_version;
    let before = program_evidence(&pool, level).await;
    for requested in [
        vec![],
        vec![programs[0].id],
        vec![programs[0].id, Uuid::new_v4()],
    ] {
        assert!(matches!(
            reorder(&pool, level, token, version, requested).await,
            Err(school_errors::AppError::Conflict(_))
        ));
        assert_eq!(before, program_evidence(&pool, level).await);
    }
    assert!(matches!(
        reorder(&pool, level, token, version, vec![programs[0].id; 2]).await,
        Err(school_errors::AppError::ValidationError(_))
    ));
    let too_large = (0..2001).map(|_| Uuid::new_v4()).collect();
    assert!(
        matches!(reorder(&pool,level,token,version,too_large).await,Err(school_errors::AppError::ValidationError(message)) if message.contains("2,000"))
    );
    let grade = curriculum_structure::get_workspace(&pool, level)
        .await
        .unwrap()
        .grade_levels[0]
        .id;
    let empty = curriculum::create_level(
        &pool,
        edition,
        CreateCurriculumLevelRequest {
            draft_id: token,
            name_th: "Empty".into(),
            grade_level_ids: vec![grade],
            description: None,
        },
    )
    .await
    .unwrap();
    let noop = reorder(&pool, empty.id, token, empty.row_version, vec![])
        .await
        .unwrap();
    assert_eq!(noop.row_version, empty.row_version);
    assert!(noop.programs.is_empty());
    assert!(noop.previous_program_row_versions.is_empty());
    let foreign = curriculum::create_program(
        &pool,
        empty.id,
        CreateStudyProgramRequest {
            draft_id: token,
            name_th: "Foreign level".into(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        reorder(
            &pool,
            level,
            token,
            version,
            vec![programs[0].id, foreign.id]
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert!(matches!(
        reorder(&pool, level, Uuid::new_v4(), version, ids(&programs)).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let mut reversed = ids(&programs);
    reversed.reverse();
    let moved = reorder(&pool, level, token, version, reversed)
        .await
        .unwrap();
    assert!(matches!(
        reorder(&pool, level, token, version, ids(&programs)).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let other = school_test_db::create_named_test_pool("program_order_other_tenant").await;
    apply_migrations_through(&other, 134).await.unwrap();
    assert!(matches!(
        reorder(&other, level, token, moved.row_version, ids(&programs)).await,
        Err(school_errors::AppError::NotFound(_))
    ));
    // An oversized legacy level is refused, not truncated into a falsely complete set.
    sqlx::query("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status) SELECT gen_random_uuid(),$1,'EXCESS-'||n,'Excess','draft' FROM generate_series(1,1999) n").bind(level).execute(&pool).await.unwrap();
    let subset:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM study_programs WHERE curriculum_level_id=$1 ORDER BY display_order,code,id LIMIT 2000").bind(level).fetch_all(&pool).await.unwrap();
    let raw = program_evidence(&pool, level).await;
    assert!(
        matches!(reorder(&pool,level,token,moved.row_version,subset).await,Err(school_errors::AppError::ValidationError(message)) if message.contains("2,000"))
    );
    assert_eq!(raw, program_evidence(&pool, level).await);
}

#[tokio::test]
async fn curriculum_program_order_reports_actual_before_revisions_after_structure_change() {
    let (pool, _, level, token) = fixture("program_order_structure_transition", 2).await;
    let loaded = curriculum_structure::get_workspace(&pool, level)
        .await
        .unwrap();
    let program = &loaded.programs[0];
    let requirements = loaded
        .requirements
        .iter()
        .filter(|row| row.study_program_id == program.id)
        .map(|row| CurriculumStructureRequirementInput {
            resource_kind: row.resource_kind,
            catalog_version_id: row.catalog_version_id,
            grade_level_id: row.grade_level.id,
            term_slot_id: row.term_slot_id,
            requirement_kind: row.requirement_kind,
            display_order: row.display_order + 50,
        })
        .collect();
    let changed = curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        ReplaceCurriculumStructureRequest {
            draft_id: token,
            row_version: program.row_version,
            requirements,
        },
    )
    .await
    .unwrap();
    let changed_version = changed
        .programs
        .iter()
        .find(|row| row.id == program.id)
        .unwrap()
        .row_version;
    assert!(changed_version > program.row_version);
    assert_eq!(loaded.row_version, changed.row_version);
    let mut requested = ids(&loaded.programs);
    requested.reverse();
    let result = reorder(&pool, level, token, loaded.row_version, requested)
        .await
        .unwrap();
    let prior = result
        .previous_program_row_versions
        .iter()
        .find(|row| row.id == program.id)
        .unwrap();
    assert_eq!(prior.row_version, changed_version);
    assert_ne!(
        prior.row_version, program.row_version,
        "staged requirement baseline must not be silently rebased"
    );
    let after = curriculum_structure::get_workspace(&pool, level)
        .await
        .unwrap();
    assert!(after
        .requirements
        .iter()
        .filter(|row| row.study_program_id == program.id)
        .all(|row| row.display_order >= 51));
    assert!(matches!(
        curriculum_structure::replace_program_structure(
            &pool,
            program.id,
            ReplaceCurriculumStructureRequest {
                draft_id: token,
                row_version: program.row_version,
                requirements: vec![]
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn curriculum_program_order_publish_history_options_and_discard_pin_order() {
    let (pool, edition, level, token) = fixture("program_order_history", 3).await;
    let original = curriculum::get(&pool, edition).await.unwrap();
    let old_publication = original.current_publication_id.unwrap();
    let old = publications::read_workspace(
        &pool,
        level,
        &CurriculumViewQuery {
            publication_id: Some(old_publication),
            draft_id: None,
        },
    )
    .await
    .unwrap();
    let programs = curriculum::list_programs(&pool, level).await.unwrap();
    let mut requested = ids(&programs);
    requested.reverse();
    reorder(
        &pool,
        level,
        token,
        curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .row_version,
        requested.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(&old).unwrap(),
        serde_json::to_value(
            publications::read_workspace(
                &pool,
                level,
                &CurriculumViewQuery {
                    publication_id: Some(old_publication),
                    draft_id: None
                }
            )
            .await
            .unwrap()
        )
        .unwrap()
    );
    let published = curriculum::publish(
        &pool,
        edition,
        PublishCurriculumRequest {
            draft_id: token,
            row_version: original.row_version,
            change_note: "Publish custom order".into(),
        },
        fixture_actor(&pool).await,
    )
    .await
    .unwrap();
    let current = published.current_publication_id.unwrap();
    assert_eq!(
        ids(
            &publications::read_programs(&pool, level, &CurriculumViewQuery::default())
                .await
                .unwrap()
        ),
        requested
    );
    assert_eq!(
        ids(&curriculum::list_programs(&pool, level).await.unwrap()),
        requested
    );
    let options = curriculum::list_study_program_options_for_year(
        &pool,
        CURRENT_YEAR_ID,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let options = options
        .iter()
        .filter(|row| row.curriculum_level_id == level)
        .collect::<Vec<_>>();
    assert_eq!(
        options.iter().map(|row| row.id).collect::<Vec<_>>(),
        requested
    );
    assert_eq!(
        options
            .iter()
            .map(|row| row.display_order)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    let term: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 ORDER BY sequence_no,id LIMIT 1",
    )
    .bind(CURRENT_YEAR_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let delivery_options =
        school_academic_delivery::services::workspaces::delivery_management_options(
            &pool, term, &filter, &filter,
        )
        .await
        .unwrap();
    assert_eq!(
        delivery_options
            .study_programs
            .iter()
            .filter(|program| program.curriculum_level_id == level)
            .map(|program| program.id)
            .collect::<Vec<_>>(),
        requested
    );
    let reader = ActorContext {
        user_id: fixture_actor(&pool).await,
        permissions: vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL.into()],
    };
    let policy_options =
        school_academic_lifecycle::services::get_promotion_policy_options(&pool, &reader)
            .await
            .unwrap();
    assert_eq!(
        policy_options
            .programs
            .iter()
            .filter(|program| program.curriculum_level_id == level)
            .map(|program| program.id)
            .collect::<Vec<_>>(),
        requested
    );
    let history = serde_json::to_value(
        publications::history(&pool, edition, current)
            .await
            .unwrap(),
    )
    .unwrap();
    let changed = publications::history(&pool, edition, current)
        .await
        .unwrap();
    assert!(changed
        .changes
        .iter()
        .any(|row| row.resource_kind == "program" && row.before.is_some() && row.after.is_some()));
    let opened = publications::open_draft(
        &pool,
        edition,
        OpenCurriculumDraftRequest {
            row_version: published.row_version,
        },
    )
    .await
    .unwrap();
    let next_token = opened.draft_id.unwrap();
    let next_level_version = curriculum::get_level(&pool, level)
        .await
        .unwrap()
        .row_version;
    let preview = publications::preview_discard(&pool, edition, next_token)
        .await
        .unwrap();
    let restored = ids(&programs);
    reorder(&pool, level, next_token, next_level_version, restored)
        .await
        .unwrap();
    assert_eq!(
        ids(&publications::read_programs(
            &pool,
            level,
            &CurriculumViewQuery {
                publication_id: Some(current),
                draft_id: None
            }
        )
        .await
        .unwrap()),
        requested
    );
    assert!(matches!(
        publications::discard_draft(
            &pool,
            edition,
            school_academic_core::models::DiscardCurriculumDraftRequest {
                draft_id: next_token,
                row_version: preview.row_version,
                content_hash: preview.content_hash
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let preview = publications::preview_discard(&pool, edition, next_token)
        .await
        .unwrap();
    publications::discard_draft(
        &pool,
        edition,
        school_academic_core::models::DiscardCurriculumDraftRequest {
            draft_id: next_token,
            row_version: preview.row_version,
            content_hash: preview.content_hash,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        ids(&curriculum::list_programs(&pool, level).await.unwrap()),
        requested
    );
    assert_eq!(
        history,
        serde_json::to_value(
            publications::history(&pool, edition, current)
                .await
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        publications::list_publications(&pool, edition)
            .await
            .unwrap()
            .len(),
        2
    );
    assert!(matches!(
        reorder(
            &pool,
            level,
            next_token,
            curriculum::get_level(&pool, level)
                .await
                .unwrap()
                .row_version,
            ids(&programs)
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn curriculum_program_order_concurrent_reorders_have_one_winner() {
    let (pool, _, level, token) = fixture("program_order_reorder_race", 3).await;
    let programs = curriculum::list_programs(&pool, level).await.unwrap();
    let version = curriculum::get_level(&pool, level)
        .await
        .unwrap()
        .row_version;
    let first = vec![programs[1].id, programs[0].id, programs[2].id];
    let second = vec![programs[2].id, programs[1].id, programs[0].id];
    let (a, b) = tokio::join!(
        reorder(&pool, level, token, version, first),
        reorder(&pool, level, token, version, second)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let (winner, loser) = if let Ok(value) = a {
        (value, b)
    } else {
        (b.unwrap(), a)
    };
    assert!(matches!(loser, Err(school_errors::AppError::Conflict(_))));
    assert_eq!(winner.row_version, version + 1);
    assert_eq!(
        ids(&curriculum::list_programs(&pool, level).await.unwrap()),
        ids(&winner.programs)
    );
}

async fn wait_for_blocked(
    boundary: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pid: i32,
    expected: i64,
) {
    for _ in 0..200 {
        sqlx::query("SELECT pg_stat_clear_snapshot()")
            .execute(&mut **boundary)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar(
            "WITH RECURSIVE blocked AS (SELECT pid FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) UNION SELECT activity.pid FROM pg_stat_activity activity JOIN blocked parent ON parent.pid=ANY(pg_blocking_pids(activity.pid))) SELECT count(*) FROM blocked",
        )
        .bind(pid)
        .fetch_one(&mut **boundary)
        .await
        .unwrap();
        if count >= expected {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("expected {expected} blocked workers");
}

#[tokio::test]
async fn curriculum_program_order_concurrent_create_invalidates_complete_set() {
    let (pool, _, level, token) = fixture("program_order_create_race", 2).await;
    let programs = curriculum::list_programs(&pool, level).await.unwrap();
    let version = curriculum::get_level(&pool, level)
        .await
        .unwrap()
        .row_version;
    let mut boundary = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *boundary)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM curriculum_levels WHERE id=$1 FOR UPDATE")
        .bind(level)
        .fetch_one(&mut *boundary)
        .await
        .unwrap();
    let create_pool = pool.clone();
    let create = tokio::spawn(async move {
        curriculum::create_program(
            &create_pool,
            level,
            CreateStudyProgramRequest {
                draft_id: token,
                name_th: "Concurrent append".into(),
            },
        )
        .await
    });
    wait_for_blocked(&mut boundary, pid, 1).await;
    let order_pool = pool.clone();
    let requested = ids(&programs);
    let order =
        tokio::spawn(async move { reorder(&order_pool, level, token, version, requested).await });
    wait_for_blocked(&mut boundary, pid, 2).await;
    boundary.commit().await.unwrap();
    let added = create.await.unwrap().unwrap();
    assert_eq!(added.display_order, 3);
    assert!(matches!(
        order.await.unwrap(),
        Err(school_errors::AppError::Conflict(_))
    ));
    assert_eq!(
        curriculum::list_programs(&pool, level).await.unwrap().len(),
        3
    );
    assert_eq!(
        curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .row_version,
        version
    );
}

#[tokio::test]
async fn curriculum_program_order_publication_and_discard_close_token_before_waiting_reorder() {
    for publish in [true, false] {
        let (pool, edition, level, token) = fixture(
            if publish {
                "program_order_publish_race"
            } else {
                "program_order_discard_race"
            },
            2,
        )
        .await;
        let programs = curriculum::list_programs(&pool, level).await.unwrap();
        let revision = curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .row_version;
        let edition_revision = curriculum::get(&pool, edition).await.unwrap().row_version;
        let preview = publications::preview_discard(&pool, edition, token)
            .await
            .unwrap();
        let actor = fixture_actor(&pool).await;
        let mut boundary = pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *boundary)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM curriculum_editions WHERE id=$1 FOR UPDATE")
            .bind(edition)
            .fetch_one(&mut *boundary)
            .await
            .unwrap();
        let winner_pool = pool.clone();
        let winner = tokio::spawn(async move {
            if publish {
                curriculum::publish(
                    &winner_pool,
                    edition,
                    PublishCurriculumRequest {
                        draft_id: token,
                        row_version: edition_revision,
                        change_note: "Concurrent order publication".into(),
                    },
                    actor,
                )
                .await
            } else {
                publications::discard_draft(
                    &winner_pool,
                    edition,
                    school_academic_core::models::DiscardCurriculumDraftRequest {
                        draft_id: token,
                        row_version: preview.row_version,
                        content_hash: preview.content_hash,
                    },
                )
                .await
            }
        });
        wait_for_blocked(&mut boundary, pid, 1).await;
        let order_pool = pool.clone();
        let mut requested = ids(&programs);
        requested.reverse();
        let order =
            tokio::spawn(
                async move { reorder(&order_pool, level, token, revision, requested).await },
            );
        wait_for_blocked(&mut boundary, pid, 2).await;
        boundary.commit().await.unwrap();
        assert!(winner.await.unwrap().unwrap().draft_id.is_none());
        assert!(matches!(
            order.await.unwrap(),
            Err(school_errors::AppError::Conflict(_))
        ));
        let after = publications::read_programs(&pool, level, &CurriculumViewQuery::default())
            .await
            .unwrap();
        assert_eq!(after[0].id, programs[0].id);
        assert_eq!(after[0].display_order, programs[0].display_order);
        assert_eq!(after.len(), if publish { 2 } else { 1 });
    }
}

#[tokio::test]
async fn curriculum_program_order_history_ignores_stationary_legacy_zero_normalization() {
    let pool = prepare_core_fixture_through("program_order_semantic_history", 133).await;
    let (edition, level) = historical_publication_fixture(&pool, "SEMANTIC-ORDER").await;
    let first = curriculum::get(&pool, edition).await.unwrap();
    publications::open_draft(
        &pool,
        edition,
        OpenCurriculumDraftRequest {
            row_version: first.row_version,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE study_programs SET code='AAA',name_th='Plan A' WHERE curriculum_level_id=$1",
    )
    .bind(level)
    .execute(&pool)
    .await
    .unwrap();
    for (code, name) in [("BBB", "Plan B"), ("CCC", "Plan C")] {
        let program:Uuid=sqlx::query_scalar("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status) VALUES(gen_random_uuid(),$1,$2,$3,'published') RETURNING id").bind(level).bind(code).bind(name).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO curriculum_course_requirements(id,curriculum_level_id,study_program_id,subject_version_id,grade_level_id,term_slot_id,requirement_kind,display_order) SELECT gen_random_uuid(),curriculum_level_id,$2,subject_version_id,grade_level_id,term_slot_id,requirement_kind,display_order FROM curriculum_course_requirements WHERE curriculum_level_id=$1 ORDER BY id LIMIT 1").bind(level).bind(program).execute(&pool).await.unwrap();
    }
    sqlx::query("SELECT capture_curriculum_publication($1,$2,'Legacy zero ordering')")
        .bind(edition)
        .bind(fixture_actor(&pool).await)
        .execute(&pool)
        .await
        .unwrap();
    apply_migrations_through(&pool, 134).await.unwrap();
    let old = curriculum::get(&pool, edition).await.unwrap();
    let opened = publications::open_draft(
        &pool,
        edition,
        OpenCurriculumDraftRequest {
            row_version: old.row_version,
        },
    )
    .await
    .unwrap();
    let programs = curriculum::list_programs(&pool, level).await.unwrap();
    assert_eq!(
        programs.iter().map(|p| p.display_order).collect::<Vec<_>>(),
        vec![0, 0, 0]
    );
    reorder(
        &pool,
        level,
        opened.draft_id.unwrap(),
        curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .row_version,
        vec![programs[1].id, programs[0].id, programs[2].id],
    )
    .await
    .unwrap();
    let published = curriculum::publish(
        &pool,
        edition,
        PublishCurriculumRequest {
            draft_id: opened.draft_id.unwrap(),
            row_version: opened.row_version,
            change_note: "Swap A and B".into(),
        },
        fixture_actor(&pool).await,
    )
    .await
    .unwrap();
    let history = publications::history(&pool, edition, published.current_publication_id.unwrap())
        .await
        .unwrap();
    let changes = history
        .changes
        .iter()
        .filter(|change| change.resource_kind == "program")
        .collect::<Vec<_>>();
    assert_eq!(changes.len(), 2);
    assert!(changes
        .iter()
        .all(|change| change.name != "Plan C" && change.before != change.after));
}
