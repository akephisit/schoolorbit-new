//! Read-only coverage from dated providers, aggregated before crossing the database boundary.
use super::*;
use std::collections::BTreeMap;

type CoverageKey = (Uuid, String, String, Vec<Uuid>);
fn add(
    rows: &mut BTreeMap<CoverageKey, AttendanceExpectedSummary>,
    student: Uuid,
    category: &str,
    scope: &str,
    label: &str,
    teachers: &[Uuid],
) {
    let mut ids = teachers.to_vec();
    ids.sort();
    ids.dedup();
    rows.entry((student, category.into(), scope.into(), ids.clone()))
        .or_insert_with(|| AttendanceExpectedSummary {
            student_id: student,
            category: category.into(),
            scope_key: scope.into(),
            scope_label: label.into(),
            teacher_ids: ids,
            expected: 0,
        })
        .expected += 1;
}
pub async fn expected(
    connection: &mut sqlx::PgConnection,
    term: Uuid,
    range: Option<(NaiveDate, NaiveDate)>,
) -> Result<Vec<AttendanceExpectedSummary>, AppError> {
    let context = settings::AttendanceReadContext::load(connection, term).await?;
    expected_with_context(connection, &context, range).await
}
pub(crate) async fn expected_with_context(
    connection: &mut sqlx::PgConnection,
    context: &settings::AttendanceReadContext,
    range: Option<(NaiveDate, NaiveDate)>,
) -> Result<Vec<AttendanceExpectedSummary>, AppError> {
    Ok(project(connection, context, range, None).await?.0)
}
pub(super) async fn history(
    connection: &mut sqlx::PgConnection,
    context: &settings::AttendanceReadContext,
    start: NaiveDate,
    end: NaiveDate,
    student: Uuid,
    teacher: Option<Uuid>,
) -> Result<Vec<AttendanceHistoryItem>, AppError> {
    Ok(project(
        connection,
        context,
        Some((start, end)),
        Some((student, teacher)),
    )
    .await?
    .1)
}
fn history_row(
    rows: &mut Vec<AttendanceHistoryItem>,
    scope: Option<(Uuid, Option<Uuid>)>,
    student: Uuid,
    teachers: &[Uuid],
    id: Uuid,
    date: NaiveDate,
    kind: AttendanceKind,
    title: &str,
) {
    if scope.is_some_and(|(selected, teacher)| {
        selected == student && teacher.is_none_or(|t| teachers.contains(&t))
    }) {
        rows.push(AttendanceHistoryItem {
            session_id: id,
            date,
            kind,
            title: title.into(),
            result: AttendanceResult::Unchecked,
            observed_at: None,
            evidence_file_id: None,
            cancelled: false,
        });
    }
}
async fn project(
    connection: &mut sqlx::PgConnection,
    context: &settings::AttendanceReadContext,
    range: Option<(NaiveDate, NaiveDate)>,
    history_scope: Option<(Uuid, Option<Uuid>)>,
) -> Result<(Vec<AttendanceExpectedSummary>, Vec<AttendanceHistoryItem>), AppError> {
    let term = context.settings.academic_term_id;
    let now = Utc::now().with_timezone(&chrono_tz::Asia::Bangkok);
    let (term_start,term_end,coverage_start):(NaiveDate,NaiveDate,NaiveDate)=sqlx::query_as("SELECT t.start_date,COALESCE(t.closed_on,t.planned_end_date,y.end_date),m.coverage_start FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id CROSS JOIN attendance_module_state m WHERE t.id=$1 AND m.singleton")
        .bind(term).fetch_optional(&mut *connection).await?.ok_or_else(||AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    let start = term_start
        .max(coverage_start)
        .max(range.map(|r| r.0).unwrap_or(term_start));
    let end = term_end
        .min(now.date_naive())
        .min(range.map(|r| r.1).unwrap_or(term_end));
    if start > end {
        return Ok((vec![], vec![]));
    }
    if (end - start).num_days() > 369 {
        return Err(invalid("ช่วงภาคเรียนเกินขนาดที่รองรับ"));
    }
    let configuration = &context.settings.configuration;
    let days = settings::days_on_connection(connection, term, start, end).await?;
    let overrides: HashMap<_, _> = days.into_iter().map(|d| (d.date, d.counted)).collect();
    // Existing occurrences suppress virtual recreation even if their frozen roster
    // excludes a pupil who joined later. This set must not be student-filtered.
    let existing: HashSet<Uuid> = sqlx::query_scalar(
        "SELECT id FROM attendance_sessions WHERE academic_term_id=$1 AND date BETWEEN $2 AND $3",
    )
    .bind(term)
    .bind(start)
    .bind(end)
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .collect();
    // A saved arrival roster owns that pupil's school day, including a cancelled round.
    // A later placement change must not add another virtual school day in the new room.
    let saved_arrivals: HashSet<(NaiveDate, Uuid)> = if let Some((student, _)) = history_scope {
        sqlx::query_as("SELECT DISTINCT s.date,$4::uuid FROM attendance_sessions s WHERE s.academic_term_id=$1 AND s.date BETWEEN $2 AND $3 AND s.kind='arrival' AND s.roster @> jsonb_build_array(jsonb_build_object('studentId',$4::uuid))")
            .bind(term).bind(start).bind(end).bind(student).fetch_all(&mut *connection).await?.into_iter().collect()
    } else {
        sqlx::query_as(
        "SELECT DISTINCT s.date,p.\"studentId\" FROM attendance_sessions s CROSS JOIN LATERAL jsonb_to_recordset(s.roster) p(\"studentId\" uuid) WHERE s.academic_term_id=$1 AND s.date BETWEEN $2 AND $3 AND s.kind='arrival'",
    )
    .bind(term)
    .bind(start)
    .bind(end)
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .collect()
    };
    let placements = if let Some((student, _)) = history_scope {
        core::placement_spans_for_student(connection, term, start, end, student).await?
    } else {
        core::placement_spans(connection, term, start, end).await?
    };
    let rooms = core::rooms_on_connection(connection, term).await?;
    let spans = school_academic_timetable::services::attendance::published_spans(
        connection, term, start, end,
    )
    .await?;
    let group_ids: Vec<_> = spans
        .iter()
        .flat_map(|v| v.blocks.iter())
        .flat_map(|b| b.groups.iter())
        .filter(|g| g.is_active)
        .map(|g| g.learning_group_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let memberships = if let Some((student, _)) = history_scope {
        school_academic_delivery::services::attendance_roster::membership_spans_for_student(
            connection, &group_ids, start, end, student,
        )
        .await?
    } else {
        school_academic_delivery::services::attendance_roster::membership_spans(
            connection, &group_ids, start, end,
        )
        .await?
    };
    let mut groups = HashMap::<Uuid, Vec<_>>::new();
    for membership in &memberships {
        groups
            .entry(membership.learning_group_id)
            .or_default()
            .push(membership);
    }
    let mut rows = BTreeMap::new();
    let mut history = Vec::new();
    let mut date = start;
    while date <= end {
        if crate::rules::counted(&configuration, date, overrides.get(&date).copied()) {
            // Provider ordering resolves an end/start boundary in favor of the latest placement.
            let mut students = HashMap::new();
            for p in &placements {
                if p.start_date <= date && p.end_date.is_none_or(|d| d >= date) {
                    students.entry(p.student_id).or_insert(p);
                }
            }
            if date < now.date_naive() || configuration.late_after <= now.time() {
                for room in &rooms {
                    let source = room.id.to_string();
                    let id = Uuid::new_v5(&term, format!("{date}:arrival:{source}").as_bytes());
                    if !existing.contains(&id) {
                        for p in students.values().filter(|p| {
                            p.homeroom_id == room.id
                                && !saved_arrivals.contains(&(date, p.student_id))
                        }) {
                            if history_scope.is_none() {
                                add(
                                    &mut rows,
                                    p.student_id,
                                    "school",
                                    "",
                                    "การเข้าโรงเรียน",
                                    &room.teacher_ids,
                                );
                            }
                            history_row(
                                &mut history,
                                history_scope,
                                p.student_id,
                                &room.teacher_ids,
                                id,
                                date,
                                AttendanceKind::Arrival,
                                &format!("เข้าโรงเรียน · {}", room.name),
                            );
                        }
                    }
                }
            }
            if let Some(span) = spans
                .iter()
                .find(|v| v.effective_from <= date && date <= v.effective_until)
            {
                let day =
                    school_academic_timetable::services::daily_teaching::day_code_from_date(date);
                for block in span.blocks.iter().filter(|b| {
                    b.day_of_week == day
                        && b.block_kind != TimetableBlockKind::Structural
                        && (date < now.date_naive() || b.start_time <= now.time())
                }) {
                    for group in block.groups.iter().filter(|g| g.is_active) {
                        let source = format!(
                            "{}:{}",
                            group.learning_group_id, block.bell_schedule_period_id
                        );
                        let id = Uuid::new_v5(&term, format!("{date}:lesson:{source}").as_bytes());
                        if existing.contains(&id) {
                            continue;
                        }
                        let teachers = group
                            .instructors
                            .iter()
                            .map(|i| i.teacher_id)
                            .collect::<Vec<_>>();
                        let label = format!(
                            "{} · {} · {}",
                            block.offering_name.as_deref().unwrap_or("คาบเรียน"),
                            group.name,
                            block.period_name
                        );
                        let scope = block
                            .learning_offering_id
                            .map(|id| id.to_string())
                            .unwrap_or_default();
                        if let Some(members) = groups.get(&group.learning_group_id) {
                            for member in members.iter().filter(|m| {
                                m.joined_at <= date && m.left_at.is_none_or(|d| d >= date)
                            }) {
                                if history_scope.is_none() {
                                    add(
                                        &mut rows,
                                        member.student_id,
                                        "lesson",
                                        &scope,
                                        &label,
                                        &teachers,
                                    );
                                }
                                history_row(
                                    &mut history,
                                    history_scope,
                                    member.student_id,
                                    &teachers,
                                    id,
                                    date,
                                    AttendanceKind::Lesson,
                                    &label,
                                );
                            }
                        }
                    }
                }
            }
        }
        if rows.len() > 200000 {
            return Err(invalid("ข้อมูลสรุปเกินขนาดที่รองรับ"));
        }
        let Some(next) = date.succ_opt() else {
            break;
        };
        date = next;
    }
    let rounds:Vec<(Uuid,Uuid,NaiveDate,sqlx::types::Json<Vec<SpecialRoundParticipants>>,String)>=sqlx::query_as("SELECT r.id,t.id,r.date,r.participants,t.definition->>'title' FROM attendance_special_rounds r JOIN attendance_special_templates t ON t.id=r.template_id WHERE t.academic_term_id=$1 AND r.date BETWEEN $2 AND $3 ORDER BY r.date,r.id LIMIT 10001")
        .bind(term).bind(start).bind(end).fetch_all(&mut *connection).await?;
    if rounds.len() > 10000 {
        return Err(invalid("รอบพิเศษเกินขนาดที่รองรับ"));
    }
    for (_, template, date, sqlx::types::Json(participants), label) in rounds {
        for g in participants {
            if existing.contains(&g.session_id)
                || (date == now.date_naive() && g.start_time > now.time())
            {
                continue;
            }
            if !g.count_override.unwrap_or_else(|| {
                crate::rules::counted(&configuration, date, overrides.get(&date).copied())
            }) {
                continue;
            }
            for student in g.students.into_iter().filter(|student| {
                history_scope.is_none_or(|(selected, _)| selected == student.student_id)
            }) {
                if history_scope.is_none() {
                    add(
                        &mut rows,
                        student.student_id,
                        "special",
                        &template.to_string(),
                        &label,
                        &g.teacher_ids,
                    );
                }
                history_row(
                    &mut history,
                    history_scope,
                    student.student_id,
                    &g.teacher_ids,
                    g.session_id,
                    date,
                    AttendanceKind::Special,
                    &g.title,
                );
            }
        }
    }
    if history.len() > 1000 {
        return Err(invalid("รอบในช่วงวันที่เกินขนาดที่รองรับ กรุณาลดช่วงวันที่"));
    }
    Ok((rows.into_values().collect(), history))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coverage_aggregates_dates_but_preserves_teacher_scope() {
        let student = Uuid::new_v4();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut rows = BTreeMap::new();
        add(
            &mut rows,
            student,
            "lesson",
            "offering",
            "title",
            &[a, b, a],
        );
        add(&mut rows, student, "lesson", "offering", "title", &[b, a]);
        add(&mut rows, student, "lesson", "offering", "title", &[b]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.values().map(|r| r.expected).sum::<i64>(), 3);
        assert_eq!(
            rows.values()
                .find(|r| r.teacher_ids.len() == 2)
                .unwrap()
                .expected,
            2
        );
    }
    #[test]
    fn virtual_history_does_not_expose_another_student_or_teacher() {
        let student = Uuid::new_v4();
        let teacher = Uuid::new_v4();
        let date = NaiveDate::MIN;
        let mut rows = Vec::new();
        let id = Uuid::new_v4();
        history_row(
            &mut rows,
            Some((student, Some(teacher))),
            student,
            &[],
            id,
            date,
            AttendanceKind::Lesson,
            "title",
        );
        history_row(
            &mut rows,
            Some((student, Some(teacher))),
            Uuid::new_v4(),
            &[teacher],
            id,
            date,
            AttendanceKind::Lesson,
            "title",
        );
        assert!(rows.is_empty());
        history_row(
            &mut rows,
            Some((student, Some(teacher))),
            student,
            &[teacher],
            id,
            date,
            AttendanceKind::Lesson,
            "title",
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].result, AttendanceResult::Unchecked);
    }
}
