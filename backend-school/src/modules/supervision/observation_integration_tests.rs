#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};
    use uuid::Uuid;

    use crate::modules::academic::cutover_test_support::{
        apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
        CutoverFixture,
    };
    use school_academic_timetable::models::timetable_version::CloneTimetableVersionRequest;
    use school_academic_timetable::services::timetable_version_service;
    use school_errors::AppError;
    use school_supervision::services::{
        load_timetable_block_group_context_for_teacher_for_test, resolve_lesson_input_for_test,
        test_bangkok_observation_date, test_day_of_week_matches_observed_at, LessonResolutionCycle,
    };
    use school_test_db::create_named_test_pool;

    #[test]
    fn timetable_version_date_uses_bangkok_across_local_midnight() {
        let observed_at = DateTime::parse_from_rfc3339("2026-08-31T18:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(
            test_bangkok_observation_date(observed_at),
            NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()
        );
    }

    #[tokio::test]
    async fn exact_timetable_block_group_context_rejects_inactive_groups() {
        let pool = create_named_test_pool("supervision_exact_active_timetable_block_group").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_phase_b_runtime_migrations(&pool).await.unwrap();
        apply_migrations_through(&pool, 58).await.unwrap();
        let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
        let (source_id, source_row_version, term_start): (Uuid, i64, NaiveDate) = sqlx::query_as(
            r#"SELECT version.id, version.row_version, term.start_date
                   FROM academic_timetable_versions version
                   JOIN academic_terms term ON term.id = version.academic_term_id
                   WHERE version.status = 'published' AND term.status = 'active'
                   ORDER BY version.effective_from, version.id
                   LIMIT 1"#,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let (source_group, source_teacher, source_year, source_term, source_day): (Uuid, Uuid, Uuid, Uuid, String) = sqlx::query_as(
            "SELECT block_group.id, instructor.instructor_id, block.academic_year_id, block.academic_term_id, block.day_of_week
             FROM academic_timetable_blocks block
             JOIN academic_timetable_block_groups block_group ON block_group.block_id=block.id
             JOIN academic_timetable_block_group_instructors instructor ON instructor.block_group_id=block_group.id
             WHERE block.timetable_version_id=$1 AND block.is_active AND block_group.is_active
             ORDER BY block_group.id,instructor.instructor_id LIMIT 1",
        ).bind(source_id).fetch_one(&pool).await.unwrap();
        let observed_at = (0..7)
            .map(|days| {
                (term_start + chrono::Duration::days(days))
                    .and_hms_opt(5, 0, 0)
                    .unwrap()
                    .and_utc()
            })
            .find(|at| test_day_of_week_matches_observed_at(&source_day, *at))
            .unwrap();
        let cycle = LessonResolutionCycle {
            id: Uuid::new_v4(),
            academic_year_id: source_year,
            academic_term_id: Some(source_term),
            template_id: Uuid::new_v4(),
            status: "open".into(),
            booking_opens_at: None,
            booking_closes_at: None,
            starts_at: observed_at - chrono::Duration::days(1),
            ends_at: observed_at + chrono::Duration::days(1),
        };
        let mut transaction = pool.begin().await.unwrap();
        let lesson = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            resolve_lesson_input_for_test(
                &mut transaction,
                &cycle,
                source_term,
                source_teacher,
                Some(source_group),
                Some(observed_at),
                None,
            ),
        )
        .await
        .expect("timetable resolution must reuse the transaction's only connection")
        .unwrap();
        assert_eq!(lesson.timetable_block_group_id, Some(source_group));
        assert_eq!(lesson.source.as_deref(), Some("timetable"));
        transaction.rollback().await.unwrap();
        let draft = timetable_version_service::clone_draft(
            &pool,
            actor_id,
            source_id,
            CloneTimetableVersionRequest {
                effective_from: term_start.succ_opt().unwrap(),
                source_row_version,
            },
        )
        .await
        .unwrap();
        let (block_group_id, teacher_id, academic_year_id, academic_term_id): (
            Uuid,
            Uuid,
            Uuid,
            Uuid,
        ) = sqlx::query_as(
            r#"SELECT block_group.id, instructor.instructor_id,
                      block.academic_year_id, block.academic_term_id
               FROM academic_timetable_blocks block
               JOIN academic_timetable_block_groups block_group
                 ON block_group.block_id = block.id
               JOIN academic_timetable_block_group_instructors instructor
                 ON instructor.block_group_id = block_group.id
               WHERE block.timetable_version_id = $1
                 AND block.is_active AND block_group.is_active
               ORDER BY block_group.id, instructor.instructor_id
               LIMIT 1"#,
        )
        .bind(draft.id)
        .fetch_one(&pool)
        .await
        .unwrap();

        load_timetable_block_group_context_for_teacher_for_test(
            &pool,
            block_group_id,
            draft.id,
            teacher_id,
            academic_year_id,
            academic_term_id,
        )
        .await
        .expect("active exact timetable block group must be selectable");
        sqlx::query("UPDATE academic_timetable_block_groups SET is_active = false WHERE id = $1")
            .bind(block_group_id)
            .execute(&pool)
            .await
            .unwrap();
        let inactive = load_timetable_block_group_context_for_teacher_for_test(
            &pool,
            block_group_id,
            draft.id,
            teacher_id,
            academic_year_id,
            academic_term_id,
        )
        .await;
        assert!(matches!(inactive, Err(AppError::Forbidden(_))));
    }
}
