//! Compose current timetable/Delivery providers; Supervision never parses room labels.
use std::collections::{HashMap, HashSet};

use crate::models::{
    LessonSnapshot, SupervisionBookingOptions, SupervisionBookingOptionsQuery,
    SupervisionTimetableOption,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc, Weekday};
use school_academic_timetable::services::dated_teaching;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub(super) const SCHOOL_TIMEZONE: chrono_tz::Tz = chrono_tz::Asia::Bangkok;

#[derive(Debug, Clone)]
pub(super) struct ResolvedPeriod {
    pub allocation_id: Uuid,
    pub offering_id: Option<Uuid>,
    pub learning_group_id: Uuid,
    pub homeroom_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub snapshot: LessonSnapshot,
}

fn invalid(message: &str) -> AppError {
    AppError::ValidationError(message.into())
}

pub(super) fn monday(date: NaiveDate) -> Result<NaiveDate, AppError> {
    date.checked_sub_signed(Duration::days(i64::from(
        date.weekday().num_days_from_monday(),
    )))
    .ok_or_else(|| invalid("วันที่นิเทศอยู่นอกช่วงที่รองรับ"))
}

pub async fn booking_options(
    pool: &PgPool,
    teacher: Uuid,
    query: SupervisionBookingOptionsQuery,
) -> Result<SupervisionBookingOptions, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "supervision.booking.options")
            .await?;
    let cycle =
        super::observations::load_cycle_for_request(&mut connection, query.cycle_id).await?;
    if cycle.academic_year_id != query.academic_year_id {
        return Err(invalid("ปีการศึกษาไม่ตรงกับรอบนิเทศ"));
    }
    super::observations::validate_observation_context(
        &mut connection,
        &cycle,
        query.academic_term_id,
    )
    .await?;
    super::observations::ensure_cycle_target_allows_teacher(&mut connection, cycle.id, teacher)
        .await?;
    options_on_connection(
        &mut connection,
        teacher,
        &cycle,
        query.academic_term_id,
        query.week_start_date,
    )
    .await
}

pub(super) async fn options_on_connection(
    connection: &mut PgConnection,
    teacher: Uuid,
    cycle: &super::observations::CycleForRequestRow,
    term: Uuid,
    week: NaiveDate,
) -> Result<SupervisionBookingOptions, AppError> {
    weekly_evidence(connection, teacher, cycle, term, week)
        .await
        .map(|(options, _)| options)
}

async fn weekly_evidence(
    connection: &mut PgConnection,
    teacher: Uuid,
    cycle: &super::observations::CycleForRequestRow,
    term: Uuid,
    week: NaiveDate,
) -> Result<(SupervisionBookingOptions, HashMap<Uuid, Option<Uuid>>), AppError> {
    let year = cycle.academic_year_id;
    if week.weekday() != Weekday::Mon {
        return Err(invalid("วันเริ่มสัปดาห์ต้องเป็นวันจันทร์"));
    }
    let end = week
        .checked_add_signed(Duration::days(6))
        .ok_or_else(|| invalid("ช่วงสัปดาห์ไม่ถูกต้อง"))?;
    let last = NaiveTime::from_hms_micro_opt(23, 59, 59, 999999)
        .ok_or_else(|| invalid("เวลาสิ้นสุดไม่ถูกต้อง"))?;
    let snapshot = dated_teaching::snapshot_on_connection(
        connection,
        year,
        term,
        teacher,
        week,
        end,
        NaiveTime::MIN,
        last,
    )
    .await?;
    let assigned = school_academic_delivery::services::current::assigned_group_ids_on_connection(
        connection, year, term, teacher,
    )
    .await?;
    let mut items = Vec::new();
    let mut offerings = HashMap::new();
    for lesson in snapshot.lessons {
        let Some(group) = lesson.learning_group_id else {
            continue;
        };
        if !assigned.learning_group_ids.contains(&group) || lesson.homeroom_ids.len() != 1 {
            continue;
        }
        let Some(homeroom) = lesson.homeroom_ids.first().copied() else {
            continue;
        };
        let Some(name) = lesson.homeroom_names.first().cloned() else {
            continue;
        };
        offerings.insert(lesson.allocation_id, lesson.offering_id);
        items.push(SupervisionTimetableOption {
            observed_date: lesson.date,
            block_group_id: lesson.allocation_id,
            block_id: lesson.block_id,
            bell_period_id: lesson.bell_schedule_period_id,
            period_name: lesson.period_name,
            start_time: lesson.start_time,
            end_time: lesson.end_time,
            learning_group_id: group,
            learning_group_name: lesson.learning_group_name.unwrap_or_default(),
            subject_name: lesson.offering_name,
            homeroom_id: homeroom,
            homeroom_name: name,
            room_name: lesson.room_name,
        });
    }
    items.retain(|item| cycle.contains_date(item.observed_date));
    items.sort_by_key(|p| (p.observed_date, p.start_time, p.end_time, p.block_group_id));
    // Include Delivery eligibility, not merely the underlying timetable revision.
    let digest:String=DbSession::new(connection).scalar("supervision.booking.source_digest",
        "SELECT encode(sha256(convert_to(jsonb_build_array($1::text,$2::uuid[],$3::uuid,$4::uuid,$5::uuid,$6::date,$7::uuid,$8::date,$9::date)::text,'UTF8')),'hex')")
        .bind(snapshot.source_digest).bind(&assigned.learning_group_ids).bind(year).bind(term).bind(teacher).bind(week).bind(cycle.id).bind(cycle.observation_start_date).bind(cycle.observation_end_date)
        .fetch_one().await?;
    Ok((
        SupervisionBookingOptions {
            source_digest: digest,
            week_start_date: week,
            week_end_date: end,
            items,
        },
        offerings,
    ))
}

