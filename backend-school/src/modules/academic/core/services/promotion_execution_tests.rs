use super::super::{lifecycle_guard, promotion_targets::tests::fixture};
use super::*;
use crate::modules::academic::lifecycle::models::PromotionDecisionOutcome as Outcome;

async fn source_year(pool: &sqlx::PgPool, source: &PromotionStudentContext) -> Uuid {
    sqlx::query_scalar("SELECT academic_year_id FROM student_academic_years WHERE id=$1")
        .bind(source.student_academic_year_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn placements(
    tx: &mut Transaction<'_, Postgres>,
    source: &PromotionStudentContext,
) -> String {
    sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(p) ORDER BY id)::text,'[]') FROM homeroom_placements p WHERE student_academic_year_id=$1")
        .bind(source.student_academic_year_id).fetch_one(&mut **tx).await.unwrap()
}

#[tokio::test]
async fn promotion_core_execution_creates_only_planned_destination_and_preserves_source() {
    let (pool, source, target, mut decision) = fixture("promotion_core_planned").await;
    let year = source_year(&pool, &source).await;
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::lock_transition(&mut tx).await.unwrap();
    let before = placements(&mut tx, &source).await;
    let room: Uuid = sqlx::query_scalar("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) VALUES(gen_random_uuid(),'E2E-EXEC','E2E-EXEC',$1,$2,$3,'1',1,true) RETURNING id")
        .bind(target).bind(decision.target_grade_level_id).bind(decision.target_study_program_id).fetch_one(&mut *tx).await.unwrap();
    decision.target_homeroom_id = Some(room);
    let result = execute_decision(&mut tx, source.student_id, year, target, &source, &decision)
        .await
        .unwrap();
    let state: (String, Uuid, Uuid, Uuid) = sqlx::query_as("SELECT status,academic_year_id,grade_level_id,study_program_id FROM student_academic_years WHERE id=$1")
        .bind(result.target_student_year_id).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(
        state,
        (
            "planned".into(),
            target,
            decision.target_grade_level_id.unwrap(),
            decision.target_study_program_id.unwrap()
        )
    );
    let placement: (String, Uuid, chrono::NaiveDate) =
        sqlx::query_as("SELECT status,homeroom_id,start_date FROM homeroom_placements WHERE id=$1")
            .bind(result.target_placement_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    let start = sqlx::query_scalar::<_, chrono::NaiveDate>(
        "SELECT start_date FROM academic_years WHERE id=$1",
    )
    .bind(target)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(placement, ("planned".into(), room, start));
    assert_eq!(placements(&mut tx, &source).await, before);
    assert_eq!(result.source_row_version, source.row_version);
    assert!(matches!(
        execute_decision(&mut tx, source.student_id, year, target, &source, &decision).await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM student_academic_years WHERE academic_year_id=$1")
            .bind(target)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count, 0,
        "Core must not commit outside the caller's receipt transaction"
    );
}

#[tokio::test]
async fn promotion_core_execution_terminal_and_hold_never_change_source_placements() {
    let (pool, source, target, mut decision) = fixture("promotion_core_terminal").await;
    let year = source_year(&pool, &source).await;
    decision.target_grade_level_id = None;
    decision.target_study_program_id = None;
    for outcome in [Outcome::Hold, Outcome::TransferOut, Outcome::Graduate] {
        decision.outcome = outcome;
        let mut tx = pool.begin().await.unwrap();
        lifecycle_guard::lock_transition(&mut tx).await.unwrap();
        if outcome == Outcome::Graduate {
            sqlx::query("INSERT INTO grade_level_progressions(from_grade_level_id,transition_kind) VALUES($1,'graduate') ON CONFLICT DO NOTHING").bind(source.grade_level_id).execute(&mut *tx).await.unwrap();
        }
        let before = placements(&mut tx, &source).await;
        let result = execute_decision(&mut tx, source.student_id, year, target, &source, &decision)
            .await
            .unwrap();
        assert!(result.target_student_year_id.is_none());
        assert!(result.target_placement_id.is_none());
        let state: (String, i64) =
            sqlx::query_as("SELECT status,row_version FROM student_academic_years WHERE id=$1")
                .bind(source.student_academic_year_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        let expected = match outcome {
            Outcome::Hold => ("active", source.row_version),
            Outcome::TransferOut => ("withdrawn", source.row_version + 1),
            _ => ("graduated", source.row_version + 1),
        };
        assert_eq!(state, (expected.0.into(), expected.1));
        assert_eq!(result.source_row_version, expected.1);
        assert_eq!(placements(&mut tx, &source).await, before);
        tx.rollback().await.unwrap();
    }
}

#[tokio::test]
async fn promotion_core_execution_rechecks_source_version_and_foreign_destination() {
    let (pool, source, target, decision) = fixture("promotion_core_stale").await;
    let year = source_year(&pool, &source).await;
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::lock_transition(&mut tx).await.unwrap();
    sqlx::query("UPDATE student_academic_years SET row_version=row_version+1 WHERE id=$1")
        .bind(source.student_academic_year_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(matches!(
        execute_decision(&mut tx, source.student_id, year, target, &source, &decision).await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::lock_transition(&mut tx).await.unwrap();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES(gen_random_uuid(),$1,$2,$3,$4,'planned')")
        .bind(source.student_id).bind(target).bind(decision.target_grade_level_id).bind(decision.target_study_program_id).execute(&mut *tx).await.unwrap();
    assert!(matches!(
        execute_decision(&mut tx, source.student_id, year, target, &source, &decision).await,
        Err(AppError::Conflict(_))
    ));
}

#[tokio::test]
async fn promotion_core_execution_rejects_full_room_without_creating_enrollment() {
    let (pool, source, target, mut decision) = fixture("promotion_core_capacity").await;
    let year = source_year(&pool, &source).await;
    let mut tx = pool.begin().await.unwrap();
    lifecycle_guard::lock_transition(&mut tx).await.unwrap();
    let room: Uuid = sqlx::query_scalar("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) VALUES(gen_random_uuid(),'E2E-FULL','E2E-FULL',$1,$2,$3,'1',1,true) RETURNING id")
        .bind(target).bind(decision.target_grade_level_id).bind(decision.target_study_program_id).fetch_one(&mut *tx).await.unwrap();
    let other: Uuid = sqlx::query_scalar("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES(gen_random_uuid(),'E2E-ROOM-CAPACITY','not-an-auth-credential','E2E','Capacity','student','active') RETURNING id")
        .fetch_one(&mut *tx).await.unwrap();
    let enrolled: Uuid = sqlx::query_scalar("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) VALUES(gen_random_uuid(),$1,$2,$3,$4,'planned') RETURNING id")
        .bind(other).bind(target).bind(decision.target_grade_level_id).bind(decision.target_study_program_id).fetch_one(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type) SELECT gen_random_uuid(),$1,$2,$3,start_date,'planned','promotion' FROM academic_years WHERE id=$2")
        .bind(enrolled).bind(target).bind(room).execute(&mut *tx).await.unwrap();
    decision.target_homeroom_id = Some(room);
    assert!(matches!(
        execute_decision(&mut tx, source.student_id, year, target, &source, &decision).await,
        Err(AppError::Conflict(_))
    ));
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM student_academic_years WHERE academic_year_id=$1 AND student_id=$2",
    )
    .bind(target)
    .bind(source.student_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(count, 0);
    // Releasing the planned place must make the same approved destination usable.
    sqlx::query("UPDATE homeroom_placements SET status='ended',end_date=start_date WHERE student_academic_year_id=$1").bind(enrolled).execute(&mut *tx).await.unwrap();
    assert!(
        execute_decision(&mut tx, source.student_id, year, target, &source, &decision)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn promotion_core_execution_repeat_and_conditional_keep_explicit_targets() {
    let (pool, source, target, mut decision) = fixture("promotion_core_repeat_condition").await;
    let year = source_year(&pool, &source).await;
    for outcome in [Outcome::Repeat, Outcome::Conditional] {
        let mut tx = pool.begin().await.unwrap();
        lifecycle_guard::lock_transition(&mut tx).await.unwrap();
        decision.outcome = outcome;
        if outcome == Outcome::Repeat {
            decision.target_grade_level_id = Some(source.grade_level_id);
            sqlx::query("INSERT INTO grade_level_progressions(from_grade_level_id,to_grade_level_id,transition_kind) VALUES($1,$1,'repeat') ON CONFLICT DO NOTHING")
                .bind(source.grade_level_id).execute(&mut *tx).await.unwrap();
        } else {
            decision.condition = Some("ติดตามงานที่ต้องปรับปรุง".into());
            sqlx::query("INSERT INTO grade_level_progressions(from_grade_level_id,to_grade_level_id,transition_kind) VALUES($1,$1,'repeat') ON CONFLICT DO NOTHING")
                .bind(source.grade_level_id).execute(&mut *tx).await.unwrap();
        }
        let result = execute_decision(&mut tx, source.student_id, year, target, &source, &decision)
            .await
            .unwrap();
        assert!(result.target_placement_id.is_none());
        let grade: Uuid =
            sqlx::query_scalar("SELECT grade_level_id FROM student_academic_years WHERE id=$1")
                .bind(result.target_student_year_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        assert_eq!(grade, decision.target_grade_level_id.unwrap());
        tx.rollback().await.unwrap();
    }
}
