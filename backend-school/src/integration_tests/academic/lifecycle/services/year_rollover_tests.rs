use super::*;
use crate::integration_tests::academic::core;
use school_academic_lifecycle::services::{confirm_year_rollover, preview_year_rollover};
use school_permissions::registry::codes;

fn event_date() -> chrono::NaiveDate {
    chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
}

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
        let annual:Uuid=sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES(uuidv7(),$1,$2,$3,$4,'active') RETURNING id").bind(student).bind(year).bind(source.grade_level_id).bind(source.study_program_id).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) SELECT uuidv7(),$1,$2,homeroom_id,$3,'current','promotion' FROM homeroom_placements WHERE student_academic_year_id=$4 ORDER BY start_date DESC LIMIT 1").bind(annual).bind(year).bind(end).bind(source.student_academic_year_id).execute(&pool).await.unwrap();
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
            let room:Uuid=sqlx::query_scalar("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) VALUES(uuidv7(),$1,$1,$2,$3,$4,$1,10,true) RETURNING id")
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
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM homeroom_placements WHERE academic_year_id=$1 AND placement_scope='academic_year' AND start_date IS NULL AND end_date IS NULL").bind(input.target_year_id).fetch_one(&pool).await.unwrap(), 2);
    let workspace =
        school_academic_lifecycle::services::get_run_workspace(&pool, &actor, first.run_id)
            .await
            .unwrap();
    assert!(workspace.run.target_placement_starts_on.is_none());
    assert!(workspace
        .students
        .iter()
        .all(|row| row.item.target_placement_starts_on.is_none()));
    assert!(sqlx::query_scalar::<_, bool>("SELECT NOT(intent ? 'outcomeEffectiveOn') AND NOT(intent ? 'targetPlacementStartsOn') FROM academic_promotion_run_approvals WHERE run_id=$1").bind(first.run_id).fetch_one(&pool).await.unwrap());
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
    wrong.preview.decisions[0].decision.reason = Some("เปลี่ยนผลการยืนยัน".into());
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
    let annual:Uuid=sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT uuidv7(),$1,academic_year_id,grade_level_id,study_program_id,'planned' FROM homerooms WHERE id=$2 RETURNING id").bind(filler).bind(room).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) VALUES(uuidv7(),$1,$2,$3,$4,'planned','promotion')").bind(annual).bind(input.target_year_id).bind(room).bind(event_date()).execute(&pool).await.unwrap();
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
    let promoted = input
        .decisions
        .iter()
        .find(|row| row.decision.outcome == PromotionDecisionOutcome::Promote)
        .unwrap();
    sqlx::query("UPDATE homeroom_placements SET placement_scope='academic_year',start_date=NULL WHERE student_academic_year_id=$1 AND status='current'")
        .bind(promoted.student_academic_year_id).execute(&pool).await.unwrap();
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
        event_date(),
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
    assert_annual_transfer_and_old_year_authorization(&pool, &actor, &input, term.id).await;
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

