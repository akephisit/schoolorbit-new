use school_academic_delivery::{
    models::{LearningTeacherRole, ReplaceLearningGroupTeachersRequest, TeacherAssignmentInput},
    ports::AssessmentCurrentPort,
    services::{
        current::{
            assigned_group_ids_on_connection, group_teacher_assignments_on_connection,
            teacher_allocations_on_connection,
        },
        groups,
    },
};
use school_errors::AppError;
use school_test_db::cutover_test_support::{
    apply_migrations_through, seed_release_two_predecessor,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

struct Fixture {
    pool: PgPool,
    year: Uuid,
    term: Uuid,
    offering: Uuid,
    group: Uuid,
    teacher: Uuid,
    other: Uuid,
}
async fn fixture(name: &str) -> Fixture {
    let pool = school_test_db::create_named_test_pool(name).await;
    seed_release_two_predecessor(&pool).await.unwrap();
    apply_migrations_through(&pool, 157).await.unwrap();
    let source:Uuid=sqlx::query_scalar("SELECT learning_offering_id FROM course_offering_details ORDER BY learning_offering_id LIMIT 1").fetch_one(&pool).await.unwrap();
    let term = Uuid::now_v7();
    let offering = Uuid::now_v7();
    let group = Uuid::now_v7();
    let teacher = Uuid::now_v7();
    let other = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    let year:Uuid=sqlx::query_scalar("INSERT INTO academic_terms(school_days,id,academic_year_id,name,sequence_no,code,term_type,included_in_year_result,blocks_year_closure,status,bell_schedule_id) SELECT term.school_days,$1,offering.academic_year_id,'Current teachers',1000,'CURRENT-TEACHERS','custom',false,false,'planning',term.bell_schedule_id FROM learning_offerings offering JOIN academic_terms term ON term.id=offering.academic_term_id WHERE offering.id=$2 RETURNING academic_year_id").bind(term).bind(source).fetch_one(&mut *tx).await.unwrap();
    // The predecessor's oldest source year is archived; this isolated fixture
    // deliberately exercises the writable current-term operations in a planning year.
    sqlx::query("UPDATE academic_years SET status='planning' WHERE id=$1")
        .bind(year)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO learning_offerings(id,academic_term_id,academic_year_id,kind,code_snapshot,name_snapshot,weekly_period_target,owning_organization_unit_id) SELECT $1,$2,academic_year_id,'course','CURRENT-TEACHERS','Current teachers',0,owning_organization_unit_id FROM learning_offerings WHERE id=$3").bind(offering).bind(term).bind(source).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO course_offering_details(learning_offering_id,academic_term_id,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score) SELECT $1,$2,academic_year_id,subject_version_id,subject_id,credit,hours,assessment_total_score FROM course_offering_details WHERE learning_offering_id=$3").bind(offering).bind(term).bind(source).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) SELECT id,id::text,'fixture-not-a-login','Current','Teacher','staff','active' FROM unnest($1::uuid[]) id").bind(vec![teacher,other]).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name) VALUES($1,$2,$3,$4,'CURRENT','Current group')").bind(group).bind(offering).bind(term).bind(year).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role) VALUES(uuidv7(),$1,$2,$3,$4,'primary')").bind(group).bind(term).bind(year).bind(teacher).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    Fixture {
        pool,
        year,
        term,
        offering,
        group,
        teacher,
        other,
    }
}

