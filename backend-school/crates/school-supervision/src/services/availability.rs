//! One overlap policy for booking hints and authoritative booking decisions.
use std::collections::{BTreeSet, HashSet};

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use school_academic_timetable::services::dated_teaching::{self, AvailabilityWindow};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

use super::{
    booking::{ResolvedPeriod, SCHOOL_TIMEZONE},
    shared::{evaluator_conflict_status_codes, parse_observation_status},
};
use crate::models::{
    SupervisionBookingAvailability, SupervisionEvaluatorAvailability,
    SupervisionEvaluatorAvailabilityResponse, SupervisionEvaluatorConflict, SupervisionObservation,
    SupervisionObservationPeriod, SupervisionOverlappingBooking,
};

#[derive(Clone)]
pub(super) struct Window {
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
    pub homeroom: Option<Uuid>,
}
impl From<&ResolvedPeriod> for Window {
    fn from(p: &ResolvedPeriod) -> Self {
        Self {
            start: p.observed_at,
            end: Some(p.ends_at),
            homeroom: Some(p.homeroom_id),
        }
    }
}
impl From<&SupervisionObservationPeriod> for Window {
    fn from(p: &SupervisionObservationPeriod) -> Self {
        Self {
            start: p.observed_at,
            end: p.ends_at,
            homeroom: p.homeroom_id,
        }
    }
}

#[derive(sqlx::FromRow)]
pub(super) struct Fact {
    pub window_index: i64,
    pub observation_id: Uuid,
    cycle_id: Uuid,
    pub observed_user_id: Uuid,
    observed_display_name: Option<String>,
    status: String,
    pub participants: Vec<Uuid>,
    same_homeroom: bool,
    observed_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    subject_name: Option<String>,
    period_label: Option<String>,
}
impl Fact {
    fn confirmed(&self) -> bool {
        evaluator_conflict_status_codes().contains(&self.status.as_str())
    }
    fn blocks(&self, teacher: Uuid) -> bool {
        self.confirmed() && (self.same_homeroom || self.participants.contains(&teacher))
    }
}

/// Half-open intervals; a historical unknown end is an exact point, including
/// equality only for another point. No duration is guessed or persisted.
fn overlaps(
    a: DateTime<Utc>,
    ae: Option<DateTime<Utc>>,
    b: DateTime<Utc>,
    be: Option<DateTime<Utc>>,
) -> bool {
    match (ae, be) {
        (Some(ae), Some(be)) => a < be && b < ae,
        (None, Some(be)) => b <= a && a < be,
        (Some(ae), None) => a <= b && b < ae,
        (None, None) => a == b,
    }
}