pub(super) async fn resolve(
    connection: &mut PgConnection,
    teacher: Uuid,
    cycle: &super::observations::CycleForRequestRow,
    term: Uuid,
    date: NaiveDate,
    ids: &[Uuid],
    expected: &str,
) -> Result<Vec<ResolvedPeriod>, AppError> {
    cycle.require_date(date)?;
    let (options, offerings) =
        weekly_evidence(connection, teacher, cycle, term, monday(date)?).await?;
    if expected != options.source_digest {
        return Err(AppError::Conflict(
            "ตารางสอนเปลี่ยนแล้ว กรุณาโหลดคาบสอนใหม่".into(),
        ));
    }
    let selected = validate_selection(&options.items, date, ids)?;
    selected
        .into_iter()
        .map(|p| {
            let observed_at = SCHOOL_TIMEZONE
                .from_local_datetime(&date.and_time(p.start_time))
                .single()
                .ok_or_else(|| invalid("วันเวลานิเทศไม่ถูกต้อง"))?
                .with_timezone(&Utc);
            let ends_at = SCHOOL_TIMEZONE
                .from_local_datetime(&date.and_time(p.end_time))
                .single()
                .ok_or_else(|| invalid("วันเวลานิเทศไม่ถูกต้อง"))?
                .with_timezone(&Utc);
            Ok(ResolvedPeriod {
                allocation_id: p.block_group_id,
                offering_id: offerings.get(&p.block_group_id).copied().flatten(),
                learning_group_id: p.learning_group_id,
                homeroom_id: p.homeroom_id,
                observed_at,
                ends_at,
                snapshot: LessonSnapshot {
                    source: Some("timetable".into()),
                    timetable_block_group_id: Some(p.block_group_id),
                    subject_name: p.subject_name.clone(),
                    classroom_label: Some(p.homeroom_name.clone()),
                    room_label: p.room_name.clone(),
                    observed_at: Some(observed_at),
                    period_label: Some(p.period_name.clone()),
                },
            })
        })
        .collect()
}

fn validate_selection<'a>(
    items: &'a [SupervisionTimetableOption],
    date: NaiveDate,
    ids: &[Uuid],
) -> Result<Vec<&'a SupervisionTimetableOption>, AppError> {
    if ids.is_empty() {
        return Err(invalid("ต้องเลือกคาบนิเทศอย่างน้อย 1 คาบ"));
    }
    if ids.iter().copied().collect::<HashSet<_>>().len() != ids.len() {
        return Err(invalid("เลือกคาบนิเทศซ้ำไม่ได้"));
    }
    let mut selected = ids
        .iter()
        .map(|id| {
            items
                .iter()
                .find(|p| p.block_group_id == *id && p.observed_date == date)
                .ok_or_else(|| invalid("ต้องเลือกคาบสอนปัจจุบันของตนเองในวันที่และภาคเรียนที่เลือก"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    selected.sort_by_key(|p| (p.start_time, p.end_time, p.block_group_id));
    let homeroom = selected[0].homeroom_id;
    if selected
        .iter()
        .any(|p| p.homeroom_id != homeroom || p.start_time >= p.end_time)
        || selected
            .windows(2)
            .any(|p| p[0].end_time != p[1].start_time)
    {
        return Err(invalid(
            "คาบนิเทศต้องต่อเนื่องในวันเดียวกันและเป็นห้องเรียนเดียวกันทุกคาบ",
        ));
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn option(id: u128, start: u32, end: u32, room: u128) -> SupervisionTimetableOption {
        SupervisionTimetableOption {
            observed_date: NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
            block_group_id: Uuid::from_u128(id),
            block_id: Uuid::from_u128(id + 100),
            bell_period_id: Uuid::from_u128(id + 200),
            period_name: format!("{id}"),
            start_time: NaiveTime::from_hms_opt(start, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(end, 0, 0).unwrap(),
            learning_group_id: Uuid::from_u128(id + 300),
            learning_group_name: format!("group {id}"),
            subject_name: Some(format!("subject {id}")),
            homeroom_id: Uuid::from_u128(room),
            homeroom_name: "ม.3/2".into(),
            room_name: Some(format!("Physical room {id}")),
        }
    }
    #[test]
    fn one_two_three_different_subjects_are_one_ordered_observation() {
        let items = vec![
            option(1, 8, 9, 50),
            option(2, 9, 10, 50),
            option(3, 10, 11, 50),
        ];
        let date = items[0].observed_date;
        for n in 1..=3 {
            let ids = items[..n]
                .iter()
                .rev()
                .map(|p| p.block_group_id)
                .collect::<Vec<_>>();
            let selected = validate_selection(&items, date, &ids).unwrap();
            assert_eq!(selected.len(), n);
            assert_eq!(selected[0].start_time, items[0].start_time);
        }
    }
    #[test]
    fn rejects_duplicate_gap_lunch_overlap_wrong_date_and_exact_room() {
        let first = option(1, 8, 9, 50);
        let date = first.observed_date;
        let ids = vec![first.block_group_id, Uuid::from_u128(2)];
        for second in [
            option(2, 10, 11, 50),
            option(2, 8, 10, 50),
            option(2, 12, 13, 50),
            option(2, 9, 10, 51),
        ] {
            assert!(validate_selection(&[first.clone(), second], date, &ids).is_err());
        }
        assert!(validate_selection(
            &[first.clone()],
            date,
            &[first.block_group_id, first.block_group_id]
        )
        .is_err());
        assert!(validate_selection(&[first], date.succ_opt().unwrap(), &ids[..1]).is_err());
    }
}
