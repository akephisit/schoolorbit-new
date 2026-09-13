use super::*;
use crate::modules::academic::core::services::promotion_targets::tests::fixture;

async fn source_year(pool: &sqlx::PgPool, student_year: Uuid) -> Uuid {
    sqlx::query_scalar("SELECT academic_year_id FROM student_academic_years WHERE id=$1")
        .bind(student_year)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn actor(pool: &sqlx::PgPool) -> Uuid {
    sqlx::query_scalar("SELECT id FROM users ORDER BY id LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn refresh_source(
    pool: &sqlx::PgPool,
    source_year_id: Uuid,
    target_year_id: Uuid,
    student_year_id: Uuid,
) -> PromotionStudentContext {
    let mut tx = pool.begin().await.unwrap();
    let source = super::super::promotion_students::read_promotion_students(
        &mut tx,
        source_year_id,
        target_year_id,
        &[student_year_id],
    )
    .await
    .unwrap()
    .remove(0);
    tx.commit().await.unwrap();
    source
}

#[tokio::test]
async fn reconciliation_reuses_owned_target_and_placement_when_destination_changes() {
    let (pool, source, target, mut decision) = fixture("promotion_reconcile_destination").await;
    let year = source_year(&pool, source.student_academic_year_id).await;
    let user = actor(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let created = reconcile_decision(
        &mut tx,
        user,
        year,
        target,
        source.student_academic_year_id,
        None,
        None,
        &decision,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let target_student_year_id = created.target_student_year_id.unwrap();
    let mut rooms = Vec::new();
    for number in ["1", "2"] {
        rooms.push(
            sqlx::query_scalar(
                "INSERT INTO homerooms(
                     code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active
                 ) VALUES($1,$1,$2,$3,$4,$5,30,true) RETURNING id",
            )
            .bind(format!("E2E-RECONCILE-{number}"))
            .bind(target)
            .bind(decision.target_grade_level_id)
            .bind(decision.target_study_program_id)
            .bind(number)
            .fetch_one(&pool)
            .await
            .unwrap(),
        );
    }
    decision.target_homeroom_id = Some(rooms[0]);
    let source = refresh_source(&pool, year, target, source.student_academic_year_id).await;
    let mut tx = pool.begin().await.unwrap();
    let placed = reconcile_decision(
        &mut tx,
        user,
        year,
        target,
        source.student_academic_year_id,
        Some(target_student_year_id),
        None,
        &decision,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let placement_id = placed.target_placement_id.unwrap();
    decision.target_homeroom_id = Some(rooms[1]);
    let source = refresh_source(&pool, year, target, source.student_academic_year_id).await;
    let mut tx = pool.begin().await.unwrap();
    let moved = reconcile_decision(
        &mut tx,
        user,
        year,
        target,
        source.student_academic_year_id,
        Some(target_student_year_id),
        Some(placement_id),
        &decision,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(moved.target_student_year_id, Some(target_student_year_id));
    assert_eq!(moved.target_placement_id, Some(placement_id));
    let destination: (Uuid, String, String) = sqlx::query_as(
        "SELECT placement.homeroom_id,student_year.status,placement.status
         FROM student_academic_years student_year
         JOIN homeroom_placements placement ON placement.student_academic_year_id=student_year.id
         WHERE student_year.id=$1 AND placement.id=$2",
    )
    .bind(target_student_year_id)
    .bind(placement_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(destination, (rooms[1], "planned".into(), "planned".into()));
}

#[tokio::test]
async fn reconciliation_rejects_capacity_foreign_ownership_and_nonplanning_year_without_writes() {
    let (pool, source, target, mut decision) = fixture("promotion_reconcile_guards").await;
    let room: Uuid = sqlx::query_scalar(
        "INSERT INTO homerooms(
             code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active
         ) VALUES('E2E-RECONCILE','E2E-LIFECYCLE-full',$1,$2,$3,'1',0,true)
         RETURNING id",
    )
    .bind(target)
    .bind(decision.target_grade_level_id)
    .bind(decision.target_study_program_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    decision.target_homeroom_id = Some(room);
    let year = source_year(&pool, source.student_academic_year_id).await;
    let user = actor(&pool).await;
    let before: (String, i64) =
        sqlx::query_as("SELECT status,row_version FROM student_academic_years WHERE id=$1")
            .bind(source.student_academic_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        reconcile_decision(
            &mut tx,
            user,
            year,
            target,
            source.student_academic_year_id,
            None,
            None,
            &decision,
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    let target_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM student_academic_years WHERE student_id=$1 AND academic_year_id=$2",
    )
    .bind(source.student_id)
    .bind(target)
    .fetch_one(&pool)
    .await
    .unwrap();
    let after: (String, i64) =
        sqlx::query_as("SELECT status,row_version FROM student_academic_years WHERE id=$1")
            .bind(source.student_academic_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(target_rows, 0);
    assert_eq!(after, before);

    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        reconcile_decision(
            &mut tx,
            user,
            year,
            target,
            source.student_academic_year_id,
            Some(Uuid::new_v4()),
            None,
            &decision,
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();

    sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        reconcile_decision(
            &mut tx,
            user,
            year,
            target,
            source.student_academic_year_id,
            None,
            None,
            &decision,
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn reconciliation_sets_terminal_source_status_without_creating_target_rows() {
    for (suffix, outcome, expected) in [
        ("graduate", PromotionDecisionOutcome::Graduate, "graduated"),
        (
            "transfer",
            PromotionDecisionOutcome::TransferOut,
            "withdrawn",
        ),
    ] {
        let (pool, source, target, _) = fixture(&format!("promotion_reconcile_{suffix}")).await;
        let year = source_year(&pool, source.student_academic_year_id).await;
        if outcome == PromotionDecisionOutcome::Graduate {
            sqlx::query(
                "INSERT INTO grade_level_progressions(from_grade_level_id,transition_kind)
                 VALUES($1,'graduate') ON CONFLICT DO NOTHING",
            )
            .bind(source.grade_level_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        let decision = PromotionDecisionInput {
            outcome,
            target_grade_level_id: None,
            target_study_program_id: None,
            target_homeroom_id: None,
            reason: Some("ยืนยันสถานะปลายปี".into()),
            condition: None,
        };
        let mut tx = pool.begin().await.unwrap();
        let result = reconcile_decision(
            &mut tx,
            actor(&pool).await,
            year,
            target,
            source.student_academic_year_id,
            None,
            None,
            &decision,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert!(result.target_student_year_id.is_none());
        assert!(result.target_placement_id.is_none());
        let status: String =
            sqlx::query_scalar("SELECT status FROM student_academic_years WHERE id=$1")
                .bind(source.student_academic_year_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let target_rows: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM student_academic_years WHERE student_id=$1 AND academic_year_id=$2",
        )
        .bind(source.student_id)
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, expected);
        assert_eq!(target_rows, 0);
    }
}
