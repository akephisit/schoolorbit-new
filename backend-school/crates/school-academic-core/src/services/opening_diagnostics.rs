use std::collections::{BTreeSet, HashSet};

use uuid::Uuid;

use crate::models::{ActivationIssueCode, ActivationState, BellSchedulePeriod, VersionStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticResource {
    BellSchedule,
    Homeroom(Uuid),
}

#[derive(Debug)]
pub struct OpeningDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub resource: DiagnosticResource,
}

/// Explain the exact facts already accepted by the Core activation projection.
/// No independent reads or room counts can diverge from that readiness evidence.
pub fn explain(state: &ActivationState) -> Vec<OpeningDiagnostic> {
    let mut diagnostics = Vec::new();
    if state
        .issues
        .iter()
        .any(|issue| issue.code == ActivationIssueCode::BellSchedule)
    {
        diagnostics.extend(bell_diagnostics(
            state.bell_schedule.as_ref().map(|schedule| {
                (
                    schedule.academic_year_id,
                    schedule.name.as_str(),
                    schedule.status,
                )
            }),
            state.context.academic_year_id,
            &state.bell_periods,
            &state.school_days,
        ));
    }
    for room in &state.over_capacity_rooms {
        let name = room.homeroom_name.as_deref().unwrap_or("ไม่พบชื่อห้อง");
        diagnostics.push(OpeningDiagnostic {
            code: "opening.room_capacity",
            message: format!(
                "ห้อง {name} จัดนักเรียนไว้ {} คน แต่ความจุ {} คน กรุณาปรับความจุหรือย้ายสมาชิกห้อง",
                room.student_count, room.capacity
            ),
            resource: DiagnosticResource::Homeroom(room.homeroom_id),
        });
    }
    diagnostics
}

fn bell_diagnostics(
    schedule: Option<(Uuid, &str, VersionStatus)>,
    year: Uuid,
    periods: &[BellSchedulePeriod],
    school_days: &str,
) -> Vec<OpeningDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut push = |code, message| {
        diagnostics.push(OpeningDiagnostic {
            code,
            message,
            resource: DiagnosticResource::BellSchedule,
        })
    };
    let Some((schedule_year, name, status)) = schedule else {
        push(
            "opening.bell_schedule_missing",
            "ไม่พบตารางคาบที่ภาคเรียนเลือก กรุณาเลือกตารางคาบของปีนี้".into(),
        );
        return diagnostics;
    };
    if schedule_year != year {
        push(
            "opening.bell_schedule_wrong_year",
            format!("ตารางคาบ {name} อยู่คนละปีการศึกษากับภาคเรียน กรุณาเลือกตารางคาบของปีนี้"),
        );
    }
    if status == VersionStatus::Archived {
        push(
            "opening.bell_schedule_archived",
            format!("ตารางคาบ {name} เก็บถาวรแล้ว กรุณาเลือกตารางคาบที่ใช้งานได้"),
        );
    }
    let active: Vec<_> = periods.iter().filter(|period| period.is_active).collect();
    if active.is_empty() {
        push(
            "opening.bell_schedule_no_active_periods",
            format!("ตารางคาบ {name} ไม่มีคาบที่เปิดใช้งาน กรุณาเพิ่มหรือเปิดใช้งานคาบเรียน"),
        );
    }
    let supported = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let allowed: BTreeSet<_> = school_days.split(',').collect();
    let mut orders = HashSet::new();
    for period in &active {
        let label = period_label(period);
        if period.order_index <= 0 || !orders.insert(period.order_index) {
            push(
                "opening.bell_period_order_invalid",
                format!("ตารางคาบ {name}: {label} มีลำดับคาบไม่เป็นจำนวนบวกหรือซ้ำกับคาบอื่น"),
            );
        }
        if period.start_time >= period.end_time {
            push(
                "opening.bell_period_time_invalid",
                format!("ตารางคาบ {name}: {label} เวลาเริ่มต้องอยู่ก่อนเวลาสิ้นสุด"),
            );
        }
        let raw = period.applicable_days.as_deref().unwrap_or(school_days);
        if raw.is_empty() {
            push(
                "opening.bell_period_days_missing",
                format!("ตารางคาบ {name}: {label} ยังไม่ได้กำหนดวันที่ใช้คาบเรียน"),
            );
            continue;
        }
        let days: Vec<_> = raw.split(',').collect();
        let unique: BTreeSet<_> = days.iter().copied().collect();
        if days.iter().any(|day| !supported.contains(day)) || unique.len() != days.len() {
            push(
                "opening.bell_period_days_invalid",
                format!(
                    "ตารางคาบ {name}: {label} มีรูปแบบวันเรียนไม่ถูกต้องหรือซ้ำ กรุณาเลือกวันเรียนแล้วบันทึกใหม่"
                ),
            );
        } else if days.iter().any(|day| !allowed.contains(day)) {
            push(
                "opening.bell_period_days_outside_term",
                format!("ตารางคาบ {name}: {label} ใช้วันที่ไม่อยู่ในวันเรียนของภาคเรียน กรุณาแก้วันที่ใช้คาบ"),
            );
        }
    }
    for (index, period) in active.iter().enumerate() {
        let days: BTreeSet<_> = period
            .applicable_days
            .as_deref()
            .unwrap_or(school_days)
            .split(',')
            .collect();
        for other in active.iter().skip(index + 1) {
            let shares_day = other
                .applicable_days
                .as_deref()
                .unwrap_or(school_days)
                .split(',')
                .any(|day| !day.is_empty() && days.contains(day));
            if shares_day
                && period.start_time < other.end_time
                && other.start_time < period.end_time
            {
                push(
                    "opening.bell_period_overlap",
                    format!(
                        "ตารางคาบ {name}: {} ซ้อนเวลากับ {} ในวันเดียวกัน กรุณาแก้เวลาให้ไม่ซ้อนกัน",
                        period_label(period),
                        period_label(other)
                    ),
                );
            }
        }
    }
    diagnostics
}