pub(super) async fn facts(
    connection: &mut PgConnection,
    windows: &[Window],
    except: Option<Uuid>,
) -> Result<Vec<Fact>, AppError> {
    if windows.is_empty() {
        return Ok(vec![]);
    }
    let starts = windows.iter().map(|w| w.start).collect::<Vec<_>>();
    let ends = windows.iter().map(|w| w.end).collect::<Vec<_>>();
    let rooms = windows.iter().map(|w| w.homeroom).collect::<Vec<_>>();
    let mut statuses = evaluator_conflict_status_codes().to_vec();
    statuses.extend(["requested", "returned"]);
    let rows = DbSession::new(connection).query_as::<Fact>("supervision.availability.overlaps", r#"
        WITH chosen AS (
            SELECT * FROM unnest($1::timestamptz[],$2::timestamptz[],$3::uuid[])
            WITH ORDINALITY AS c(starts,ends,homeroom,position)
        ), matched AS (
            SELECT c.position,c.homeroom,p.homeroom_id AS period_homeroom,p.id AS period_id,
                p.position AS period_position,p.observation_id,p.observed_at,p.ends_at,
                COALESCE(NULLIF(p.manual_subject_name,''),NULLIF(p.lesson_snapshot->>'subjectName','')) AS subject_name,
                COALESCE(NULLIF(p.manual_period_label,''),NULLIF(p.lesson_snapshot->>'periodLabel','')) AS period_label
            FROM chosen c JOIN supervision_observation_periods p ON (
                (p.ends_at IS NULL AND c.ends IS NULL AND p.observed_at=c.starts)
                OR (p.ends_at IS NULL AND c.ends IS NOT NULL AND c.starts<=p.observed_at AND p.observed_at<c.ends)
                OR (p.ends_at IS NOT NULL AND c.ends IS NULL AND p.observed_at<=c.starts AND c.starts<p.ends_at)
                OR (p.ends_at IS NOT NULL AND c.ends IS NOT NULL AND p.observed_at<c.ends AND c.starts<p.ends_at))
            JOIN supervision_observations o ON o.id=p.observation_id
            WHERE ($4::uuid IS NULL OR o.id<>$4) AND o.status=ANY($5)
        ), people AS (
            SELECT e.observation_id,array_agg(DISTINCT e.evaluator_user_id ORDER BY e.evaluator_user_id) AS users
            FROM supervision_evaluators e JOIN (SELECT DISTINCT observation_id FROM matched) m ON m.observation_id=e.observation_id
            GROUP BY e.observation_id
        )
        SELECT DISTINCT ON(m.position,o.id) m.position-1 AS window_index,o.id AS observation_id,o.cycle_id,
            o.observed_user_id,NULLIF(u.display_name,'') AS observed_display_name,o.status,
            array_append(COALESCE(people.users,ARRAY[]::uuid[]),o.observed_user_id) AS participants,
            bool_or(m.homeroom=m.period_homeroom) OVER(PARTITION BY m.position,o.id) IS TRUE AS same_homeroom,
            m.observed_at,m.ends_at,m.subject_name,m.period_label
        FROM matched m JOIN supervision_observations o ON o.id=m.observation_id
        LEFT JOIN user_display_names u ON u.id=o.observed_user_id
        LEFT JOIN people ON people.observation_id=o.id
        ORDER BY m.position,o.id,m.observed_at,m.period_position,m.period_id LIMIT 100001
    "#).bind(starts).bind(ends).bind(rooms).bind(except).bind(statuses).fetch_all().await?;
    if rows.len() > 100000 {
        return Err(AppError::ValidationError("ขอบเขตตรวจการจองเกินขนาด".into()));
    }
    Ok(rows)
}

pub(super) fn aggregate<'a>(
    facts: impl IntoIterator<Item = &'a Fact>,
    teacher: Uuid,
    cycle: Uuid,
) -> SupervisionBookingAvailability {
    let mut pending = HashSet::new();
    let mut confirmed = HashSet::new();
    let mut blocked = false;
    for f in facts {
        if f.confirmed() {
            confirmed.insert(f.observation_id);
        } else if f.status == "requested" {
            pending.insert(f.observation_id);
        }
        blocked |= f.blocks(teacher)
            || (!f.confirmed() && f.cycle_id == cycle && f.observed_user_id == teacher);
    }
    SupervisionBookingAvailability {
        pending_count: pending.len() as i64,
        confirmed_count: confirmed.len() as i64,
        blocked,
    }
}

fn dates(windows: &[Window]) -> Result<BTreeSet<NaiveDate>, AppError> {
    let mut dates = BTreeSet::new();
    for w in windows {
        let mut date = w.start.with_timezone(&SCHOOL_TIMEZONE).date_naive();
        let last = w
            .end
            .map(|end| end - Duration::microseconds(1))
            .unwrap_or(w.start)
            .with_timezone(&SCHOOL_TIMEZONE)
            .date_naive();
        loop {
            dates.insert(date);
            if date >= last {
                break;
            }
            date = date
                .succ_opt()
                .ok_or_else(|| AppError::ValidationError("วันนิเทศอยู่นอกช่วงที่รองรับ".into()))?;
            if dates.len() > 10000 {
                return Err(AppError::ValidationError("ขอบเขตตรวจวันนิเทศเกินขนาด".into()));
            }
        }
    }
    Ok(dates)
}