#[tokio::test]
async fn manual_year_rollover_uses_current_core_room_and_program_after_prepared_roster_adjustments()
{
    use school_academic_core::{
        models as cm,
        services::{homeroom_roster, student_years},
    };
    let (pool, actor, input) = fixture("manual_rollover_current_target").await;
    let providers = &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS;
    let first = confirm_year_rollover(
        providers,
        &pool,
        &actor,
        confirm_input(&pool, &actor, input.clone()).await,
    )
    .await
    .unwrap();
    let declaration = input
        .decisions
        .iter()
        .find(|row| row.decision.outcome == PromotionDecisionOutcome::Promote)
        .unwrap();
    let old_room = declaration.decision.target_homeroom_id.unwrap();
    let source = preview_year_rollover(
        &pool,
        &actor,
        YearRolloverPreviewInput {
            decisions: vec![],
            ..input.clone()
        },
    )
    .await
    .unwrap()
    .students
    .into_iter()
    .find(|row| row.student_academic_year_id == declaration.student_academic_year_id)
    .unwrap();
    let target = source
        .current_target
        .as_ref()
        .unwrap()
        .student_academic_year_id;
    let second_room = student_years::create_homeroom(
        &pool,
        cm::CreateHomeroomRequest {
            academic_year_id: input.target_year_id,
            custom_name: Some("Prepared room B".into()),
            grade_level_id: declaration.decision.target_grade_level_id.unwrap(),
            room_number: "90".into(),
            study_program_id: declaration.decision.target_study_program_id.unwrap(),
            capacity: 10,
        },
    )
    .await
    .unwrap();
    let roster = homeroom_roster::get_roster(&pool, old_room).await.unwrap();
    let selected = roster
        .students
        .iter()
        .find(|row| row.student_academic_year_id == target)
        .unwrap();
    homeroom_roster::mutate_roster(
        &pool,
        &crate::http_state::PLACEMENT_ROSTERS,
        actor.user_id,
        old_room,
        cm::MutateHomeroomRosterRequest {
            revision: roster.revision,
            action: cm::HomeroomRosterAction::Transfer,
            selections: vec![cm::HomeroomRosterSelection {
                student_academic_year_id: target,
                student_year_row_version: selected.student_year_row_version,
                placement_id: Some(selected.placement_id),
                placement_row_version: Some(selected.row_version),
            }],
            effective_date: event_date(),
            target_homeroom_id: Some(second_room.id),
            reason: "จัดห้องปีใหม่ก่อนเริ่มใช้".into(),
        },
    )
    .await
    .unwrap();
    let moved = preview_year_rollover(
        &pool,
        &actor,
        YearRolloverPreviewInput {
            decisions: vec![],
            ..input.clone()
        },
    )
    .await
    .unwrap();
    let row = moved
        .students
        .iter()
        .find(|row| row.student_academic_year_id == declaration.student_academic_year_id)
        .unwrap();
    assert_eq!(
        row.current_target.as_ref().unwrap().homeroom_id,
        Some(second_room.id)
    );
    assert_eq!(
        row.suggested_decision.as_ref().unwrap().target_homeroom_id,
        Some(old_room)
    );
    let roster = homeroom_roster::get_roster(&pool, second_room.id)
        .await
        .unwrap();
    let selected = &roster.students[0];
    homeroom_roster::mutate_roster(
        &pool,
        &crate::http_state::PLACEMENT_ROSTERS,
        actor.user_id,
        second_room.id,
        cm::MutateHomeroomRosterRequest {
            revision: roster.revision,
            action: cm::HomeroomRosterAction::Remove,
            selections: vec![cm::HomeroomRosterSelection {
                student_academic_year_id: target,
                student_year_row_version: selected.student_year_row_version,
                placement_id: Some(selected.placement_id),
                placement_row_version: Some(selected.row_version),
            }],
            effective_date: event_date(),
            target_homeroom_id: None,
            reason: "ปรับชั้นและแผนก่อนจัดห้องใหม่".into(),
        },
    )
    .await
    .unwrap();
    let annual = student_years::get_student_year(&pool, target)
        .await
        .unwrap();
    student_years::update_student_year(
        &pool,
        actor.user_id,
        target,
        cm::UpdateStudentAcademicYearRequest {
            grade_level_id: source.source_grade_level_id,
            study_program_id: source.source_study_program_id,
            row_version: annual.row_version,
        },
    )
    .await
    .unwrap();
    let final_room = student_years::create_homeroom(
        &pool,
        cm::CreateHomeroomRequest {
            academic_year_id: input.target_year_id,
            custom_name: Some("Prepared room C".into()),
            grade_level_id: source.source_grade_level_id,
            room_number: "91".into(),
            study_program_id: source.source_study_program_id,
            capacity: 10,
        },
    )
    .await
    .unwrap();
    let roster = homeroom_roster::get_roster(&pool, final_room.id)
        .await
        .unwrap();
    let annual = student_years::get_student_year(&pool, target)
        .await
        .unwrap();
    homeroom_roster::mutate_roster(
        &pool,
        &crate::http_state::PLACEMENT_ROSTERS,
        actor.user_id,
        final_room.id,
        cm::MutateHomeroomRosterRequest {
            revision: roster.revision,
            action: cm::HomeroomRosterAction::Add,
            selections: vec![cm::HomeroomRosterSelection {
                student_academic_year_id: target,
                student_year_row_version: annual.row_version,
                placement_id: None,
                placement_row_version: None,
            }],
            effective_date: event_date(),
            target_homeroom_id: None,
            reason: "จัดห้องตามแผนปีใหม่ปัจจุบัน".into(),
        },
    )
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
    let row = preview
        .students
        .iter()
        .find(|row| row.student_academic_year_id == declaration.student_academic_year_id)
        .unwrap();
    assert_eq!(
        row.current_target.as_ref().unwrap().homeroom_id,
        Some(final_room.id)
    );
    assert_eq!(
        row.current_target.as_ref().unwrap().study_program_id,
        source.source_study_program_id
    );
    let decisions = preview
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
    let second_input = YearRolloverPreviewInput {
        decisions,
        ..input.clone()
    };
    confirm_year_rollover(
        providers,
        &pool,
        &actor,
        confirm_input(&pool, &actor, second_input).await,
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let proof = super::promotion_opening::read_in_transaction(
        &mut tx,
        input.source_year_id,
        input.target_year_id,
    )
    .await
    .unwrap();
    assert_eq!(proof.inconsistent_receipts, 0);
    tx.rollback().await.unwrap();
    let original: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_promotion_execution_receipts WHERE run_id=$1",
    )
    .bind(first.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(original, 5);
    let target_number: i32 = sqlx::query_scalar("SELECT year FROM academic_years WHERE id=$1")
        .bind(input.target_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE year<$1")
        .bind(target_number)
        .execute(&pool)
        .await
        .unwrap();
    let term = core::services::activation_context_tests::ready_term_for_existing_year(
        &pool,
        actor.user_id,
        input.target_year_id,
        event_date(),
    )
    .await;
    let workspace = get_activation_workspace(&pool, &actor, input.target_year_id, term.id)
        .await
        .unwrap();
    assert!(workspace.can_activate, "{:?}", workspace.findings);
    term_transitions::transition_term(
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
        student_years::get_student_year(&pool, target)
            .await
            .unwrap()
            .status,
        cm::StudentAcademicYearStatus::Active
    );
}

#[tokio::test]
async fn manual_year_rollover_reaffirms_audited_withdrawal_without_reactivating_student_or_blocking_others(
) {
    let (pool, actor, input) = fixture("manual_rollover_withdrawn_target").await;
    let providers = &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS;
    let first = confirm_year_rollover(
        providers,
        &pool,
        &actor,
        confirm_input(&pool, &actor, input.clone()).await,
    )
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
    let selected = preview
        .students
        .iter()
        .find(|row| row.existing_target_student_year_id.is_some())
        .unwrap();
    let student = selected.student_id;
    let target = selected.existing_target_student_year_id.unwrap();
    school_students::services::delete_student(
        &pool,
        student,
        actor.user_id,
        &crate::http_state::PLACEMENT_ROSTERS,
    )
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
    let withdrawn = preview
        .students
        .iter()
        .find(|row| row.student_id == student)
        .unwrap()
        .current_target
        .as_ref()
        .unwrap();
    assert_eq!(
        withdrawn.status,
        school_academic_core::models::StudentAcademicYearStatus::Withdrawn
    );
    assert!(withdrawn.homeroom_id.is_none());
    let decisions = preview
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
    confirm_year_rollover(
        providers,
        &pool,
        &actor,
        confirm_input(
            &pool,
            &actor,
            YearRolloverPreviewInput {
                decisions,
                ..input.clone()
            },
        )
        .await,
    )
    .await
    .unwrap();
    let number: i32 = sqlx::query_scalar("SELECT year FROM academic_years WHERE id=$1")
        .bind(input.target_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE year<$1")
        .bind(number)
        .execute(&pool)
        .await
        .unwrap();
    let term = core::services::activation_context_tests::ready_term_for_existing_year(
        &pool,
        actor.user_id,
        input.target_year_id,
        event_date(),
    )
    .await;
    let workspace = get_activation_workspace(&pool, &actor, input.target_year_id, term.id)
        .await
        .unwrap();
    assert!(workspace.can_activate, "{:?}", workspace.findings);
    assert_eq!(workspace.planned_students, 1);
    term_transitions::transition_term(
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
    let state:(String,String)=sqlx::query_as("SELECT u.status,annual.status FROM users u JOIN student_academic_years annual ON annual.student_id=u.id WHERE u.id=$1 AND annual.id=$2").bind(student).bind(target).fetch_one(&pool).await.unwrap();
    assert_eq!(state, ("inactive".into(), "withdrawn".into()));
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM homeroom_placements WHERE student_academic_year_id=$1 AND status IN ('planned','current')").bind(target).fetch_one(&pool).await.unwrap(),0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_promotion_execution_receipts WHERE run_id=$1"
        )
        .bind(first.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        5
    );
}

#[tokio::test]
async fn manual_year_rollover_501_cross_batch_failure_rolls_back_every_owner_then_exact_retry_succeeds(
) {
    let (pool, actor, mut input) = fixture("manual_rollover_501_atomic").await;
    let template = input
        .decisions
        .iter()
        .find(|row| row.decision.outcome == PromotionDecisionOutcome::Promote)
        .unwrap()
        .decision
        .clone();
    let students: Vec<Uuid> = (0..496).map(|_| Uuid::now_v7()).collect();
    sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) SELECT id,'synthetic-not-login','Batch pupil',id::text,'student','active' FROM unnest($1::uuid[]) AS id")
        .bind(&students).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO student_info(user_id,student_id,registration_status) SELECT id,id::text,'studying' FROM unnest($1::uuid[]) AS id")
        .bind(&students).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status)
        SELECT uuidv7(),student,$2,source.grade_level_id,source.study_program_id,'active' FROM unnest($1::uuid[]) AS student
        CROSS JOIN LATERAL(SELECT grade_level_id,study_program_id FROM student_academic_years WHERE academic_year_id=$2 ORDER BY id LIMIT 1) source")
        .bind(&students).bind(input.source_year_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type)
        SELECT uuidv7(),annual.id,$2,source.homeroom_id,$3,'current','promotion' FROM student_academic_years annual
        CROSS JOIN LATERAL(SELECT homeroom_id FROM homeroom_placements WHERE academic_year_id=$2 ORDER BY id LIMIT 1) source
        WHERE annual.student_id=ANY($1)")
        .bind(&students).bind(input.source_year_id).bind(event_date().pred_opt().unwrap()).execute(&pool).await.unwrap();
    let mut decision = template;
    decision.target_homeroom_id = None;
    let mut empty = input.clone();
    empty.decisions.clear();
    let rows = preview_year_rollover(&pool, &actor, empty)
        .await
        .unwrap()
        .students;
    assert_eq!(rows.len(), 501);
    let last_student = rows[500].student_id;
    input.decisions = rows
        .into_iter()
        .map(|row| YearRolloverStudentInput {
            student_academic_year_id: row.student_academic_year_id,
            decision: decision.clone(),
        })
        .collect();
    let command = confirm_input(&pool, &actor, input.clone()).await;
    let before = batch_owner_fingerprint(&pool).await;
    // A sequence survives transaction abort and proves that the second batch's
    // real INSERT observed all 500 first-batch target/source/audit writes.
    sqlx::query("CREATE SEQUENCE manual_rollover_batch_probe")
        .execute(&pool)
        .await
        .unwrap();
    let function = format!("CREATE FUNCTION manual_rollover_fail_second_batch() RETURNS trigger LANGUAGE plpgsql AS $body$
        BEGIN IF NEW.student_id='{last_student}'::uuid AND NEW.academic_year_id='{}'::uuid THEN
          IF (SELECT count(*) FROM student_academic_years WHERE academic_year_id=NEW.academic_year_id)=500
             AND (SELECT count(*) FROM student_academic_years WHERE academic_year_id='{}'::uuid AND status='completed')=500
             AND (SELECT count(*) FROM academic_audit_events WHERE event_code='student_academic_year.rollover_finalized')=500 THEN
            PERFORM setval('manual_rollover_batch_probe',2,true);
          ELSE PERFORM setval('manual_rollover_batch_probe',1,true); END IF;
          RAISE EXCEPTION 'MANUAL_ROLLOVER_SECOND_BATCH_TEST_FAILURE';
        END IF; RETURN NEW; END $body$",input.target_year_id,input.source_year_id);
    sqlx::query(sqlx::AssertSqlSafe(function))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER manual_rollover_batch_failure BEFORE INSERT ON student_academic_years FOR EACH ROW EXECUTE FUNCTION manual_rollover_fail_second_batch()")
        .execute(&pool).await.unwrap();
    let providers = &crate::adapters::academic_lifecycle::LIFECYCLE_PROVIDERS;
    assert!(
        confirm_year_rollover(providers, &pool, &actor, command.clone())
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT last_value FROM manual_rollover_batch_probe")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(batch_owner_fingerprint(&pool).await, before);
    let ledger: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM academic_promotion_runs)+(SELECT count(*) FROM academic_promotion_run_items)+(SELECT count(*) FROM academic_promotion_run_approvals)+(SELECT count(*) FROM academic_promotion_execution_batches)+(SELECT count(*) FROM academic_promotion_execution_receipts)")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(ledger, 0);
    sqlx::query("DROP TRIGGER manual_rollover_batch_failure ON student_academic_years")
        .execute(&pool)
        .await
        .unwrap();
    let prepared = confirm_year_rollover(providers, &pool, &actor, command.clone())
        .await
        .unwrap();
    assert_eq!(prepared.receipts.len(), 501);
    let workspace =
        school_academic_lifecycle::services::get_run_workspace(&pool, &actor, prepared.run_id)
            .await
            .unwrap();
    assert_eq!(workspace.students.len(), 501);
    assert_eq!(
        workspace
            .students
            .iter()
            .filter(|row| row.receipt.is_some())
            .count(),
        501
    );
    assert!(workspace.run.target_placement_starts_on.is_none());
    assert_eq!(
        prepared
            .receipts
            .iter()
            .filter_map(|row| row.target_student_year_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        501
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM student_academic_years WHERE academic_year_id=$1 AND status='planned'").bind(input.target_year_id).fetch_one(&pool).await.unwrap(),501);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM student_academic_years WHERE academic_year_id=$1 AND status='completed'").bind(input.source_year_id).fetch_one(&pool).await.unwrap(),501);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM homeroom_placements WHERE academic_year_id=$1 AND status='current'").bind(input.source_year_id).fetch_one(&pool).await.unwrap(),0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_promotion_execution_batches WHERE run_id=$1"
        )
        .bind(prepared.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        6
    );
    let accepted = batch_owner_fingerprint(&pool).await;
    let replay = confirm_year_rollover(providers, &pool, &actor, command)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(replay).unwrap(),
        serde_json::to_value(prepared).unwrap()
    );
    assert_eq!(batch_owner_fingerprint(&pool).await, accepted);
}

