use super::*;
use crate::modules::academic::{
    core::services_tests::prepare_core_fixture,
    lifecycle::models::PromotionDecisionOutcome as Outcome,
};
use sqlx::{types::Json, PgPool};

#[tokio::test]
async fn promotion_destination_batch_rejects_empty_duplicate_oversized_and_mixed_invalid_targets() {
    let (pool, source, target, input) = fixture("promotion_destination_batch").await;
    let mut second = source.clone();
    second.student_academic_year_id = Uuid::new_v4();
    second.student_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        validate_destinations(&mut tx, target, &[(&source, &input), (&second, &input)])
            .await
            .is_ok()
    );
    for invalid in [
        vec![],
        vec![(&source, &input), (&source, &input)],
        vec![(&source, &input); 501],
    ] {
        assert!(matches!(
            validate_destinations(&mut tx, target, &invalid).await,
            Err(AppError::ValidationError(_))
        ));
    }
    let mut invalid = input.clone();
    invalid.target_study_program_id = Some(Uuid::new_v4());
    assert!(matches!(
        validate_destinations(&mut tx, target, &[(&source, &input), (&second, &invalid)]).await,
        Err(AppError::ValidationError(_))
    ));
}

async fn program(
    pool: &PgPool,
    start: Uuid,
    end: Option<Uuid>,
    grades: Vec<Uuid>,
    published: bool,
) -> Uuid {
    let id = Uuid::new_v4();
    let curriculum:Uuid=sqlx::query_scalar("INSERT INTO curricula(code,identity_key,name_th,grade_level_ids) VALUES ($1,$1,'E2E-LIFECYCLE-target',$2) RETURNING id")
        .bind(format!("e2e-{}",id.simple())).bind(Json(grades)).fetch_one(pool).await.unwrap();
    let version:Uuid=sqlx::query_scalar("INSERT INTO curriculum_versions(curriculum_id,version_name,start_academic_year_id,end_academic_year_id,status) VALUES ($1,'E2E-LIFECYCLE',$2,$3,'draft') RETURNING id")
        .bind(curriculum).bind(start).bind(end).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO study_programs(id,curriculum_version_id,code,name_th,status) VALUES ($1,$2,'E2E','E2E-LIFECYCLE-target',$3)")
        .bind(id).bind(version).bind(if published {"published"} else {"draft"}).execute(pool).await.unwrap();
    if published {
        sqlx::query(
            "UPDATE curriculum_versions SET status='published',published_at=now() WHERE id=$1",
        )
        .bind(version)
        .execute(pool)
        .await
        .unwrap();
    }
    id
}

pub(crate) async fn fixture(
    name: &str,
) -> (
    PgPool,
    PromotionStudentContext,
    Uuid,
    PromotionDecisionInput,
) {
    let pool = prepare_core_fixture(name).await;
    let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE status='active'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let student: Uuid = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(year)
    .fetch_one(&pool)
    .await
    .unwrap();
    let target:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) SELECT max(year)+1,'E2E-LIFECYCLE-target',max(end_date)+1,max(end_date)+366,'MON','planning' FROM academic_years RETURNING id").fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let source = super::super::promotion_students::read_promotion_students(
        &mut tx,
        year,
        target,
        &[student],
    )
    .await
    .unwrap()
    .remove(0);
    tx.commit().await.unwrap();
    let grade: Uuid = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE id<>$1 AND is_active IS TRUE ORDER BY id LIMIT 1",
    )
    .bind(source.grade_level_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO grade_level_progressions(from_grade_level_id,to_grade_level_id,transition_kind) VALUES ($1,$2,'exception')")
        .bind(source.grade_level_id).bind(grade).execute(&pool).await.unwrap();
    let target_program = program(
        &pool,
        target,
        None,
        vec![source.grade_level_id, grade],
        true,
    )
    .await;
    (
        pool,
        source,
        target,
        PromotionDecisionInput {
            outcome: Outcome::Promote,
            target_grade_level_id: Some(grade),
            target_study_program_id: Some(target_program),
            target_homeroom_id: None,
            reason: Some("พิจารณาย้ายหลักสูตร".into()),
            condition: None,
        },
    )
}

