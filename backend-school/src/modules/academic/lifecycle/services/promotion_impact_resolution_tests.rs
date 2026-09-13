use super::*;
use crate::{
    middleware::permission::ActorContext,
    modules::academic::{
        lifecycle::models::*,
        results::{models as rm, services as rs},
    },
    permissions::registry::codes,
};
use uuid::Uuid;

#[tokio::test]
async fn authorized_keep_existing_resolves_only_the_exact_impact_and_replays() {
    let (pool, executor, corrector, context, calc, approved) =
        super::promotion_execution::tests::approved_fixture(
            "promotion_impact_resolve_keep",
            PromotionDecisionOutcome::Hold,
        )
        .await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
        .await
        .unwrap();
    execute_run(
        &pool,
        &executor,
        calc.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    let course: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(calc.items[0].student_academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    rs::correct_result(
        &pool,
        &corrector,
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
    let actor = ActorContext {
        user_id: executor.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_CORRECT_SCHOOL.into(),
        ],
    };
    let before: String = sqlx::query_scalar(
        "SELECT md5(jsonb_build_object(
            'students',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM student_academic_years r),
            'placements',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM homeroom_placements r)
        )::text)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let impact = get_promotion_impacts(
        &pool,
        &actor,
        calc.run.id,
        PromotionImpactQuery { after_id: None },
    )
    .await
    .unwrap();
    let request = ResolvePromotionImpactInput {
        request_id: Uuid::new_v4(),
        source_checksum: impact.source_checksum.clone(),
        resolution_kind: PromotionImpactResolutionKind::KeepExisting,
        replacement_decision: None,
        reason: "ตรวจแล้ว ผลแก้ไขไม่เปลี่ยนคำสั่งเลื่อนชั้นเดิม".into(),
    };
    for permissions in [
        vec![],
        vec![codes::ACADEMIC_PROMOTION_READ_SCHOOL],
        vec![codes::ACADEMIC_PROMOTION_CORRECT_SCHOOL],
    ] {
        let denied = ActorContext {
            user_id: actor.user_id,
            permissions: permissions.into_iter().map(String::from).collect(),
        };
        assert!(matches!(
            resolve_promotion_impact(
                &pool,
                &denied,
                calc.run.id,
                impact.impacts[0].id,
                request.clone(),
            )
            .await,
            Err(crate::error::AppError::Forbidden(_))
        ));
    }
    let mut invalid_reason = request.clone();
    invalid_reason.request_id = Uuid::new_v4();
    invalid_reason.reason = "เลขที่ไม่ควรเก็บ 1234567890123".into();
    assert!(matches!(
        resolve_promotion_impact(
            &pool,
            &actor,
            calc.run.id,
            impact.impacts[0].id,
            invalid_reason,
        )
        .await,
        Err(crate::error::AppError::ValidationError(_))
    ));
    let mut stale = request.clone();
    stale.request_id = Uuid::new_v4();
    stale.source_checksum = "0".repeat(64);
    assert!(matches!(
        resolve_promotion_impact(&pool, &actor, calc.run.id, impact.impacts[0].id, stale,).await,
        Err(crate::error::AppError::Conflict(_))
    ));
    let mut foreign_impact = request.clone();
    foreign_impact.request_id = Uuid::new_v4();
    assert!(matches!(
        resolve_promotion_impact(&pool, &actor, calc.run.id, Uuid::new_v4(), foreign_impact,).await,
        Err(crate::error::AppError::NotFound(_))
    ));
    let (resolved, replay) = tokio::join!(
        resolve_promotion_impact(
            &pool,
            &actor,
            calc.run.id,
            impact.impacts[0].id,
            request.clone(),
        ),
        resolve_promotion_impact(
            &pool,
            &actor,
            calc.run.id,
            impact.impacts[0].id,
            request.clone(),
        )
    );
    let resolved = resolved.unwrap();
    let replay = replay.unwrap();
    assert_eq!(
        resolved.resolution_kind,
        PromotionImpactResolutionKind::KeepExisting
    );
    assert!(!resolved.outcome.adjusted);
    assert_eq!(
        serde_json::to_value(&resolved).unwrap(),
        serde_json::to_value(replay).unwrap()
    );
    let mut request_collision = request.clone();
    request_collision.reason = "เปลี่ยนเนื้อหาของคำขอเดิม".into();
    assert!(matches!(
        resolve_promotion_impact(
            &pool,
            &actor,
            calc.run.id,
            impact.impacts[0].id,
            request_collision,
        )
        .await,
        Err(crate::error::AppError::Conflict(_))
    ));
    let after = get_promotion_impacts(
        &pool,
        &actor,
        calc.run.id,
        PromotionImpactQuery { after_id: None },
    )
    .await
    .unwrap();
    assert_eq!(after.pending_count, 0);
    assert_eq!(
        after.impacts[0].resolution.as_ref().unwrap().id,
        resolved.id
    );
    let retained: String = sqlx::query_scalar(
        "SELECT md5(jsonb_build_object(
            'students',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM student_academic_years r),
            'placements',(SELECT jsonb_agg(to_jsonb(r) ORDER BY id) FROM homeroom_placements r)
        )::text)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained, before);
    let already_resolved = ResolvePromotionImpactInput {
        request_id: Uuid::new_v4(),
        source_checksum: after.source_checksum,
        resolution_kind: PromotionImpactResolutionKind::KeepExisting,
        replacement_decision: None,
        reason: "ลองจัดการผลกระทบเดิมซ้ำด้วยคำขอใหม่".into(),
    };
    assert!(matches!(
        resolve_promotion_impact(
            &pool,
            &actor,
            calc.run.id,
            impact.impacts[0].id,
            already_resolved.clone(),
        )
        .await,
        Err(crate::error::AppError::Conflict(_))
    ));
    // A newer official correction is new evidence and must not inherit the old
    // resolution merely because it belongs to the same student and run item.
    rs::correct_result(
        &pool,
        &corrector,
        &context,
        rm::ResultCorrectionInput::Course {
            course_result_id: course,
            outcome: rm::CourseOfficialOutcome::Numeric,
            numeric_grade: Some("3".into()),
            expected_effective_version: 2,
        },
    )
    .await
    .unwrap();
    let later = get_promotion_impacts(
        &pool,
        &actor,
        calc.run.id,
        PromotionImpactQuery { after_id: None },
    )
    .await
    .unwrap();
    assert_eq!(later.total_count, 2);
    assert_eq!(later.pending_count, 1);
    assert_eq!(
        later
            .impacts
            .iter()
            .filter(|row| row.resolution.is_some())
            .count(),
        1
    );
}

