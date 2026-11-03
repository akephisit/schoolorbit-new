use super::*;
use crate::integration_tests::academic::core;
use school_academic_lifecycle::services::{confirm_year_rollover, preview_year_rollover};
use school_permissions::registry::codes;

async fn fixture(name: &str) -> (PgPool, ActorContext, YearRolloverPreviewInput) {
    let (pool, source, target, destination) =
        core::services::promotion_targets::tests::fixture(name).await;
    let actor = ActorContext {
        user_id: sqlx::query_scalar(
            "SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        permissions: vec![codes::WILDCARD.into()],
    };
    let year: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM student_academic_years WHERE id=$1")
            .bind(source.student_academic_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let today = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let end = today.pred_opt().unwrap();
    sqlx::query("INSERT INTO student_info(user_id,student_id,registration_status) SELECT student_id,student_id::text,'studying' FROM student_academic_years WHERE academic_year_id=$1 ON CONFLICT(user_id) DO NOTHING").bind(year).execute(&pool).await.unwrap();
    for index in 0..4 {
        let student = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) VALUES($1,'synthetic-not-login','นักเรียนทดสอบ',$2,'student','active')").bind(student).bind(format!("{index}")).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO student_info(user_id,student_id,registration_status) VALUES($1,$1::text,'studying')")
            .bind(student)
            .execute(&pool)
            .await
            .unwrap();
        let annual:Uuid=sqlx::query_scalar("INSERT INTO student_academic_years(student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES($1,$2,$3,$4,'active') RETURNING id").bind(student).bind(year).bind(source.grade_level_id).bind(source.study_program_id).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO homeroom_placements(student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) SELECT $1,$2,homeroom_id,$3,'current','promotion' FROM homeroom_placements WHERE student_academic_year_id=$4 ORDER BY start_date DESC LIMIT 1").bind(annual).bind(year).bind(end).bind(source.student_academic_year_id).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE academic_year_id=$1")
        .bind(year)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(year)
        .execute(&pool)
        .await
        .unwrap();
    let mut input = YearRolloverPreviewInput {
        source_year_id: year,
        target_year_id: target,
        outcome_effective_on: end,
        target_placement_starts_on: today,
        decisions: vec![],
    };
    let preview = preview_year_rollover(&pool, &actor, input.clone())
        .await
        .unwrap();
    assert_eq!(preview.students.len(), 5);
    for (index, student) in preview.students.iter().enumerate() {
        let mut decision = PromotionDecisionInput {
            outcome: PromotionDecisionOutcome::Hold,
            target_grade_level_id: None,
            target_study_program_id: None,
            target_homeroom_id: None,
            reason: None,
            condition: None,
        };
        decision.outcome = match index {
            0 => PromotionDecisionOutcome::Promote,
            1 => PromotionDecisionOutcome::Repeat,
            2 => PromotionDecisionOutcome::Graduate,
            3 => PromotionDecisionOutcome::TransferOut,
            _ => PromotionDecisionOutcome::Hold,
        };
        if index < 2 {
            decision.target_grade_level_id = Some(if index == 0 {
                destination.target_grade_level_id.unwrap()
            } else {
                source.grade_level_id
            });
            decision.target_study_program_id = destination.target_study_program_id;
            let room:Uuid=sqlx::query_scalar("INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) VALUES($1,$1,$2,$3,$4,$1,10,true) RETURNING id")
                .bind(format!("MANUAL-{index}")).bind(target).bind(decision.target_grade_level_id).bind(decision.target_study_program_id).fetch_one(&pool).await.unwrap();
            decision.target_homeroom_id = Some(room);
        }
        input.decisions.push(YearRolloverStudentInput {
            student_academic_year_id: student.student_academic_year_id,
            decision,
        });
    }
    (pool, actor, input)
}

