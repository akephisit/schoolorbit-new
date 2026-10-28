use super::*;
use school_academic_delivery::{
    models as dm,
    services::{change_sets, versions},
};

#[tokio::test]
async fn results_current_obligation_omitted_unused_group_does_not_block_ready_course() {
    let (pool, actor, ctx, group, unused) =
        fixture_with_unused_group("results_current_obligation_unused", true).await;
    let unused = unused.unwrap();
    // Keep this runtime closing fixture writable on the execution date without changing
    // historical published source graphs or bypassing Delivery's effective-date guard.
    let today = (chrono::Utc::now() + chrono::Duration::hours(7)).date_naive();
    let end = today + chrono::Duration::days(30);
    sqlx::query("UPDATE academic_years SET end_date=GREATEST(end_date,$2) WHERE id=$1")
        .bind(ctx.academic_year_id)
        .bind(end)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE academic_terms SET planned_end_date=GREATEST(planned_end_date,$2) WHERE id=$1",
    )
    .bind(ctx.academic_term_id)
    .bind(end)
    .execute(&pool)
    .await
    .unwrap();
    let (offering,subject):(Uuid,Uuid)=sqlx::query_as("SELECT g.learning_offering_id,d.subject_id FROM learning_groups g JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE g.id=$1").bind(group).fetch_one(&pool).await.unwrap();
    let old_source:Uuid=sqlx::query_scalar("SELECT id FROM academic_delivery_versions WHERE academic_term_id=$1 AND status='published' AND EXISTS(SELECT 1 FROM jsonb_array_elements(snapshot->'offerings') o CROSS JOIN jsonb_array_elements(o->'groups') g WHERE (g->>'id')::uuid=$2) ORDER BY effective_from DESC,id DESC LIMIT 1").bind(ctx.academic_term_id).bind(unused).fetch_one(&pool).await.expect("087 must capture the pre-cutover unused group");
    let old_bytes: String = sqlx::query_scalar(
        "SELECT to_jsonb(v)::text FROM academic_delivery_versions v WHERE id=$1",
    )
    .bind(old_source)
    .fetch_one(&pool)
    .await
    .unwrap();
    for (candidate, teacher) in course_subject_rooms(&pool, &ctx, subject)
        .await
        .into_iter()
        .filter(|(id, _)| *id != unused)
    {
        let teacher = ActorContext {
            user_id: teacher,
            permissions: actor.permissions.clone(),
        };
        prepare_phases(&pool, &teacher, &ctx, candidate).await;
        let workspace = get_course_workspace(&pool, &teacher, &ctx, candidate)
            .await
            .unwrap();
        let confirmed =
            confirm_group_results(&pool, &teacher, &ctx, candidate, confirm_input(&workspace))
                .await
                .unwrap();
        assert!(
            confirmed.blockers.is_empty(),
            "retained group must really be ready: {:?}",
            confirmed.blockers
        );
        assert!(confirmed.confirmation_is_current);
    }
    let absence:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM learning_group_students WHERE learning_group_id=$1) AND NOT EXISTS(SELECT 1 FROM learning_group_score_items WHERE learning_group_id=$1) AND NOT EXISTS(SELECT 1 FROM learning_group_phase_confirmations WHERE learning_group_id=$1) AND NOT EXISTS(SELECT 1 FROM learning_group_result_confirmations WHERE learning_group_id=$1)").bind(unused).fetch_one(&pool).await.unwrap();
    assert!(absence, "unused group has no learner or grade obligation");
    let revision = change_sets::create_change_set(
        &pool,
        actor.user_id,
        dm::CreateAcademicTermChangeSetRequest {
            academic_term_id: ctx.academic_term_id,
            reason: "Remove unused group from current scope".into(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let target = versions::get_version(&pool, revision.target_delivery_version_id)
        .await
        .unwrap();
    change_sets::remove_draft_group(
        &pool,
        actor.user_id,
        revision.id,
        unused,
        dm::RemoveDeliveryDraftResourceRequest {
            change_set_row_version: revision.row_version,
            target_delivery_version_row_version: target.row_version,
        },
    )
    .await
    .unwrap();
    let preview =
        change_sets::preview_change_set_at(&pool, revision.id, Some(revision.reference_date))
            .await
            .unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|f| f.severity == dm::AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        &pool,
        actor.user_id,
        revision.id,
        dm::PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: preview
                .findings
                .iter()
                .filter(|f| f.severity == dm::AcademicChangeFindingSeverity::Warning)
                .map(|f| f.code)
                .collect(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let selected = versions::get_version(&pool, target.id).await.unwrap();
    let included = selected
        .snapshot
        .offerings
        .iter()
        .find(|o| o.id == offering)
        .unwrap();
    assert!(included.groups.iter().all(|g| g.id != unused));
    assert!(included.groups.iter().any(|g| g.id == group));
    let outcome = lock_course_subject(&pool, &academic_affairs_actor(&actor), &ctx, subject)
        .await
        .unwrap();
    assert!(
        outcome.lock.is_some(),
        "ready included M6 group must lock despite unused omitted M4 group: {:?}",
        outcome.groups
    );
    assert!(outcome.groups.iter().all(|g| g.learning_group_id != unused));
    let after: String = sqlx::query_scalar(
        "SELECT to_jsonb(v)::text FROM academic_delivery_versions v WHERE id=$1",
    )
    .bind(old_source)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old_bytes, after);
}
