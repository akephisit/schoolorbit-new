use super::super::{
    promotion_approval,
    promotion_run_review::{
        review_item,
        tests::{hold, ready_run},
    },
};
use super::*;
use crate::{
    modules::academic::cutover_test_support::apply_migrations_through, permissions::registry::codes,
};

#[tokio::test]
async fn promotion_execution_partial_batch_preserves_completed_items_after_another_student_is_corrected(
) {
    use crate::modules::academic::results::{models as rm, services as rs};
    let (pool, reviewer, results_actor, context, calc) =
        super::super::promotion_run_review::tests::ready_run_with_extra_student(
            "promotion_execution_partial",
        )
        .await;
    assert_eq!(calc.items.len(), 2);
    let mut reviewed = Vec::new();
    let mut run = calc.run.clone();
    for (index, item) in calc.items.iter().enumerate() {
        let mut input = hold(item.row_version);
        if index == 0 {
            input.decision.outcome = PromotionDecisionOutcome::TransferOut;
        }
        let result = review_item(&pool, &reviewer, run.id, item.id, input)
            .await
            .unwrap();
        run = result.run;
        reviewed.push(result.item);
    }
    let approver = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL.into(),
        ],
    };
    let approved = promotion_approval::approve_run(
        &pool,
        &approver,
        run.id,
        ApprovePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: run.row_version,
            source_checksum: promotion_approval::intent_checksum(&run, &reviewed).unwrap(),
        },
    )
    .await
    .unwrap();
    let executor = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_EXECUTE_SCHOOL.into(),
        ],
    };
    let first_input = ExecutePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: approved.row_version,
        limit: 1,
    };
    let first = execute_run(&pool, &executor, run.id, first_input.clone())
        .await
        .unwrap();
    assert_eq!(first.run.status, PromotionRunStatus::Executing);
    assert_eq!(first.receipts.len(), 1);
    assert_eq!(first.remaining_count, 1);
    let completed_id = first.receipts[0].item_id;
    let completed_before: PromotionRunItem = sqlx::query_as(&format!(
        "SELECT {ITEM_COLUMNS} FROM academic_promotion_run_items WHERE id=$1"
    ))
    .bind(completed_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let pending = calc
        .items
        .iter()
        .find(|item| item.id != completed_id)
        .unwrap();
    let course: Uuid = sqlx::query_scalar("SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1").bind(pending.student_academic_year_id).fetch_one(&pool).await.unwrap();
    rs::correct_result(
        &pool,
        &results_actor,
        &context,
        rm::ResultCorrectionInput::Course {
            course_result_id: course,
            outcome: rm::CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let second = execute_run(
        &pool,
        &executor,
        run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: first.run.row_version,
            limit: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(second.run.status, PromotionRunStatus::Failed);
    assert_eq!(second.remaining_count, 1);
    assert_eq!(second.failures.len(), 1);
    assert!(second.receipts.is_empty());
    let recalculated = super::super::calculate_run(
        &pool,
        &reviewer,
        run.id,
        CalculatePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: second.run.row_version,
        },
    )
    .await
    .unwrap();
    let completed_after = recalculated
        .items
        .iter()
        .find(|item| item.id == completed_id)
        .unwrap();
    assert_eq!(
        serde_json::to_value(&completed_before).unwrap(),
        serde_json::to_value(completed_after).unwrap()
    );
    let unfinished = recalculated
        .items
        .iter()
        .find(|item| item.id == pending.id)
        .unwrap();
    assert_eq!(unfinished.status, PromotionItemStatus::Calculated);
    assert!(unfinished.decision.is_none());
    assert!(recalculated.run.approval_id.is_none());
    let receipts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_promotion_execution_receipts WHERE run_id=$1",
    )
    .bind(run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipts, 1);
    let replay = execute_run(&pool, &executor, run.id, first_input)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(replay).unwrap()
    );
    // Relock the corrected student's evidence, review again and complete only
    // the unfinished item. The first execution keeps its original approval.
    let policy: Uuid = sqlx::query_scalar("SELECT policy_id FROM academic_term_aggregate_revisions WHERE student_academic_year_id=$1 AND academic_term_id=$2 ORDER BY revision DESC LIMIT 1").bind(pending.student_academic_year_id).bind(context.academic_term_id).fetch_one(&pool).await.unwrap();
    let preview = rs::preview_aggregate(
        &pool,
        &results_actor,
        pending.student_academic_year_id,
        &rm::AggregatePreviewQuery {
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            policy_id: policy,
        },
    )
    .await
    .unwrap();
    rs::lock_term_aggregate(
        &pool,
        &results_actor,
        &context,
        pending.student_academic_year_id,
        rm::AggregateLockInput {
            policy_id: policy,
            source_checksum: preview.source_checksum,
            expected_revision: Some(1),
            request_id: Uuid::new_v4(),
            hold_reason: None,
        },
    )
    .await
    .unwrap();
    let annual = rs::preview_annual(
        &pool,
        &results_actor,
        context.academic_year_id,
        pending.student_academic_year_id,
    )
    .await
    .unwrap();
    rs::lock_annual(
        &pool,
        &results_actor,
        context.academic_year_id,
        pending.student_academic_year_id,
        rm::AnnualLockInput {
            expected_revision: Some(1),
            source_checksum: annual.source_checksum,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        },
    )
    .await
    .unwrap();
    let recalculated = super::super::calculate_run(
        &pool,
        &reviewer,
        run.id,
        CalculatePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: recalculated.run.row_version,
        },
    )
    .await
    .unwrap();
    let pending = recalculated
        .items
        .iter()
        .find(|item| item.id == pending.id)
        .unwrap();
    let reviewed = review_item(
        &pool,
        &reviewer,
        run.id,
        pending.id,
        hold(pending.row_version),
    )
    .await
    .unwrap();
    let items: Vec<PromotionRunItem> = sqlx::query_as(&format!("SELECT {ITEM_COLUMNS} FROM academic_promotion_run_items WHERE run_id=$1 ORDER BY student_academic_year_id")).bind(run.id).fetch_all(&pool).await.unwrap();
    let approved_again = promotion_approval::approve_run(
        &pool,
        &approver,
        run.id,
        ApprovePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: reviewed.run.row_version,
            source_checksum: promotion_approval::intent_checksum(&reviewed.run, &items).unwrap(),
        },
    )
    .await
    .unwrap();
    let finished = execute_run(
        &pool,
        &executor,
        run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved_again.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    assert_eq!(finished.run.status, PromotionRunStatus::Completed);
    assert_eq!(finished.remaining_count, 0);
    assert_eq!(finished.hold_count, 1);
    assert_eq!(finished.receipts.len(), 1);
    assert_eq!(finished.receipts[0].item_id, pending.id);
    let original: PromotionExecutionReceipt = sqlx::query_as(&format!(
        "SELECT {RECEIPT_COLUMNS} FROM academic_promotion_execution_receipts WHERE item_id=$1"
    ))
    .bind(completed_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(original).unwrap(),
        serde_json::to_value(&first.receipts[0]).unwrap()
    );
    let completed_after: PromotionRunItem = sqlx::query_as(&format!(
        "SELECT {ITEM_COLUMNS} FROM academic_promotion_run_items WHERE id=$1"
    ))
    .bind(completed_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(completed_before).unwrap(),
        serde_json::to_value(completed_after).unwrap()
    );
}

