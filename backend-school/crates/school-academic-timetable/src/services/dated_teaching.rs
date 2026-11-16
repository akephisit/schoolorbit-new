//! Dated duties are a narrow overlay over the current weekly timetable. They never
//! mutate Delivery assignments or saved Attendance occurrences.
use chrono::{NaiveDate, NaiveTime};
use school_database::DbSession;
use school_errors::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DatedTeachingLesson {
    pub lesson_key: String,
    pub date: NaiveDate,
    pub block_id: Uuid,
    pub allocation_id: Uuid,
    pub teacher_id: Uuid,
    pub bell_schedule_period_id: Uuid,
    pub period_name: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub learning_group_id: Option<Uuid>,
    pub learning_group_name: Option<String>,
    pub offering_id: Option<Uuid>,
    pub offering_name: Option<String>,
    pub grade_level_ids: Vec<Uuid>,
    pub homeroom_ids: Vec<Uuid>,
    pub homeroom_names: Vec<String>,
    pub room_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub source_revision: i64,
    pub allocation_revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DatedTeachingSnapshot {
    pub source_digest: String,
    pub timetable_state: String,
    pub lessons: Vec<DatedTeachingLesson>,
}
#[derive(Clone, Debug)]
pub struct DatedDutyInput {
    pub id: Uuid,
    pub teacher_id: Uuid,
    pub original_teacher_id: Uuid,
    pub date: NaiveDate,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub lesson: Option<DatedTeachingLesson>,
    pub kind: String,
    pub pair_id: Option<Uuid>,
    pub pair_leg: Option<i16>,
    pub taught_offering_id: Option<Uuid>,
    pub taught_learning_group_id: Option<Uuid>,
    pub taught_activity: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatedTeachingContent {
    pub offering_id: Option<Uuid>,
    pub offering_name: Option<String>,
    pub learning_group_id: Option<Uuid>,
    pub learning_group_name: Option<String>,
    pub subject_id: Option<Uuid>,
    pub activity_id: Option<Uuid>,
    pub scheduling_mode: Option<school_academic_delivery::models::ActivitySchedulingMode>,
    pub activity: Option<String>,
}
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ApprovedDatedDuty {
    pub id: Uuid,
    pub teacher_name: String,
    pub teaching_content: sqlx::types::Json<DatedTeachingContent>,
    pub teacher_id: Uuid,
    pub original_teacher_id: Uuid,
    pub source_snapshot: sqlx::types::Json<DatedTeachingLesson>,
    pub taught_offering_id: Option<Uuid>,
    pub taught_learning_group_id: Option<Uuid>,
    pub taught_activity: Option<String>,
}
fn invalid(message: &str) -> AppError {
    AppError::ValidationError(message.into())
}
pub fn intervals_overlap(a: NaiveTime, b: NaiveTime, c: NaiveTime, d: NaiveTime) -> bool {
    a < d && c < b
}
pub fn lesson_key(date: NaiveDate, allocation: Uuid, teacher: Uuid) -> String {
    format!("{date}:{allocation}:{teacher}")
}
pub async fn lock_context(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Uuid,
) -> Result<(), AppError> {
    school_academic_core::services::lifecycle_guard::require_term_write_exclusive(tx, year, term)
        .await
}
pub async fn lock_teacher_dates(
    connection: &mut PgConnection,
    keys: &[(Uuid, NaiveDate)],
) -> Result<(), AppError> {
    for (teacher, date) in keys.iter().copied().collect::<BTreeSet<_>>() {
        DbSession::new(connection)
            .query(
                "academic_timetable.dated.lock_teacher_dates",
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            )
            .bind(format!("teaching-duty:{teacher}:{date}"))
            .execute()
            .await?;
    }
    Ok(())
}
pub async fn snapshot_on_connection(
    connection: &mut PgConnection,
    year: Uuid,
    term: Uuid,
    teacher: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    start: NaiveTime,
    end: NaiveTime,
) -> Result<DatedTeachingSnapshot, AppError> {
    if to < from || (to - from).num_days() > 366 || start >= end {
        return Err(invalid("ช่วงวันและเวลาสำหรับจัดสอนไม่ถูกต้อง"));
    }
    let current = super::timetable_service::get_for_term_on_connection(connection, term).await?;
    let Some(current) = current else {
        return Ok(DatedTeachingSnapshot {
            source_digest: String::new(),
            timetable_state: "missing".into(),
            lessons: vec![],
        });
    };
    if current.academic_year_id != year {
        return Err(invalid("ภาคเรียนไม่อยู่ในปีการศึกษาที่เลือก"));
    }
    let blocks = super::attendance::current_pattern_on_connection(connection, term).await?;
    let affected = blocks
        .iter()
        .flat_map(|b| b.groups.iter())
        .filter(|g| g.is_active && g.instructors.iter().any(|t| t.teacher_id == teacher))
        .collect::<Vec<_>>();
    let group_ids = affected
        .iter()
        .map(|g| g.learning_group_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let spans = school_academic_delivery::services::attendance_roster::membership_spans(
        connection, &group_ids, from, to,
    )
    .await?;
    let pupils = spans
        .iter()
        .map(|m| m.student_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let student_grades =
        school_academic_core::services::dated_student_directory::grades_for_students_on_connection(
            connection, year, &pupils,
        )
        .await?
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let homerooms = affected
        .iter()
        .flat_map(|g| g.homeroom_ids.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let homeroom_grades=school_academic_core::services::dated_student_directory::grades_for_homerooms_on_connection(connection,year,&homerooms).await?.into_iter().collect::<std::collections::BTreeMap<_,_>>();
    let mut lessons = vec![];
    for n in 0..=(to - from).num_days() {
        let date = from + chrono::Duration::days(n);
        let day = super::daily_teaching::day_code_from_date(date);
        for block in blocks.iter().filter(|b| {
            b.day_of_week == day && intervals_overlap(start, end, b.start_time, b.end_time)
        }) {
            for group in block
                .groups
                .iter()
                .filter(|g| g.is_active && g.instructors.iter().any(|t| t.teacher_id == teacher))
            {
                lessons.push(DatedTeachingLesson {
                    lesson_key: lesson_key(date, group.id, teacher),
                    date,
                    block_id: block.id,
                    allocation_id: group.id,
                    teacher_id: teacher,
                    bell_schedule_period_id: block.bell_schedule_period_id,
                    period_name: block.period_name.clone(),
                    start_time: block.start_time,
                    end_time: block.end_time,
                    learning_group_id: Some(group.learning_group_id),
                    learning_group_name: Some(group.name.clone()),
                    offering_id: block.learning_offering_id,
                    offering_name: block.offering_name.clone(),
                    grade_level_ids: {
                        let pupils = spans
                            .iter()
                            .filter(|m| {
                                m.learning_group_id == group.learning_group_id
                                    && m.joined_at <= date
                                    && m.left_at.is_none_or(|left| left >= date)
                            })
                            .map(|m| m.student_id)
                            .collect::<BTreeSet<_>>();
                        if pupils.is_empty() {
                            group
                                .homeroom_ids
                                .iter()
                                .filter_map(|h| homeroom_grades.get(h).copied())
                                .collect::<BTreeSet<_>>()
                                .into_iter()
                                .collect()
                        } else {
                            pupils
                                .iter()
                                .filter_map(|p| student_grades.get(p).copied())
                                .collect::<BTreeSet<_>>()
                                .into_iter()
                                .collect()
                        }
                    },
                    homeroom_ids: group.homeroom_ids.clone(),
                    homeroom_names: group.homeroom_names.clone(),
                    room_id: group.room_id,
                    room_name: group.room_name.clone(),
                    source_revision: block.row_version,
                    allocation_revision: group.row_version,
                });
            }
            if !block
                .groups
                .iter()
                .any(|g| g.is_active && g.instructors.iter().any(|t| t.teacher_id == teacher))
            {
                for assigned in block
                    .teachers
                    .iter()
                    .filter(|t| t.is_active && t.teacher_id == teacher)
                {
                    lessons.push(DatedTeachingLesson {
                        lesson_key: lesson_key(date, assigned.id, teacher),
                        date,
                        block_id: block.id,
                        allocation_id: assigned.id,
                        teacher_id: teacher,
                        bell_schedule_period_id: block.bell_schedule_period_id,
                        period_name: block.period_name.clone(),
                        start_time: block.start_time,
                        end_time: block.end_time,
                        learning_group_id: None,
                        learning_group_name: None,
                        offering_id: block.learning_offering_id,
                        offering_name: block.offering_name.clone().or(block.title.clone()),
                        grade_level_ids: vec![],
                        homeroom_ids: block
                            .homerooms
                            .iter()
                            .filter(|r| r.is_active)
                            .map(|r| r.homeroom_id)
                            .collect(),
                        homeroom_names: block
                            .homerooms
                            .iter()
                            .filter(|r| r.is_active)
                            .map(|r| r.name.clone())
                            .collect(),
                        room_id: None,
                        room_name: None,
                        source_revision: block.row_version,
                        allocation_revision: assigned.row_version,
                    });
                }
            }
        }
    }
    lessons.sort_by_key(|l| (l.date, l.start_time, l.allocation_id, l.teacher_id));
    let bytes = serde_json::to_vec(&(year, term, teacher, from, to, start, end, &lessons))
        .map_err(|_| invalid("ข้อมูลตารางเรียนไม่ถูกต้อง"))?;
    let source_digest = Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect();
    Ok(DatedTeachingSnapshot {
        source_digest,
        timetable_state: "ready".into(),
        lessons,
    })
}
/// Weekly occupancy belongs to the explicitly selected context. Dated promises
/// remain global across terms; a released weekly source is not offered again,
/// because its cancellation may restore the original obligation.
#[derive(Clone, Debug)]
pub struct AvailabilityWindow {
    pub date: NaiveDate,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
}
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct BusyTeacherInterval {
    pub teacher_id: Uuid,
    pub date: NaiveDate,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub allocation_id: Option<Uuid>,
}
pub fn free_in_intervals(
    rows: &[BusyTeacherInterval],
    teacher: Uuid,
    window: &AvailabilityWindow,
    ignored: &[String],
) -> bool {
    !rows.iter().any(|r| {
        r.teacher_id == teacher
            && r.date == window.date
            && intervals_overlap(r.start_time, r.end_time, window.start_time, window.end_time)
            && !r
                .allocation_id
                .is_some_and(|a| ignored.contains(&lesson_key(r.date, a, teacher)))
    })
}
pub struct AvailabilityIndex {
    rows: std::collections::BTreeMap<(Uuid, NaiveDate), Vec<BusyTeacherInterval>>,
}
impl AvailabilityIndex {
    pub fn new(rows: Vec<BusyTeacherInterval>) -> Self {
        let mut indexed = std::collections::BTreeMap::<_, Vec<_>>::new();
        for row in rows {
            indexed
                .entry((row.teacher_id, row.date))
                .or_default()
                .push(row);
        }
        Self { rows: indexed }
    }
    pub fn is_free(&self, teacher: Uuid, window: &AvailabilityWindow, ignored: &[String]) -> bool {
        self.rows
            .get(&(teacher, window.date))
            .is_none_or(|rows| free_in_intervals(rows, teacher, window, ignored))
    }
    pub fn dated_overlap(&self, teacher: Uuid, window: &AvailabilityWindow) -> bool {
        self.rows.get(&(teacher, window.date)).is_some_and(|rows| {
            rows.iter().any(|r| {
                r.allocation_id.is_none()
                    && intervals_overlap(
                        r.start_time,
                        r.end_time,
                        window.start_time,
                        window.end_time,
                    )
            })
        })
    }
}
pub async fn occupancy_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    teachers: &[Uuid],
    windows: &[AvailabilityWindow],
    request: Uuid,
) -> Result<Vec<BusyTeacherInterval>, AppError> {
    if teachers.len() > 5000 || windows.len() > 10000 {
        return Err(invalid("ขอบเขตตรวจเวลาว่างเกินขนาด"));
    }
    let dates = windows.iter().map(|w| w.date).collect::<BTreeSet<_>>();
    if dates.is_empty() || teachers.is_empty() {
        return Ok(vec![]);
    }
    let blocks = super::attendance::current_pattern_on_connection(connection, term).await?;
    let teacher_set = teachers.iter().copied().collect::<BTreeSet<_>>();
    let mut rows = vec![];
    for date in dates {
        let day = super::daily_teaching::day_code_from_date(date);
        for block in blocks.iter().filter(|b| b.day_of_week == day) {
            for group in block.groups.iter().filter(|g| g.is_active) {
                for teacher in group
                    .instructors
                    .iter()
                    .filter(|t| teacher_set.contains(&t.teacher_id))
                {
                    rows.push(BusyTeacherInterval {
                        teacher_id: teacher.teacher_id,
                        date,
                        start_time: block.start_time,
                        end_time: block.end_time,
                        allocation_id: Some(group.id),
                    });
                }
            }
            for teacher in block.teachers.iter().filter(|t| {
                t.is_active
                    && teacher_set.contains(&t.teacher_id)
                    && !block.groups.iter().any(|g| {
                        g.is_active && g.instructors.iter().any(|i| i.teacher_id == t.teacher_id)
                    })
            }) {
                rows.push(BusyTeacherInterval {
                    teacher_id: teacher.teacher_id,
                    date,
                    start_time: block.start_time,
                    end_time: block.end_time,
                    allocation_id: Some(teacher.id),
                });
            }
        }
    }
    let dates = windows
        .iter()
        .map(|w| w.date)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    rows.extend(DbSession::new(connection).query_as::<BusyTeacherInterval>("academic_timetable.dated.batch_occupancy","SELECT teacher_id,occurrence_date AS date,start_time,end_time,NULL::uuid AS allocation_id FROM academic_teaching_coverage_duties WHERE teacher_id=ANY($1) AND occurrence_date=ANY($2) AND state<>'released' AND request_id<>$3 ORDER BY teacher_id,occurrence_date,start_time,id LIMIT 100001").bind(teachers).bind(&dates).bind(request).fetch_all().await?);
    if rows.len() > 100000 {
        return Err(invalid("หน้าที่สอนเกินขนาดที่ตรวจได้"));
    }
    Ok(rows)
}
pub async fn teacher_free_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    teacher: Uuid,
    date: NaiveDate,
    start: NaiveTime,
    end: NaiveTime,
    request: Uuid,
    ignored: &[String],
) -> Result<bool, AppError> {
    let active:bool=DbSession::new(connection).scalar("academic_timetable.dated.active_teacher","SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND user_type='staff' AND status='active')").bind(teacher).fetch_one().await?;
    if !active {
        return Ok(false);
    }
    let window = AvailabilityWindow {
        date,
        start_time: start,
        end_time: end,
    };
    let rows = occupancy_on_connection(
        connection,
        term,
        &[teacher],
        std::slice::from_ref(&window),
        request,
    )
    .await?;
    Ok(free_in_intervals(&rows, teacher, &window, ignored))
}
fn coherent_joint(a: &DatedDutyInput, b: &DatedDutyInput) -> bool {
    matches!((&a.lesson,&b.lesson),(Some(x),Some(y)) if x.block_id==y.block_id&&a.date==b.date&&a.start_time==b.start_time&&a.end_time==b.end_time&&x.room_id==y.room_id&&a.taught_offering_id==b.taught_offering_id&&a.taught_activity==b.taught_activity&&a.kind==b.kind)
}
#[derive(Serialize)]
struct PersistedTeachingDuty {
    id: Uuid,
    occurrence_date: NaiveDate,
    start_time: NaiveTime,
    end_time: NaiveTime,
    teacher_id: Uuid,
    original_teacher_id: Uuid,
    source_block_id: Option<Uuid>,
    source_allocation_id: Option<Uuid>,
    learning_group_id: Option<Uuid>,
    room_id: Option<Uuid>,
    kind: String,
    pair_id: Option<Uuid>,
    pair_leg: Option<i16>,
    source_snapshot: Option<DatedTeachingLesson>,
    taught_offering_id: Option<Uuid>,
    taught_learning_group_id: Option<Uuid>,
    taught_activity: Option<String>,
    teaching_content: Option<DatedTeachingContent>,
    source_pupils: Vec<Uuid>,
}
pub async fn reserve_on_connection(
    connection: &mut PgConnection,
    request: Uuid,
    revision: i64,
    year: Uuid,
    term: Uuid,
    duties: &[DatedDutyInput],
) -> Result<(), AppError> {
    let mut keys = duties
        .iter()
        .map(|d| (d.teacher_id, d.date))
        .collect::<Vec<_>>();
    keys.extend(request_teacher_dates_on_connection(connection, request).await?);
    lock_teacher_dates(connection, &keys).await?;
    let teachers = duties
        .iter()
        .map(|d| d.teacher_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let active: Vec<Uuid> = DbSession::new(connection)
        .scalar(
            "academic_timetable.dated.active_teachers",
            "SELECT id FROM users WHERE id=ANY($1) AND user_type='staff' AND status='active'",
        )
        .bind(&teachers)
        .fetch_all()
        .await?;
    if active.len() != teachers.len() {
        return Err(invalid("มีผู้สอนที่ไม่พร้อมใช้งานแล้ว"));
    }
    let windows = duties
        .iter()
        .map(|d| AvailabilityWindow {
            date: d.date,
            start_time: d.start_time,
            end_time: d.end_time,
        })
        .collect::<Vec<_>>();
    let busy = AvailabilityIndex::new(
        occupancy_on_connection(connection, term, &teachers, &windows, request).await?,
    );
    let ignored = duties
        .iter()
        .filter_map(|d| d.lesson.as_ref().map(|l| l.lesson_key.clone()))
        .collect::<Vec<_>>();
    for (n, a) in duties.iter().enumerate() {
        let window = &windows[n];
        if a.start_time >= a.end_time {
            return Err(invalid("เวลาสอนไม่ถูกต้อง"));
        }
        if duties[..n].iter().any(|b| {
            a.teacher_id == b.teacher_id
                && a.date == b.date
                && intervals_overlap(a.start_time, a.end_time, b.start_time, b.end_time)
                && !coherent_joint(a, b)
        }) {
            return Err(invalid("ผู้สอนมีหน้าที่ซ้อนกันในแผน"));
        }
        if a.kind == "absence" {
            if busy.dated_overlap(a.teacher_id, window) {
                return Err(invalid("ช่วงลาซ้อนหน้าที่สอนที่รับไว้แล้ว"));
            }
        } else if !busy.is_free(a.teacher_id, window, &ignored) {
            return Err(invalid("ผู้สอนที่เลือกไม่ว่างแล้ว กรุณาเลือกใหม่"));
        }
    }
    let changes = duties
        .iter()
        .filter_map(|d| {
            d.lesson
                .as_ref()
                .map(|l| (l.clone(), d.taught_offering_id, d.taught_activity.clone()))
        })
        .collect::<Vec<_>>();
    validate_content_changes_on_connection(connection, &changes).await?;
    let source = school_academic_delivery::services::current::complete_workspace_on_connection(
        connection, year, term,
    )
    .await?;
    let groups = duties
        .iter()
        .filter_map(|d| d.lesson.as_ref().and_then(|l| l.learning_group_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let from = duties.iter().map(|d| d.date).min();
    let to = duties.iter().map(|d| d.date).max();
    let spans = if let (Some(from), Some(to)) = (from, to) {
        school_academic_delivery::services::attendance_roster::membership_spans(
            connection, &groups, from, to,
        )
        .await?
    } else {
        vec![]
    };
    let mut rows = vec![];
    for d in duties {
        let offering = d
            .taught_offering_id
            .and_then(|id| source.offerings.iter().find(|o| o.id == id));
        let group = offering.and_then(|o| {
            d.taught_learning_group_id
                .and_then(|id| o.groups.iter().find(|g| g.id == id))
        });
        let (subject_id, activity_id, scheduling_mode) = match offering.map(|o| &o.catalog) {
            Some(school_academic_delivery::models::LearningOfferingSnapshot::Course(c)) => {
                (Some(c.subject_id), None, None)
            }
            Some(school_academic_delivery::models::LearningOfferingSnapshot::Activity(a)) => {
                (None, Some(a.activity_id), Some(a.scheduling_mode))
            }
            None => (None, None, None),
        };
        let content = d.lesson.as_ref().map(|lesson| DatedTeachingContent {
            offering_id: d.taught_offering_id,
            offering_name: offering.map(|o| o.name.clone()),
            learning_group_id: d.taught_learning_group_id.or(lesson.learning_group_id),
            learning_group_name: group
                .map(|g| g.name.clone())
                .or(lesson.learning_group_name.clone()),
            subject_id,
            activity_id,
            scheduling_mode,
            activity: d.taught_activity.clone(),
        });
        rows.push(PersistedTeachingDuty {
            id: d.id,
            occurrence_date: d.date,
            start_time: d.start_time,
            end_time: d.end_time,
            teacher_id: d.teacher_id,
            original_teacher_id: d.original_teacher_id,
            source_block_id: d.lesson.as_ref().map(|l| l.block_id),
            source_allocation_id: d.lesson.as_ref().map(|l| l.allocation_id),
            learning_group_id: d.lesson.as_ref().and_then(|l| l.learning_group_id),
            room_id: d.lesson.as_ref().and_then(|l| l.room_id),
            kind: d.kind.clone(),
            pair_id: d.pair_id,
            pair_leg: d.pair_leg,
            source_snapshot: d.lesson.clone(),
            taught_offering_id: d.taught_offering_id,
            taught_learning_group_id: d.taught_learning_group_id,
            taught_activity: d.taught_activity.clone(),
            teaching_content: content,
            source_pupils: spans
                .iter()
                .filter(|m| {
                    Some(m.learning_group_id) == d.lesson.as_ref().and_then(|l| l.learning_group_id)
                        && m.joined_at <= d.date
                        && m.left_at.is_none_or(|left| left >= d.date)
                })
                .map(|m| m.student_id)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        });
    }
    release_locked_on_connection(connection, request, false).await?;
    if !rows.is_empty() {
        DbSession::new(connection).query("academic_timetable.dated.reserve_batch",r#"INSERT INTO academic_teaching_coverage_duties(id,request_id,plan_revision,academic_year_id,academic_term_id,occurrence_date,start_time,end_time,teacher_id,original_teacher_id,source_block_id,source_allocation_id,learning_group_id,room_id,kind,pair_id,pair_leg,state,source_snapshot,taught_offering_id,taught_learning_group_id,taught_activity,teaching_content,source_pupils)
 SELECT r.id,$1,$2,$3,$4,r.occurrence_date,r.start_time,r.end_time,r.teacher_id,r.original_teacher_id,r.source_block_id,r.source_allocation_id,r.learning_group_id,r.room_id,r.kind,r.pair_id,r.pair_leg,'reserved',r.source_snapshot,r.taught_offering_id,r.taught_learning_group_id,r.taught_activity,r.teaching_content,r.source_pupils
 FROM jsonb_to_recordset($5) AS r(id uuid,occurrence_date date,start_time time,end_time time,teacher_id uuid,original_teacher_id uuid,source_block_id uuid,source_allocation_id uuid,learning_group_id uuid,room_id uuid,kind text,pair_id uuid,pair_leg smallint,source_snapshot jsonb,taught_offering_id uuid,taught_learning_group_id uuid,taught_activity text,teaching_content jsonb,source_pupils uuid[])"#).bind(request).bind(revision).bind(year).bind(term).bind(sqlx::types::Json(&rows)).execute().await?;
    }
    Ok(())
}
pub async fn request_teacher_dates_on_connection(
    connection: &mut PgConnection,
    request: Uuid,
) -> Result<Vec<(Uuid, NaiveDate)>, AppError> {
    Ok(DbSession::new(connection).query_as("academic_timetable.dated.request_teacher_dates","SELECT DISTINCT teacher_id,occurrence_date FROM academic_teaching_coverage_duties WHERE request_id=$1 AND state<>'released' ORDER BY teacher_id,occurrence_date").bind(request).fetch_all().await?)
}
pub async fn release_on_connection(
    connection: &mut PgConnection,
    request: Uuid,
    future_only: bool,
) -> Result<(), AppError> {
    let keys = request_teacher_dates_on_connection(connection, request).await?;
    lock_teacher_dates(connection, &keys).await?;
    release_locked_on_connection(connection, request, future_only).await
}
async fn release_locked_on_connection(
    connection: &mut PgConnection,
    request: Uuid,
    future_only: bool,
) -> Result<(), AppError> {
    DbSession::new(connection).query("academic_timetable.dated.release",r#"UPDATE academic_teaching_coverage_duties d SET state='released'
 WHERE d.request_id=$1 AND d.state<>'released' AND (NOT $2
 OR (d.kind='absence' AND (d.occurrence_date+d.end_time)>(clock_timestamp() AT TIME ZONE 'Asia/Bangkok'))
 OR (d.kind='substitution' AND (d.occurrence_date+d.start_time)>(clock_timestamp() AT TIME ZONE 'Asia/Bangkok'))
 OR (d.kind='reciprocal' AND NOT EXISTS(SELECT 1 FROM academic_teaching_coverage_duties started WHERE started.request_id=d.request_id AND started.plan_revision=d.plan_revision AND started.pair_id=d.pair_id AND (started.occurrence_date+started.start_time)<=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok'))))"#).bind(request).bind(future_only).execute().await?;
    Ok(())
}
pub async fn approve_on_connection(
    connection: &mut PgConnection,
    request: Uuid,
) -> Result<(), AppError> {
    let keys = request_teacher_dates_on_connection(connection, request).await?;
    lock_teacher_dates(connection, &keys).await?;
    DbSession::new(connection).query("academic_timetable.dated.approve","UPDATE academic_teaching_coverage_duties SET state='approved' WHERE request_id=$1 AND state='reserved'").bind(request).execute().await?;
    Ok(())
}
pub async fn approved_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<ApprovedDatedDuty>, AppError> {
    Ok(DbSession::new(connection).query_as("academic_timetable.dated.approved","SELECT d.id,d.teacher_id,u.display_name AS teacher_name,d.original_teacher_id,d.source_snapshot,d.taught_offering_id,d.taught_learning_group_id,d.taught_activity,d.teaching_content FROM academic_teaching_coverage_duties d JOIN user_display_names u ON u.id=d.teacher_id WHERE d.academic_term_id=$1 AND d.occurrence_date=$2 AND d.state='approved' AND d.kind<>'absence' ORDER BY d.start_time,d.id").bind(term).bind(date).fetch_all().await?)
}
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AttendanceDatedGrant {
    pub date: NaiveDate,
    pub allocation_id: Uuid,
    pub source_group_id: Uuid,
    pub actual_group_id: Option<Uuid>,
    pub period_id: Uuid,
    pub source_pupils: Vec<Uuid>,
}
impl AttendanceDatedGrant {
    pub fn source_keys(&self) -> [String; 2] {
        [
            format!("placement:{}", self.allocation_id),
            format!("{}:{}", self.source_group_id, self.period_id),
        ]
    }
}
pub async fn attendance_grants_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    teacher: Uuid,
) -> Result<Vec<AttendanceDatedGrant>, AppError> {
    let rows:Vec<AttendanceDatedGrant>=DbSession::new(connection).query_as("academic_timetable.dated.attendance_grants","SELECT occurrence_date AS date,source_allocation_id AS allocation_id,learning_group_id AS source_group_id,taught_learning_group_id AS actual_group_id,(source_snapshot->>'bellSchedulePeriodId')::uuid AS period_id,source_pupils FROM academic_teaching_coverage_duties WHERE academic_term_id=$1 AND teacher_id=$2 AND state='approved' AND kind<>'absence' AND learning_group_id IS NOT NULL ORDER BY occurrence_date,id LIMIT 20001").bind(term).bind(teacher).fetch_all().await?;
    if rows.len() > 20000 {
        return Err(invalid("หน้าที่สอนเกินขนาดที่รองรับ"));
    }
    Ok(rows)
}
/// A shared student allocation retains one coherent original lesson. Teacher
/// substitutions remain per-instructor; a new subject cannot rewrite colleagues.
pub async fn joint_allocation_ids_on_connection(
    connection: &mut PgConnection,
    allocations: &[Uuid],
) -> Result<BTreeSet<Uuid>, AppError> {
    if allocations.len() > 6000 {
        return Err(invalid("กลุ่มจัดสอนเกินขนาด"));
    }
    let ids:Vec<Uuid>=DbSession::new(connection).scalar("academic_timetable.dated.joint_allocations","SELECT block_group_id FROM academic_timetable_block_group_instructors WHERE block_group_id=ANY($1) GROUP BY block_group_id HAVING count(*)>1").bind(allocations).fetch_all().await?;
    Ok(ids.into_iter().collect())
}
pub async fn validate_content_changes_on_connection(
    connection: &mut PgConnection,
    changes: &[(DatedTeachingLesson, Option<Uuid>, Option<String>)],
) -> Result<(), AppError> {
    let ids = changes
        .iter()
        .map(|(l, _, _)| l.allocation_id)
        .collect::<Vec<_>>();
    let joint = joint_allocation_ids_on_connection(connection, &ids).await?;
    if changes.iter().any(|(lesson, offering, activity)| {
        joint.contains(&lesson.allocation_id)
            && (*offering != lesson.offering_id || activity.is_some())
    }) {
        return Err(invalid(
            "คาบที่มีครูร่วมสอนต้องคงวิชาเดิม สามารถเลือกครูต่างกลุ่มสาระมาสอนแทนวิชาเดิมได้",
        ));
    }
    Ok(())
}
pub async fn apply_blocks_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
    blocks: Vec<crate::models::timetable_block::TimetableBlock>,
) -> Result<Vec<crate::models::timetable_block::TimetableBlock>, AppError> {
    use crate::models::timetable_block::{TimetableBlockInstructor, TimetableBlockKind};
    let changes = approved_on_connection(connection, term, date).await?;
    if changes.is_empty() {
        return Ok(blocks);
    }
    let mut result = vec![];
    for mut block in blocks {
        let mut retained = vec![];
        for mut group in std::mem::take(&mut block.groups) {
            let relevant = changes
                .iter()
                .filter(|d| d.source_snapshot.allocation_id == group.id)
                .collect::<Vec<_>>();
            if relevant.is_empty() {
                retained.push(group);
                continue;
            }
            let mut projected = block.clone();
            let content = &relevant[0].teaching_content;
            for duty in &relevant {
                group
                    .instructors
                    .retain(|i| i.teacher_id != duty.original_teacher_id);
                if !group
                    .instructors
                    .iter()
                    .any(|i| i.teacher_id == duty.teacher_id)
                {
                    group.instructors.push(TimetableBlockInstructor {
                        teacher_id: duty.teacher_id,
                        display_name: duty.teacher_name.clone(),
                        role: "substitute".into(),
                        order_index: 0,
                    });
                }
            }
            if let Some(id) = content.learning_group_id {
                group.learning_group_id = id;
            }
            if let Some(id) = content.offering_id {
                group.learning_offering_id = id;
            }
            if let Some(name) = &content.learning_group_name {
                group.name = name.clone();
            }
            projected.learning_offering_id = content.offering_id;
            projected.offering_name = content.offering_name.clone().or(content.activity.clone());
            projected.title = content.activity.clone();
            projected.scheduling_mode = content.scheduling_mode;
            projected.block_kind = if content.activity_id.is_some() {
                TimetableBlockKind::Activity
            } else {
                TimetableBlockKind::Course
            };
            projected.groups = vec![group];
            result.push(projected);
        }
        block.groups = retained;
        for teacher in &mut block.teachers {
            if let Some(duty) = changes.iter().find(|d| {
                d.source_snapshot.allocation_id == teacher.id
                    && d.original_teacher_id == teacher.teacher_id
            }) {
                teacher.teacher_id = duty.teacher_id;
                teacher.display_name = duty.teacher_name.clone();
                block.title = duty.teaching_content.activity.clone();
            }
        }
        if !block.groups.is_empty() || block.block_kind == TimetableBlockKind::Structural {
            result.push(block)
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    #[test]
    fn actual_intervals_exclude_touching_boundaries() {
        let t = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap();
        assert!(intervals_overlap(t(9, 0), t(10, 0), t(9, 59), t(11, 0)));
        assert!(!intervals_overlap(t(9, 0), t(10, 0), t(10, 0), t(11, 0)));
    }
    #[test]
    fn identity_includes_absent_instructor_and_civil_date() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let d = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        assert_ne!(lesson_key(d, a, b), lesson_key(d, a, c));
        assert_ne!(lesson_key(d, a, b), lesson_key(d.succ_opt().unwrap(), a, b));
        assert_eq!(d.weekday().number_from_monday(), 2);
    }
}
