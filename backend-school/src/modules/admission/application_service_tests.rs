#[cfg(test)]
mod tests {
    use crate::modules::academic::cutover_test_support::{
        apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
        CutoverFixture,
    };
    use school_admission::applications::{
        complete_enrollment, AssignRoomsGlobalRequest, AssignRoomsRequest, BulkScoreEntry,
        CompleteEnrollmentRequest, UpdateScoreEntry,
    };
    use school_admission::scores::{bulk_update_scores, update_application_scores};
    use school_admission::selections::{assign_rooms, assign_rooms_global, get_round_ranking};
    use school_errors::AppError;
    use school_test_db::{create_named_test_pool_with_max_connections, create_test_user};
    use sqlx::PgPool;
    use uuid::Uuid;

    struct EnrollmentFixture {
        pool: PgPool,
        application_id: Uuid,
        academic_year_id: Uuid,
        enroller_id: Uuid,
    }

    async fn enrollment_fixture(name: &str, connections: u32) -> EnrollmentFixture {
        let pool = create_named_test_pool_with_max_connections(name, connections).await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_phase_b_runtime_migrations(&pool).await.unwrap();
        apply_migrations_through(&pool, 67).await.unwrap();

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
                     ORDER BY is_default DESC, id
                     LIMIT 1
                 ) program
                 ORDER BY grade.id
                 LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let academic_year_id: Uuid = sqlx::query_scalar(
            "INSERT INTO academic_years (
                 year, name, start_date, end_date, school_days, status
             ) VALUES (
                 2570, 'ปีการศึกษาอนาคต', '2027-05-01', '2028-03-31',
                 'MON,TUE,WED,THU,FRI', 'planning'
             ) RETURNING id",
        )
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
        let first = complete_enrollment(&pool, application_id, enrollment_payload(), enroller_id)
            .await
            .unwrap();
        let replay = complete_enrollment(&pool, application_id, enrollment_payload(), enroller_id)
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
            let replay =
                complete_enrollment(&pool, application_id, enrollment_payload(), enroller_id)
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
}
