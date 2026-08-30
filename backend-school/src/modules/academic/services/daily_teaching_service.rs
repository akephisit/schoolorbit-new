use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Local, NaiveDate, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::delivery::models::ActivitySchedulingMode;
use crate::modules::academic::services::timetable_version_service;

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct DailyTeachingQuery {
    pub academic_term_id: Uuid,
    pub date: Option<NaiveDate>,
    pub include_empty_teachers: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingOverview {
    pub date: NaiveDate,
    pub day_of_week: String,
    pub academic_term_id: Uuid,
    pub periods: Vec<DailyTeachingPeriod>,
    pub teachers: Vec<DailyTeachingTeacher>,
    pub summary: DailyTeachingSummary,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingPeriod {
    pub id: Uuid,
    pub name: Option<String>,
    #[schema(value_type = String)]
    pub start_time: NaiveTime,
    #[schema(value_type = String)]
    pub end_time: NaiveTime,
    pub order_index: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingTeacher {
    pub id: Uuid,
    pub display_name: String,
    pub periods: Vec<DailyTeachingPeriodCell>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingPeriodCell {
    pub bell_schedule_period_id: Uuid,
    pub entries: Vec<DailyTeachingEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingEntry {
    pub entry_id: Uuid,
    pub entry_type: String,
    pub learning_group_id: Option<Uuid>,
    pub offering_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub subject_version_display_label: Option<String>,
    pub activity_id: Option<Uuid>,
    pub activity_version_display_label: Option<String>,
    pub activity_scheduling_mode: Option<ActivitySchedulingMode>,
    pub offering_code: Option<String>,
    pub offering_name: Option<String>,
    pub learning_group_name: Option<String>,
    pub homeroom_names: Vec<String>,
    pub room_code: Option<String>,
    pub title: Option<String>,
    pub note: Option<String>,
    pub is_team_teaching: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyTeachingSummary {
    pub total_teacher_count: i64,
    pub displayed_teacher_count: i64,
    pub teachers_teaching_count: i64,
    pub lesson_count: i64,
    pub empty_teacher_count: i64,
}

#[derive(Debug, Clone, FromRow)]
struct TeacherSeed {
    id: Uuid,
    display_name: String,
}

#[derive(Debug, Clone, FromRow)]
struct EntrySeed {
    teacher_id: Uuid,
    bell_schedule_period_id: Uuid,
    period_order_index: i32,
    entry_id: Uuid,
    entry_type: String,
    learning_group_id: Option<Uuid>,
    offering_id: Option<Uuid>,
    subject_id: Option<Uuid>,
    subject_version_display_label: Option<String>,
    activity_id: Option<Uuid>,
    activity_version_display_label: Option<String>,
    activity_scheduling_mode: Option<ActivitySchedulingMode>,
    offering_code: Option<String>,
    offering_name: Option<String>,
    learning_group_name: Option<String>,
    homeroom_names: Vec<String>,
    room_code: Option<String>,
    title: Option<String>,
    note: Option<String>,
    instructor_count: i64,
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

pub async fn get_daily_teaching_overview(
    pool: &PgPool,
    query: DailyTeachingQuery,
) -> Result<DailyTeachingOverview, AppError> {
    let date = query.date.unwrap_or_else(|| Local::now().date_naive());
    let day = day_code_from_date(date).to_string();
    let version =
        timetable_version_service::resolve_for_date(pool, query.academic_term_id, date).await?;
    let periods: Vec<DailyTeachingPeriod> = sqlx::query_as(
        r#"SELECT id, name, start_time, end_time, order_index
           FROM bell_schedule_periods
           WHERE bell_schedule_id = $1 AND is_active
           ORDER BY order_index, start_time, id"#,
    )
    .bind(version.bell_schedule_id)
    .fetch_all(pool)
    .await?;
    let teachers: Vec<TeacherSeed> = sqlx::query_as(
        r#"SELECT DISTINCT user_account.id,
                  concat_ws(' ',
                      nullif(concat(coalesce(user_account.title, ''), user_account.first_name), ''),
                      nullif(user_account.last_name, '')
                  ) AS display_name
           FROM users user_account
           WHERE user_account.status = 'active'
             AND (
                 EXISTS (
                     SELECT 1
                     FROM learning_group_teachers teacher
                     WHERE teacher.teacher_id = user_account.id
                       AND teacher.academic_term_id = $1
                 )
                 OR EXISTS (
                     SELECT 1
                     FROM timetable_entry_instructors instructor
                     JOIN academic_timetable_entries entry ON entry.id = instructor.entry_id
                     WHERE instructor.instructor_id = user_account.id
                       AND entry.academic_term_id = $1
                       AND entry.timetable_version_id = $2
                       AND entry.is_active
                 )
             )
           ORDER BY display_name, user_account.id"#,
    )
    .bind(query.academic_term_id)
    .bind(version.id)
    .fetch_all(pool)
    .await?;
    let entries: Vec<EntrySeed> = sqlx::query_as(
        r#"WITH effective_teacher AS (
               SELECT entry.id AS entry_id, teacher.teacher_id
               FROM academic_timetable_entries entry
               JOIN learning_group_teachers teacher
                 ON teacher.learning_group_id = entry.learning_group_id
               WHERE entry.academic_term_id = $1
                 AND entry.timetable_version_id = $3
                 AND entry.day_of_week = $2
                 AND entry.is_active
               UNION ALL
               SELECT entry.id, instructor.instructor_id
               FROM academic_timetable_entries entry
               JOIN timetable_entry_instructors instructor ON instructor.entry_id = entry.id
               WHERE entry.academic_term_id = $1
                 AND entry.timetable_version_id = $3
                 AND entry.day_of_week = $2
                 AND entry.learning_group_id IS NULL
                 AND entry.is_active
           )
           SELECT effective_teacher.teacher_id,
                  entry.bell_schedule_period_id,
                  period.order_index AS period_order_index,
                  entry.id AS entry_id,
                  lower(entry.entry_type::text) AS entry_type,
                  entry.learning_group_id,
                  entry.learning_offering_id AS offering_id,
                  course_detail.subject_id,
                  CASE WHEN subject_version.id IS NULL THEN NULL ELSE concat(
                      coalesce(subject_version.name_th, subject_version.name_en, offering.name_snapshot),
                      ' · v', subject_version.version_no
                  ) END AS subject_version_display_label,
                  activity_detail.activity_id,
                  CASE WHEN activity_version.id IS NULL THEN NULL ELSE concat(
                      activity_version.name, ' · v', activity_version.version_no
                  ) END AS activity_version_display_label,
                  activity_detail.scheduling_mode AS activity_scheduling_mode,
                  offering.code_snapshot AS offering_code,
                  offering.name_snapshot AS offering_name,
                  learning_group.name AS learning_group_name,
                  CASE
                      WHEN entry.learning_group_id IS NOT NULL THEN ARRAY(
                          SELECT homeroom.name
                          FROM learning_group_homerooms coverage
                          JOIN homerooms homeroom ON homeroom.id = coverage.homeroom_id
                          WHERE coverage.learning_group_id = entry.learning_group_id
                          ORDER BY homeroom.name, homeroom.id
                      )
                      WHEN entry.homeroom_id IS NOT NULL THEN ARRAY(
                          SELECT homeroom.name FROM homerooms homeroom WHERE homeroom.id = entry.homeroom_id
                      )
                      ELSE ARRAY[]::text[]
                  END AS homeroom_names,
                  room.code AS room_code,
                  entry.title,
                  entry.note,
                  (SELECT count(*) FROM effective_teacher team WHERE team.entry_id = entry.id)::bigint
                      AS instructor_count
           FROM effective_teacher
           JOIN academic_timetable_entries entry ON entry.id = effective_teacher.entry_id
           JOIN bell_schedule_periods period ON period.id = entry.bell_schedule_period_id
           LEFT JOIN learning_groups learning_group ON learning_group.id = entry.learning_group_id
           LEFT JOIN learning_offerings offering ON offering.id = entry.learning_offering_id
           LEFT JOIN course_offering_details course_detail
             ON course_detail.learning_offering_id = offering.id
           LEFT JOIN subject_versions subject_version
             ON subject_version.id = course_detail.subject_version_id
           LEFT JOIN activity_offering_details activity_detail
             ON activity_detail.learning_offering_id = offering.id
           LEFT JOIN activity_versions activity_version
             ON activity_version.id = activity_detail.activity_version_id
           LEFT JOIN rooms room ON room.id = entry.room_id
           ORDER BY effective_teacher.teacher_id, period.order_index, entry.id"#,
    )
    .bind(query.academic_term_id)
    .bind(&day)
    .bind(version.id)
    .fetch_all(pool)
    .await?;

    Ok(build_overview(
        date,
        day,
        query.academic_term_id,
        periods,
        teachers,
        entries,
        query.include_empty_teachers.unwrap_or(false),
    ))
}

fn build_overview(
    date: NaiveDate,
    day_of_week: String,
    academic_term_id: Uuid,
    periods: Vec<DailyTeachingPeriod>,
    teachers: Vec<TeacherSeed>,
    mut entries: Vec<EntrySeed>,
    include_empty_teachers: bool,
) -> DailyTeachingOverview {
    entries.sort_by_key(|entry| (entry.period_order_index, entry.entry_id));
    let mut grouped: HashMap<(Uuid, Uuid), Vec<DailyTeachingEntry>> = HashMap::new();
    let mut teaching_teacher_ids = HashSet::new();
    for entry in entries {
        teaching_teacher_ids.insert(entry.teacher_id);
        grouped
            .entry((entry.teacher_id, entry.bell_schedule_period_id))
            .or_default()
            .push(DailyTeachingEntry {
                entry_id: entry.entry_id,
                entry_type: entry.entry_type,
                learning_group_id: entry.learning_group_id,
                offering_id: entry.offering_id,
                subject_id: entry.subject_id,
                subject_version_display_label: entry.subject_version_display_label,
                activity_id: entry.activity_id,
                activity_version_display_label: entry.activity_version_display_label,
                activity_scheduling_mode: entry.activity_scheduling_mode,
                offering_code: entry.offering_code,
                offering_name: entry.offering_name,
                learning_group_name: entry.learning_group_name,
                homeroom_names: entry.homeroom_names,
                room_code: entry.room_code,
                title: entry.title,
                note: entry.note,
                is_team_teaching: entry.instructor_count > 1,
            });
    }
    let total_teacher_count = teachers.len() as i64;
    let teachers_teaching_count = teaching_teacher_ids.len() as i64;
    let lesson_count = grouped.values().map(|values| values.len() as i64).sum();
    let teachers: Vec<DailyTeachingTeacher> = teachers
        .into_iter()
        .filter(|teacher| include_empty_teachers || teaching_teacher_ids.contains(&teacher.id))
        .map(|teacher| DailyTeachingTeacher {
            id: teacher.id,
            display_name: teacher.display_name,
            periods: periods
                .iter()
                .map(|period| DailyTeachingPeriodCell {
                    bell_schedule_period_id: period.id,
                    entries: grouped.remove(&(teacher.id, period.id)).unwrap_or_default(),
                })
                .collect(),
        })
        .collect();
    DailyTeachingOverview {
        date,
        day_of_week,
        academic_term_id,
        periods,
        summary: DailyTeachingSummary {
            total_teacher_count,
            displayed_teacher_count: teachers.len() as i64,
            teachers_teaching_count,
            lesson_count,
            empty_teacher_count: total_teacher_count.saturating_sub(teachers_teaching_count),
        },
        teachers,
    }
}

#[cfg(test)]
mod tests {
    use super::day_code_from_date;
    use chrono::NaiveDate;

    #[test]
    fn maps_calendar_date_to_timetable_day() {
        assert_eq!(
            day_code_from_date(NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()),
            "MON"
        );
    }
}