#[tokio::test]
async fn replacement_decision_reconciles_only_receipt_owned_planned_target() {
    let (pool, executor, corrector, context, calc, approved) =
        super::promotion_execution::tests::approved_fixture(
            "promotion_impact_replace",
            PromotionDecisionOutcome::Promote,
        )
        .await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
        .await
        .unwrap();
    let execution = execute_run(
        &pool,
        &executor,
        calc.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    let receipt = execution.receipts[0].clone();
    let course: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(calc.items[0].student_academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    rs::correct_result(
        &pool,
        &corrector,
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
    let actor = ActorContext {
        user_id: executor.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_CORRECT_SCHOOL.into(),
        ],
    };
    let impact = get_promotion_impacts(
        &pool,
        &actor,
        calc.run.id,
        PromotionImpactQuery { after_id: None },
    )
    .await
    .unwrap();
    let resolution = resolve_promotion_impact(
        &pool,
        &actor,
        calc.run.id,
        impact.impacts[0].id,
        ResolvePromotionImpactInput {
            request_id: Uuid::new_v4(),
            source_checksum: impact.source_checksum,
            resolution_kind: PromotionImpactResolutionKind::ReplaceDecision,
            replacement_decision: Some(PromotionDecisionInput {
                outcome: PromotionDecisionOutcome::Hold,
                target_grade_level_id: None,
                target_study_program_id: None,
                target_homeroom_id: None,
                reason: Some("รอตรวจผลแก้ไขรอบถัดไป".into()),
                condition: None,
            }),
            reason: "ผลรายปีเปลี่ยนและต้องพักการสร้างข้อมูลปีใหม่".into(),
        },
    )
    .await
    .unwrap();
    assert!(resolution.outcome.adjusted);
    assert!(resolution.outcome.target_student_year_id.is_none());
    assert!(resolution.outcome.target_placement_id.is_none());
    let target_status: String =
        sqlx::query_scalar("SELECT status FROM student_academic_years WHERE id=$1")
            .bind(receipt.target_student_year_id.unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    let placement_status: String =
        sqlx::query_scalar("SELECT status FROM homeroom_placements WHERE id=$1")
            .bind(receipt.target_placement_id.unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(target_status, "withdrawn");
    assert_eq!(placement_status, "ended");
    let retained_receipt: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT target_student_year_id,target_placement_id FROM academic_promotion_execution_receipts WHERE item_id=$1",
    )
    .bind(calc.items[0].id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        retained_receipt,
        (receipt.target_student_year_id, receipt.target_placement_id)
    );
    let mut tx = pool.begin().await.unwrap();
    let opening = super::promotion_opening::read_in_transaction(
        &mut tx,
        calc.run.source_year_id,
        calc.run.target_year_id,
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(opening.unresolved_impacts, 0);
    assert_eq!(opening.inconsistent_receipts, 0);
    assert_eq!(opening.covered_students, 1);

    // A later official correction is a distinct impact. Replacing the prior hold
    // with a target-producing decision must reuse the receipt-owned target rows,
    // rather than collide with their retained immutable history.
    rs::correct_result(
        &pool,
        &corrector,
        &context,
        rm::ResultCorrectionInput::Course {
            course_result_id: course,
            outcome: rm::CourseOfficialOutcome::Numeric,
            numeric_grade: Some("3".into()),
            expected_effective_version: 2,
        },
    )
    .await
    .unwrap();
    let impacts = get_promotion_impacts(
        &pool,
        &actor,
        calc.run.id,
        PromotionImpactQuery { after_id: None },
    )
    .await
    .unwrap();
    assert_eq!(impacts.total_count, 2);
    assert_eq!(impacts.pending_count, 1);
    let pending = impacts
        .impacts
        .iter()
        .find(|impact| impact.resolution.is_none())
        .unwrap();
    let sqlx::types::Json(original_decision): sqlx::types::Json<PromotionDecisionInput> =
        sqlx::query_scalar("SELECT decision FROM academic_promotion_run_items WHERE id=$1")
            .bind(calc.items[0].id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let restored = resolve_promotion_impact(
        &pool,
        &actor,
        calc.run.id,
        pending.id,
        ResolvePromotionImpactInput {
            request_id: Uuid::new_v4(),
            source_checksum: impacts.source_checksum,
            resolution_kind: PromotionImpactResolutionKind::ReplaceDecision,
            replacement_decision: Some(original_decision),
            reason: "ผลแก้ไขล่าสุดรองรับการเลื่อนชั้นตามผลเดิม".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        restored.outcome.target_student_year_id,
        receipt.target_student_year_id
    );
    assert_eq!(
        restored.outcome.target_placement_id,
        receipt.target_placement_id
    );
    let restored_state: (String, String) = sqlx::query_as(
        "SELECT student_year.status,placement.status
		 FROM student_academic_years student_year
		 JOIN homeroom_placements placement ON placement.student_academic_year_id=student_year.id
		 WHERE student_year.id=$1 AND placement.id=$2",
    )
    .bind(receipt.target_student_year_id.unwrap())
    .bind(receipt.target_placement_id.unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(restored_state, ("planned".into(), "planned".into()));
}

#[tokio::test]
async fn replacement_resolution_rolls_back_core_rows_when_audit_write_fails() {
    let (pool, executor, corrector, context, calc, approved) =
        super::promotion_execution::tests::approved_fixture(
            "promotion_impact_resolution_rollback",
            PromotionDecisionOutcome::Promote,
        )
        .await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 78)
        .await
        .unwrap();
    let execution = execute_run(
        &pool,
        &executor,
        calc.run.id,
        ExecutePromotionRunInput {
            request_id: Uuid::new_v4(),
            row_version: approved.row_version,
            limit: 100,
        },
    )
    .await
    .unwrap();
    let receipt = execution.receipts[0].clone();
    let course: Uuid = sqlx::query_scalar(
		"SELECT id FROM academic_course_results WHERE student_academic_year_id=$1 ORDER BY id LIMIT 1",
	)
	.bind(calc.items[0].student_academic_year_id)
	.fetch_one(&pool)
	.await
	.unwrap();
    rs::correct_result(
        &pool,
        &corrector,
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
    let actor = ActorContext {
        user_id: executor.user_id,
        permissions: vec![
            codes::ACADEMIC_PROMOTION_READ_SCHOOL.into(),
            codes::ACADEMIC_PROMOTION_CORRECT_SCHOOL.into(),
        ],
    };
    let impact = get_promotion_impacts(
        &pool,
        &actor,
        calc.run.id,
        PromotionImpactQuery { after_id: None },
    )
    .await
    .unwrap();
    let input = ResolvePromotionImpactInput {
        request_id: Uuid::new_v4(),
        source_checksum: impact.source_checksum,
        resolution_kind: PromotionImpactResolutionKind::ReplaceDecision,
        replacement_decision: Some(PromotionDecisionInput {
            outcome: PromotionDecisionOutcome::Hold,
            target_grade_level_id: None,
            target_study_program_id: None,
            target_homeroom_id: None,
            reason: Some("รอตรวจหลักฐานเพิ่มเติม".into()),
            condition: None,
        }),
        reason: "ทดสอบความเป็นธุรกรรมของการจัดการผลกระทบ".into(),
    };
    sqlx::raw_sql(
        "CREATE FUNCTION fail_promotion_impact_audit() RETURNS trigger LANGUAGE plpgsql AS $$
		 BEGIN IF NEW.event_code='promotion_impact.resolved' THEN
		 RAISE EXCEPTION 'fixture audit unavailable'; END IF; RETURN NEW; END $$;
		 CREATE TRIGGER fail_promotion_impact_audit BEFORE INSERT ON academic_audit_events
		 FOR EACH ROW EXECUTE FUNCTION fail_promotion_impact_audit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(resolve_promotion_impact(
        &pool,
        &actor,
        calc.run.id,
        impact.impacts[0].id,
        input.clone(),
    )
    .await
    .is_err());
    let retained: (String, String, i64) = sqlx::query_as(
        "SELECT student_year.status,placement.status,
		        (SELECT count(*) FROM academic_promotion_impact_resolutions)
		 FROM student_academic_years student_year
		 JOIN homeroom_placements placement ON placement.student_academic_year_id=student_year.id
		 WHERE student_year.id=$1 AND placement.id=$2",
    )
    .bind(receipt.target_student_year_id.unwrap())
    .bind(receipt.target_placement_id.unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained, ("planned".into(), "planned".into(), 0));
    sqlx::raw_sql(
        "DROP TRIGGER fail_promotion_impact_audit ON academic_audit_events;
		 DROP FUNCTION fail_promotion_impact_audit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let retried = resolve_promotion_impact(&pool, &actor, calc.run.id, impact.impacts[0].id, input)
        .await
        .unwrap();
    assert!(retried.outcome.adjusted);
    let changed: (String, String, i64) = sqlx::query_as(
        "SELECT student_year.status,placement.status,
		        (SELECT count(*) FROM academic_promotion_impact_resolutions)
		 FROM student_academic_years student_year
		 JOIN homeroom_placements placement ON placement.student_academic_year_id=student_year.id
		 WHERE student_year.id=$1 AND placement.id=$2",
    )
    .bind(receipt.target_student_year_id.unwrap())
    .bind(receipt.target_placement_id.unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(changed, ("withdrawn".into(), "ended".into(), 1));
}