#[tokio::test]
async fn current_assignments_are_immediate_context_scoped_and_require_active_accounts() {
    let f = fixture("delivery_current_teacher_membership").await;
    let mut connection = f.pool.acquire().await.unwrap();
    let assignment = group_teacher_assignments_on_connection(
        &mut connection,
        f.year,
        f.term,
        &[f.group, f.group],
    )
    .await
    .unwrap();
    assert_eq!(assignment.groups.len(), 1);
    assert_eq!(assignment.groups[0].teacher_ids, vec![f.teacher]);
    assert_eq!(assignment.groups[0].primary_teacher_ids, vec![f.teacher]);
    assert_eq!(
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, f.teacher)
            .await
            .unwrap()
            .learning_group_ids,
        vec![f.group]
    );
    assert!(
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, f.other)
            .await
            .unwrap()
            .learning_group_ids
            .is_empty()
    );
    assert!(group_teacher_assignments_on_connection(
        &mut connection,
        Uuid::new_v4(),
        f.term,
        &[f.group]
    )
    .await
    .unwrap()
    .groups
    .is_empty());
    assert!(group_teacher_assignments_on_connection(
        &mut connection,
        f.year,
        Uuid::new_v4(),
        &[f.group]
    )
    .await
    .unwrap()
    .groups
    .is_empty());
    assert!(
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &[])
            .await
            .unwrap()
            .groups
            .is_empty()
    );
    let allocation = teacher_allocations_on_connection(&mut connection, &[f.offering])
        .await
        .unwrap();
    assert_eq!(allocation.len(), 1);
    assert_eq!(allocation[0].learning_group_id, f.group);
    assert_eq!(allocation[0].academic_year_id, f.year);
    let bound:i64=sqlx::query_scalar("SELECT count(*) FROM jsonb_to_recordset($1::jsonb) AS assignment(learning_group_id uuid,teacher_ids uuid[],primary_teacher_ids uuid[]) WHERE $2=ANY(teacher_ids) AND $2=ANY(primary_teacher_ids)").bind(sqlx::types::Json(&assignment.groups)).bind(f.teacher).fetch_one(&mut *connection).await.unwrap();
    assert_eq!(bound, 1);
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(f.teacher)
        .execute(&mut *connection)
        .await
        .unwrap();
    assert!(
        group_teacher_assignments_on_connection(&mut connection, f.year, f.term, &[f.group])
            .await
            .unwrap()
            .groups
            .is_empty()
    );
    assert!(
        assigned_group_ids_on_connection(&mut connection, f.year, f.term, f.teacher)
            .await
            .unwrap()
            .learning_group_ids
            .is_empty()
    );
    assert!(
        teacher_allocations_on_connection(&mut connection, &[f.offering])
            .await
            .unwrap()
            .is_empty()
    );
}

struct AssessmentInitialization {
    fail: bool,
    expected_teacher: Uuid,
}
#[async_trait::async_trait]
impl AssessmentCurrentPort for AssessmentInitialization {
    async fn fill_missing_coordinators(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        _actor: Uuid,
        offerings: &[Uuid],
    ) -> Result<(), AppError> {
        let visible = teacher_allocations_on_connection(tx, offerings).await?;
        assert!(visible
            .iter()
            .any(|teacher| teacher.teacher_id == self.expected_teacher));
        if self.fail {
            Err(AppError::Conflict(
                "assessment initialization refused".into(),
            ))
        } else {
            Ok(())
        }
    }
}

