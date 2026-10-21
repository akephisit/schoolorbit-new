use super::*;
use school_academic_core::models::UpdateCurriculumLevelRequest;
use school_academic_core::models::{
    CurriculumCreditCriteriaComparison as Comparison, CurriculumCreditCriteriaScope as Scope,
    CurriculumCreditCriterion, CurriculumViewQuery, DiscardCurriculumDraftRequest,
    OpenCurriculumDraftRequest, ReplaceCurriculumCreditCriteriaRequest,
};
use school_academic_core::services::{
    curriculum_credit_criteria as criteria, curriculum_publications as publications,
};
use serde_json::Value;

async fn fixture(name: &str) -> (PgPool, Uuid, Uuid, Uuid) {
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
    (pool, edition, level, opened.draft_id.unwrap())
}
async fn rule(pool: &PgPool, level: Uuid, target: &str) -> CurriculumCreditCriterion {
    CurriculumCreditCriterion {
        scope: Scope::Annual,
        comparison: Comparison::Equal,
        target_credits: target.into(),
        enabled: true,
        grade_level_ids: curriculum::get_level(pool, level)
            .await
            .unwrap()
            .grade_level_ids,
    }
}
async fn replace(
    pool: &PgPool,
    level: Uuid,
    token: Uuid,
    version: i64,
    rules: Vec<CurriculumCreditCriterion>,
) -> Result<school_academic_core::models::CurriculumCreditCriteriaResult, school_errors::AppError> {
    criteria::replace(
        pool,
        level,
        ReplaceCurriculumCreditCriteriaRequest {
            draft_id: token,
            row_version: version,
            criteria: rules,
        },
    )
    .await
}
async fn level_json(pool: &PgPool, level: Uuid) -> Value {
    sqlx::query_scalar("SELECT to_jsonb(l) FROM curriculum_levels l WHERE id=$1")
        .bind(level)
        .fetch_one(pool)
        .await
        .unwrap()
}
async fn graph(pool: &PgPool) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('levels',(SELECT jsonb_agg(to_jsonb(r)-'credit_criteria'-'credit_criteria_row_version' ORDER BY id) FROM curriculum_levels r),'history',(SELECT jsonb_agg(to_jsonb(r)-'credit_criteria'-'credit_criteria_row_version' ORDER BY publication_id,id) FROM curriculum_publication_levels r),'programs',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM study_programs r),'sources',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_requirement_sources r),'requirements',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM curriculum_course_requirements r))").fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn curriculum_credit_criteria_migration_preserves_populated_134_and_view_grants() {
    let pool = prepare_core_fixture_through("credit_criteria_upgrade", 134).await;
    let (edition, level) =
        curriculum_program_order_tests::historical_publication_fixture(&pool, "CRITERIA-LEGACY")
            .await;
    let old = curriculum::get(&pool, edition).await.unwrap();
    publications::open_draft(
        &pool,
        edition,
        OpenCurriculumDraftRequest {
            row_version: old.row_version,
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE curriculum_levels SET description='Deliberate draft-only difference',row_version=row_version+1,updated_at=now() WHERE id=$1").bind(level).execute(&pool).await.unwrap();
    let before = graph(&pool).await;
    let actual = publication_actual_evidence(&pool).await;
    sqlx::query("GRANT SELECT ON published_curriculum_levels TO PUBLIC")
        .execute(&pool)
        .await
        .unwrap();
    let view:(i64,Option<String>)=sqlx::query_as("SELECT oid::bigint,relacl::text FROM pg_class WHERE oid='published_curriculum_levels'::regclass").fetch_one(&pool).await.unwrap();
    apply_migrations_through(&pool, 135).await.unwrap();
    apply_migrations_through(&pool, 135).await.unwrap();
    assert_eq!(before, graph(&pool).await);
    assert_eq!(actual, publication_actual_evidence(&pool).await);
    assert_eq!(view,sqlx::query_as::<_,(i64,Option<String>)>("SELECT oid::bigint,relacl::text FROM pg_class WHERE oid='published_curriculum_levels'::regclass").fetch_one(&pool).await.unwrap());
    let empty:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM curriculum_levels WHERE credit_criteria<>'[]'::jsonb OR credit_criteria_row_version<>1) AND NOT EXISTS(SELECT 1 FROM curriculum_publication_levels WHERE credit_criteria<>'[]'::jsonb OR credit_criteria_row_version<>1)").fetch_one(&pool).await.unwrap();
    assert!(empty);
    let audits:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE event_code='curriculum.credit_criteria_added.reconciled' AND payload->>'passed'='true'").fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 1);
    assert!(sqlx::query(
        "UPDATE curriculum_publication_levels SET credit_criteria='[]' WHERE id=$1"
    )
    .bind(level)
    .execute(&pool)
    .await
    .is_err());
    let published = publications::read_level(
        &pool,
        level,
        &CurriculumViewQuery {
            publication_id: old.current_publication_id,
            draft_id: None,
        },
    )
    .await
    .unwrap();
    assert!(published.credit_criteria.is_empty());
    assert_ne!(
        published.description,
        curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .description
    );
}