async fn lock_resources(
    connection: &mut PgConnection,
    windows: &[Window],
    users: &[Uuid],
) -> Result<(), AppError> {
    let dates = dates(windows)?;
    let keys = users
        .iter()
        .flat_map(|u| dates.iter().map(move |d| (*u, *d)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    // Timetable's dated-duty writer shares these locks; all supervision paths
    // acquire the complete participant set before taking any homeroom lock.
    dated_teaching::lock_teacher_dates(connection, &keys).await?;
    super::periods::lock_evaluator_dates(connection, users, dates.iter().copied()).await?;
    let mut rooms = BTreeSet::new();
    for window in windows {
        if let Some(room) = window.homeroom {
            for date in self::dates(std::slice::from_ref(window))? {
                rooms.insert((room, date));
            }
        }
    }
    for (room, date) in rooms {
        DbSession::new(connection)
            .query(
                "supervision.availability.lock_homeroom",
                "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            )
            .bind(format!("supervision-homeroom:{room}:{date}"))
            .execute()
            .await?;
    }
    Ok(())
}

async fn teaching_busy(
    connection: &mut PgConnection,
    term: Uuid,
    windows: &[Window],
    users: &[Uuid],
    except: Option<Uuid>,
) -> Result<HashSet<Uuid>, AppError> {
    // Query occupied civil dates, then compare the actual interval or exact
    // historical point below. This query range is never a guessed lesson end.
    let day_end = NaiveTime::from_hms_micro_opt(23, 59, 59, 999999)
        .ok_or_else(|| AppError::ValidationError("เวลาสิ้นสุดวันไม่ถูกต้อง".into()))?;
    let windows_by_date = dates(windows)?
        .into_iter()
        .map(|date| AvailabilityWindow {
            date,
            start_time: NaiveTime::MIN,
            end_time: day_end,
        })
        .collect::<Vec<_>>();
    let rows = dated_teaching::occupancy_on_connection(
        connection,
        term,
        users,
        &windows_by_date,
        except.unwrap_or_else(Uuid::nil),
    )
    .await?;
    let mut busy = HashSet::new();
    for row in rows {
        let start = SCHOOL_TIMEZONE
            .from_local_datetime(&row.date.and_time(row.start_time))
            .single();
        let end = SCHOOL_TIMEZONE
            .from_local_datetime(&row.date.and_time(row.end_time))
            .single();
        if let Some((start, end)) = start.zip(end) {
            if windows.iter().any(|w| {
                overlaps(
                    w.start,
                    w.end,
                    start.with_timezone(&Utc),
                    Some(end.with_timezone(&Utc)),
                )
            }) {
                busy.insert(row.teacher_id);
            }
        } else {
            return Err(AppError::ValidationError("เวลาในตารางสอนไม่ถูกต้อง".into()));
        }
    }
    Ok(busy)
}

/// Pending requests reserve only their own teacher within a cycle. Confirmed
/// visits reserve the exact class and every accepted participant across cycles.
pub(super) async fn validate(
    connection: &mut PgConnection,
    teacher: Uuid,
    cycle: Uuid,
    term: Uuid,
    except: Option<Uuid>,
    windows: &[Window],
    evaluators: &[Uuid],
) -> Result<(), AppError> {
    let mut users = evaluators.to_vec();
    users.push(teacher);
    users.sort_unstable();
    users.dedup();
    lock_resources(connection, windows, &users).await?;
    let facts = facts(connection, windows, except).await?;
    let conflict = facts.iter().any(|f| {
        f.blocks(teacher)
            || (!f.confirmed() && f.cycle_id == cycle && f.observed_user_id == teacher)
            || (f.confirmed() && evaluators.iter().any(|user| f.participants.contains(user)))
    });
    if conflict
        || !teaching_busy(connection, term, windows, evaluators, except)
            .await?
            .is_empty()
    {
        return Err(AppError::Conflict(
            "ช่วงเวลาที่เลือกมีการจองหรือภาระงานที่ไม่สามารถจัดนิเทศได้".into(),
        ));
    }
    Ok(())
}

/// Reassigning evaluators keeps the already saved class/person/date facts;
/// legacy cycles without configured dates remain editable under existing policy.
pub(super) async fn validate_replacement(
    connection: &mut PgConnection,
    observation: &SupervisionObservation,
    evaluators: &[Uuid],
) -> Result<(), AppError> {
    let windows = observation
        .periods
        .iter()
        .map(Window::from)
        .collect::<Vec<_>>();
    let mut users = evaluators.to_vec();
    users.push(observation.observed_user_id);
    users.sort_unstable();
    users.dedup();
    lock_resources(connection, &windows, &users).await?;
    let facts = facts(connection, &windows, Some(observation.id)).await?;
    if facts
        .iter()
        .any(|f| f.confirmed() && evaluators.iter().any(|user| f.participants.contains(user)))
        || !teaching_busy(
            connection,
            observation.academic_term_id,
            &windows,
            evaluators,
            Some(observation.id),
        )
        .await?
        .is_empty()
    {
        return Err(AppError::Conflict(
            "ผู้ประเมินที่เลือกมีงานนิเทศหรือภาระสอนในช่วงเวลาเดียวกัน".into(),
        ));
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Staff {
    id: Uuid,
    title: Option<String>,
    display_name: String,
}

pub(super) async fn evaluator_response(
    connection: &mut PgConnection,
    actor: &ActorContext,
    observation: &SupervisionObservation,
    within_cycle_dates: bool,
) -> Result<SupervisionEvaluatorAvailabilityResponse, AppError> {
    let windows = observation
        .periods
        .iter()
        .map(Window::from)
        .collect::<Vec<_>>();
    let facts = facts(connection, &windows, Some(observation.id)).await?;
    let observed_users = facts
        .iter()
        .map(|f| f.observed_user_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let permitted = crate::access_policy::observation_management_user_ids_on_connection(
        connection,
        actor,
        &observed_users,
    )
    .await?;
    let staff = DbSession::new(connection).query_as::<Staff>("supervision.availability.staff", "SELECT id,title,display_name FROM user_display_names WHERE user_type='staff' AND status='active' AND id<>$1 ORDER BY first_name,last_name,id LIMIT 5001")
        .bind(observation.observed_user_id).fetch_all().await?;
    if staff.len() > 5000 {
        return Err(AppError::ValidationError(
            "จำนวนบุคลากรเกินขนาดที่ตรวจได้".into(),
        ));
    }
    let users = staff.iter().map(|s| s.id).collect::<Vec<_>>();
    let teaching = teaching_busy(
        connection,
        observation.academic_term_id,
        &windows,
        &users,
        Some(observation.id),
    )
    .await?;
    let overlap = aggregate(&facts, observation.observed_user_id, observation.cycle_id);
    let mut items = Vec::with_capacity(staff.len());
    for person in staff {
        let blocking = facts
            .iter()
            .filter(|f| f.confirmed() && f.participants.contains(&person.id))
            .collect::<Vec<_>>();
        let available = blocking.is_empty() && !teaching.contains(&person.id);
        let conflict = blocking
            .iter()
            .find(|f| permitted.contains(&f.observed_user_id))
            .map(|f| SupervisionEvaluatorConflict {
                observation_id: f.observation_id,
                observed_display_name: f.observed_display_name.clone(),
                observed_at: f.observed_at,
                lesson_title: match (&f.subject_name, &f.period_label) {
                    (Some(s), Some(p)) => Some(format!("{s} / {p}")),
                    (Some(s), None) => Some(s.clone()),
                    (None, Some(p)) => Some(p.clone()),
                    _ => None,
                },
            });
        let conflict_reason = if available {
            None
        } else if let Some(f) = &conflict {
            Some(format!(
                "มีงานนิเทศ {} ในช่วงเวลาเดียวกัน",
                f.observed_display_name
                    .as_deref()
                    .unwrap_or("ครูในขอบเขตที่ดูแล")
            ))
        } else {
            Some(
                if teaching.contains(&person.id) {
                    "มีภาระสอนในช่วงเวลาเดียวกัน"
                } else {
                    "มีงานนิเทศในช่วงเวลาเดียวกัน"
                }
                .into(),
            )
        };
        items.push(SupervisionEvaluatorAvailability {
            id: person.id,
            name: person.display_name,
            title: person.title,
            available,
            conflict_reason,
            conflict,
        });
    }
    items.sort_by_key(|i| !i.available);
    let mut seen = HashSet::new();
    let mut visible = facts
        .iter()
        .filter(|f| f.status != "returned" && permitted.contains(&f.observed_user_id))
        .collect::<Vec<_>>();
    visible.sort_by_key(|f| (f.observed_at, f.observation_id, f.window_index));
    let mut overlapping_bookings = vec![];
    for f in visible {
        if seen.insert(f.observation_id) {
            overlapping_bookings.push(SupervisionOverlappingBooking {
                observed_display_name: f.observed_display_name.clone(),
                observed_at: f.observed_at,
                ends_at: f.ends_at,
                status: parse_observation_status(&f.status)?,
            });
            if overlapping_bookings.len() == 12 {
                break;
            }
        }
    }
    Ok(SupervisionEvaluatorAvailabilityResponse {
        items,
        overlap,
        within_cycle_dates,
        overlapping_bookings,
    })
}