async fn seed_score_and_confirmation(f: &Fixture) -> (Uuid, Uuid) {
    let mut tx = f.pool.begin().await.unwrap();
    let plan:Uuid=sqlx::query_scalar("INSERT INTO course_assessment_plans(learning_offering_id,academic_year_id,academic_term_id,subject_version_id) SELECT learning_offering_id,academic_year_id,academic_term_id,subject_version_id FROM course_offering_details WHERE learning_offering_id=$1 RETURNING id").bind(f.offering).fetch_one(&mut *tx).await.unwrap();
    let phase:Uuid=sqlx::query_scalar("INSERT INTO course_assessment_phases(plan_id,phase_code,max_score,exam_arrangement) VALUES($1,'before_midterm',20,'none') RETURNING id").bind(plan).fetch_one(&mut *tx).await.unwrap();
    let item:Uuid=sqlx::query_scalar("INSERT INTO learning_group_score_items(learning_group_id,learning_offering_id,course_assessment_plan_id,assessment_phase_id,academic_term_id,academic_year_id,name,max_score) VALUES($1,$2,$3,$4,$5,$6,'Preserved score',20) RETURNING id").bind(f.group).bind(f.offering).bind(plan).bind(phase).bind(f.term).bind(f.year).fetch_one(&mut *tx).await.unwrap();
    let student: Uuid = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(f.year)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let score:Uuid=sqlx::query_scalar("INSERT INTO learning_group_student_scores(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,score_item_id,student_academic_year_id,score) VALUES($1,$2,$3,$4,$5,$6,12.5) RETURNING id").bind(f.group).bind(f.offering).bind(f.term).bind(f.year).bind(item).bind(student).fetch_one(&mut *tx).await.unwrap();
    let confirmation:Uuid=sqlx::query_scalar("INSERT INTO learning_group_phase_confirmations(learning_group_id,learning_offering_id,academic_term_id,academic_year_id,course_assessment_plan_id,assessment_phase_id,phase_row_version,roster_checksum,source_checksum,source_snapshot,confirmed_by) VALUES($1,$2,$3,$4,$5,$6,1,repeat('a',64),repeat('b',64),'{}',$7) RETURNING id").bind(f.group).bind(f.offering).bind(f.term).bind(f.year).bind(plan).bind(phase).bind(f.teacher).fetch_one(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    (score, confirmation)
}

#[tokio::test]
async fn direct_teacher_save_retains_identity_and_rolls_back_with_assessment_initialization() {
    let f = fixture("delivery_current_teacher_save").await;
    let (score, confirmation) = seed_score_and_confirmation(&f).await;
    let before = groups::get(&f.pool, f.group).await.unwrap();
    let id = before.teacher_assignments[0].id;
    let unchanged = groups::replace_teachers(
        &AssessmentInitialization {
            fail: false,
            expected_teacher: f.teacher,
        },
        &f.pool,
        f.teacher,
        f.group,
        ReplaceLearningGroupTeachersRequest {
            row_version: before.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: f.teacher,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(unchanged.row_version, before.row_version);
    assert_eq!(
        unchanged.teacher_assignments[0].row_version,
        before.teacher_assignments[0].row_version
    );
    let confirmation_before:(i64,bool)=sqlx::query_as("SELECT row_version,COALESCE((source_snapshot->>'invalidated')::boolean,false) FROM learning_group_phase_confirmations WHERE id=$1").bind(confirmation).fetch_one(&f.pool).await.unwrap();
    assert_eq!(confirmation_before, (1, false));
    let saved = groups::replace_teachers(
        &AssessmentInitialization {
            fail: false,
            expected_teacher: f.teacher,
        },
        &f.pool,
        f.teacher,
        f.group,
        ReplaceLearningGroupTeachersRequest {
            row_version: before.row_version,
            teachers: vec![
                TeacherAssignmentInput {
                    teacher_id: f.teacher,
                    role: LearningTeacherRole::Secondary,
                },
                TeacherAssignmentInput {
                    teacher_id: f.other,
                    role: LearningTeacherRole::Primary,
                },
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        saved
            .teacher_assignments
            .iter()
            .find(|row| row.teacher_id == f.teacher)
            .unwrap()
            .id,
        id
    );
    assert!(saved.row_version > before.row_version);
    let confirmation_after:(i64,bool)=sqlx::query_as("SELECT row_version,COALESCE((source_snapshot->>'invalidated')::boolean,false) FROM learning_group_phase_confirmations WHERE id=$1").bind(confirmation).fetch_one(&f.pool).await.unwrap();
    assert_eq!(confirmation_after, (2, true));
    let saved_score: (String, i64) = sqlx::query_as(
        "SELECT score::text,row_version FROM learning_group_student_scores WHERE id=$1",
    )
    .bind(score)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(saved_score, ("12.50".into(), 1));

    let result = groups::replace_teachers(
        &AssessmentInitialization {
            fail: true,
            expected_teacher: f.teacher,
        },
        &f.pool,
        f.teacher,
        f.group,
        ReplaceLearningGroupTeachersRequest {
            row_version: saved.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: f.teacher,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    let retained = groups::get(&f.pool, f.group).await.unwrap();
    assert_eq!(retained.row_version, saved.row_version);
    assert_eq!(retained.teacher_assignments.len(), 2);
    assert_eq!(
        retained
            .teacher_assignments
            .iter()
            .find(|row| row.teacher_id == f.teacher)
            .unwrap()
            .role,
        LearningTeacherRole::Secondary
    );
    let stale = groups::replace_teachers(
        &AssessmentInitialization {
            fail: false,
            expected_teacher: f.teacher,
        },
        &f.pool,
        f.teacher,
        f.group,
        ReplaceLearningGroupTeachersRequest {
            row_version: before.row_version,
            teachers: vec![],
        },
    )
    .await;
    assert!(matches!(stale, Err(AppError::Conflict(_))));
}

#[derive(Clone)]
struct DeletionDependencies {
    fail: bool,
}
#[async_trait::async_trait]
impl school_academic_delivery::ports::DeliveryDeletionPort for DeletionDependencies {
    async fn preview(
        &self,
        _connection: &mut sqlx::PgConnection,
        _scope: &school_academic_delivery::models::current::DeliveryDeletionScope,
    ) -> Result<school_academic_delivery::models::current::DeliveryDeletionDependencies, AppError>
    {
        Ok(
            school_academic_delivery::models::current::DeliveryDeletionDependencies {
                impact: Default::default(),
                source_digest: "test dependency evidence".into(),
            },
        )
    }
    async fn acquire(
        &self,
        _context: &school_academic_delivery::ports::DeliveryDeletionCommandContext,
        _target: school_academic_delivery::ports::DeliveryDeletionTarget,
    ) -> Result<Box<dyn school_academic_delivery::ports::DeliveryDeletionAdmission>, AppError> {
        Ok(Box::new(self.clone()))
    }
}
#[async_trait::async_trait]
impl school_academic_delivery::ports::DeliveryDeletionAdmission for DeletionDependencies {
    async fn lock_dependencies_before_term(
        &mut self,
        _tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError> {
        Ok(())
    }
    async fn lock_dependencies_after_term(
        &mut self,
        _tx: &mut Transaction<'_, Postgres>,
        _scope: &school_academic_delivery::models::current::DeliveryDeletionScope,
    ) -> Result<(), AppError> {
        Ok(())
    }
    async fn commit(
        self: Box<Self>,
        tx: school_tenancy::ObservedTransaction,
        _changed: bool,
    ) -> Result<(), AppError> {
        tx.commit().await?;
        Ok(())
    }
    async fn delete_dependencies(
        &mut self,
        tx: &mut Transaction<'_, Postgres>,
        scope: &school_academic_delivery::models::current::DeliveryDeletionScope,
    ) -> Result<(), AppError> {
        if self.fail {
            sqlx::query("UPDATE learning_groups SET name='Must roll back' WHERE id=ANY($1)")
                .bind(&scope.learning_group_ids)
                .execute(&mut **tx)
                .await?;
            return Err(AppError::Conflict("dependency refused deletion".into()));
        }
        Ok(())
    }
}

struct TestAuthorization;
#[async_trait::async_trait]
impl school_academic_delivery::ports::DeliveryDeletionAuthorizationPort for TestAuthorization {
    async fn validate_admission(
        &self,
        _actor: &school_authorization::ActorContext,
    ) -> Result<(), AppError> {
        Ok(())
    }
    fn committed(
        &self,
        _result: &school_academic_delivery::models::current::DeletedDeliveryResource,
    ) {
    }
}
fn command(
    f: &Fixture,
    actor: &school_authorization::ActorContext,
) -> school_academic_delivery::ports::DeliveryDeletionCommandContext {
    school_academic_delivery::ports::DeliveryDeletionCommandContext {
        actor: actor.clone(),
        tenant_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        pool_lease: f.pool.clone().into(),
        authorization: std::sync::Arc::new(TestAuthorization),
    }
}

#[tokio::test]
async fn current_group_deletion_checks_school_authority_digest_and_atomic_scope() {
    use school_academic_delivery::{
        models::current::DeleteDeliveryResourceRequest, services::deletion,
    };
    use school_authorization::ActorContext;
    let f = fixture("delivery_current_group_deletion").await;
    let school = ActorContext {
        user_id: f.teacher,
        permissions: vec![
            school_permissions::registry::codes::LEARNING_OFFERING_MANAGE_SCHOOL.into(),
        ],
    };
    let assigned = ActorContext {
        user_id: f.teacher,
        permissions: vec![
            school_permissions::registry::codes::LEARNING_OFFERING_MANAGE_ASSIGNED.into(),
        ],
    };
    let port = DeletionDependencies { fail: false };
    assert!(matches!(
        deletion::preview(
            &port,
            &f.pool,
            &assigned,
            f.offering,
            f.year,
            f.term,
            Some(f.group)
        )
        .await,
        Err(AppError::Forbidden(_))
    ));
    let sibling = Uuid::now_v7();
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name) VALUES($1,$2,$3,$4,'SIBLING','Other group')").bind(sibling).bind(f.offering).bind(f.term).bind(f.year).execute(&f.pool).await.unwrap();
    let preview = deletion::preview(
        &port,
        &f.pool,
        &school,
        f.offering,
        f.year,
        f.term,
        Some(f.group),
    )
    .await
    .unwrap();
    assert_eq!(preview.impact.learning_groups, 1);
    assert_eq!(preview.impact.teacher_assignments, 1);
    let request = || DeleteDeliveryResourceRequest {
        academic_year_id: f.year,
        academic_term_id: f.term,
        row_version: preview.row_version,
        source_digest: preview.source_digest.clone(),
    };
    let failed = deletion::delete(
        std::sync::Arc::new(DeletionDependencies { fail: true }),
        command(&f, &school),
        f.offering,
        Some(f.group),
        request(),
    )
    .await;
    assert!(matches!(failed, Err(AppError::Conflict(_))));
    assert_eq!(
        groups::get(&f.pool, f.group).await.unwrap().name,
        "Current group"
    );
    let mut stale = request();
    stale.source_digest = "0".repeat(64);
    assert!(matches!(
        deletion::delete(
            std::sync::Arc::new(port.clone()),
            command(&f, &school),
            f.offering,
            Some(f.group),
            stale
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let deleted = deletion::delete(
        std::sync::Arc::new(port.clone()),
        command(&f, &school),
        f.offering,
        Some(f.group),
        request(),
    )
    .await
    .unwrap();
    assert!(deleted.deleted);
    assert!(matches!(
        groups::get(&f.pool, f.group).await,
        Err(AppError::NotFound(_))
    ));
    assert!(groups::get(&f.pool, sibling).await.is_ok());
    assert!(
        school_academic_delivery::services::offerings::get(&f.pool, f.offering)
            .await
            .is_ok()
    );
    let teacher_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_teachers WHERE learning_group_id=$1",
    )
    .bind(f.group)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(teacher_count, 0);
}

#[test]
fn destructive_school_scope_uses_canonical_wildcards() {
    use school_academic_delivery::services::deletion::require_school_management;
    for grant in ["*", "learning_offering.manage.school"] {
        assert!(
            require_school_management(&school_authorization::ActorContext {
                user_id: Uuid::new_v4(),
                permissions: vec![grant.into()]
            })
            .is_ok()
        );
    }
    for grant in [
        "*.manage.school",
        "*.manage.assigned",
        "learning_offering.manage.assigned",
        "learning_offering.read.school",
    ] {
        assert!(matches!(
            require_school_management(&school_authorization::ActorContext {
                user_id: Uuid::new_v4(),
                permissions: vec![grant.into()]
            }),
            Err(AppError::Forbidden(_))
        ));
    }
}