#[tokio::test]
async fn promotion_destination_requires_published_applicable_program_and_explicit_grade_coverage() {
    let (pool, source, target, input) = fixture("promotion_destination_program").await;
    let source_year: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM student_academic_years WHERE id=$1")
            .bind(source.student_academic_year_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let grade = input.target_grade_level_id.unwrap();
    let invalid_programs = [
        program(&pool, target, None, vec![grade], false).await,
        program(&pool, source_year, Some(source_year), vec![grade], true).await,
        program(&pool, target, None, vec![source.grade_level_id], true).await,
        program(&pool, target, None, vec![], true).await,
        Uuid::new_v4(),
    ];
    let mut tx = pool.begin().await.unwrap();
    assert!(validate_destination(&mut tx, &source, target, &input)
        .await
        .is_ok());
    for program in invalid_programs {
        let mut invalid = input.clone();
        invalid.target_study_program_id = Some(program);
        assert!(matches!(
            validate_destination(&mut tx, &source, target, &invalid).await,
            Err(AppError::ValidationError(_))
        ));
    }
    assert!(
        validate_destination(&mut tx, &source, Uuid::new_v4(), &input)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn promotion_destination_checks_transition_and_never_adopts_existing_enrollment() {
    let (pool, source, target, input) = fixture("promotion_destination_transition").await;
    let mut tx = pool.begin().await.unwrap();
    assert!(validate_destination(&mut tx, &source, target, &input)
        .await
        .is_ok());
    let mut occupied = source.clone();
    occupied.existing_target_student_year_id = Some(Uuid::new_v4());
    assert!(matches!(
        validate_destination(&mut tx, &occupied, target, &input).await,
        Err(AppError::Conflict(_))
    ));
    for status in [
        super::super::super::models::StudentAcademicYearStatus::Planned,
        super::super::super::models::StudentAcademicYearStatus::Withdrawn,
        super::super::super::models::StudentAcademicYearStatus::Graduated,
    ] {
        let mut ended = source.clone();
        ended.status = status;
        assert!(matches!(
            validate_destination(&mut tx, &ended, target, &input).await,
            Err(AppError::Conflict(_))
        ));
    }
    let mut terminal = input.clone();
    terminal.outcome = Outcome::Graduate;
    terminal.target_grade_level_id = None;
    terminal.target_study_program_id = None;
    sqlx::query("DELETE FROM grade_level_progressions WHERE from_grade_level_id=$1 AND transition_kind='graduate'").bind(source.grade_level_id).execute(&mut *tx).await.unwrap();
    assert!(validate_destination(&mut tx, &source, target, &terminal)
        .await
        .is_err());
    sqlx::query("INSERT INTO grade_level_progressions(from_grade_level_id,transition_kind) VALUES ($1,'graduate')").bind(source.grade_level_id).execute(&mut *tx).await.unwrap();
    assert!(validate_destination(&mut tx, &source, target, &terminal)
        .await
        .is_ok());
    terminal.outcome = Outcome::TransferOut;
    assert!(validate_destination(&mut tx, &source, target, &terminal)
        .await
        .is_ok());
    terminal.outcome = Outcome::Hold;
    assert!(validate_destination(&mut tx, &occupied, target, &terminal)
        .await
        .is_ok());
    sqlx::query("UPDATE grade_level_progressions SET is_active=false WHERE from_grade_level_id=$1")
        .bind(source.grade_level_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(validate_destination(&mut tx, &source, target, &input)
        .await
        .is_err());
}

#[tokio::test]
async fn promotion_destination_checks_homeroom_year_grade_program_and_active_status() {
    let (pool, source, target, mut input) = fixture("promotion_destination_homeroom").await;
    let other_program = program(
        &pool,
        target,
        None,
        vec![input.target_grade_level_id.unwrap()],
        true,
    )
    .await;
    let mut tx = pool.begin().await.unwrap();
    let room:Uuid=sqlx::query_scalar("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,room_number,capacity,is_active) VALUES (gen_random_uuid(),'E2E-LIFECYCLE-room','E2E-LIFECYCLE-room',$1,$2,$3,'E2E',30,true) RETURNING id")
        .bind(target).bind(input.target_grade_level_id).bind(input.target_study_program_id).fetch_one(&mut *tx).await.unwrap();
    input.target_homeroom_id = Some(room);
    assert!(validate_destination(&mut tx, &source, target, &input)
        .await
        .is_ok());
    let old_room: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id<>$1 ORDER BY id LIMIT 1",
    )
    .bind(target)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    input.target_homeroom_id = Some(old_room);
    assert!(validate_destination(&mut tx, &source, target, &input)
        .await
        .is_err());
    input.target_homeroom_id = Some(room);
    sqlx::query("UPDATE homerooms SET is_active=false WHERE id=$1")
        .bind(room)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(validate_destination(&mut tx, &source, target, &input)
        .await
        .is_err());
    sqlx::query("UPDATE homerooms SET is_active=true,grade_level_id=$2 WHERE id=$1")
        .bind(room)
        .bind(source.grade_level_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(validate_destination(&mut tx, &source, target, &input)
        .await
        .is_err());
    sqlx::query("UPDATE homerooms SET grade_level_id=$2,study_program_id=$3 WHERE id=$1")
        .bind(room)
        .bind(input.target_grade_level_id)
        .bind(other_program)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(validate_destination(&mut tx, &source, target, &input)
        .await
        .is_err());
}
