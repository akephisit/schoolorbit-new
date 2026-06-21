use crate::error::AppError;
use chrono::{Datelike, Local, NaiveDate, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const TEACHING_ENTRY_TYPES: [&str; 5] = ["COURSE", "ACTIVITY", "HOMEROOM", "ACADEMIC", "BREAK"];

fn teaching_entry_types() -> Vec<String> {
    TEACHING_ENTRY_TYPES
        .iter()
        .map(|entry_type| (*entry_type).to_string())
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
pub struct DailyTeachingQuery {
    pub date: Option<NaiveDate>,
    pub academic_semester_id: Option<Uuid>,
    pub include_empty_teachers: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingOverview {
    pub date: NaiveDate,
    pub day_of_week: String,
    pub academic_semester_id: Uuid,
    pub periods: Vec<DailyTeachingPeriod>,
    pub teachers: Vec<DailyTeachingTeacher>,
    pub summary: DailyTeachingSummary,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingPeriod {
    pub id: Uuid,
    pub name: Option<String>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub order_index: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingTeacher {
    pub id: Uuid,
    pub display_name: String,
    pub organization_unit_names: Vec<String>,
    pub periods: Vec<DailyTeachingPeriodCell>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingPeriodCell {
    pub period_id: Uuid,
    pub entries: Vec<DailyTeachingEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingEntry {
    pub entry_id: Uuid,
    pub entry_type: String,
    pub subject_code: Option<String>,
    pub subject_name: Option<String>,
    pub subject_group_name: Option<String>,
    pub classroom_name: Option<String>,
    pub room_code: Option<String>,
    pub title: Option<String>,
    pub note: Option<String>,
    pub is_team_teaching: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingSummary {
    pub total_teacher_count: i64,
    pub displayed_teacher_count: i64,
    pub teachers_teaching_count: i64,
    pub lesson_count: i64,
    pub empty_teacher_count: i64,
}

#[derive(Debug, Clone, FromRow)]
struct DailyTeachingTeacherSeed {
    id: Uuid,
    display_name: String,
    organization_unit_names: Vec<String>,
    sort_order: i32,
}

#[derive(Debug, Clone, FromRow)]
struct DailyTeachingEntrySeed {
    teacher_id: Uuid,
    period_id: Uuid,
    entry_id: Uuid,
    entry_type: String,
    subject_code: Option<String>,
    subject_name: Option<String>,
    subject_group_name: Option<String>,
    classroom_name: Option<String>,
    room_code: Option<String>,
    title: Option<String>,
    note: Option<String>,
    instructor_count: i64,
    period_order_index: i32,
}

pub fn day_code_from_date(date: NaiveDate) -> &'static str {
    match date.weekday() {
        Weekday::Mon => "MON",
        Weekday::Tue => "TUE",
        Weekday::Wed => "WED",
        Weekday::Thu => "THU",
        Weekday::Fri => "FRI",
        Weekday::Sat => "SAT",
        Weekday::Sun => "SUN",
    }
}

fn entry_from_seed(seed: DailyTeachingEntrySeed) -> DailyTeachingEntry {
    DailyTeachingEntry {
        entry_id: seed.entry_id,
        entry_type: seed.entry_type,
        subject_code: seed.subject_code,
        subject_name: seed.subject_name,
        subject_group_name: seed.subject_group_name,
        classroom_name: seed.classroom_name,
        room_code: seed.room_code,
        title: seed.title,
        note: seed.note,
        is_team_teaching: seed.instructor_count > 1,
    }
}

fn build_daily_teaching_overview(
    date: NaiveDate,
    day_of_week: String,
    academic_semester_id: Uuid,
    mut periods: Vec<DailyTeachingPeriod>,
    mut teachers: Vec<DailyTeachingTeacherSeed>,
    mut entries: Vec<DailyTeachingEntrySeed>,
    include_empty_teachers: bool,
) -> DailyTeachingOverview {
    periods.sort_by(|a, b| {
        a.order_index
            .cmp(&b.order_index)
            .then_with(|| a.start_time.cmp(&b.start_time))
            .then_with(|| a.id.cmp(&b.id))
    });
    teachers.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.display_name.cmp(&b.display_name))
            .then_with(|| a.id.cmp(&b.id))
    });
    entries.sort_by(|a, b| {
        a.period_order_index
            .cmp(&b.period_order_index)
            .then_with(|| a.entry_id.cmp(&b.entry_id))
            .then_with(|| a.classroom_name.cmp(&b.classroom_name))
    });

    let mut entries_by_teacher_period: HashMap<(Uuid, Uuid), Vec<DailyTeachingEntry>> =
        HashMap::new();
    let mut teaching_teacher_ids = HashSet::new();
    for entry in entries {
        teaching_teacher_ids.insert(entry.teacher_id);
        entries_by_teacher_period
            .entry((entry.teacher_id, entry.period_id))
            .or_default()
            .push(entry_from_seed(entry));
    }

    let total_teacher_count = teachers.len() as i64;
    let teachers_teaching_count = teaching_teacher_ids.len() as i64;
    let lesson_count = entries_by_teacher_period
        .values()
        .map(|period_entries| period_entries.len() as i64)
        .sum();

    let teachers = teachers
        .into_iter()
        .filter(|teacher| include_empty_teachers || teaching_teacher_ids.contains(&teacher.id))
        .map(|teacher| {
            let teacher_periods = periods
                .iter()
                .map(|period| DailyTeachingPeriodCell {
                    period_id: period.id,
                    entries: entries_by_teacher_period
                        .remove(&(teacher.id, period.id))
                        .unwrap_or_default(),
                })
                .collect();

            DailyTeachingTeacher {
                id: teacher.id,
                display_name: teacher.display_name,
                organization_unit_names: teacher.organization_unit_names,
                periods: teacher_periods,
            }
        })
        .collect::<Vec<_>>();

    let summary = DailyTeachingSummary {
        total_teacher_count,
        displayed_teacher_count: teachers.len() as i64,
        teachers_teaching_count,
        lesson_count,
        empty_teacher_count: total_teacher_count.saturating_sub(teachers_teaching_count),
    };

    DailyTeachingOverview {
        date,
        day_of_week,
        academic_semester_id,
        periods,
        teachers,
        summary,
    }
}

async fn resolve_semester_id(
    pool: &PgPool,
    requested_semester_id: Option<Uuid>,
) -> Result<Uuid, AppError> {
    if let Some(semester_id) = requested_semester_id {
        return Ok(semester_id);
    }

    sqlx::query_scalar(
        "SELECT id
         FROM academic_semesters
         WHERE is_active = true
         ORDER BY start_date DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to resolve active academic semester: {}", error);
        AppError::InternalServerError("ไม่สามารถโหลดภาคเรียนปัจจุบันได้".to_string())
    })?
    .ok_or_else(|| AppError::BadRequest("ยังไม่มีภาคเรียนที่ใช้งานอยู่".to_string()))
}

async fn list_periods_for_semester(
    pool: &PgPool,
    semester_id: Uuid,
) -> Result<Vec<DailyTeachingPeriod>, AppError> {
    sqlx::query_as::<_, DailyTeachingPeriod>(
        "SELECT ap.id, ap.name, ap.start_time, ap.end_time, ap.order_index
         FROM academic_periods ap
         JOIN academic_semesters sem ON sem.academic_year_id = ap.academic_year_id
         WHERE sem.id = $1
           AND ap.is_active = true
         ORDER BY ap.order_index, ap.start_time, ap.id",
    )
    .bind(semester_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load daily teaching periods: {}", error);
        AppError::InternalServerError("ไม่สามารถโหลดคาบเรียนได้".to_string())
    })
}

async fn list_daily_teachers(
    pool: &PgPool,
    semester_id: Uuid,
    day_of_week: &str,
) -> Result<Vec<DailyTeachingTeacherSeed>, AppError> {
    let entry_types = teaching_entry_types();

    sqlx::query_as::<_, DailyTeachingTeacherSeed>(
        r#"
        SELECT
            u.id,
            concat_ws(' ', u.first_name, u.last_name) AS display_name,
            COALESCE(
                ARRAY_AGG(ou.name ORDER BY ou.display_order, ou.name)
                    FILTER (WHERE ou.id IS NOT NULL),
                ARRAY[]::text[]
            ) AS organization_unit_names,
            COALESCE(MIN(ou.display_order), 9999)::int AS sort_order
        FROM users u
        LEFT JOIN organization_members om
            ON om.user_id = u.id
           AND om.ended_at IS NULL
        LEFT JOIN organization_units ou
            ON ou.id = om.organization_unit_id
           AND ou.is_active = true
        WHERE u.user_type = 'staff'
          AND u.status = 'active'
          AND (
              EXISTS (
                  SELECT 1
                  FROM user_roles ur
                  JOIN roles role_def ON role_def.id = ur.role_id
                  WHERE ur.user_id = u.id
                    AND ur.ended_at IS NULL
                    AND role_def.is_active = true
                    AND role_def.code IN ('TEACHER', 'HEAD')
              )
              OR EXISTS (
                  SELECT 1
                  FROM timetable_entry_instructors tei_scope
                  JOIN academic_timetable_entries te_scope ON te_scope.id = tei_scope.entry_id
                  WHERE tei_scope.instructor_id = u.id
                    AND te_scope.is_active = true
                    AND te_scope.academic_semester_id = $1
                    AND te_scope.day_of_week = $2
                    AND te_scope.entry_type = ANY($3::text[])
              )
          )
        GROUP BY u.id, u.first_name, u.last_name
        ORDER BY sort_order, display_name, u.id
        "#,
    )
    .bind(semester_id)
    .bind(day_of_week)
    .bind(entry_types)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load daily teaching teachers: {}", error);
        AppError::InternalServerError("ไม่สามารถโหลดรายชื่อครูได้".to_string())
    })
}

