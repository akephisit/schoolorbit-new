#[cfg(test)]
mod tests {
    use crate::integration_tests::academic::cutover_test_support::{
        apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
        CutoverFixture,
    };
    use school_academic_core::ports::PlacementRosterPort;
    use school_admission::applications::{
        complete_enrollment, AssignRoomsGlobalRequest, AssignRoomsRequest, BulkScoreEntry,
        CompleteEnrollmentRequest, UpdateScoreEntry,
    };
    use school_admission::scores::{bulk_update_scores, update_application_scores};
    use school_admission::selections::{assign_rooms, assign_rooms_global, get_round_ranking};
    use school_errors::AppError;
    use school_test_db::{create_named_test_pool_with_max_connections, create_test_user};
    use sqlx::PgPool;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    struct EnrollmentFixture {
        pool: PgPool,
        application_id: Uuid,
        academic_year_id: Uuid,
        enroller_id: Uuid,
    }

    struct EnrollmentRosters {
        actor: Uuid,
        fail: bool,
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl PlacementRosterPort for EnrollmentRosters {
        async fn reconcile(
            &self,
            transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            actor: Uuid,
            student_year_ids: &[Uuid],
        ) -> Result<(), AppError> {
            assert_eq!(actor, self.actor);
            assert_eq!(student_year_ids.len(), 1);
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM student_academic_years WHERE id=ANY($1)")
                    .bind(student_year_ids)
                    .fetch_one(&mut **transaction)
                    .await
                    .unwrap();
            assert_eq!(
                count, 1,
                "provider must see the new row in the caller's transaction"
            );
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(AppError::InternalServerError(
                    "roster provider unavailable".into(),
                ));
            }
            crate::http_state::PLACEMENT_ROSTERS
                .reconcile(transaction, actor, student_year_ids)
                .await
        }
    }