fn period_label(period: &BellSchedulePeriod) -> String {
    match period
        .name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
    {
        Some(name) => format!(
            "คาบ {} ({name}, {}–{})",
            period.order_index,
            period.start_time.format("%H:%M"),
            period.end_time.format("%H:%M")
        ),
        None => format!(
            "คาบ {} ({}–{})",
            period.order_index,
            period.start_time.format("%H:%M"),
            period.end_time.format("%H:%M")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn period(days: Option<&str>) -> BellSchedulePeriod {
        BellSchedulePeriod {
            id: Uuid::new_v4(),
            bell_schedule_id: Uuid::new_v4(),
            name: Some("คณิตศาสตร์".into()),
            start_time: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            order_index: 1,
            applicable_days: days.map(str::to_owned),
            is_active: true,
        }
    }

    #[test]
    fn bell_diagnostics_distinguishes_real_causes_and_names_the_period() {
        let year = Uuid::new_v4();
        for (days, code) in [
            (Some(""), "opening.bell_period_days_missing"),
            (Some("{MON,TUE}"), "opening.bell_period_days_invalid"),
            (Some(" MON "), "opening.bell_period_days_invalid"),
            (Some("TUE"), "opening.bell_period_days_outside_term"),
        ] {
            let facts = period(days);
            let diagnostics = bell_diagnostics(
                Some((year, "ตารางหลัก", VersionStatus::Published)),
                year,
                std::slice::from_ref(&facts),
                "MON",
            );
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, code);
            assert!(diagnostics[0].message.contains("คณิตศาสตร์"));
            assert_eq!(facts.applicable_days.as_deref(), days);
        }
        let first = period(Some("MON"));
        assert!(bell_diagnostics(
            Some((year, "ตารางหลัก", VersionStatus::Published)),
            year,
            std::slice::from_ref(&first),
            "MON"
        )
        .is_empty());
        let mut other = first.clone();
        other.order_index = 2;
        let diagnostics = bell_diagnostics(
            Some((year, "ตารางหลัก", VersionStatus::Published)),
            year,
            &[first, other],
            "MON",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "opening.bell_period_overlap");
    }
}