async fn list_daily_entries(
    pool: &PgPool,
    semester_id: Uuid,
    day_of_week: &str,
) -> Result<Vec<DailyTeachingEntrySeed>, AppError> {
    let entry_types = teaching_entry_types();

    sqlx::query_as::<_, DailyTeachingEntrySeed>(
        r#"
        SELECT
            tei.instructor_id AS teacher_id,
            te.period_id,
            te.id AS entry_id,
            te.entry_type,
            s.code AS subject_code,
            s.name_th AS subject_name,
            sg.name AS subject_group_name,
            cr.name AS classroom_name,
            r.code AS room_code,
            te.title,
            te.note,
            COUNT(*) OVER (PARTITION BY te.id) AS instructor_count,
            ap.order_index AS period_order_index
        FROM academic_timetable_entries te
        JOIN timetable_entry_instructors tei ON tei.entry_id = te.id
        JOIN academic_periods ap ON ap.id = te.period_id
        LEFT JOIN classroom_courses cc ON cc.id = te.classroom_course_id
        LEFT JOIN subjects s ON s.id = cc.subject_id
        LEFT JOIN subject_groups sg ON sg.id = s.subject_group_id
        LEFT JOIN class_rooms cr ON cr.id = te.classroom_id
        LEFT JOIN rooms r ON r.id = te.room_id
        WHERE te.is_active = true
          AND te.academic_semester_id = $1
          AND te.day_of_week = $2
          AND te.entry_type = ANY($3::text[])
        ORDER BY ap.order_index, tei.created_at, te.id
        "#,
    )
    .bind(semester_id)
    .bind(day_of_week)
    .bind(entry_types)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load daily teaching entries: {}", error);
        AppError::InternalServerError("ไม่สามารถโหลดตารางสอนวันนี้ได้".to_string())
    })
}