    async fn enrollment_fixture(name: &str, connections: u32) -> EnrollmentFixture {
        enrollment_fixture_through(
            name,
            connections,
            school_migrations::latest_migration_version(),
        )
        .await
    }
    async fn enrollment_fixture_through(
        name: &str,
        connections: u32,
        version: i64,
    ) -> EnrollmentFixture {
        let pool = create_named_test_pool_with_max_connections(name, connections).await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_phase_b_runtime_migrations(&pool).await.unwrap();
        apply_migrations_through(&pool, version).await.unwrap();

        let enroller_id = create_test_user(
            &pool,
            "admission-future-enroller@example.invalid",
            "test-password",
        )
        .await
        .unwrap();
        let (grade_level_id, study_program_id): (Uuid, Uuid) = sqlx::query_as(
            "SELECT grade.id, program.id
                 FROM grade_levels grade
                 CROSS JOIN LATERAL (
                     SELECT id
                     FROM study_programs
                     WHERE status = 'published'
                     ORDER BY code, id
                     LIMIT 1
                 ) program
                 ORDER BY grade.id
                 LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let year_sql = if version < 147 {
            "INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) VALUES(2570,'ปีการศึกษาอนาคต','2027-05-01','2028-03-31','MON,TUE,WED,THU,FRI','planning') RETURNING id"
        } else {
            "INSERT INTO academic_years(year,name,school_days,status) VALUES(2570,'ปีการศึกษาอนาคต','MON,TUE,WED,THU,FRI','planning') RETURNING id"
        };
        let academic_year_id: Uuid = sqlx::query_scalar(sqlx::AssertSqlSafe(year_sql))
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO academic_year_grade_levels (academic_year_id, grade_level_id)
             VALUES ($1, $2)",
        )
        .bind(academic_year_id)
        .bind(grade_level_id)
        .execute(&pool)
        .await
        .unwrap();
        let homeroom_id: Uuid = sqlx::query_scalar(
            "INSERT INTO homerooms (
                 code, name, academic_year_id, grade_level_id, room_number,
                 study_program_id, capacity
             ) VALUES ('ADM-FUTURE-1', 'ห้องอนาคต 1', $1, $2, '1', $3, 40)
             RETURNING id",
        )
        .bind(academic_year_id)
        .bind(grade_level_id)
        .bind(study_program_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let round_id: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_rounds (
                 academic_year_id, grade_level_id, name, apply_start_date, apply_end_date
             ) VALUES ($1, $2, 'รอบอนาคต', '2026-08-01', '2027-03-31')
             RETURNING id",
        )
        .bind(academic_year_id)
        .bind(grade_level_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let track_id: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_tracks (
                 admission_round_id, academic_year_id, study_program_id, name
             ) VALUES ($1, $2, $3, 'แผนอนาคต')
             RETURNING id",
        )
        .bind(round_id)
        .bind(academic_year_id)
        .bind(study_program_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let application_id: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_applications (
                 admission_round_id, admission_track_id, national_id, national_id_hash,
                 first_name, last_name, status
             ) VALUES ($1, $2, 'encrypted-admission-fixture', repeat('f', 64),
                       'นักเรียน', 'อนาคต', 'accepted')
             RETURNING id",
        )
        .bind(round_id)
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO admission_room_assignments (
                 application_id, homeroom_id, academic_year_id, rank_in_track, rank_in_room
             ) VALUES ($1, $2, $3, 1, 1)",
        )
        .bind(application_id)
        .bind(homeroom_id)
        .bind(academic_year_id)
        .execute(&pool)
        .await
        .unwrap();

        EnrollmentFixture {
            pool,
            application_id,
            academic_year_id,
            enroller_id,
        }
    }

    fn enrollment_payload() -> CompleteEnrollmentRequest {
        CompleteEnrollmentRequest {
            placement_starts_on: chrono::NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
            student_code: Some("970001".to_string()),
            form_data: None,
        }
    }

    #[tokio::test]
    async fn round_ranking_keeps_empty_tracks_without_reading_national_ids() {
        let fixture = enrollment_fixture("admission_round_ranking_no_pii", 1).await;
        let (round_id, track_id, study_program_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
            "SELECT aa.admission_round_id, aa.admission_track_id, at.study_program_id
             FROM admission_applications aa
             JOIN admission_tracks at ON at.id = aa.admission_track_id
             WHERE aa.id = $1",
        )
        .bind(fixture.application_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        let empty_track_id: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_tracks (
                 admission_round_id, academic_year_id, study_program_id, name
             ) VALUES ($1, $2, $3, 'แผนไม่มีผู้สมัคร') RETURNING id",
        )
        .bind(round_id)
        .bind(fixture.academic_year_id)
        .bind(study_program_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();

        let rankings = get_round_ranking(&fixture.pool, round_id).await.unwrap();
        assert_eq!(rankings.len(), 2);
        let populated = rankings
            .iter()
            .find(|result| result.track_id == track_id)
            .unwrap();
        assert_eq!(populated.applications.len(), 1);
        assert_eq!(
            populated.applications[0].application_id,
            fixture.application_id
        );
        assert_eq!(populated.applications[0].rank, 1);
        assert!(rankings
            .iter()
            .find(|result| result.track_id == empty_track_id)
            .unwrap()
            .applications
            .is_empty());
        let payload = serde_json::to_string(&rankings).unwrap();
        assert!(!payload.contains("nationalId"));
        assert!(!payload.contains("encrypted-admission-fixture"));

        let per_track_count = assign_rooms(
            &fixture.pool,
            round_id,
            AssignRoomsRequest {
                track_id,
                selection_subject_ids: None,
                room_assignment_method: None,
            },
            fixture.enroller_id,
        )
        .await
        .unwrap();
        assert_eq!(per_track_count, 1);
        let global_count = assign_rooms_global(
            &fixture.pool,
            round_id,
            AssignRoomsGlobalRequest {
                room_assignment_method: None,
                room_order: None,
            },
            fixture.enroller_id,
        )
        .await
        .unwrap();
        assert_eq!(global_count, 1);
    }

    #[tokio::test]
    async fn bulk_scores_reject_cross_round_rows_and_roll_back_valid_rows() {
        let fixture = enrollment_fixture("admission_bulk_scores_round_scope", 1).await;
        let (round_id, grade_level_id, study_program_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
            "SELECT aa.admission_round_id, ar.grade_level_id, at.study_program_id
             FROM admission_applications aa
             JOIN admission_rounds ar ON ar.id = aa.admission_round_id
             JOIN admission_tracks at ON at.id = aa.admission_track_id
             WHERE aa.id = $1",
        )
        .bind(fixture.application_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        sqlx::query("UPDATE admission_applications SET status = 'verified' WHERE id = $1")
            .bind(fixture.application_id)
            .execute(&fixture.pool)
            .await
            .unwrap();
        let other_round_id: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_rounds (
                 academic_year_id, grade_level_id, name, apply_start_date, apply_end_date
             ) VALUES ($1, $2, 'รอบทดสอบอื่น', '2026-08-01', '2027-03-31') RETURNING id",
        )
        .bind(fixture.academic_year_id)
        .bind(grade_level_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        let other_track_id: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_tracks (
                 admission_round_id, academic_year_id, study_program_id, name
             ) VALUES ($1, $2, $3, 'แผนทดสอบอื่น') RETURNING id",
        )
        .bind(other_round_id)
        .bind(fixture.academic_year_id)
        .bind(study_program_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        let other_application_id: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_applications (
                 admission_round_id, admission_track_id, national_id, national_id_hash,
                 first_name, last_name, status
             ) VALUES ($1, $2, 'encrypted-score-test-fixture', repeat('e', 64),
                       'นักเรียน', 'รอบอื่น', 'verified') RETURNING id",
        )
        .bind(other_round_id)
        .bind(other_track_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        let subject_a: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_exam_subjects (admission_round_id, name)
             VALUES ($1, 'วิชาหนึ่ง') RETURNING id",
        )
        .bind(round_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        let subject_b: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_exam_subjects (admission_round_id, name)
             VALUES ($1, 'วิชาสอง') RETURNING id",
        )
        .bind(round_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        let other_subject: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_exam_subjects (admission_round_id, name)
             VALUES ($1, 'วิชาอีกรอบ') RETURNING id",
        )
        .bind(other_round_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        let entry = |application_id, exam_subject_id, score| BulkScoreEntry {
            application_id,
            scores: vec![UpdateScoreEntry {
                exam_subject_id,
                score: Some(score),
            }],
        };

        assert_eq!(
            bulk_update_scores(
                &fixture.pool,
                round_id,
                fixture.enroller_id,
                &[entry(fixture.application_id, subject_a, 10.0)],
            )
            .await
            .unwrap(),
            1
        );
        for invalid in [
            entry(other_application_id, subject_a, 17.0),
            entry(fixture.application_id, other_subject, 17.0),
        ] {
            let result = bulk_update_scores(
                &fixture.pool,
                round_id,
                fixture.enroller_id,
                &[entry(fixture.application_id, subject_b, 15.0), invalid],
            )
            .await;
            assert!(matches!(result, Err(AppError::BadRequest(_))));
            let rolled_back: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM admission_exam_scores WHERE exam_subject_id = $1",
            )
            .bind(subject_b)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
            assert_eq!(rolled_back, 0);
        }
        let single_result = update_application_scores(
            &fixture.pool,
            fixture.application_id,
            fixture.enroller_id,
            &[
                UpdateScoreEntry {
                    exam_subject_id: subject_b,
                    score: Some(15.0),
                },
                UpdateScoreEntry {
                    exam_subject_id: other_subject,
                    score: Some(17.0),
                },
            ],
        )
        .await;
        assert!(matches!(single_result, Err(AppError::BadRequest(_))));
        let rolled_back: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM admission_exam_scores WHERE exam_subject_id IN ($1, $2)",
        )
        .bind(subject_b)
        .bind(other_subject)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(rolled_back, 0);
        let status: String =
            sqlx::query_scalar("SELECT status FROM admission_applications WHERE id = $1")
                .bind(fixture.application_id)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert_eq!(status, "verified");
        assert_eq!(
            bulk_update_scores(
                &fixture.pool,
                round_id,
                fixture.enroller_id,
                &[entry(fixture.application_id, subject_b, 20.0)],
            )
            .await
            .unwrap(),
            1
        );
        let state: (String, String, i64) = sqlx::query_as(
            "SELECT own.status, other.status,
                    (SELECT COUNT(*) FROM admission_exam_scores score
                     WHERE score.application_id = other.id)
             FROM admission_applications own
             CROSS JOIN admission_applications other
             WHERE own.id = $1 AND other.id = $2",
        )
        .bind(fixture.application_id)
        .bind(other_application_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(state, ("scored".into(), "verified".into(), 0));
    }

    #[tokio::test]
    async fn future_year_enrollment_is_idempotent_and_creates_only_planned_placement() {
        let EnrollmentFixture {
            pool,
            application_id,
            academic_year_id,
            enroller_id,
        } = enrollment_fixture("admission_future_year_enrollment", 1).await;
        let rosters = EnrollmentRosters {
            actor: enroller_id,
            fail: false,
            calls: AtomicUsize::new(0),
        };
        let first = complete_enrollment(
            &pool,
            application_id,
            enrollment_payload(),
            enroller_id,
            &rosters,
        )
        .await
        .unwrap();
        let replay = complete_enrollment(
            &pool,
            application_id,
            enrollment_payload(),
            enroller_id,
            &rosters,
        )
        .await
        .unwrap();

        assert_eq!(first.user_id, replay.user_id);
        assert_eq!(first.student_code, replay.student_code);
        let state: (i64, i64, String, String) = sqlx::query_as(
            "SELECT
                 (SELECT COUNT(*) FROM student_academic_years
                  WHERE student_id = $1 AND academic_year_id = $2),
                 (SELECT COUNT(*) FROM homeroom_placements placement
                  JOIN student_academic_years student_year
                    ON student_year.id = placement.student_academic_year_id
                  WHERE student_year.student_id = $1
                    AND student_year.academic_year_id = $2),
                 student_year.status,
                 placement.status
             FROM student_academic_years student_year
             JOIN homeroom_placements placement
               ON placement.student_academic_year_id = student_year.id
             WHERE student_year.student_id = $1
               AND student_year.academic_year_id = $2",
        )
        .bind(first.user_id)
        .bind(academic_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(state, (1, 1, "planned".to_string(), "planned".to_string()));
        let before: String = sqlx::query_scalar("SELECT to_jsonb(student_year)::text FROM student_academic_years student_year WHERE student_id=$1 AND academic_year_id=$2")
            .bind(first.user_id).bind(academic_year_id).fetch_one(&pool).await.unwrap();
        for status in ["closed", "archived"] {
            sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
                .bind(academic_year_id)
                .bind(status)
                .execute(&pool)
                .await
                .unwrap();
            let replay = complete_enrollment(
                &pool,
                application_id,
                enrollment_payload(),
                enroller_id,
                &rosters,
            )
            .await
            .unwrap();
            assert_eq!(replay.user_id, first.user_id);
            assert_eq!(replay.student_code, first.student_code);
            let after: String = sqlx::query_scalar("SELECT to_jsonb(student_year)::text FROM student_academic_years student_year WHERE student_id=$1 AND academic_year_id=$2")
                .bind(first.user_id).bind(academic_year_id).fetch_one(&pool).await.unwrap();
            assert_eq!(
                before, after,
                "completed receipt replay never rewrites closed academic history"
            );
        }
        assert_eq!(
            rosters.calls.load(Ordering::SeqCst),
            1,
            "receipt replay must not reconcile rosters again"
        );
    }

    #[tokio::test]
    async fn enrollment_roster_failure_rolls_back_all_created_records() {
        let fixture = enrollment_fixture("enrollment_roster_rollback", 1).await;
        let users_before: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
        let rosters = EnrollmentRosters {
            actor: fixture.enroller_id,
            fail: true,
            calls: AtomicUsize::new(0),
        };
        let result = complete_enrollment(
            &fixture.pool,
            fixture.application_id,
            enrollment_payload(),
            fixture.enroller_id,
            &rosters,
        )
        .await;
        assert!(
            matches!(result, Err(AppError::InternalServerError(message)) if message == "roster provider unavailable")
        );
        assert_eq!(rosters.calls.load(Ordering::SeqCst), 1);
        let state: (String, Option<Uuid>, i64, i64, i64) = sqlx::query_as(
            "SELECT status,created_user_id,(SELECT count(*) FROM users WHERE username='970001'),
             (SELECT count(*) FROM student_academic_years WHERE academic_year_id=$2),
             (SELECT count(*) FROM homeroom_placements WHERE academic_year_id=$2)
             FROM admission_applications WHERE id=$1",
        )
        .bind(fixture.application_id)
        .bind(fixture.academic_year_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(state, ("accepted".into(), None, 0, 0, 0));
        let users_after: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
        assert_eq!(
            users_after, users_before,
            "parent and student accounts must roll back together"
        );
    }

    fn guardian_enrollment_payload() -> CompleteEnrollmentRequest {
        CompleteEnrollmentRequest {
            placement_starts_on: chrono::NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
            student_code: Some("970001".into()),
            form_data: Some(serde_json::json!({
                "father": { "phone": "0800000001", "firstName": "Guardian", "lastName": "Fixture" }
            })),
        }
    }

    async fn assert_guardian_failure_rolled_back(fixture: &EnrollmentFixture, users_before: i64) {
        let state: (String, Option<Uuid>, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT status,created_user_id,
             (SELECT count(*) FROM users),
             (SELECT count(*) FROM student_academic_years WHERE academic_year_id=$2),
             (SELECT count(*) FROM homeroom_placements WHERE academic_year_id=$2),
             (SELECT count(*) FROM admission_enrollment_forms WHERE application_id=$1),
             (SELECT count(*) FROM student_parents WHERE parent_user_id IN
                (SELECT id FROM users WHERE username='0800000001'))
             FROM admission_applications WHERE id=$1",
        )
        .bind(fixture.application_id)
        .bind(fixture.academic_year_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(state, ("accepted".into(), None, users_before, 0, 0, 0, 0));
    }

    #[tokio::test]
    async fn enrollment_guardian_lookup_permission_denial_propagates_and_rolls_back() {
        let fixture = enrollment_fixture("enrollment_guardian_lookup_denied", 1).await;
        let users_before: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
        let schema: String = sqlx::query_scalar("SELECT current_schema()")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
        let role = format!("guardian_fixture_{}", Uuid::new_v4().simple());
        // This role belongs only to this disposable fixture. Student INSERT RETURNING id remains
        // allowed, while the later guardian username lookup genuinely lacks SELECT permission.
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE ROLE {role} NOLOGIN; GRANT USAGE ON SCHEMA \"{schema}\" TO {role};
             GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA \"{schema}\" TO {role};
             REVOKE SELECT ON \"{schema}\".users FROM {role};
             GRANT SELECT(id) ON \"{schema}\".users TO {role};"
        )))
        .execute(&fixture.pool)
        .await
        .unwrap();
        let setup = format!("SET ROLE {role}; SET search_path TO \"{schema}\", public");
        let restricted = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .after_connect(move |connection, _| {
                let setup = setup.clone();
                Box::pin(async move {
                    sqlx::raw_sql(sqlx::AssertSqlSafe(setup))
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect_with((*fixture.pool.connect_options()).clone())
            .await
            .unwrap();
        let rosters = EnrollmentRosters {
            actor: fixture.enroller_id,
            fail: false,
            calls: AtomicUsize::new(0),
        };
        let result = complete_enrollment(
            &restricted,
            fixture.application_id,
            guardian_enrollment_payload(),
            fixture.enroller_id,
            &rosters,
        )
        .await;
        restricted.close().await;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP OWNED BY {role}; DROP ROLE {role}"
        )))
        .execute(&fixture.pool)
        .await
        .unwrap();
        let error = result.err().expect("guardian lookup must fail closed");
        assert_eq!(error.database_code().as_deref(), Some("42501"));
        assert_eq!(rosters.calls.load(Ordering::SeqCst), 0);
        assert_guardian_failure_rolled_back(&fixture, users_before).await;
    }

    #[tokio::test]
    async fn enrollment_guardian_form_and_role_database_failures_preserve_the_original_error() {
        for (stage, setup, expected_code) in [
            (
                "form_write",
                "CREATE FUNCTION reject_guardian_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rejected form' USING ERRCODE='23514'; END $$; CREATE TRIGGER reject_guardian_fixture BEFORE INSERT ON admission_enrollment_forms FOR EACH ROW EXECUTE FUNCTION reject_guardian_fixture();",
                "23514",
            ),
            (
                "role_read",
                "ALTER TABLE roles RENAME COLUMN code TO unavailable_code",
                "42703",
            ),
            (
                "role_write",
                "CREATE FUNCTION reject_guardian_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rejected role' USING ERRCODE='23514'; END $$; CREATE TRIGGER reject_guardian_fixture BEFORE INSERT ON user_roles FOR EACH ROW EXECUTE FUNCTION reject_guardian_fixture();",
                "23514",
            ),
        ] {
            let fixture = enrollment_fixture(&format!("enrollment_guardian_{stage}"), 1).await;
            let users_before: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
            if stage == "role_write" {
                // The cutover fixture intentionally has no default parent role. Seed one so
                // this fault reaches the real role INSERT rather than the optional-None branch.
                sqlx::query("INSERT INTO roles(code,name,user_type,is_active) VALUES('PARENT','Guardian fixture','parent',true)")
                    .execute(&fixture.pool).await.unwrap();
            }
            sqlx::raw_sql(sqlx::AssertSqlSafe(setup))
                .execute(&fixture.pool)
                .await
                .unwrap();
            let rosters = EnrollmentRosters {
                actor: fixture.enroller_id,
                fail: false,
                calls: AtomicUsize::new(0),
            };
            let result = complete_enrollment(
                &fixture.pool,
                fixture.application_id,
                guardian_enrollment_payload(),
                fixture.enroller_id,
                &rosters,
            )
            .await;
            let error = result.err().unwrap_or_else(|| {
                panic!("{stage}: critical guardian database error must propagate")
            });
            assert_eq!(
                error.database_code().as_deref(),
                Some(expected_code),
                "{stage}: {error:?}"
            );
            assert_eq!(rosters.calls.load(Ordering::SeqCst), 0, "{stage}");
            assert_guardian_failure_rolled_back(&fixture, users_before).await;
        }
    }

    #[tokio::test]
    async fn enrollment_guardian_missing_optional_role_still_creates_and_links_the_parent() {
        let fixture = enrollment_fixture("enrollment_guardian_optional_role", 1).await;
        sqlx::query("UPDATE roles SET is_active=false WHERE code='PARENT'")
            .execute(&fixture.pool)
            .await
            .unwrap();
        let result = complete_enrollment(
            &fixture.pool,
            fixture.application_id,
            guardian_enrollment_payload(),
            fixture.enroller_id,
            &crate::http_state::PLACEMENT_ROSTERS,
        )
        .await
        .unwrap();
        let state: (String, i64) = sqlx::query_as(
            "SELECT relationship,(SELECT count(*) FROM user_roles WHERE user_id=parent.id)
             FROM student_parents link JOIN users parent ON parent.id=link.parent_user_id
             WHERE link.student_user_id=$1 AND parent.username='0800000001'",
        )
        .bind(result.user_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(state, ("father".into(), 0));
    }

    #[tokio::test]
    async fn admission_lifecycle_closed_year_rejects_new_enrollment_without_partial_records() {
        let fixture = enrollment_fixture("admission_lifecycle_closed", 1).await;
        for status in ["closed", "archived"] {
            sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
                .bind(fixture.academic_year_id)
                .bind(status)
                .execute(&fixture.pool)
                .await
                .unwrap();
            let result = complete_enrollment(
                &fixture.pool,
                fixture.application_id,
                enrollment_payload(),
                fixture.enroller_id,
                &crate::http_state::PLACEMENT_ROSTERS,
            )
            .await;
            assert!(
                matches!(result, Err(AppError::Conflict(_))),
                "closed academic year must return a lifecycle conflict"
            );
            let state: (String, Option<Uuid>, i64, i64, i64) = sqlx::query_as(
                "SELECT status,created_user_id,(SELECT count(*) FROM users WHERE username='970001'),
                 (SELECT count(*) FROM student_academic_years WHERE academic_year_id=$2),
                 (SELECT count(*) FROM homeroom_placements WHERE academic_year_id=$2)
                 FROM admission_applications WHERE id=$1",
            ).bind(fixture.application_id).bind(fixture.academic_year_id).fetch_one(&fixture.pool).await.unwrap();
            assert_eq!(state, ("accepted".into(), None, 0, 0, 0));
        }
    }

    #[tokio::test]
    async fn admission_lifecycle_waits_before_application_locks_and_rechecks_year_closure() {
        let fixture = enrollment_fixture("admission_lifecycle_lock_order", 3).await;
        let mut boundary = fixture.pool.begin().await.unwrap();
        let boundary_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *boundary)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM academic_years WHERE id=$1 FOR UPDATE")
            .bind(fixture.academic_year_id)
            .execute(&mut *boundary)
            .await
            .unwrap();
        let worker_pool = fixture.pool.clone();
        let application_id = fixture.application_id;
        let enroller_id = fixture.enroller_id;
        let worker = tokio::spawn(async move {
            complete_enrollment(
                &worker_pool,
                application_id,
                enrollment_payload(),
                enroller_id,
                &crate::http_state::PLACEMENT_ROSTERS,
            )
            .await
        });
        let mut waiting = false;
        for _ in 0..200 {
            waiting = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))").bind(boundary_pid).fetch_one(&fixture.pool).await.unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let mut probe = fixture.pool.begin().await.unwrap();
        let application_free =
            sqlx::query("SELECT id FROM admission_applications WHERE id=$1 FOR UPDATE NOWAIT")
                .bind(application_id)
                .execute(&mut *probe)
                .await
                .is_ok();
        let assignment_free = sqlx::query(
            "SELECT id FROM admission_room_assignments WHERE application_id=$1 FOR UPDATE NOWAIT",
        )
        .bind(application_id)
        .execute(&mut *probe)
        .await
        .is_ok();
        probe.rollback().await.unwrap();
        sqlx::query("UPDATE academic_years SET status='closed' WHERE id=$1")
            .bind(fixture.academic_year_id)
            .execute(&mut *boundary)
            .await
            .unwrap();
        boundary.commit().await.unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(10), worker)
            .await
            .unwrap()
            .unwrap();
        assert!(
            waiting && application_free && assignment_free,
            "academic context must lock before enrollment entities"
        );
        assert!(matches!(result, Err(AppError::Conflict(_))));
        let accounts: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE username='970001'")
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert_eq!(accounts, 0);
    }

    async fn sibling_application(fixture: &EnrollmentFixture) -> Uuid {
        let hash = Uuid::new_v4().simple().to_string().repeat(2);
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO admission_applications(admission_round_id,admission_track_id,national_id,national_id_hash,first_name,last_name,status)
             SELECT admission_round_id,admission_track_id,national_id,$2,'Sibling','Fixture','accepted'
             FROM admission_applications WHERE id=$1 RETURNING id",
        ).bind(fixture.application_id).bind(hash).fetch_one(&fixture.pool).await.unwrap();
        sqlx::query(
            "INSERT INTO admission_room_assignments(application_id,homeroom_id,academic_year_id,rank_in_track,rank_in_room)
             SELECT $2,homeroom_id,academic_year_id,2,2 FROM admission_room_assignments WHERE application_id=$1",
        ).bind(fixture.application_id).bind(id).execute(&fixture.pool).await.unwrap();
        id
    }

    #[derive(Default)]
    struct PausingEnrollmentRosters {
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
        calls: AtomicUsize,
        pid: std::sync::atomic::AtomicI32,
    }

    #[async_trait::async_trait]
    impl PlacementRosterPort for PausingEnrollmentRosters {
        async fn reconcile(
            &self,
            transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
            actor: Uuid,
            ids: &[Uuid],
        ) -> Result<(), AppError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
                    .fetch_one(&mut **transaction)
                    .await?;
                self.pid.store(pid, Ordering::SeqCst);
                self.entered.notify_one();
                self.release.notified().await;
            }
            crate::http_state::PLACEMENT_ROSTERS
                .reconcile(transaction, actor, ids)
                .await
        }
    }

    #[tokio::test]
    async fn enrollment_shared_year_allows_independent_enrollment_and_attendance_guards() {
        let fixture = enrollment_fixture("admission_enrollment_shared_year", 4).await;
        let sibling = sibling_application(&fixture).await;
        apply_migrations_through(&fixture.pool, 108).await.unwrap();
        let schedule = school_academic_core::services::bell_schedules::create(
            &fixture.pool,
            fixture.enroller_id,
            school_academic_core::models::CreateBellScheduleRequest {
                academic_year_id: fixture.academic_year_id,
                name: "Future enrollment schedule".into(),
                owning_organization_unit_id: None,
            },
        )
        .await
        .unwrap();
        let term: Uuid = sqlx::query_scalar(
            "INSERT INTO academic_terms(academic_year_id,name,sequence_no,code,term_type,status,included_in_year_result,blocks_year_closure,bell_schedule_id)
             VALUES($1,'Future term',1,'1','regular','planning',true,true,$2) RETURNING id",
        ).bind(fixture.academic_year_id).bind(schedule.id).fetch_one(&fixture.pool).await.unwrap();
        let rosters = std::sync::Arc::new(PausingEnrollmentRosters::default());
        let first_rosters = rosters.clone();
        let pool = fixture.pool.clone();
        let id = fixture.application_id;
        let actor = fixture.enroller_id;
        let first = tokio::spawn(async move {
            complete_enrollment(
                &pool,
                id,
                enrollment_payload(),
                actor,
                first_rosters.as_ref(),
            )
            .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rosters.entered.notified(),
        )
        .await
        .unwrap();
        let mut attendance = fixture.pool.begin().await.unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            school_attendance::services::lock_term(&mut attendance, term, true, false),
        )
        .await
        .expect("enrollment must not exclude attendance's shared year guard")
        .unwrap();
        attendance.rollback().await.unwrap();
        let second = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            complete_enrollment(
                &fixture.pool,
                sibling,
                CompleteEnrollmentRequest {
                    placement_starts_on: chrono::NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                    student_code: Some("970002".into()),
                    form_data: None,
                },
                fixture.enroller_id,
                rosters.as_ref(),
            ),
        )
        .await
        .expect("a different explicit code must commit before the first enrollment")
        .unwrap();
        rosters.release.notify_one();
        let first = first.await.unwrap().unwrap();
        assert_ne!(first.user_id, second.user_id);
        assert_eq!(rosters.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn enrollment_concurrent_replay_creates_one_student_and_one_roster_change() {
        let fixture = enrollment_fixture("admission_enrollment_concurrent_replay", 3).await;
        let rosters = std::sync::Arc::new(PausingEnrollmentRosters::default());
        let first_rosters = rosters.clone();
        let pool = fixture.pool.clone();
        let id = fixture.application_id;
        let actor = fixture.enroller_id;
        let first = tokio::spawn(async move {
            complete_enrollment(
                &pool,
                id,
                enrollment_payload(),
                actor,
                first_rosters.as_ref(),
            )
            .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rosters.entered.notified(),
        )
        .await
        .unwrap();
        let pool = fixture.pool.clone();
        let second_rosters = rosters.clone();
        let second = tokio::spawn(async move {
            complete_enrollment(
                &pool,
                id,
                CompleteEnrollmentRequest {
                    placement_starts_on: chrono::NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                    student_code: Some("ignored-replay".into()),
                    form_data: None,
                },
                actor,
                second_rosters.as_ref(),
            )
            .await
        });
        let mut blocked = false;
        for _ in 0..200 {
            blocked = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) AND query LIKE '%FOR UPDATE OF aa%')")
                .bind(rosters.pid.load(Ordering::SeqCst)).fetch_one(&fixture.pool).await.unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        rosters.release.notify_one();
        let first = first.await.unwrap().unwrap();
        let second = second.await.unwrap().unwrap();
        assert!(
            blocked,
            "same-application replay must wait on its authoritative row"
        );
        assert_eq!(first.user_id, second.user_id);
        assert_eq!(second.student_code, "970001");
        assert_eq!(rosters.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn enrollment_auto_number_rolls_back_without_gap_and_ignores_behavior_lock() {
        let fixture = enrollment_fixture("admission_enrollment_auto_number", 3).await;
        let sibling = sibling_application(&fixture).await;
        for code in ["985555", "NONNUMERIC-STUDENT"] {
            let student: Uuid = sqlx::query_scalar("INSERT INTO users(username,password_hash,first_name,last_name,user_type,status) VALUES($1,'fixture-hash','Number','Fixture','student','active') RETURNING id")
                .bind(format!("number-fixture-{code}")).fetch_one(&fixture.pool).await.unwrap();
            sqlx::query("INSERT INTO student_info(user_id,student_id) VALUES($1,$2)")
                .bind(student)
                .bind(code)
                .execute(&fixture.pool)
                .await
                .unwrap();
        }
        let max: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(student_id::bigint),0) FROM student_info WHERE student_id ~ '^[0-9]+$'")
            .fetch_one(&fixture.pool).await.unwrap();
        let mut behavior = fixture.pool.begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(17020000)")
            .execute(&mut *behavior)
            .await
            .unwrap();
        let rejected = EnrollmentRosters {
            actor: fixture.enroller_id,
            fail: true,
            calls: AtomicUsize::new(0),
        };
        let payload = || CompleteEnrollmentRequest {
            placement_starts_on: chrono::NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
            student_code: None,
            form_data: None,
        };
        let failed = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            complete_enrollment(
                &fixture.pool,
                fixture.application_id,
                payload(),
                fixture.enroller_id,
                &rejected,
            ),
        )
        .await
        .expect("the allocator must not share Behavior's bigint advisory key");
        assert!(matches!(failed, Err(AppError::InternalServerError(_))));
        let (first, second) = tokio::join!(
            complete_enrollment(
                &fixture.pool,
                fixture.application_id,
                payload(),
                fixture.enroller_id,
                &crate::http_state::PLACEMENT_ROSTERS
            ),
            complete_enrollment(
                &fixture.pool,
                sibling,
                payload(),
                fixture.enroller_id,
                &crate::http_state::PLACEMENT_ROSTERS
            ),
        );
        behavior.rollback().await.unwrap();
        let mut numbers = [
            first.unwrap().student_code.parse::<i64>().unwrap(),
            second.unwrap().student_code.parse::<i64>().unwrap(),
        ];
        numbers.sort();
        assert_eq!(numbers, [max + 1, max + 2]);
    }

    #[tokio::test]
    async fn enrollment_concurrent_siblings_reuse_parents_without_overwriting_accounts() {
        let fixture = enrollment_fixture("admission_enrollment_shared_parents", 3).await;
        let sibling = sibling_application(&fixture).await;
        let existing: Uuid = sqlx::query_scalar(
            "INSERT INTO users(username,password_hash,first_name,last_name,user_type,status)
             VALUES('0800000002','existing-secret-hash','Existing','Parent','parent','active') RETURNING id",
        ).fetch_one(&fixture.pool).await.unwrap();
        let before_roles: Vec<Uuid> =
            sqlx::query_scalar("SELECT role_id FROM user_roles WHERE user_id=$1 ORDER BY role_id")
                .bind(existing)
                .fetch_all(&fixture.pool)
                .await
                .unwrap();
        let payload = |code: &str| CompleteEnrollmentRequest {
            placement_starts_on: chrono::NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
            student_code: Some(code.into()),
            form_data: Some(serde_json::json!({
                "father": { "phone": "0800000001", "firstName": "New", "lastName": "Parent" },
                "mother": { "phone": "0800000002", "firstName": "Changed", "lastName": "Ignored" },
                "guardians": [{ "phone": "0800000001", "firstName": "Duplicate" }]
            })),
        };
        let (first, second) = tokio::join!(
            complete_enrollment(
                &fixture.pool,
                fixture.application_id,
                payload("970001"),
                fixture.enroller_id,
                &crate::http_state::PLACEMENT_ROSTERS
            ),
            complete_enrollment(
                &fixture.pool,
                sibling,
                payload("970002"),
                fixture.enroller_id,
                &crate::http_state::PLACEMENT_ROSTERS
            ),
        );
        let first = first.unwrap();
        let second = second.unwrap();
        let parents: Vec<(String,String,String)> = sqlx::query_as("SELECT username,password_hash,first_name FROM users WHERE username IN ('0800000001','0800000002') ORDER BY username")
            .fetch_all(&fixture.pool).await.unwrap();
        assert_eq!(parents.len(), 2);
        assert!(bcrypt::verify("0800000001", &parents[0].1).unwrap());
        assert_eq!(
            parents[1],
            (
                "0800000002".into(),
                "existing-secret-hash".into(),
                "Existing".into()
            )
        );
        let after_roles: Vec<Uuid> =
            sqlx::query_scalar("SELECT role_id FROM user_roles WHERE user_id=$1 ORDER BY role_id")
                .bind(existing)
                .fetch_all(&fixture.pool)
                .await
                .unwrap();
        assert_eq!(before_roles, after_roles);
        let links: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM student_parents WHERE student_user_id=ANY($1)",
        )
        .bind([first.user_id, second.user_id])
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(links, 4);
        let student_hash: String =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
                .bind(first.user_id)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert!(bcrypt::verify("970001", student_hash.as_str()).unwrap());
    }

    fn submit_payload(
        track: Uuid,
        national_id: &str,
        first_name: &str,
    ) -> school_admission::applications::SubmitApplicationRequest {
        serde_json::from_value(serde_json::json!({ "admissionTrackId": track, "nationalId": national_id, "firstName": first_name, "lastName": "Fixture" })).unwrap()
    }

    async fn open_submission(fixture: &EnrollmentFixture) -> (Uuid, Uuid) {
        let (round, track): (Uuid, Uuid) = sqlx::query_as(
            "SELECT admission_round_id,admission_track_id FROM admission_applications WHERE id=$1",
        )
        .bind(fixture.application_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        sqlx::query("UPDATE admission_rounds SET status='open' WHERE id=$1")
            .bind(round)
            .execute(&fixture.pool)
            .await
            .unwrap();
        (round, track)
    }

    #[tokio::test]
    async fn admission_number_counter_backfills_and_rolls_back_failed_or_duplicate_submissions() {
        use chrono::Datelike;
        use school_admission::applications::submit_application;
        let fixture = enrollment_fixture_through("admission_number_counter_backfill", 3, 117).await;
        let (round, track) = open_submission(&fixture).await;
        let today =
            chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(7 * 3600).unwrap());
        let prefix = format!(
            "{:02}{:02}{:02}01",
            (today.year() + 543) % 100,
            today.month(),
            today.day()
        );
        sqlx::query("UPDATE admission_applications SET application_number=$2 WHERE id=$1")
            .bind(fixture.application_id)
            .bind(format!("{prefix}00042"))
            .execute(&fixture.pool)
            .await
            .unwrap();
        let before: serde_json::Value =
            sqlx::query_scalar("SELECT to_jsonb(a) FROM admission_applications a WHERE id=$1")
                .bind(fixture.application_id)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        apply_migrations_through(&fixture.pool, 118).await.unwrap();
        let after: serde_json::Value =
            sqlx::query_scalar("SELECT to_jsonb(a) FROM admission_applications a WHERE id=$1")
                .bind(fixture.application_id)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert_eq!(
            before, after,
            "counter backfill must preserve the entire application row"
        );
        apply_migrations_through(&fixture.pool, school_migrations::latest_migration_version())
            .await
            .unwrap();
        let (number, application) = submit_application(
            &fixture.pool,
            round,
            submit_payload(track, "1100000000001", "First"),
        )
        .await
        .unwrap();
        assert_eq!(number, format!("{prefix}00043"));
        assert_eq!(application.national_id, "1100000000001");
        assert!(submit_application(
            &fixture.pool,
            round,
            submit_payload(track, "1100000000001", "Duplicate")
        )
        .await
        .is_err());
        sqlx::raw_sql("CREATE FUNCTION reject_number_fixture() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.first_name='Fail' THEN RAISE EXCEPTION 'fixture rejected'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_number_fixture BEFORE INSERT ON admission_applications FOR EACH ROW EXECUTE FUNCTION reject_number_fixture();")
            .execute(&fixture.pool).await.unwrap();
        assert!(submit_application(
            &fixture.pool,
            round,
            submit_payload(track, "1100000000002", "Fail")
        )
        .await
        .is_err());
        let (number, retry) = submit_application(
            &fixture.pool,
            round,
            submit_payload(track, "1100000000002", "Retry"),
        )
        .await
        .unwrap();
        assert_eq!(number, format!("{prefix}00044"));
        sqlx::query("DELETE FROM admission_applications WHERE id=$1")
            .bind(retry.id)
            .execute(&fixture.pool)
            .await
            .unwrap();
        let (number, _) = submit_application(
            &fixture.pool,
            round,
            submit_payload(track, "1100000000004", "After deletion"),
        )
        .await
        .unwrap();
        assert_eq!(
            number,
            format!("{prefix}00045"),
            "deleted application numbers must never identify a later applicant"
        );
        sqlx::query(
            "UPDATE admission_application_number_counters SET last_sequence=99999 WHERE prefix=$1",
        )
        .bind(&prefix)
        .execute(&fixture.pool)
        .await
        .unwrap();
        let full = submit_application(
            &fixture.pool,
            round,
            submit_payload(track, "1100000000003", "Overflow"),
        )
        .await;
        assert!(matches!(full, Err(AppError::Conflict(_))));
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM admission_applications WHERE admission_round_id=$1",
        )
        .bind(round)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn admission_number_counter_concurrent_submit_is_unique_and_ignores_other_prefixes() {
        use school_admission::applications::submit_application;
        let fixture = enrollment_fixture("admission_number_counter_concurrent", 5).await;
        let (round, track) = open_submission(&fixture).await;
        apply_migrations_through(&fixture.pool, 118).await.unwrap();
        let mut unrelated = fixture.pool.begin().await.unwrap();
        sqlx::query("INSERT INTO admission_application_number_counters VALUES('00000000',1)")
            .execute(&mut *unrelated)
            .await
            .unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock(2570::bigint)")
            .execute(&mut *unrelated)
            .await
            .unwrap();
        let results = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            futures::future::join_all((1..=12).map(|number| {
                let pool = fixture.pool.clone();
                async move {
                    submit_application(
                        &pool,
                        round,
                        submit_payload(track, &format!("120000000{number:04}"), "Concurrent"),
                    )
                    .await
                }
            })),
        )
        .await
        .expect("another prefix and the former year advisory lock must not block submissions");
        unrelated.rollback().await.unwrap();
        let mut numbers: Vec<_> = results
            .into_iter()
            .map(|result| result.unwrap().0[8..].parse::<i32>().unwrap())
            .collect();
        numbers.sort();
        assert_eq!(numbers, (1..=12).collect::<Vec<_>>());
        // An ambiguous request retry remains subject to the authoritative duplicate constraint.
        assert!(submit_application(
            &fixture.pool,
            round,
            submit_payload(track, "1200000000001", "Retry")
        )
        .await
        .is_err());
        let mut boundary = fixture.pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *boundary)
            .await
            .unwrap();
        sqlx::query("SELECT prefix FROM admission_application_number_counters FOR UPDATE")
            .execute(&mut *boundary)
            .await
            .unwrap();
        let duplicate = |name: &'static str| {
            let pool = fixture.pool.clone();
            tokio::spawn(async move {
                submit_application(&pool, round, submit_payload(track, "1200000000099", name)).await
            })
        };
        let first = duplicate("Duplicate first");
        let second = duplicate("Duplicate second");
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let waiting: i64 = sqlx::query_scalar(
                    "WITH RECURSIVE waiting(pid) AS (SELECT pid FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) UNION SELECT a.pid FROM pg_stat_activity a JOIN waiting w ON w.pid=ANY(pg_blocking_pids(a.pid))) SELECT count(*) FROM waiting",
                )
                .bind(pid)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
                if waiting == 2 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("both duplicate requests must pass the precheck before allocating");
        boundary.rollback().await.unwrap();
        let results = [first.await.unwrap(), second.await.unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| matches!(result, Err(AppError::BadRequest(message)) if message=="เลขบัตรประชาชนนี้ได้สมัครรอบนี้ไปแล้ว")).count(),1);
        let last: i32 =
            sqlx::query_scalar("SELECT last_sequence FROM admission_application_number_counters")
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        assert_eq!(last, 13);
    }

    #[tokio::test]
    async fn enrollment_prehash_revalidates_assigned_code_after_row_lock_wait() {
        let fixture = enrollment_fixture("admission_enrollment_stale_prepared_hash", 3).await;
        sqlx::query("UPDATE admission_applications SET assigned_student_id='970001' WHERE id=$1")
            .bind(fixture.application_id)
            .execute(&fixture.pool)
            .await
            .unwrap();
        let mut boundary = fixture.pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *boundary)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM admission_applications WHERE id=$1 FOR UPDATE")
            .bind(fixture.application_id)
            .execute(&mut *boundary)
            .await
            .unwrap();
        let pool = fixture.pool.clone();
        let id = fixture.application_id;
        let actor = fixture.enroller_id;
        let worker = tokio::spawn(async move {
            complete_enrollment(
                &pool,
                id,
                CompleteEnrollmentRequest {
                    placement_starts_on: chrono::NaiveDate::from_ymd_opt(2027, 5, 1).unwrap(),
                    student_code: None,
                    form_data: None,
                },
                actor,
                &crate::http_state::PLACEMENT_ROSTERS,
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let waiting: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))")
                    .bind(pid).fetch_one(&fixture.pool).await.unwrap();
                if waiting { break; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        sqlx::query("UPDATE admission_applications SET assigned_student_id='970099' WHERE id=$1")
            .bind(fixture.application_id)
            .execute(&mut *boundary)
            .await
            .unwrap();
        boundary.commit().await.unwrap();
        let student = worker.await.unwrap().unwrap();
        assert_eq!(student.student_code, "970099");
        let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
            .bind(student.user_id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
        assert!(bcrypt::verify("970099", &hash).unwrap());
        assert!(!bcrypt::verify("970001", &hash).unwrap());
    }
    async fn room_assignment_state(pool: &PgPool, application: Uuid) -> serde_json::Value {
        sqlx::query_scalar(
            "SELECT jsonb_build_object('application',(SELECT to_jsonb(a) FROM admission_applications a WHERE id=$1),'assignment',(SELECT to_jsonb(r) FROM admission_room_assignments r WHERE application_id=$1))",
        ).bind(application).fetch_one(pool).await.unwrap()
    }

    fn assert_assignment_database_fault(error: AppError) {
        let AppError::DbError(sqlx::Error::Database(error)) = error else {
            panic!("assignment failure must preserve its typed database cause");
        };
        assert_eq!(error.code().as_deref(), Some("P0001"));
    }

    #[tokio::test]
    async fn admission_database_error_policy_real_missing_round_and_assignment_keep_domain_errors()
    {
        let fixture = enrollment_fixture("admission_missing_assignment_policy", 1).await;
        assert!(matches!(
            school_admission::rounds::update_round_status(&fixture.pool, Uuid::new_v4(), "open")
                .await,
            Err(AppError::NotFound(_))
        ));
        sqlx::query("DELETE FROM admission_room_assignments WHERE application_id=$1")
            .bind(fixture.application_id)
            .execute(&fixture.pool)
            .await
            .unwrap();
        assert!(matches!(
            school_admission::applications::move_application_room(
                &fixture.pool,
                fixture.application_id,
                Uuid::new_v4()
            )
            .await,
            Err(AppError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn admission_database_error_policy_reset_rolls_back_delete_when_following_update_fails() {
        let fixture = enrollment_fixture("admission_reset_rollback_policy", 1).await;
        let round: Uuid =
            sqlx::query_scalar("SELECT admission_round_id FROM admission_applications WHERE id=$1")
                .bind(fixture.application_id)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        let before = room_assignment_state(&fixture.pool, fixture.application_id).await;
        sqlx::raw_sql("CREATE FUNCTION fail_admission_reset_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture reset update failure'; END $$; CREATE TRIGGER fail_admission_reset_update BEFORE UPDATE OF room_assignment_track_id ON admission_applications FOR EACH ROW EXECUTE FUNCTION fail_admission_reset_update();")
            .execute(&fixture.pool).await.unwrap();
        let error = school_admission::selections::reset_all_room_assignments(&fixture.pool, round)
            .await
            .unwrap_err();
        assert_assignment_database_fault(error);
        assert_eq!(
            room_assignment_state(&fixture.pool, fixture.application_id).await,
            before
        );

        sqlx::query("DROP TRIGGER fail_admission_reset_update ON admission_applications")
            .execute(&fixture.pool)
            .await
            .unwrap();
        assert_eq!(
            school_admission::selections::reset_all_room_assignments(&fixture.pool, round)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            school_admission::selections::reset_all_room_assignments(&fixture.pool, round)
                .await
                .unwrap(),
            0
        );
        let state: (i64, String) = sqlx::query_as("SELECT (SELECT count(*) FROM admission_room_assignments WHERE application_id=$1),status FROM admission_applications WHERE id=$1")
            .bind(fixture.application_id).fetch_one(&fixture.pool).await.unwrap();
        assert_eq!(state, (0, "verified".into()));
    }

    #[tokio::test]
    async fn admission_database_error_policy_move_rolls_back_assignment_when_rank_update_fails() {
        let fixture = enrollment_fixture("admission_move_rank_rollback_policy", 1).await;
        let target: Uuid = sqlx::query_scalar("INSERT INTO homerooms(code,name,academic_year_id,grade_level_id,room_number,study_program_id,capacity) SELECT 'ADM-RANK-FAULT','Fault target',academic_year_id,grade_level_id,'2',study_program_id,40 FROM homerooms WHERE id=(SELECT homeroom_id FROM admission_room_assignments WHERE application_id=$1) RETURNING id")
            .bind(fixture.application_id).fetch_one(&fixture.pool).await.unwrap();
        let before = room_assignment_state(&fixture.pool, fixture.application_id).await;
        sqlx::raw_sql("CREATE FUNCTION fail_admission_move_rank() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rank update failure'; END $$; CREATE TRIGGER fail_admission_move_rank BEFORE UPDATE OF rank_in_room ON admission_room_assignments FOR EACH ROW EXECUTE FUNCTION fail_admission_move_rank();")
            .execute(&fixture.pool).await.unwrap();
        let error = school_admission::applications::move_application_room(
            &fixture.pool,
            fixture.application_id,
            target,
        )
        .await
        .unwrap_err();
        assert_assignment_database_fault(error);
        assert_eq!(
            room_assignment_state(&fixture.pool, fixture.application_id).await,
            before
        );
    }
}