async fn confirm_input(
    pool: &PgPool,
    actor: &ActorContext,
    input: YearRolloverPreviewInput,
) -> ConfirmYearRolloverInput {
    let preview = preview_year_rollover(pool, actor, input.clone())
        .await
        .unwrap();
    assert!(preview.can_confirm, "{:?}", preview.findings);
    ConfirmYearRolloverInput {
        request_id: Uuid::new_v4(),
        source_checksum: preview.source_checksum,
        preview: input,
    }
}

#[tokio::test]
async fn manual_year_rollover_all_outcomes_prepare_identity_history_and_exact_concurrent_replay_without_results(
) {
    let (pool, actor, input) = fixture("manual_rollover_outcomes").await;
    let before: String =
        sqlx::query_scalar("SELECT md5(string_agg(to_jsonb(u)::text,'' ORDER BY id)) FROM users u")
            .fetch_one(&pool)
            .await
            .unwrap();
    let command = confirm_input(&pool, &actor, input.clone()).await;
    let providers = &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS;
    let (first, second) = tokio::join!(
        confirm_year_rollover(providers, &pool, &actor, command.clone()),
        confirm_year_rollover(providers, &pool, &actor, command.clone())
    );
    let first = first.unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(second.unwrap()).unwrap()
    );
    assert!(first.prepared && !first.activated);
    assert_eq!(first.receipts.len(), 5);
    assert_eq!(
        first
            .receipts
            .iter()
            .filter(|r| r.target_student_year_id.is_some())
            .count(),
        2
    );
    let states:Vec<(String,i64)>=sqlx::query_as("SELECT status,count(*) FROM student_academic_years WHERE academic_year_id=$1 GROUP BY status ORDER BY status").bind(input.source_year_id).fetch_all(&pool).await.unwrap();
    assert_eq!(
        states,
        vec![
            ("active".into(), 1),
            ("completed".into(), 2),
            ("graduated".into(), 1),
            ("withdrawn".into(), 1)
        ]
    );
    let target_states: Vec<String> =
        sqlx::query_scalar("SELECT status FROM student_academic_years WHERE academic_year_id=$1")
            .bind(input.target_year_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(target_states, vec!["planned", "planned"]);
    let year: String = sqlx::query_scalar("SELECT status FROM academic_years WHERE id=$1")
        .bind(input.target_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(year, "planning");
    let terminal:Vec<String>=sqlx::query_scalar("SELECT info.registration_status FROM student_info info JOIN student_academic_years annual ON annual.student_id=info.user_id WHERE annual.academic_year_id=$1 AND annual.status IN ('graduated','withdrawn') ORDER BY info.registration_status").bind(input.source_year_id).fetch_all(&pool).await.unwrap();
    assert_eq!(terminal, vec!["graduated", "transferred_out"]);
    let open_terminal:i64=sqlx::query_scalar("SELECT count(*) FROM homeroom_placements p JOIN student_academic_years annual ON annual.id=p.student_academic_year_id WHERE annual.academic_year_id=$1 AND annual.status<>'active' AND p.status IN ('current','planned')").bind(input.source_year_id).fetch_one(&pool).await.unwrap();
    assert_eq!(open_terminal, 0);
    let absent_evidence:bool=sqlx::query_scalar("SELECT run.policy_id IS NULL AND bool_and(item.recommendation IS NULL AND item.annual_revision_id IS NULL) FROM academic_promotion_runs run JOIN academic_promotion_run_items item ON item.run_id=run.id WHERE run.id=$1 GROUP BY run.id").bind(first.run_id).fetch_one(&pool).await.unwrap();
    assert!(absent_evidence);
    let after: String =
        sqlx::query_scalar("SELECT md5(string_agg(to_jsonb(u)::text,'' ORDER BY id)) FROM users u")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    let mut tx = pool.begin().await.unwrap();
    let evidence = super::promotion_opening::read_in_transaction(
        &mut tx,
        input.source_year_id,
        input.target_year_id,
    )
    .await
    .unwrap();
    assert_eq!(evidence.inconsistent_receipts, 0);
    assert_eq!(evidence.missing_students, 0);
    tx.rollback().await.unwrap();
    let mut wrong = command.clone();
    wrong.preview.outcome_effective_on = wrong.preview.outcome_effective_on.pred_opt().unwrap();
    assert!(matches!(
        confirm_year_rollover(providers, &pool, &actor, wrong).await,
        Err(AppError::Conflict(_))
    ));
    let other = ActorContext {
        user_id: Uuid::new_v4(),
        permissions: actor.permissions.clone(),
    };
    assert!(matches!(
        confirm_year_rollover(providers, &pool, &other, command.clone()).await,
        Err(AppError::Conflict(_))
    ));
    let denied = ActorContext {
        user_id: actor.user_id,
        permissions: vec![],
    };
    assert!(matches!(
        confirm_year_rollover(providers, &pool, &denied, command).await,
        Err(AppError::Forbidden(_))
    ));
}

#[tokio::test]
async fn manual_year_rollover_stale_capacity_closed_source_and_other_successor_refusals_are_atomic()
{
    let (pool, actor, input) = fixture("manual_rollover_refusals").await;
    let command = confirm_input(&pool, &actor, input.clone()).await;
    sqlx::query("UPDATE student_academic_years SET row_version=row_version+1 WHERE id=$1")
        .bind(input.decisions[0].student_academic_year_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        confirm_year_rollover(
            &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS,
            &pool,
            &actor,
            command
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_promotion_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(input.source_year_id)
        .execute(&pool)
        .await
        .unwrap();
    let preview = preview_year_rollover(&pool, &actor, input.clone())
        .await
        .unwrap();
    assert!(preview
        .findings
        .iter()
        .any(|f| f.code == "rollover.source_year_open"));
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(input.source_year_id)
        .execute(&pool)
        .await
        .unwrap();
    let room = input.decisions[0].decision.target_homeroom_id.unwrap();
    sqlx::query("UPDATE homerooms SET capacity=1 WHERE id=$1")
        .bind(room)
        .execute(&pool)
        .await
        .unwrap();
    let filler = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) VALUES($1,'synthetic-not-login','filler','fixture','student','active')").bind(filler).execute(&pool).await.unwrap();
    let annual:Uuid=sqlx::query_scalar("INSERT INTO student_academic_years(student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,academic_year_id,grade_level_id,study_program_id,'planned' FROM homerooms WHERE id=$2 RETURNING id").bind(filler).bind(room).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) VALUES($1,$2,$3,$4,'planned','promotion')").bind(annual).bind(input.target_year_id).bind(room).bind(input.target_placement_starts_on).execute(&pool).await.unwrap();
    let preview = preview_year_rollover(&pool, &actor, input.clone())
        .await
        .unwrap();
    assert!(preview
        .findings
        .iter()
        .any(|f| f.code == "rollover.room_capacity"));
    sqlx::query("UPDATE homerooms SET capacity=10 WHERE id=$1")
        .bind(room)
        .execute(&pool)
        .await
        .unwrap();
    let command = confirm_input(&pool, &actor, input.clone()).await;
    confirm_year_rollover(
        &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS,
        &pool,
        &actor,
        command,
    )
    .await
    .unwrap();
    let another:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,school_days,status) SELECT max(year)+1,'synthetic other successor','MON','planning' FROM academic_years RETURNING id").fetch_one(&pool).await.unwrap();
    let mut other = input;
    other.target_year_id = another;
    other.decisions.clear();
    let preview = preview_year_rollover(&pool, &actor, other).await.unwrap();
    assert!(preview
        .findings
        .iter()
        .any(|f| f.code == "rollover.other_successor_exists"));
}