pub(crate) async fn approved_fixture(
    name: &str,
    outcome: PromotionDecisionOutcome,
) -> (
    PgPool,
    ActorContext,
    ActorContext,
    crate::modules::academic::results::models::ResultContext,
    PromotionRunCalculation,
    PromotionRun,
) {
    let (pool, reviewer, results_actor, context, calc) = ready_run(name).await;
    apply_migrations_through(&pool, 76).await.unwrap();
    let mut decision = hold(1);
    decision.decision.outcome = outcome;
    if outcome == PromotionDecisionOutcome::Promote {
        let grade: Uuid = sqlx::query_scalar(
            "SELECT id FROM grade_levels WHERE level_type='secondary' AND year=2",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let curriculum: Uuid = sqlx::query_scalar("INSERT INTO curricula(code,identity_key,name_th,grade_level_ids) VALUES('E2E-IMPACT','E2E-IMPACT','E2E-LIFECYCLE-next-grade',$1) RETURNING id")
            .bind(sqlx::types::Json(vec![grade])).fetch_one(&pool).await.unwrap();
        let version: Uuid = sqlx::query_scalar("INSERT INTO curriculum_versions(curriculum_id,version_name,start_academic_year_id,status) VALUES($1,'E2E-LIFECYCLE-target',$2,'draft') RETURNING id")
            .bind(curriculum).bind(calc.run.target_year_id).fetch_one(&pool).await.unwrap();
        let program: Uuid = sqlx::query_scalar("INSERT INTO study_programs(id,curriculum_version_id,code,name_th,status) VALUES(uuid_generate_v4(),$1,'E2E-IMPACT','E2E-LIFECYCLE-target','published') RETURNING id")
            .bind(version).fetch_one(&pool).await.unwrap();
        sqlx::query(
            "UPDATE curriculum_versions SET status='published',published_at=now() WHERE id=$1",
        )
        .bind(version)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO grade_level_progressions(from_grade_level_id,to_grade_level_id,transition_kind,is_active) VALUES($1,$2,'promote',true) ON CONFLICT DO NOTHING")
            .bind(calc.items[0].source_grade_level_id).bind(grade).execute(&pool).await.unwrap();
        let room: Uuid = sqlx::query_scalar("INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) VALUES('E2E-IMPACT','E2E-LIFECYCLE-target',$1,$2,$3,'1',30,true) RETURNING id")
            .bind(calc.run.target_year_id).bind(grade).bind(program).fetch_one(&pool).await.unwrap();
        decision.decision.target_grade_level_id = Some(grade);
        decision.decision.target_study_program_id = Some(program);
        decision.decision.target_homeroom_id = Some(room);
    }
    let review = review_item(&pool, &reviewer, calc.run.id, calc.items[0].id, decision)
        .await
        .unwrap();
    let approver = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL.into(),
        ],
    };
    let approved = promotion_approval::approve_run(
        &pool,
        &approver,
        calc.run.id,
        ApprovePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: review.run.row_version,
            source_checksum: promotion_approval::intent_checksum(&review.run, &[review.item])
                .unwrap(),
        },
    )
    .await
    .unwrap();
    let executor = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_EXECUTE_SCHOOL.into(),
        ],
    };
    (pool, executor, results_actor, context, calc, approved)
}

