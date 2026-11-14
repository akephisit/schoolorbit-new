use super::*;
use school_academic_delivery::services::current;

#[tokio::test]
async fn results_current_obligations_use_exact_term_without_date_or_publication_selection() {
    let (pool, actor, ctx, group, unused) =
        fixture_with_unused_group("results_current_exact_context", true).await;
    let unused = unused.unwrap();
    let before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g),'scores',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learning_group_student_scores s),'locks',(SELECT jsonb_agg(to_jsonb(l) ORDER BY id) FROM academic_course_result_locks l))").fetch_one(&pool).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let mut expected = vec![group, unused];
    expected.sort();
    for date in [
        NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2030, 1, 1).unwrap(),
    ] {
        assert_eq!(
            current_obligation_group_ids_on_connection(
                &mut conn,
                ctx.academic_term_id,
                ctx.academic_year_id,
                date,
                &[unused, group, unused]
            )
            .await
            .unwrap(),
            expected
        );
    }
    let date = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    assert!(
        current_obligation_group_ids_on_connection(
            &mut conn,
            ctx.academic_term_id,
            ctx.academic_year_id,
            date,
            &[]
        )
        .await
        .unwrap()
        .is_empty()
    );
    for (year, ids) in [
        (Uuid::new_v4(), vec![group]),
        (ctx.academic_year_id, vec![group, Uuid::new_v4()]),
    ] {
        assert!(matches!(
            current_obligation_group_ids_on_connection(
                &mut conn,
                ctx.academic_term_id,
                year,
                date,
                &ids
            )
            .await,
            Err(AppError::Conflict(_))
        ));
    }
    let ids: Vec<_> = (0..=school_academic_assessment::ports::CURRENT_OBLIGATION_GROUP_SCAN_LIMIT)
        .map(|_| Uuid::new_v4())
        .collect();
    assert!(matches!(
        current_obligation_group_ids_on_connection(
            &mut conn,
            ctx.academic_term_id,
            ctx.academic_year_id,
            date,
            &ids
        )
        .await,
        Err(AppError::ValidationError(_))
    ));
    drop(conn);
    let queue = readiness(&pool, &actor, &ctx).await.unwrap();
    assert!(
        queue
            .courses
            .iter()
            .flat_map(|s| &s.groups)
            .any(|g| g.learning_group_id == unused && !g.ready)
    );
    let after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('groups',(SELECT jsonb_agg(to_jsonb(g) ORDER BY id) FROM learning_groups g),'scores',(SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM learning_group_student_scores s),'locks',(SELECT jsonb_agg(to_jsonb(l) ORDER BY id) FROM academic_course_result_locks l))").fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
}
#[tokio::test]
async fn results_current_allocation_preserves_saved_identity_and_filters_inactive_accounts() {
    let (pool, actor, ctx, group) = fixture("results_current_active_identity").await;
    let offering: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id=$1")
            .bind(group)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let allocations = current::teacher_allocations_on_connection(&mut conn, &[offering])
        .await
        .unwrap();
    let original = allocations
        .iter()
        .find(|a| a.learning_group_id == group && a.teacher_id == actor.user_id)
        .unwrap()
        .assignment_id;
    drop(conn);
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(actor.user_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        !current::teacher_allocations_on_connection(&mut conn, &[offering])
            .await
            .unwrap()
            .iter()
            .any(|a| a.assignment_id == original)
    );
    let workspace =
        current::workspace_on_connection(&mut conn, ctx.academic_year_id, ctx.academic_term_id)
            .await
            .unwrap();
    assert!(
        workspace
            .offerings
            .iter()
            .flat_map(|o| &o.groups)
            .filter(|g| g.id == group)
            .flat_map(|g| &g.teachers)
            .any(|a| a.assignment_id == original && !a.is_active)
    );
}
