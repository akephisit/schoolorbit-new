use crate::integration_tests::academic::{
    core,
    services::{exam_schedule_service, timetable_service},
};
use async_trait::async_trait;
use school_academic_core::ports::{
    PendingTermWork, TermPreparationContext, TermPreparationMappingKind,
    TermPreparationModuleOutcome,
};
use school_academic_lifecycle::ports::ExternalLifecyclePort;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

pub(super) struct StubExternalLifecyclePort {
    pub(super) exam_revision: &'static str,
    pub(super) fail_exams: bool,
}

#[async_trait]
impl ExternalLifecyclePort for StubExternalLifecyclePort {
    async fn admit_preparation(
        &self,
        _context: &school_academic_lifecycle::ports::PreparationCommandContext,
        _year: Uuid,
        _term: Uuid,
        _include_timetable: bool,
    ) -> Result<Box<dyn school_academic_lifecycle::ports::PreparationAdmission>, AppError> {
        Err(AppError::Conflict("unused preparation provider".into()))
    }

    async fn require_reopen_allowed(
        &self,
        _transaction: &mut Transaction<'_, Postgres>,
        _academic_year_id: Uuid,
        _academic_term_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn apply_registration_outcomes(
        &self,
        _transaction: &mut Transaction<'_, Postgres>,
        _actor_user_id: Uuid,
        _outcomes: &[(Uuid, school_academic_lifecycle::ports::RegistrationOutcome)],
    ) -> Result<(), AppError> {
        Err(AppError::Conflict("unused registration provider".into()))
    }

    async fn require_attendance_delivery_complete(
        &self,
        _transaction: &mut Transaction<'_, Postgres>,
        _academic_term_id: Uuid,
        _closed_on: chrono::NaiveDate,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn pending_exam_work(
        &self,
        _transaction: &mut Transaction<'_, Postgres>,
        _academic_year_id: Uuid,
        _academic_term_id: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError> {
        if self.fail_exams {
            return Err(AppError::Conflict("exam provider failed".into()));
        }
        Ok(vec![PendingTermWork {
            id: Uuid::from_u128(1),
            revision: self.exam_revision.into(),
            blocks_closure: false,
        }])
    }

    async fn pending_supervision_work(
        &self,
        _transaction: &mut Transaction<'_, Postgres>,
        _academic_year_id: Uuid,
        _academic_term_id: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError> {
        Ok(Vec::new())
    }

    async fn apply_exam_preparation(
        &self,
        _transaction: &mut Transaction<'_, Postgres>,
        _actor_user_id: Uuid,
        _context: &TermPreparationContext,
        _entity_mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
        _date_mappings: &HashMap<chrono::NaiveDate, chrono::NaiveDate>,
    ) -> Result<TermPreparationModuleOutcome, AppError> {
        Err(AppError::Conflict("unused exam preparation stub".into()))
    }

    async fn apply_supervision_preparation(
        &self,
        _transaction: &mut Transaction<'_, Postgres>,
        _actor_user_id: Uuid,
        _context: &TermPreparationContext,
        _entity_mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
        _date_mappings: &HashMap<chrono::NaiveDate, chrono::NaiveDate>,
    ) -> Result<TermPreparationModuleOutcome, AppError> {
        Err(AppError::Conflict(
            "unused supervision preparation stub".into(),
        ))
    }
}

#[tokio::test]
async fn lifecycle_external_port_failures_propagate_and_evidence_changes_the_checksum() {
    let pool = core::services_tests::prepare_core_fixture("lifecycle_external_port_contract").await;
    let (year, term): (Uuid, Uuid) =
        sqlx::query_as("SELECT academic_year_id,id FROM academic_terms WHERE status='active'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor = school_authorization::ActorContext {
        user_id: sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap(),
        permissions: vec![school_permissions::registry::codes::WILDCARD.into()],
    };
    let mut transaction = pool.begin().await.unwrap();
    let failure = school_academic_lifecycle::services::readiness::workspace_in_transaction(
        &StubExternalLifecyclePort {
            exam_revision: "ignored",
            fail_exams: true,
        },
        &mut transaction,
        &actor,
        year,
        term,
    )
    .await;
    assert!(
        matches!(failure, Err(AppError::Conflict(message)) if message == "exam provider failed")
    );

    let before = school_academic_lifecycle::services::readiness::workspace_in_transaction(
        &StubExternalLifecyclePort {
            exam_revision: "revision-1",
            fail_exams: false,
        },
        &mut transaction,
        &actor,
        year,
        term,
    )
    .await
    .unwrap();
    let after = school_academic_lifecycle::services::readiness::workspace_in_transaction(
        &StubExternalLifecyclePort {
            exam_revision: "revision-2",
            fail_exams: false,
        },
        &mut transaction,
        &actor,
        year,
        term,
    )
    .await
    .unwrap();
    assert_ne!(before.source_checksum, after.source_checksum);
}

#[tokio::test]
async fn lifecycle_exam_readiness_changes_when_child_assignments_change() {
    let pool = core::services_tests::prepare_core_fixture("lifecycle_exam_fingerprint").await;
    let (year, term, date): (Uuid, Uuid, chrono::NaiveDate) = sqlx::query_as(
        "SELECT academic_year_id,id,(now() AT TIME ZONE 'Asia/Bangkok')::date FROM academic_terms WHERE status='active'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let round:Uuid=sqlx::query_scalar("INSERT INTO academic_exam_rounds(academic_year_id,academic_term_id,name) VALUES($1,$2,'E2E-LIFECYCLE-details') RETURNING id").bind(year).bind(term).fetch_one(&pool).await.unwrap();
    let day:Uuid=sqlx::query_scalar("INSERT INTO academic_exam_days(exam_round_id,academic_year_id,academic_term_id,exam_date,start_time,end_time) VALUES($1,$2,$3,$4,'08:00','16:00') RETURNING id").bind(round).bind(year).bind(term).bind(date).fetch_one(&pool).await.unwrap();
    let homeroom: Uuid =
        sqlx::query_scalar("SELECT id FROM homerooms WHERE academic_year_id=$1 LIMIT 1")
            .bind(year)
            .fetch_one(&pool)
            .await
            .unwrap();
    let room: Uuid = sqlx::query_scalar("SELECT id FROM rooms LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let staff: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let student: Uuid =
        sqlx::query_scalar("SELECT id FROM users WHERE user_type='student' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let assignment:Uuid=sqlx::query_scalar("INSERT INTO academic_exam_day_room_assignments(exam_day_id,academic_year_id,academic_term_id,homeroom_id,room_id,capacity_override) VALUES($1,$2,$3,$4,$5,20) RETURNING id").bind(day).bind(year).bind(term).bind(homeroom).bind(room).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_day_blocked_windows(exam_day_id,label,start_time,end_time) VALUES($1,'พัก','12:00','13:00')").bind(day).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_day_invigilators(exam_day_id,day_room_assignment_id,staff_id,role_label) VALUES($1,$2,$3,'กรรมการ')").bind(day).bind(assignment).bind(staff).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_seat_assignments(day_room_assignment_id,student_id,seat_number) VALUES($1,$2,'1')").bind(assignment).bind(student).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_exam_day_grade_levels(exam_day_id,grade_level_id) SELECT $1,id FROM grade_levels ORDER BY id LIMIT 1").bind(day).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let mut before = exam_schedule_service::pending_term_work(&mut tx, year, term)
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == round)
        .unwrap()
        .revision;
    for (sql, id) in [
        (
            "UPDATE academic_exam_day_blocked_windows SET label='พักกลางวัน' WHERE exam_day_id=$1",
            day,
        ),
        (
            "UPDATE academic_exam_day_room_assignments SET capacity_override=25 WHERE id=$1",
            assignment,
        ),
        (
            "UPDATE academic_exam_day_invigilators SET role_label='หัวหน้าห้องสอบ' WHERE exam_day_id=$1",
            day,
        ),
        (
            "UPDATE academic_exam_seat_assignments SET seat_number='2' WHERE day_room_assignment_id=$1",
            assignment,
        ),
        (
            "DELETE FROM academic_exam_day_grade_levels WHERE exam_day_id=$1",
            day,
        ),
    ] {
        assert_eq!(
            sqlx::query(sql)
                .bind(id)
                .execute(&mut *tx)
                .await
                .unwrap()
                .rows_affected(),
            1
        );
        let after = exam_schedule_service::pending_term_work(&mut tx, year, term)
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == round)
            .unwrap()
            .revision;
        assert_ne!(
            before, after,
            "changed exam child data must invalidate readiness: {sql}"
        );
        before = after;
    }
}

#[tokio::test]
async fn lifecycle_current_delivery_has_no_pending_publication_or_change_set_work() {
    let pool = core::services_tests::prepare_core_fixture("lifecycle_current_providers").await;
    let (year, term): (Uuid, Uuid) = sqlx::query_as(
        "SELECT academic_year_id,id FROM academic_terms WHERE status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let actor = school_authorization::ActorContext {
        user_id: sqlx::query_scalar(
            "SELECT id FROM users WHERE user_type='staff' ORDER BY id LIMIT 1",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap(),
        permissions: vec![school_permissions::registry::codes::WILDCARD.into()],
    };
    let workspace = super::workspace_in_transaction(&mut tx, &actor, year, term)
        .await
        .unwrap();
    assert!(
        workspace
            .findings
            .iter()
            .all(|finding| !finding.code.starts_with("delivery."))
    );

    assert!(
        school_academic_delivery::services::pending_term_work(&mut tx, year, term)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        school_academic_delivery::services::pending_term_work(&mut tx, Uuid::new_v4(), term)
            .await
            .unwrap()
            .is_empty()
    );
    let pending = exam_schedule_service::pending_term_work(&mut tx, year, term)
        .await
        .unwrap();
    assert!(pending.iter().all(|row| !row.blocks_closure));
    assert!(
        exam_schedule_service::pending_term_work(&mut tx, year, Uuid::new_v4())
            .await
            .unwrap()
            .is_empty()
    );
    let tables = timetable_service::pending_term_work(&mut tx, year, term)
        .await
        .unwrap();
    assert!(tables.iter().all(|row| !row.blocks_closure));
}

#[tokio::test]
async fn lifecycle_permission_defaults_only_grant_verified_system_administrators() {
    use school_permissions::registry::codes;
    let pool = core::services_tests::prepare_core_fixture("lifecycle_permission_defaults").await;
    let expected = vec![
        codes::ACADEMIC_LIFECYCLE_READ_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_CLOSE_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_REOPEN_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_ACTIVATE_SCHOOL,
    ];
    let grants:Vec<(String,bool,String)>=sqlx::query_as("SELECT role.code,role.is_system,permission.code FROM role_permissions grant_row JOIN roles role ON role.id=grant_row.role_id JOIN permissions permission ON permission.id=grant_row.permission_id WHERE permission.code=ANY($1)")
        .bind(&expected).fetch_all(&pool).await.unwrap();
    for permission in expected {
        assert!(grants.iter().any(|(_, _, code)| code == permission));
    }
    assert!(
        grants
            .iter()
            .all(|(role, system, _)| role == "ADMIN" && *system)
    );
}
