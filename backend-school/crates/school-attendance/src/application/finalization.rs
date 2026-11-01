//! Application adapter: academic providers resolve historical rosters, Attendance
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
        let spans = school_academic_timetable::services::attendance::published_spans(
            connection, term, date, date,
        )
        .await?;
        let delivery_version_id = spans
            .iter()
            .find(|span| span.effective_from <= date && date <= span.effective_until)
            .map(|span| span.delivery_version_id);
        let mut blocks =
            spans
                .into_iter()
                .filter(|span| span.effective_from <= date && date <= span.effective_until)
                .flat_map(|span| span.blocks)
                .filter(|block| {
                    block.block_kind != TimetableBlockKind::Structural
                && block.day_of_week
                    == school_academic_timetable::services::daily_teaching::day_code_from_date(date)
                })
                .collect::<Vec<_>>();
        blocks.sort_by_key(|b| (b.bell_schedule_period_id, b.id));
        let groups = blocks
            .iter()
            .flat_map(|b| b.groups.iter())
            .filter(|g| g.is_active)
            .map(|g| g.learning_group_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let memberships = match delivery_version_id {
        Some(version) => {
            school_academic_delivery::services::attendance_roster::pinned_students_on_connection(
                connection, version, &groups, date,
            )
            .await?
        }
        None => Vec::new(),
    };
        let by_student: HashMap<_, _> = students.iter().map(|s| (s.student_id, s)).collect();
        for block in blocks {
            for group in block.groups.into_iter().filter(|g| g.is_active) {
                let roster = memberships
                    .iter()
                    .filter(|m| m.learning_group_id == group.learning_group_id)
                    .map(|m| AttendanceRosterStudent {
                        student_id: m.student_id,
                        student_academic_year_id: m.student_academic_year_id,
                        class_number: by_student.get(&m.student_id).and_then(|s| s.class_number),
                    })
                    .collect();
                let title = format!(
                    "{} · {} · {}",
                    block.offering_name.as_deref().unwrap_or("คาบเรียน"),
                    group.name,
                    block.period_name
                );
                let session = seed_session(
                    term,
                    date,
                    AttendanceKind::Lesson,
                    format!(
                        "{}:{}",
                        group.learning_group_id, block.bell_schedule_period_id
                    ),
                    title,
                    group.instructors.iter().map(|i| i.teacher_id).collect(),
                    None,
                    Some(group.learning_group_id),
                    block.learning_offering_id,
                    block.start_time,
                    block.end_time,
                );
                seeds.push(AttendanceSeed {
                    session,
                    students: roster,
                });
            }
        }
    }
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
