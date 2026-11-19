use crate::codes;
use chrono::{NaiveDate, NaiveTime};
use school_academic_core::ports::{
    PendingTermWork, TermPreparationContext, TermPreparationMappingKind,
    TermPreparationModuleOutcome,
};
use school_academic_lifecycle::{models::*, ports::*, services};
use school_academic_timetable::services::dated_teaching::{
    self, DatedDutyInput, DatedTeachingLesson,
};
use school_authorization::ActorContext;
use school_errors::AppError;
use serde_json::{json, Value};
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

/// These external domains are empty in this fixture. Timetable readiness and
/// every lifecycle transition are the production providers under test.
pub struct EmptyExternal;
#[async_trait::async_trait]
impl ExternalLifecyclePort for EmptyExternal {
    async fn admit_preparation(
        &self,
        _: &PreparationCommandContext,
        _: Uuid,
        _: Uuid,
        _: bool,
    ) -> Result<Box<dyn PreparationAdmission>, AppError> {
        unreachable!()
    }
    async fn require_reopen_allowed(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: Uuid,
        _: Option<Uuid>,
    ) -> Result<(), AppError> {
        unreachable!()
    }
    async fn apply_registration_outcomes(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: Uuid,
        _: &[(Uuid, RegistrationOutcome)],
    ) -> Result<(), AppError> {
        unreachable!()
    }
    async fn require_attendance_delivery_complete(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: Uuid,
        _: NaiveDate,
    ) -> Result<(), AppError> {
        Ok(())
    }
    async fn pending_exam_work(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: Uuid,
        _: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError> {
        Ok(vec![])
    }
    async fn pending_supervision_work(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: Uuid,
        _: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError> {
        Ok(vec![])
    }
    async fn apply_exam_preparation(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: Uuid,
        _: &TermPreparationContext,
        _: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
        _: &HashMap<NaiveDate, NaiveDate>,
    ) -> Result<TermPreparationModuleOutcome, AppError> {
        unreachable!()
    }
    async fn apply_supervision_preparation(
        &self,
        _: &mut Transaction<'_, Postgres>,
        _: Uuid,
        _: &TermPreparationContext,
        _: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
        _: &HashMap<NaiveDate, NaiveDate>,
    ) -> Result<TermPreparationModuleOutcome, AppError> {
        unreachable!()
    }
}

pub struct Fixture {
    pub pool: PgPool,
    pub actor: ActorContext,
    pub teacher: Uuid,
    pub year: Uuid,
    pub term: Uuid,
}

impl Fixture {
    pub async fn new(name: &str, term_status: &str) -> Self {
        let pool = school_test_db::create_named_test_pool_with_max_connections(name, 4).await;
        school_test_db::run_test_migrations(&pool).await;
        let year = Uuid::now_v7();
        let term = Uuid::now_v7();
        let bell = Uuid::now_v7();
        let actor = Uuid::now_v7();
        let teacher = Uuid::now_v7();
        for id in [actor, teacher] {
            sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','Closure','Fixture','staff','active')")
                .bind(id).bind(id.to_string()).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO academic_years(id,year,name,status) VALUES($1,2998,'Coverage closure','active')")
            .bind(year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO bell_schedules(id,academic_year_id,code,name) VALUES($1,$2,'CLOSURE','Closure')")
            .bind(bell).bind(year).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO academic_terms(id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id,school_days) VALUES($1,$2,'Optional coverage',1,'CLOSURE','custom',false,false,$3,$4,'MON,TUE,WED,THU,FRI,SAT,SUN')")
            .bind(term).bind(year).bind(term_status).bind(bell).execute(&pool).await.unwrap();
        // A current board is deliberately present: it has no publication work.
        sqlx::query("INSERT INTO academic_timetables(academic_year_id,academic_term_id,bell_schedule_id) VALUES($1,$2,$3)")
            .bind(year).bind(term).bind(bell).execute(&pool).await.unwrap();
        Self {
            pool,
            actor: ActorContext {
                user_id: actor,
                permissions: vec![codes::WILDCARD.into()],
            },
            teacher,
            year,
            term,
        }
    }

    pub async fn local_now(&self) -> chrono::NaiveDateTime {
        sqlx::query_scalar("SELECT clock_timestamp() AT TIME ZONE 'Asia/Bangkok'")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    pub fn duty(&self, date: NaiveDate, start: NaiveTime, end: NaiveTime) -> DatedDutyInput {
        let allocation = Uuid::now_v7();
        let lesson = DatedTeachingLesson {
            lesson_key: dated_teaching::lesson_key(date, allocation, self.actor.user_id),
            date,
            block_id: Uuid::now_v7(),
            allocation_id: allocation,
            teacher_id: self.actor.user_id,
            bell_schedule_period_id: Uuid::now_v7(),
            period_name: "Captured lesson".into(),
            start_time: start,
            end_time: end,
            learning_group_id: None,
            learning_group_name: None,
            offering_id: None,
            offering_name: None,
            grade_level_ids: vec![],
            homeroom_ids: vec![],
            homeroom_names: vec![],
            room_id: None,
            room_name: None,
            source_revision: 1,
            allocation_revision: 1,
        };
        DatedDutyInput {
            id: Uuid::now_v7(),
            teacher_id: self.teacher,
            original_teacher_id: self.actor.user_id,
            date,
            start_time: start,
            end_time: end,
            lesson: Some(lesson),
            kind: "substitution".into(),
            pair_id: None,
            pair_leg: None,
            taught_offering_id: None,
            taught_learning_group_id: None,
            taught_activity: Some("Captured school lesson".into()),
        }
    }

    pub async fn reserve(&self, duties: &[DatedDutyInput]) -> Uuid {
        let request = Uuid::now_v7();
        let document = json!({"fixture": "accepted dated coverage", "duties": duties.iter().map(|d| d.id).collect::<Vec<_>>()});
        let mut tx = self.pool.begin().await.unwrap();
        dated_teaching::lock_context(&mut tx, self.year, self.term)
            .await
            .unwrap();
        // Minimal persisted accepted request/plan facts. No source subject or
        // Attendance universe is needed to test the resource-lifecycle boundary.
        sqlx::query("INSERT INTO staff_leave_requests(id,owner_id,type_id,status,revision,document,content_hash) SELECT $1,$2,id,'approved',1,$3,repeat('a',64) FROM staff_leave_types WHERE code='personal'")
            .bind(request).bind(self.actor.user_id).bind(Json(&document)).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO staff_leave_coverage_plan_revisions(request_id,revision,document,plan_hash) VALUES($1,1,$2,repeat('b',64))")
            .bind(request).bind(Json(&document)).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO staff_leave_coverage_plans(request_id,revision,academic_year_id,academic_term_id,document,plan_hash) VALUES($1,1,$2,$3,$4,repeat('b',64))")
            .bind(request).bind(self.year).bind(self.term).bind(Json(&document)).execute(&mut *tx).await.unwrap();
        dated_teaching::reserve_on_connection(&mut tx, request, 1, self.year, self.term, duties)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        request
    }

    pub async fn approve(&self, request: Uuid) {
        let mut tx = self.pool.begin().await.unwrap();
        dated_teaching::lock_context(&mut tx, self.year, self.term)
            .await
            .unwrap();
        dated_teaching::approve_on_connection(&mut tx, request)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    pub async fn release(&self, request: Uuid, future_only: bool) {
        let mut tx = self.pool.begin().await.unwrap();
        dated_teaching::lock_context(&mut tx, self.year, self.term)
            .await
            .unwrap();
        dated_teaching::release_on_connection(&mut tx, request, future_only)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    pub async fn term_workspace(&self) -> TermLifecycleWorkspace {
        services::get_workspace(
            &EmptyExternal,
            &self.pool,
            &self.actor,
            self.year,
            self.term,
        )
        .await
        .unwrap()
    }

    pub async fn term_input(&self, action: TermTransitionAction) -> TermTransitionRequest {
        let ws = self.term_workspace().await;
        TermTransitionRequest {
            academic_year_id: self.year,
            request_id: Uuid::now_v7(),
            action,
            expected_year_version: ws.context.year_row_version,
            expected_term_version: ws.context.term_row_version,
            readiness_checksum: ws.source_checksum,
            acknowledged_warning_codes: vec![],
            reason: None,
        }
    }

    pub async fn year_input(&self, action: YearTransitionAction) -> YearTransitionRequest {
        let ws = services::get_year_workspace(&self.pool, &self.actor, self.year)
            .await
            .unwrap();
        YearTransitionRequest {
            request_id: Uuid::now_v7(),
            action,
            expected_year_version: ws.context.row_version,
            readiness_checksum: ws.source_checksum,
            acknowledged_warning_codes: if action == YearTransitionAction::Close {
                ws.findings
                    .iter()
                    .filter(|f| f.severity == LifecycleSeverity::Warning)
                    .map(|f| f.code.clone())
                    .collect()
            } else {
                vec![]
            },
        }
    }

    pub async fn evidence(&self, request: Uuid) -> Value {
        let Json(value): Json<Value> = sqlx::query_scalar("SELECT jsonb_build_object('request',to_jsonb(r),'plan',to_jsonb(p),'revisions',(SELECT jsonb_agg(to_jsonb(v) ORDER BY revision) FROM staff_leave_coverage_plan_revisions v WHERE v.request_id=r.id),'duties',(SELECT jsonb_agg(to_jsonb(d) ORDER BY id) FROM academic_teaching_coverage_duties d WHERE d.request_id=r.id)) FROM staff_leave_requests r JOIN staff_leave_coverage_plans p ON p.request_id=r.id WHERE r.id=$1")
            .bind(request).fetch_one(&self.pool).await.unwrap();
        value
    }

    pub async fn assert_no_attendance_materialized(&self) {
        let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM attendance_days WHERE academic_term_id=$1),(SELECT count(*) FROM attendance_sessions WHERE academic_term_id=$1)")
            .bind(self.term).fetch_one(&self.pool).await.unwrap();
        assert_eq!(counts, (0, 0));
    }
}