#[tokio::test]
async fn curriculum_credit_criteria_clean_baseline_and_retry() {
    let pool = school_test_db::create_named_test_pool("credit_criteria_clean").await;
    apply_migrations_through(&pool, 135).await.unwrap();
    apply_migrations_through(&pool, 135).await.unwrap();
    assert!(sqlx::query("SELECT credit_criteria,credit_criteria_row_version FROM published_curriculum_levels LIMIT 1").fetch_optional(&pool).await.is_ok());
}

#[tokio::test]
async fn curriculum_credit_criteria_noop_normalizes_and_keeps_unrelated_baselines() {
    let (pool, _, level, token) = fixture("credit_criteria_noop").await;
    let before = level_json(&pool, level).await;
    let program_before =
        curriculum_program_order_tests::historical_workspace_evidence(&pool, level, None).await;
    let empty = replace(&pool, level, token, 1, vec![]).await.unwrap();
    assert_eq!(empty.row_version, 1);
    assert_eq!(before, level_json(&pool, level).await);
    let saved = replace(
        &pool,
        level,
        token,
        1,
        vec![rule(&pool, level, "1.00").await],
    )
    .await
    .unwrap();
    assert_eq!(saved.row_version, 2);
    assert_eq!(saved.criteria[0].target_credits, "1");
    let after = level_json(&pool, level).await;
    for value in ["1", "1.0", "1.00"] {
        let noop = replace(
            &pool,
            level,
            token,
            2,
            vec![rule(&pool, level, value).await],
        )
        .await
        .unwrap();
        assert_eq!(noop.row_version, 2);
        assert_eq!(after, level_json(&pool, level).await);
    }
    let mut expected = before;
    let mut actual = after;
    for key in [
        "credit_criteria",
        "credit_criteria_row_version",
        "updated_at",
    ] {
        expected.as_object_mut().unwrap().remove(key);
        actual.as_object_mut().unwrap().remove(key);
    }
    assert_eq!(expected, actual);
    assert_eq!(
        program_before,
        curriculum_program_order_tests::historical_workspace_evidence(&pool, level, None).await
    );
    assert!(matches!(
        replace(&pool, level, token, 1, vec![]).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert_eq!(
        replace(&pool, level, token, 2, vec![rule(&pool, level, "0").await])
            .await
            .unwrap()
            .criteria[0]
            .target_credits,
        "0"
    );
    let annual = rule(&pool, level, "0").await;
    let mut whole_level = rule(&pool, level, "41").await;
    whole_level.scope = Scope::Level;
    let paired = replace(
        &pool,
        level,
        token,
        3,
        vec![whole_level.clone(), annual.clone()],
    )
    .await
    .unwrap();
    assert_eq!(paired.row_version, 4);
    assert_eq!(paired.criteria, vec![annual, whole_level]);
    assert_eq!(
        curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .credit_criteria,
        paired.criteria
    );
}

#[tokio::test]
async fn curriculum_credit_criteria_database_shape_input_and_closed_token_are_guarded() {
    let (pool, edition, level, token) = fixture("credit_criteria_validation").await;
    let good = serde_json::json!([rule(&pool, level, "22").await]);
    for bad in [
        serde_json::json!({}),
        serde_json::json!([{}, {}]),
        serde_json::json!([good[0], good[0]]),
        serde_json::json!([{"scope":"annual","comparison":"equal","targetCredits":"10000","enabled":true,"gradeLevelIds":[Uuid::new_v4()]}]),
    ] {
        assert!(
            sqlx::query("UPDATE curriculum_levels SET credit_criteria=$2 WHERE id=$1")
                .bind(level)
                .bind(sqlx::types::Json(bad))
                .execute(&pool)
                .await
                .is_err()
        );
    }
    assert!(replace(
        &pool,
        level,
        token,
        1,
        vec![rule(&pool, level, "10000").await]
    )
    .await
    .is_err());
    let mut bad = rule(&pool, level, "22").await;
    bad.grade_level_ids = vec![Uuid::new_v4()];
    assert!(matches!(
        replace(&pool, level, token, 1, vec![bad]).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    assert!(matches!(
        replace(&pool, level, Uuid::nil(), 1, vec![]).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let other = school_test_db::create_named_test_pool("credit_criteria_other_tenant").await;
    apply_migrations_through(&other, 135).await.unwrap();
    assert!(matches!(
        replace(&other, level, token, 1, vec![]).await,
        Err(school_errors::AppError::NotFound(_))
    ));
    let preview = publications::preview_discard(&pool, edition, token)
        .await
        .unwrap();
    publications::discard_draft(
        &pool,
        edition,
        DiscardCurriculumDraftRequest {
            draft_id: token,
            row_version: preview.row_version,
            content_hash: preview.content_hash,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        replace(&pool, level, token, 1, vec![]).await,
        Err(school_errors::AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn curriculum_credit_criteria_concurrent_replacements_have_exactly_one_winner() {
    let (pool, _, level, token) = fixture("credit_criteria_race").await;
    let a = rule(&pool, level, "22").await;
    let b = rule(&pool, level, "23").await;
    let (left, right) = tokio::join!(
        replace(&pool, level, token, 1, vec![a]),
        replace(&pool, level, token, 1, vec![b])
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let (winner, loser) = if let Ok(result) = left {
        (result, right)
    } else {
        (right.unwrap(), left)
    };
    assert!(matches!(loser, Err(school_errors::AppError::Conflict(_))));
    assert_eq!(winner.row_version, 2);
    assert_eq!(
        curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .credit_criteria,
        winner.criteria
    );
}

#[tokio::test]
async fn curriculum_credit_criteria_grade_change_requires_explicit_review_without_rebase() {
    let (pool, _, level, token) = fixture("credit_criteria_grades").await;
    let old = curriculum::get_level(&pool, level).await.unwrap();
    let saved_rule = rule(&pool, level, "41").await;
    replace(&pool, level, token, 1, vec![saved_rule.clone()])
        .await
        .unwrap();
    let extra: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE is_active AND NOT id=ANY($1) ORDER BY id LIMIT 1",
    )
    .bind(&old.grade_level_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut grades = old.grade_level_ids.clone();
    grades.push(extra);
    let changed = curriculum::update_level(
        &pool,
        level,
        UpdateCurriculumLevelRequest {
            draft_id: token,
            name_th: old.name_th,
            grade_level_ids: grades,
            description: old.description,
            row_version: old.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(changed.credit_criteria, vec![saved_rule.clone()]);
    assert_eq!(changed.credit_criteria_row_version, 2);
    assert!(matches!(
        replace(&pool, level, token, 2, vec![saved_rule]).await,
        Err(school_errors::AppError::Conflict(_))
    ));
    let reviewed = replace(&pool, level, token, 2, vec![rule(&pool, level, "41").await])
        .await
        .unwrap();
    assert_eq!(reviewed.row_version, 3);
    assert_eq!(
        curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .row_version,
        changed.row_version
    );
}

#[tokio::test]
async fn curriculum_credit_criteria_publication_history_preview_and_discard_preserve_settings() {
    let (pool, edition, level, token) = fixture("credit_criteria_history").await;
    let old = curriculum::get(&pool, edition).await.unwrap();
    let old_publication = old.current_publication_id.unwrap();
    let preview = publications::preview_discard(&pool, edition, token)
        .await
        .unwrap();
    let mut selected = rule(&pool, level, "9999.99").await;
    selected.scope = Scope::Level;
    selected.comparison = Comparison::Minimum;
    replace(&pool, level, token, 1, vec![selected.clone()])
        .await
        .unwrap();
    assert!(matches!(
        publications::discard_draft(
            &pool,
            edition,
            DiscardCurriculumDraftRequest {
                draft_id: token,
                row_version: preview.row_version,
                content_hash: preview.content_hash
            }
        )
        .await,
        Err(school_errors::AppError::Conflict(_))
    ));
    // A deliberately unmet warning never becomes a publication/graduation guard.
    let published = curriculum::publish(
        &pool,
        edition,
        PublishCurriculumRequest {
            draft_id: token,
            row_version: old.row_version,
            change_note: "Publish reviewed warning settings".into(),
        },
        fixture_actor(&pool).await,
    )
    .await
    .unwrap();
    let current = published.current_publication_id.unwrap();
    let historical = publications::read_level(
        &pool,
        level,
        &CurriculumViewQuery {
            publication_id: Some(old_publication),
            draft_id: None,
        },
    )
    .await
    .unwrap();
    assert!(historical.credit_criteria.is_empty());
    let pinned = publications::read_level(&pool, level, &CurriculumViewQuery::default())
        .await
        .unwrap();
    assert_eq!(pinned.credit_criteria, vec![selected.clone()]);
    let history = publications::history(&pool, edition, current)
        .await
        .unwrap();
    let description = history
        .changes
        .iter()
        .find(|change| change.resource_kind == "level")
        .unwrap()
        .after
        .as_ref()
        .unwrap();
    for text in ["9999.99", "ตลอดระดับ", "อย่างน้อย", "เปิด"]
    {
        assert!(description.contains(text), "{description}");
    }
    let opened = publications::open_draft(
        &pool,
        edition,
        OpenCurriculumDraftRequest {
            row_version: published.row_version,
        },
    )
    .await
    .unwrap();
    let token = opened.draft_id.unwrap();
    selected.enabled = false;
    replace(
        &pool,
        level,
        token,
        pinned.credit_criteria_row_version,
        vec![selected],
    )
    .await
    .unwrap();
    let preview = publications::preview_discard(&pool, edition, token)
        .await
        .unwrap();
    publications::discard_draft(
        &pool,
        edition,
        DiscardCurriculumDraftRequest {
            draft_id: token,
            row_version: preview.row_version,
            content_hash: preview.content_hash,
        },
    )
    .await
    .unwrap();
    let restored = curriculum::get_level(&pool, level).await.unwrap();
    assert_eq!(restored.credit_criteria, pinned.credit_criteria);
    assert!(restored.credit_criteria_row_version > pinned.credit_criteria_row_version + 1);
    assert_eq!(
        publications::read_level(&pool, level, &CurriculumViewQuery::default())
            .await
            .unwrap()
            .credit_criteria,
        pinned.credit_criteria
    );
    assert_eq!(
        publications::list_publications(&pool, edition)
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn curriculum_credit_criteria_revision_only_changes_do_not_pollute_history() {
    let (pool, edition, level, token) = fixture("credit_criteria_semantic").await;
    replace(&pool, level, token, 1, vec![rule(&pool, level, "22").await])
        .await
        .unwrap();
    replace(&pool, level, token, 2, vec![]).await.unwrap();
    let edition_row = curriculum::get(&pool, edition).await.unwrap();
    let published = curriculum::publish(
        &pool,
        edition,
        PublishCurriculumRequest {
            draft_id: token,
            row_version: edition_row.row_version,
            change_note: "No effective criteria change".into(),
        },
        fixture_actor(&pool).await,
    )
    .await
    .unwrap();
    assert!(
        !publications::history(&pool, edition, published.current_publication_id.unwrap())
            .await
            .unwrap()
            .changes
            .iter()
            .any(|change| change.resource_kind == "level")
    );
}

#[tokio::test]
async fn curriculum_credit_criteria_copy_keeps_destination_settings() {
    let (pool, edition, level, token) = fixture("credit_criteria_copy").await;
    let desired = vec![rule(&pool, level, "22").await];
    replace(&pool, level, token, 1, desired.clone())
        .await
        .unwrap();
    let (source_edition, source_level) = publication_fixture(&pool, "CRITERIA-SOURCE").await;
    let source = curriculum::get(&pool, source_edition).await.unwrap();
    let opened = publications::open_draft(
        &pool,
        source_edition,
        OpenCurriculumDraftRequest {
            row_version: source.row_version,
        },
    )
    .await
    .unwrap();
    replace(
        &pool,
        source_level,
        opened.draft_id.unwrap(),
        1,
        vec![rule(&pool, source_level, "41").await],
    )
    .await
    .unwrap();
    curriculum::publish(
        &pool,
        source_edition,
        PublishCurriculumRequest {
            draft_id: opened.draft_id.unwrap(),
            row_version: opened.row_version,
            change_note: "Distinct source criteria".into(),
        },
        fixture_actor(&pool).await,
    )
    .await
    .unwrap();
    let program = publications::read_programs(&pool, source_level, &CurriculumViewQuery::default())
        .await
        .unwrap()
        .remove(0);
    let source_before = level_json(&pool, source_level).await;
    curriculum::copy_program(
        &pool,
        level,
        CopyStudyProgramRequest {
            draft_id: token,
            source_program_id: program.id,
            source_row_version: program.row_version,
            destination_row_version: curriculum::get_level(&pool, level)
                .await
                .unwrap()
                .row_version,
            name_th: Some("Independent copied plan".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        curriculum::get_level(&pool, level)
            .await
            .unwrap()
            .credit_criteria,
        desired
    );
    assert_eq!(source_before, level_json(&pool, source_level).await);
    assert_eq!(
        curriculum::get(&pool, edition).await.unwrap().draft_id,
        Some(token)
    );
}
