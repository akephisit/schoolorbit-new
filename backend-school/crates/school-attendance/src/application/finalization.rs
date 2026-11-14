//! Application adapter: dated providers resolve arrival/special rosters, Attendance
//! owns facts and durable progress. All reads share the finalization transaction.
use super::*;
use chrono::{DateTime, Utc};
use school_database::DbSession;
use sqlx::{types::Json, PgConnection};

pub async fn expected_rounds_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<AttendanceSeed>, AppError> {
    let context = settings::AttendanceReadContext::load(connection, term).await?;
    expected_rounds_with_context(connection, &context, date).await
}
pub(crate) async fn expected_rounds_with_context(
    connection: &mut PgConnection,
    context: &settings::AttendanceReadContext,
    date: NaiveDate,
) -> Result<Vec<AttendanceSeed>, AppError> {
    let settings = &context.settings;
    let term = settings.academic_term_id;
    let mut seeds = Vec::new();
    let calendar_counted = crate::services::processing_dates::on_connection(connection, term, date)
        .await?
        .is_some_and(|day| day.calendar_counted);
    if calendar_counted {
        let students = core::students_on_connection(connection, term, date).await?;
        let rooms = core::rooms_for_date_on_connection(connection, term, date).await?;
        let saved_arrivals: HashSet<Uuid> = DbSession::new(&mut *connection)
            .scalar(
                "attendance.application.finalization.expected_rounds_with_context",
                r#"SELECT DISTINCT p."studentId" FROM attendance_sessions s
        CROSS JOIN LATERAL jsonb_to_recordset(s.roster) p("studentId" uuid)
        WHERE s.academic_term_id=$1 AND s.date=$2 AND s.kind='arrival'"#,
            )
            .bind(term)
            .bind(date)
            .fetch_all()
            .await?
            .into_iter()
            .collect();
        for room in rooms {
            let roster = students
                .iter()
                .filter(|s| {
                    s.homeroom_id == Some(room.id) && !saved_arrivals.contains(&s.student_id)
                })
                .map(|s| AttendanceRosterStudent {
                    student_id: s.student_id,
                    student_academic_year_id: s.student_academic_year_id,
                    class_number: s.class_number,
                })
                .collect();
            seeds.push(arrival_seed(
                term,
                date,
                room,
                roster,
                settings.configuration.late_after,
            ));
        }
    }
    // A current weekly pattern cannot prove an overdue lesson. Materialized
    // sessions retain their actual identities, times, teachers and frozen rosters.
    seeds.extend(materialized_lesson_seeds_on_connection(connection, term, date).await?);
    let rounds: Vec<(Uuid,Json<Vec<SpecialRoundParticipants>>)> = DbSession::new(&mut *connection).query_as("attendance.application.finalization.expected_rounds_with_context", "SELECT r.id,r.participants FROM attendance_special_rounds r JOIN attendance_special_templates t ON t.id=r.template_id WHERE t.academic_term_id=$1 AND r.date=$2 ORDER BY r.id")
        .bind(term).bind(date).fetch_all().await?;
    for (round, Json(groups)) in rounds {
        for group in groups {
            let mut session = seed_session(
                term,
                date,
                AttendanceKind::Special,
                group.source_key,
                group.title,
                group.teacher_ids,
                None,
                None,
                None,
                group.start_time,
                group.end_time,
            );
            session.id = group.session_id;
            session.special_round_id = Some(round);
            session.count_override = group.count_override;
            seeds.push(AttendanceSeed {
                session,
                students: group.students,
            });
        }
    }
    if seeds.len() > 2000 || seeds.iter().map(|s| s.students.len()).sum::<usize>() > 200_000 {
        return Err(invalid("รอบปิดยอดเกินขนาดที่รองรับ"));
    }
    Ok(seeds)
}

pub async fn catch_up(pool: &PgPool, now: DateTime<Utc>, limit: i64) -> Result<usize, AppError> {
    let due = crate::services::finalization::due_dates_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "attendance.finalization.read",
        )
        .await?,
        now,
        limit,
    )
    .await?;
    let count = due.len();
    for (term, date, generation) in due {
        let mut tx =
            school_tenancy::TenantPool::begin_observed(pool, "attendance.finalization.transaction")
                .await?;
        let guard = crate::services::finalization::guard_finalization(&mut tx, term).await?;
        let seeds = crate::services::finalization::ValidatedFinalizationSeeds::resolve(
            &mut tx, &guard, date, generation,
        )
        .await?;
        crate::services::finalization::finalize_guarded(
            &mut tx, &guard, date, generation, &seeds, now,
        )
        .await?;
        tx.commit().await?;
    }
    Ok(count)
}