pub(crate) async fn started_run_fixture(
    name: &str,
) -> (PgPool, ActorContext, PromotionRunCalculation, PromotionRun) {
    let (pool, executor, _, _, calc, approved) =
        approved_fixture(name, PromotionDecisionOutcome::Hold).await;
    let input = ExecutePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: approved.row_version,
        limit: 1,
    };
    let checksum =
        super::super::checksum(&(executor.user_id, calc.run.id, "execute", &input)).unwrap();
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::lock_transition(&mut tx).await.unwrap();
    start_batch(&mut tx, &executor, calc.run.id, &input, &checksum)
        .await
        .unwrap();
    let run = read_run(&mut tx, calc.run.id).await.unwrap();
    tx.commit().await.unwrap();
    (pool, executor, calc, run)
}

#[tokio::test]
async fn promotion_execution_audit_failure_rolls_back_source_and_retry_is_explicit() {
    let (pool, executor, _, _, calc, approved) = approved_fixture(
        "promotion_execution_atomic",
        PromotionDecisionOutcome::TransferOut,
    )
    .await;
    let input = ExecutePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: approved.row_version,
        limit: 1,
    };
    sqlx::raw_sql("CREATE FUNCTION fail_promotion_execution_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_code='promotion_item.executed' THEN RAISE EXCEPTION 'fixture audit unavailable'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_promotion_execution_audit BEFORE INSERT ON academic_audit_events FOR EACH ROW EXECUTE FUNCTION fail_promotion_execution_audit();").execute(&pool).await.unwrap();
    let failed = execute_run(&pool, &executor, calc.run.id, input.clone())
        .await
        .unwrap();
    assert_eq!(failed.run.status, PromotionRunStatus::Failed);
    assert_eq!(failed.failures.len(), 1);
    assert!(failed.receipts.is_empty());
    let state: (String, i64) =
        sqlx::query_as("SELECT status,row_version FROM student_academic_years WHERE id=$1")
            .bind(calc.items[0].student_academic_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, ("active".into(), calc.items[0].source_row_version));
    sqlx::raw_sql("DROP TRIGGER fail_promotion_execution_audit ON academic_audit_events; DROP FUNCTION fail_promotion_execution_audit();").execute(&pool).await.unwrap();
    let replay = execute_run(&pool, &executor, calc.run.id, input)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&failed).unwrap(),
        serde_json::to_value(replay).unwrap()
    );
    let result = execute_run(
        &pool,
        &executor,
        calc.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: failed.run.row_version,
            limit: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.run.status, PromotionRunStatus::Completed);
    assert_eq!(result.receipts.len(), 1);
    assert!(result.failures.is_empty());
    let state: (String, i64) =
        sqlx::query_as("SELECT status,row_version FROM student_academic_years WHERE id=$1")
            .bind(calc.items[0].student_academic_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        state,
        ("withdrawn".into(), calc.items[0].source_row_version + 1)
    );
    let mut tx = pool.begin().await.unwrap();
    let annual = results::services::promotion_annual_sources(
        &mut tx,
        calc.run.source_year_id,
        &[calc.items[0].student_academic_year_id],
    )
    .await
    .unwrap();
    assert!(
        annual[&calc.items[0].student_academic_year_id]
            .as_ref()
            .unwrap()
            .is_current,
        "the explicit transfer must preserve the official source-year result"
    );
}