#[tokio::test]
async fn manual_year_rollover_later_hold_resolution_uses_latest_receipt_and_preserves_prepared_ids()
{
    let (pool, actor, input) = fixture("manual_rollover_hold_resolution").await;
    let command = confirm_input(&pool, &actor, input.clone()).await;
    let providers = &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS;
    let first = confirm_year_rollover(providers, &pool, &actor, command)
        .await
        .unwrap();
    let preview = preview_year_rollover(
        &pool,
        &actor,
        YearRolloverPreviewInput {
            decisions: vec![],
            ..input.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(preview.students.len(), 3);
    let original_promote = input
        .decisions
        .iter()
        .find(|row| row.decision.outcome == PromotionDecisionOutcome::Promote)
        .unwrap();
    let decisions = preview
        .students
        .iter()
        .map(|row| {
            let decision =
                row.suggested_decision
                    .clone()
                    .unwrap_or_else(|| PromotionDecisionInput {
                        outcome: PromotionDecisionOutcome::Repeat,
                        target_grade_level_id: Some(row.source_grade_level_id),
                        target_study_program_id: original_promote.decision.target_study_program_id,
                        target_homeroom_id: None,
                        reason: Some("โรงเรียนยืนยันผลภายหลัง".into()),
                        condition: None,
                    });
            YearRolloverStudentInput {
                student_academic_year_id: row.student_academic_year_id,
                decision,
            }
        })
        .collect();
    let second_input = YearRolloverPreviewInput {
        decisions,
        ..input.clone()
    };
    let command = confirm_input(&pool, &actor, second_input).await;
    let second = confirm_year_rollover(providers, &pool, &actor, command)
        .await
        .unwrap();
    assert_eq!(second.receipts.len(), 3);
    for receipt in first
        .receipts
        .iter()
        .filter(|row| row.target_student_year_id.is_some())
    {
        assert!(second.receipts.iter().any(|row| row.target_student_year_id
            == receipt.target_student_year_id
            && row.target_placement_id == receipt.target_placement_id));
    }
    let mut tx = pool.begin().await.unwrap();
    let evidence = super::promotion_opening::read_in_transaction(
        &mut tx,
        input.source_year_id,
        input.target_year_id,
    )
    .await
    .unwrap();
    assert_eq!(evidence.inconsistent_receipts, 0);
    assert_eq!(evidence.missing_students, 0);
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn manual_year_rollover_receipts_activate_the_prepared_year_without_annual_result_evidence() {
    let (pool, actor, input) = fixture("manual_rollover_activation").await;
    let command = confirm_input(&pool, &actor, input.clone()).await;
    confirm_year_rollover(
        &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS,
        &pool,
        &actor,
        command,
    )
    .await
    .unwrap();
    let source_number: i32 = sqlx::query_scalar("SELECT year FROM academic_years WHERE id=$1")
        .bind(input.source_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let target_number: i32 = sqlx::query_scalar("SELECT year FROM academic_years WHERE id=$1")
        .bind(input.target_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let intervening: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_years WHERE year>$1 AND year<$2 ORDER BY year,id",
    )
    .bind(source_number)
    .bind(target_number)
    .fetch_all(&pool)
    .await
    .unwrap();
    for (index, year) in intervening.into_iter().enumerate() {
        sqlx::query("UPDATE academic_years SET year=$2 WHERE id=$1")
            .bind(year)
            .bind(target_number + index as i32 + 1)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE academic_years SET status='closed' WHERE year<$1")
        .bind(target_number)
        .execute(&pool)
        .await
        .unwrap();
    let term = core::services::activation_context_tests::ready_term_for_existing_year(
        &pool,
        actor.user_id,
        input.target_year_id,
        input.target_placement_starts_on,
    )
    .await;
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(term.id)
        .execute(&pool)
        .await
        .unwrap();
    let workspace = get_activation_workspace(&pool, &actor, input.target_year_id, term.id)
        .await
        .unwrap();
    assert!(workspace.can_activate, "{:?}", workspace.findings);
    assert!(workspace.opens_year);
    assert_eq!(workspace.planned_students, 2);
    let receipt = term_transitions::transition_term(
        &pool,
        &actor,
        term.id,
        TermTransitionRequest {
            academic_year_id: input.target_year_id,
            request_id: Uuid::new_v4(),
            action: TermTransitionAction::Activate,
            expected_year_version: workspace.context.year_row_version,
            expected_term_version: workspace.context.term_row_version,
            readiness_checksum: workspace.source_checksum,
            acknowledged_warning_codes: vec![],
            reason: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        receipt.context.year_status,
        school_academic_core::models::AcademicYearStatus::Active
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM student_academic_years WHERE academic_year_id=$1 AND status='active'").bind(input.target_year_id).fetch_one(&pool).await.unwrap(),2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_annual_result_revisions WHERE academic_year_id=$1"
        )
        .bind(input.source_year_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn manual_year_rollover_reaffirms_legacy_conditional_target_while_resolving_another_hold() {
    let (pool, reviewer, _, _, calculation) =
        super::promotion_review_tests::ready_run_with_extra_student(
            "manual_rollover_legacy_conditional",
        )
        .await;
    let actor = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![codes::WILDCARD.into()],
    };
    let mut run = calculation.run.clone();
    let mut reviewed = vec![];
    for (index, item) in calculation.items.iter().enumerate() {
        let mut declaration = super::promotion_review_tests::hold(item.row_version);
        if index == 0 {
            declaration.decision = PromotionDecisionInput {
                outcome: PromotionDecisionOutcome::Conditional,
                target_grade_level_id: Some(item.source_grade_level_id),
                target_study_program_id: Some(item.source_study_program_id),
                target_homeroom_id: None,
                reason: Some("โรงเรียนอนุมัติตามเงื่อนไขเดิม".into()),
                condition: Some("ติดตามเอกสารตามทะเบียนเดิม".into()),
            };
        }
        let result = review_item(&pool, &actor, run.id, item.id, declaration)
            .await
            .unwrap();
        run = result.run;
        reviewed.push(result.item);
    }
    let run = approve_run(
        &pool,
        &actor,
        run.id,
        ApprovePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: run.row_version,
            source_checksum: super::promotion_approval::intent_checksum(&run, &reviewed).unwrap(),
        },
    )
    .await
    .unwrap();
    let original = execute_run(
        &pool,
        &actor,
        run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: run.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    let old_evidence:sqlx::types::Json<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r) ORDER BY item_id) FROM academic_promotion_execution_receipts r WHERE run_id=$1").bind(run.id).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE academic_year_id=$1")
        .bind(run.source_year_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(run.source_year_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO student_info(user_id,student_id,registration_status) SELECT student_id,student_id::text,'studying' FROM student_academic_years WHERE academic_year_id=$1 ON CONFLICT(user_id) DO NOTHING").bind(run.source_year_id).execute(&pool).await.unwrap();
    let mut input = YearRolloverPreviewInput {
        source_year_id: run.source_year_id,
        target_year_id: run.target_year_id,
        outcome_effective_on: run.target_placement_starts_on.pred_opt().unwrap(),
        target_placement_starts_on: run.target_placement_starts_on,
        decisions: vec![],
    };
    let preview = preview_year_rollover(&pool, &actor, input.clone())
        .await
        .unwrap();
    input.decisions = preview
        .students
        .iter()
        .map(|row| YearRolloverStudentInput {
            student_academic_year_id: row.student_academic_year_id,
            decision: row
                .suggested_decision
                .clone()
                .filter(|_| row.existing_target_student_year_id.is_some())
                .unwrap_or(PromotionDecisionInput {
                    outcome: PromotionDecisionOutcome::Graduate,
                    target_grade_level_id: None,
                    target_study_program_id: None,
                    target_homeroom_id: None,
                    reason: None,
                    condition: None,
                }),
        })
        .collect();
    let command = confirm_input(&pool, &actor, input).await;
    let result = confirm_year_rollover(
        &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS,
        &pool,
        &actor,
        command,
    )
    .await
    .unwrap();
    let target = original
        .receipts
        .iter()
        .find_map(|r| r.target_student_year_id)
        .unwrap();
    assert!(result
        .receipts
        .iter()
        .any(|r| r.target_student_year_id == Some(target)));
    let preserved:sqlx::types::Json<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r) ORDER BY item_id) FROM academic_promotion_execution_receipts r WHERE run_id=$1").bind(run.id).fetch_one(&pool).await.unwrap();
    assert_eq!(old_evidence.0, preserved.0);
}