pub async fn get_daily_teaching_overview(
    pool: &PgPool,
    query: DailyTeachingQuery,
    include_empty_teachers_allowed: bool,
) -> Result<DailyTeachingOverview, AppError> {
    let selected_date = query.date.unwrap_or_else(|| Local::now().date_naive());
    let day_of_week = day_code_from_date(selected_date).to_string();
    let semester_id = resolve_semester_id(pool, query.academic_semester_id).await?;
    let include_empty_teachers =
        include_empty_teachers_allowed && query.include_empty_teachers.unwrap_or(false);

    let periods = list_periods_for_semester(pool, semester_id).await?;
    let teachers = list_daily_teachers(pool, semester_id, &day_of_week).await?;
    let entries = list_daily_entries(pool, semester_id, &day_of_week).await?;

    Ok(build_daily_teaching_overview(
        selected_date,
        day_of_week,
        semester_id,
        periods,
        teachers,
        entries,
        include_empty_teachers,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn day_code_from_date_maps_chrono_weekdays_to_timetable_codes() {
        let monday = NaiveDate::from_ymd_opt(2026, 6, 22).unwrap();
        let sunday = NaiveDate::from_ymd_opt(2026, 6, 28).unwrap();

        assert_eq!(day_code_from_date(monday), "MON");
        assert_eq!(day_code_from_date(sunday), "SUN");
    }

    #[test]
    fn build_overview_groups_team_teaching_entry_under_each_assigned_teacher() {
        let period_id = id(1);
        let semester_id = id(2);
        let entry_id = id(3);
        let teacher_a = id(10);
        let teacher_b = id(11);

        let overview = build_daily_teaching_overview(
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
            "MON".to_string(),
            semester_id,
            vec![DailyTeachingPeriod {
                id: period_id,
                name: Some("คาบ 1".to_string()),
                start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
                end_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
                order_index: 1,
            }],
            vec![
                DailyTeachingTeacherSeed {
                    id: teacher_a,
                    display_name: "ครูก".to_string(),
                    organization_unit_names: vec!["คณิตศาสตร์".to_string()],
                    sort_order: 10,
                },
                DailyTeachingTeacherSeed {
                    id: teacher_b,
                    display_name: "ครูข".to_string(),
                    organization_unit_names: vec!["คณิตศาสตร์".to_string()],
                    sort_order: 10,
                },
            ],
            vec![
                DailyTeachingEntrySeed {
                    teacher_id: teacher_a,
                    period_id,
                    entry_id,
                    entry_type: "COURSE".to_string(),
                    subject_code: Some("ค21101".to_string()),
                    subject_name: Some("คณิตศาสตร์".to_string()),
                    subject_group_name: Some("คณิตศาสตร์".to_string()),
                    classroom_name: Some("ม.1/1".to_string()),
                    room_code: Some("321".to_string()),
                    title: None,
                    note: None,
                    instructor_count: 2,
                    period_order_index: 1,
                },
                DailyTeachingEntrySeed {
                    teacher_id: teacher_b,
                    period_id,
                    entry_id,
                    entry_type: "COURSE".to_string(),
                    subject_code: Some("ค21101".to_string()),
                    subject_name: Some("คณิตศาสตร์".to_string()),
                    subject_group_name: Some("คณิตศาสตร์".to_string()),
                    classroom_name: Some("ม.1/1".to_string()),
                    room_code: Some("321".to_string()),
                    title: None,
                    note: None,
                    instructor_count: 2,
                    period_order_index: 1,
                },
            ],
            false,
        );

        assert_eq!(overview.summary.total_teacher_count, 2);
        assert_eq!(overview.summary.teachers_teaching_count, 2);
        assert_eq!(overview.summary.lesson_count, 2);
        assert_eq!(overview.teachers.len(), 2);
        assert!(overview.teachers[0].periods[0].entries[0].is_team_teaching);
        assert!(overview.teachers[1].periods[0].entries[0].is_team_teaching);
    }

    #[test]
    fn build_overview_includes_empty_teachers_only_when_requested() {
        let period_id = id(1);
        let semester_id = id(2);
        let teacher_with_lesson = id(10);
        let empty_teacher = id(11);
        let period = DailyTeachingPeriod {
            id: period_id,
            name: Some("คาบ 1".to_string()),
            start_time: NaiveTime::from_hms_opt(8, 30, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(9, 20, 0).unwrap(),
            order_index: 1,
        };
        let teachers = vec![
            DailyTeachingTeacherSeed {
                id: teacher_with_lesson,
                display_name: "ครูมีคาบ".to_string(),
                organization_unit_names: vec![],
                sort_order: 0,
            },
            DailyTeachingTeacherSeed {
                id: empty_teacher,
                display_name: "ครูว่าง".to_string(),
                organization_unit_names: vec![],
                sort_order: 0,
            },
        ];
        let entries = vec![DailyTeachingEntrySeed {
            teacher_id: teacher_with_lesson,
            period_id,
            entry_id: id(3),
            entry_type: "HOMEROOM".to_string(),
            subject_code: None,
            subject_name: None,
            subject_group_name: None,
            classroom_name: Some("ม.1/1".to_string()),
            room_code: None,
            title: Some("โฮมรูม".to_string()),
            note: None,
            instructor_count: 1,
            period_order_index: 1,
        }];

        let without_empty = build_daily_teaching_overview(
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
            "MON".to_string(),
            semester_id,
            vec![period.clone()],
            teachers.clone(),
            entries.clone(),
            false,
        );
        let with_empty = build_daily_teaching_overview(
            NaiveDate::from_ymd_opt(2026, 6, 22).unwrap(),
            "MON".to_string(),
            semester_id,
            vec![period],
            teachers,
            entries,
            true,
        );

        assert_eq!(without_empty.teachers.len(), 1);
        assert_eq!(without_empty.summary.empty_teacher_count, 1);
        assert_eq!(with_empty.teachers.len(), 2);
        assert_eq!(with_empty.summary.displayed_teacher_count, 2);
    }
}