#[tokio::test]
async fn promotion_execution_rechecks_actual_result_correction_before_each_item() {
    use crate::modules::academic::results::{models as rm, services as rs};
    let (pool, executor, results_actor, context, calc, approved) = approved_fixture(
        "promotion_execution_correction",
        PromotionDecisionOutcome::Hold,
    )
    .await;
    let result:Uuid=sqlx::query_scalar("SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1").bind(calc.items[0].student_academic_year_id).fetch_one(&pool).await.unwrap();
    rs::correct_result(
        &pool,
        &results_actor,
        &context,
        rm::ResultCorrectionInput::Course {
            course_result_id: result,
            outcome: rm::CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let failed = execute_run(
        &pool,
        &executor,
        calc.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(failed.run.status, PromotionRunStatus::Failed);
    assert_eq!(failed.remaining_count, 1);
    assert_eq!(failed.failures.len(), 1);
    assert!(failed.receipts.is_empty());
    assert!(failed.failures[0].message.contains("ผลรายปี"));
}

#[tokio::test]
async fn promotion_execution_can_finish_after_reload_when_all_item_receipts_already_exist() {
    let (pool, executor, _, _, calc, approved) =
        approved_fixture("promotion_execution_reload", PromotionDecisionOutcome::Hold).await;
    let input = ExecutePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: approved.row_version,
        limit: 1,
    };
    let checksum =
        super::super::checksum(&(executor.user_id, calc.run.id, "execute", &input)).unwrap();
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::lock_transition(&mut tx).await.unwrap();
    let batch = start_batch(&mut tx, &executor, calc.run.id, &input, &checksum)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    execute_item(&pool, &batch, calc.items[0].id).await.unwrap();
    // A new browser session has lost the in-memory request ID, but not the receipts.
    let version: i64 =
        sqlx::query_scalar("SELECT row_version FROM academic_promotion_runs WHERE id=$1")
            .bind(calc.run.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let result = execute_run(
        &pool,
        &executor,
        calc.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: version,
            limit: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.run.status, PromotionRunStatus::Completed);
    assert_eq!(result.receipts.len(), 1);
    assert_eq!(result.receipts[0].request_id, batch.request_id);
    let late = execute_run(&pool, &executor, calc.run.id, input)
        .await
        .unwrap();
    assert_eq!(
        late.run.row_version, result.run.row_version,
        "late original response must not rewrite completed run metadata"
    );
    assert_eq!(late.run.executed_at, result.run.executed_at);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_promotion_execution_receipts WHERE run_id=$1",
    )
    .bind(calc.run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn promotion_execution_requires_approval_and_exact_permission_then_replays_hold_receipt() {
    let (pool, reviewer, _, _, calc) = ready_run("promotion_execution_hold").await;
    apply_migrations_through(&pool, 76).await.unwrap();
    let executor = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_EXECUTE_SCHOOL.into(),
        ],
    };
    let input = ExecutePromotionRunInput {
        request_id: Uuid::new_v4(),
        row_version: calc.run.row_version,
        limit: 100,
    };
    for limit in [0, 101, u16::MAX] {
        let mut invalid = input.clone();
        invalid.limit = limit;
        assert!(matches!(
            execute_run(&pool, &executor, calc.run.id, invalid).await,
            Err(AppError::ValidationError(_))
        ));
    }
    for permissions in [
        vec![],
        vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL],
        vec![codes::ACADEMIC_PROMOTION_EXECUTE_SCHOOL],
        vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL,
            codes::ACADEMIC_PROMOTION_MANAGE_SCHOOL,
        ],
    ] {
        let denied = ActorContext {
            user_id: reviewer.user_id,
            permissions: permissions.into_iter().map(String::from).collect(),
        };
        assert!(matches!(
            execute_run(&pool, &denied, calc.run.id, input.clone()).await,
            Err(AppError::Forbidden(_))
        ));
    }
    assert!(matches!(
        execute_run(&pool, &executor, calc.run.id, input.clone()).await,
        Err(AppError::Conflict(_))
    ));
    let review = review_item(&pool, &reviewer, calc.run.id, calc.items[0].id, hold(1))
        .await
        .unwrap();
    let approver = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_APPROVE_SCHOOL.into(),
        ],
    };
    let approved = promotion_approval::approve_run(
        &pool,
        &approver,
        calc.run.id,
        ApprovePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: review.run.row_version,
            source_checksum: promotion_approval::intent_checksum(&review.run, &[review.item])
                .unwrap(),
        },
    )
    .await
    .unwrap();
    let mut input = input;
    input.row_version = approved.row_version;
    // Simulate the process stopping after the durable batch was recorded.
    let checksum =
        super::super::checksum(&(executor.user_id, calc.run.id, "execute", &input)).unwrap();
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::lock_transition(&mut tx).await.unwrap();
    let batch = start_batch(&mut tx, &executor, calc.run.id, &input, &checksum)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::lock_transition(&mut tx).await.unwrap();
    let collision = super::super::promotion_runs::replay_command::<PromotionRun>(
        &mut tx,
        input.request_id,
        "approve",
        "different-command",
    )
    .await;
    assert!(matches!(collision, Err(AppError::Conflict(_))));
    tx.rollback().await.unwrap();
    // Simulate stopping after the item commits, but before the batch response.
    execute_item(&pool, &batch, calc.items[0].id).await.unwrap();
    let executed = execute_run(&pool, &executor, calc.run.id, input.clone())
        .await
        .unwrap();
    assert_eq!(executed.run.status, PromotionRunStatus::Completed);
    assert_eq!(executed.hold_count, 1);
    assert_eq!(executed.remaining_count, 0);
    assert!(executed.failures.is_empty());
    assert_eq!(executed.receipts.len(), 1);
    assert!(executed.receipts[0].target_student_year_id.is_none());
    let replay = execute_run(&pool, &executor, calc.run.id, input.clone())
        .await
        .unwrap();
    let other = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: executor.permissions.clone(),
    };
    assert!(matches!(
        execute_run(&pool, &other, calc.run.id, input.clone()).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        serde_json::to_value(executed).unwrap(),
        serde_json::to_value(replay).unwrap()
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_promotion_execution_receipts WHERE run_id=$1",
    )
    .bind(calc.run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
    for sql in ["UPDATE academic_promotion_execution_receipts SET source_row_version=source_row_version+1 WHERE run_id=$1","DELETE FROM academic_promotion_execution_receipts WHERE run_id=$1","DELETE FROM academic_promotion_execution_batches WHERE run_id=$1"] {
        assert!(sqlx::query(sql).bind(calc.run.id).execute(&pool).await.is_err());
    }
    input.limit = 99;
    assert!(matches!(
        execute_run(&pool, &executor, calc.run.id, input).await,
        Err(AppError::Conflict(_))
    ));
}