async fn batch_owner_fingerprint(pool: &PgPool) -> String {
    sqlx::query_scalar("SELECT jsonb_build_object(
      'users',(SELECT md5(string_agg(to_jsonb(row)::text,'' ORDER BY id)) FROM users row),
      'registration',(SELECT md5(string_agg(to_jsonb(row)::text,'' ORDER BY user_id)) FROM student_info row),
      'annuals',(SELECT md5(string_agg(to_jsonb(row)::text,'' ORDER BY id)) FROM student_academic_years row),
      'placements',(SELECT md5(string_agg(to_jsonb(row)::text,'' ORDER BY id)) FROM homeroom_placements row),
      'audits',(SELECT md5(string_agg(to_jsonb(row)::text,'' ORDER BY id)) FROM academic_audit_events row)
    )::text").fetch_one(pool).await.unwrap()
}

async fn assert_annual_transfer_and_old_year_authorization(
    pool: &PgPool,
    actor: &ActorContext,
    input: &YearRolloverPreviewInput,
    term: Uuid,
) {
    use school_academic_core::{
        models as cm,
        services::{attendance_roster, student_years},
    };
    let source = input
        .decisions
        .iter()
        .find(|row| row.decision.outcome == PromotionDecisionOutcome::Promote)
        .unwrap();
    let (student, old_room): (Uuid, Uuid) = sqlx::query_as("SELECT y.student_id,p.homeroom_id FROM student_academic_years y JOIN homeroom_placements p ON p.student_academic_year_id=y.id WHERE y.id=$1 AND p.placement_scope='academic_year' AND p.status='ended' AND p.end_date IS NULL")
        .bind(source.student_academic_year_id).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_advisors(homeroom_id,user_id,role) VALUES($1,$2,'primary') ON CONFLICT DO NOTHING")
        .bind(old_room).bind(actor.user_id).execute(pool).await.unwrap();
    assert!(
        !attendance_roster::advisor_for_student(pool, actor.user_id, student, event_date())
            .await
            .unwrap()
    );
    let old_term: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE academic_year_id=$1 ORDER BY sequence_no LIMIT 1",
    )
    .bind(input.source_year_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let historical = attendance_roster::students(pool, old_term, event_date())
        .await
        .unwrap();
    assert_eq!(
        historical
            .iter()
            .find(|row| row.student_id == student)
            .unwrap()
            .homeroom_id,
        Some(old_room)
    );
    let (placement, current_room, version): (Uuid, Uuid, i64) = sqlx::query_as("SELECT p.id,p.homeroom_id,p.row_version FROM homeroom_placements p JOIN student_academic_years y ON y.id=p.student_academic_year_id WHERE y.student_id=$1 AND y.academic_year_id=$2 AND p.status='current' AND p.placement_scope='academic_year' AND p.start_date IS NULL")
        .bind(student).bind(input.target_year_id).fetch_one(pool).await.unwrap();
    let target: Uuid = sqlx::query_scalar("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) SELECT uuidv7(),code||'-MOVE',name||' ย้ายจริง',academic_year_id,grade_level_id,study_program_id,'MOVE',10,true FROM homerooms WHERE id=$1 RETURNING id")
        .bind(current_room).fetch_one(pool).await.unwrap();
    let request = cm::TransferHomeroomPlacementRequest {
        target_homeroom_id: target,
        transfer_date: event_date(),
        enrollment_type: "transfer".into(),
        class_number: Some(1),
        reason: "ย้ายห้องจริงตามทะเบียน".into(),
        row_version: version,
        idempotency_key: Uuid::now_v7(),
    };
    let moved = student_years::transfer_placement(
        pool,
        &crate::http_state::PLACEMENT_ROSTERS,
        actor.user_id,
        placement,
        request.clone(),
    )
    .await
    .unwrap();
    assert_eq!(moved.ended_placement.start_date, None);
    assert_eq!(moved.ended_placement.end_date, event_date().pred_opt());
    assert_eq!(
        moved.new_placement.placement_scope,
        cm::HomeroomPlacementScope::Dated
    );
    assert_eq!(moved.new_placement.start_date, Some(event_date()));
    let replay = student_years::transfer_placement(
        pool,
        &crate::http_state::PLACEMENT_ROSTERS,
        actor.user_id,
        placement,
        request,
    )
    .await
    .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.new_placement.id, moved.new_placement.id);
    for (date, room) in [
        (event_date().pred_opt().unwrap(), current_room),
        (event_date(), target),
    ] {
        let roster = attendance_roster::students(pool, term, date).await.unwrap();
        assert_eq!(
            roster
                .iter()
                .find(|row| row.student_id == student)
                .unwrap()
                .homeroom_id,
            Some(room)
        );
        let wrong = attendance_roster::students_in_room(
            pool,
            term,
            date,
            if room == target { current_room } else { target },
        )
        .await
        .unwrap();
        assert!(!wrong.iter().any(|row| row.student_id == student));
    }
    sqlx::query("INSERT INTO homeroom_advisors(homeroom_id,user_id,role) VALUES($1,$2,'primary') ON CONFLICT DO NOTHING")
        .bind(target).bind(actor.user_id).execute(pool).await.unwrap();
    assert!(
        attendance_roster::advisor_for_student(pool, actor.user_id, student, event_date())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn manual_year_rollover_legacy_conditional_current_edits_and_withdrawal_activate_without_reconfirmation(
) {
    use school_academic_core::{
        models as cm,
        services::{homeroom_roster, student_years},
    };
    let (pool, reviewer, _, _, calculation) =
        super::promotion_review_tests::ready_run_with_extra_student(
            "manual_rollover_legacy_direct_open",
        )
        .await;
    let actor = ActorContext {
        user_id: reviewer.user_id,
        permissions: vec![codes::WILDCARD.into()],
    };
    let mut run = calculation.run.clone();
    let mut items = vec![];
    for item in &calculation.items {
        let result = review_item(
            &pool,
            &actor,
            run.id,
            item.id,
            ReviewPromotionItemInput {
                row_version: item.row_version,
                decision: PromotionDecisionInput {
                    outcome: PromotionDecisionOutcome::Conditional,
                    target_grade_level_id: Some(item.source_grade_level_id),
                    target_study_program_id: Some(item.source_study_program_id),
                    target_homeroom_id: None,
                    reason: Some("ผลเดิมจากทะเบียนโรงเรียน".into()),
                    condition: Some("ติดตามเอกสารเดิม".into()),
                },
            },
        )
        .await
        .unwrap();
        run = result.run;
        items.push(result.item);
    }
    let run = approve_run(
        &pool,
        &actor,
        run.id,
        ApprovePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: run.row_version,
            source_checksum: super::promotion_approval::intent_checksum(&run, &items).unwrap(),
        },
    )
    .await
    .unwrap();
    let accepted = execute_run(
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
    assert_eq!(accepted.receipts.len(), 2);
    let original:sqlx::types::Json<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r) ORDER BY item_id) FROM academic_promotion_execution_receipts r WHERE run_id=$1").bind(run.id).fetch_one(&pool).await.unwrap();
    let target = accepted.receipts[0].target_student_year_id.unwrap();
    let annual = student_years::get_student_year(&pool, target)
        .await
        .unwrap();
    let edition:Uuid=sqlx::query_scalar("INSERT INTO curriculum_editions(name,revision_year) VALUES('Legacy current planning',2570) RETURNING id").fetch_one(&pool).await.unwrap();
    let level:Uuid=sqlx::query_scalar("INSERT INTO curriculum_levels(edition_id,code,name_th,grade_level_ids,is_active) VALUES($1,'LEGACY-CURRENT','Legacy current',$2,true) RETURNING id")
        .bind(edition).bind(sqlx::types::Json(vec![annual.grade_level_id])).fetch_one(&pool).await.unwrap();
    let program:Uuid=sqlx::query_scalar("INSERT INTO study_programs(id,curriculum_level_id,code,name_th,status) VALUES(uuidv7(),$1,'LEGACY-CURRENT','Legacy current','published') RETURNING id").bind(level).fetch_one(&pool).await.unwrap();
    sqlx::query(
        "SELECT capture_curriculum_publication($1,NULL,'Synthetic legacy current projection')",
    )
    .bind(edition)
    .execute(&pool)
    .await
    .unwrap();
    student_years::update_student_year(
        &pool,
        actor.user_id,
        target,
        cm::UpdateStudentAcademicYearRequest {
            grade_level_id: annual.grade_level_id,
            study_program_id: program,
            row_version: annual.row_version,
        },
    )
    .await
    .unwrap();
    let room = student_years::create_homeroom(
        &pool,
        cm::CreateHomeroomRequest {
            academic_year_id: run.target_year_id,
            grade_level_id: annual.grade_level_id,
            study_program_id: program,
            custom_name: Some("Legacy current room".into()),
            room_number: "90".into(),
            capacity: 5,
        },
    )
    .await
    .unwrap();
    let roster = homeroom_roster::get_roster(&pool, room.id).await.unwrap();
    let current = student_years::get_student_year(&pool, target)
        .await
        .unwrap();
    homeroom_roster::mutate_roster(
        &pool,
        &crate::http_state::PLACEMENT_ROSTERS,
        actor.user_id,
        room.id,
        cm::MutateHomeroomRosterRequest {
            revision: roster.revision,
            action: cm::HomeroomRosterAction::Add,
            selections: vec![cm::HomeroomRosterSelection {
                student_academic_year_id: target,
                student_year_row_version: current.row_version,
                placement_id: None,
                placement_row_version: None,
            }],
            effective_date: run.target_placement_starts_on.unwrap(),
            target_homeroom_id: None,
            reason: "จัดห้องตามแผนปัจจุบัน".into(),
        },
    )
    .await
    .unwrap();
    let target_number: i32 = sqlx::query_scalar("SELECT year FROM academic_years WHERE id=$1")
        .bind(run.target_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='closed' WHERE academic_year_id IN (SELECT id FROM academic_years WHERE year<$1)").bind(target_number).execute(&pool).await.unwrap();
    sqlx::query("UPDATE academic_years SET status='closed' WHERE year<$1")
        .bind(target_number)
        .execute(&pool)
        .await
        .unwrap();
    let withdrawn_target = accepted.receipts[1].target_student_year_id.unwrap();
    let withdrawn = student_years::get_student_year(&pool, withdrawn_target)
        .await
        .unwrap();
    school_students::services::delete_student(
        &pool,
        withdrawn.student_id,
        actor.user_id,
        &crate::http_state::PLACEMENT_ROSTERS,
    )
    .await
    .unwrap();
    let term = core::services::activation_context_tests::ready_term_for_existing_year(
        &pool,
        actor.user_id,
        run.target_year_id,
        run.target_placement_starts_on.unwrap(),
    )
    .await;
    let workspace = get_activation_workspace(&pool, &actor, run.target_year_id, term.id)
        .await
        .unwrap();
    assert!(workspace.can_activate, "{:?}", workspace.findings);
    assert_eq!(workspace.planned_students, 1);
    term_transitions::transition_term(
        &pool,
        &actor,
        term.id,
        TermTransitionRequest {
            academic_year_id: run.target_year_id,
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
        student_years::get_student_year(&pool, target)
            .await
            .unwrap()
            .status,
        cm::StudentAcademicYearStatus::Active
    );
    assert_eq!(
        student_years::get_student_year(&pool, withdrawn_target)
            .await
            .unwrap()
            .status,
        cm::StudentAcademicYearStatus::Withdrawn
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM users WHERE id=$1")
            .bind(withdrawn.student_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "inactive"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_promotion_run_commands WHERE action='confirm'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let preserved:sqlx::types::Json<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(r) ORDER BY item_id) FROM academic_promotion_execution_receipts r WHERE run_id=$1").bind(run.id).fetch_one(&pool).await.unwrap();
    assert_eq!(preserved.0, original.0);
}
