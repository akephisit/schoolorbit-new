use super::{
    catalog::settings_on_connection,
    shared::{bump, current_cycle, lock},
};
use crate::{
    models::*,
    rules::{action_side, invalid, validate_score, validate_status},
};
use chrono::NaiveDate;
use school_errors::AppError;
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, FromRow, PgConnection};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DayPolicy {
    count_absent_lessons: bool,
    count_special: bool,
    daily_merit_cap: Option<i32>,
    daily_deduction_cap: Option<i32>,
}
#[derive(Debug, Clone, FromRow)]
struct Existing {
    id: Uuid,
    student_id: Uuid,
    source_key: String,
    source_status: String,
    side: BehaviorSide,
    source: BehaviorSource,
    points: i32,
    raw_points: i32,
    rule_version_id: Option<Uuid>,
    cycle_id: Uuid,
    active: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Effect {
    source_key: String,
    source_session_id: Option<Uuid>,
    student_id: Uuid,
    grade_level_id: Uuid,
    homeroom_id: Option<Uuid>,
    side: BehaviorSide,
    source: BehaviorSource,
    status: String,
    points: i32,
    raw_points: i32,
    rule_version_id: Option<Uuid>,
    cycle_id: Uuid,
    item_name: String,
}
fn select_rule(
    rules: &[BehaviorAttendanceRule],
    fact: &BehaviorAttendanceFact,
    date: NaiveDate,
) -> Option<(BehaviorScoreAction, i32, Option<Uuid>)> {
    if let Some(overrides) = &fact.overrides {
        if let Some(rule) = overrides.iter().find(|r| r.status == fact.status) {
            return Some((rule.action, rule.points, None));
        }
        return Some((BehaviorScoreAction::None, 0, None));
    }
    rules
        .iter()
        .filter(|r| {
            r.kind == fact.kind
                && r.status == fact.status
                && r.effective_from <= date
                && (r.grade_level_id.is_none() || r.grade_level_id == Some(fact.grade_level_id))
        })
        .max_by_key(|r| (r.grade_level_id.is_some(), r.effective_from, r.id))
        .map(|r| (r.action, r.points, Some(r.id)))
}
fn apply_caps(effects: &mut [Effect], policy: &DayPolicy) {
    effects.sort_by(|a, b| {
        (a.student_id, a.side, a.source, a.source_key.as_str()).cmp(&(
            b.student_id,
            b.side,
            b.source,
            b.source_key.as_str(),
        ))
    });
    let mut used: BTreeMap<(Uuid, BehaviorSide), i32> = BTreeMap::new();
    for e in effects {
        let cap = match e.side {
            BehaviorSide::Merit => policy.daily_merit_cap,
            BehaviorSide::Deduction => policy.daily_deduction_cap,
        };
        let amount = used.entry((e.student_id, e.side)).or_default();
        if let Some(cap) = cap {
            e.points = e.points.min((cap - *amount).max(0));
        }
        *amount += e.points;
    }
}
/// Replace canonical state for one date, preserving unchanged effects and their
/// rule snapshots. Root owns transaction, source/academic locking and queue ack.
pub async fn replace_attendance_day(
    connection: &mut PgConnection,
    day: BehaviorAttendanceDay,
) -> Result<BehaviorDeliveryOutcome, AppError> {
    if day.revision <= 0 || day.facts.len() > 100000 {
        return Err(invalid("ขนาดหรือเวอร์ชันผลเช็กชื่อไม่ถูกต้อง"));
    }
    let mut identities = BTreeSet::new();
    for f in &day.facts {
        if f.kind == BehaviorSource::Manual
            || f.source_key.is_empty()
            || f.source_key.len() > 500
            || !identities.insert((f.kind, f.source_key.clone(), f.student_id))
        {
            return Err(invalid("ที่มาผลเช็กชื่อไม่ถูกต้องหรือซ้ำ"));
        }
        validate_status(&f.status)?;
        if let Some(overrides) = &f.overrides {
            let mut statuses = BTreeSet::new();
            for rule in overrides {
                validate_status(&rule.status)?;
                validate_score(rule.action, rule.points)?;
                if !statuses.insert(&rule.status)
                    || (rule.status == "unchecked" && rule.action != BehaviorScoreAction::None)
                {
                    return Err(invalid("เกณฑ์รอบพิเศษซ้ำหรือคิดคะแนนผู้ที่ยังไม่ได้เช็ก"));
                }
            }
        }
    }
    lock(connection).await?;
    let old_day:Option<(i64,Json<DayPolicy>)>=sqlx::query_as("SELECT revision,policy FROM behavior_source_days WHERE generation=$1 AND academic_term_id=$2 AND date=$3").bind(day.generation).bind(day.academic_term_id).bind(day.date).fetch_optional(&mut *connection).await?;
    if let Some((revision, _)) = &old_day {
        if *revision >= day.revision {
            return Ok(if *revision == day.revision {
                BehaviorDeliveryOutcome::AlreadyApplied
            } else {
                BehaviorDeliveryOutcome::Superseded
            });
        }
    }
    let settings = settings_on_connection(connection).await?;
    let policy = old_day.map(|(_, p)| p.0).unwrap_or(DayPolicy {
        count_absent_lessons: settings.count_absent_lessons,
        count_special: settings.count_special,
        daily_merit_cap: settings.daily_merit_cap,
        daily_deduction_cap: settings.daily_deduction_cap,
    });
    let merit = current_cycle(connection, BehaviorSide::Merit).await?;
    let deduction = current_cycle(connection, BehaviorSide::Deduction).await?;
    let existing:Vec<Existing>=sqlx::query_as("SELECT e.id,t.student_id,e.source_key,e.source_status,t.side,t.source,e.points,e.raw_points,e.rule_version_id,t.cycle_id,e.cancelled_at IS NULL active FROM behavior_entries e JOIN behavior_daily_totals t ON t.id=e.total_id WHERE e.source_generation=$1 AND t.academic_term_id=$2 AND t.date=$3 ORDER BY e.created_at DESC,e.id DESC LIMIT 100001").bind(day.generation).bind(day.academic_term_id).bind(day.date).fetch_all(&mut *connection).await?;
    if existing.len() > 100000 {
        return Err(invalid("ประวัติแก้เช็กชื่อต่อวันเกินขนาดที่รองรับ"));
    }
    let rules:Vec<BehaviorAttendanceRule>=sqlx::query_as("SELECT id,kind,status,grade_level_id,effective_from,action,points FROM behavior_attendance_rules WHERE effective_from<=$1 ORDER BY effective_from DESC,id").bind(day.date).fetch_all(&mut *connection).await?;
    let blocked:Vec<(BehaviorSide,BehaviorSource)>=sqlx::query_as("SELECT DISTINCT side,source FROM behavior_suppressions WHERE $1 BETWEEN start_date AND end_date").bind(day.date).fetch_all(&mut *connection).await?;
    let blocked: BTreeSet<_> = blocked.into_iter().collect();
    let daily_absent: BTreeSet<_> = day
        .facts
        .iter()
        .filter(|f| f.kind == BehaviorSource::Arrival && f.counted && f.status == "absent")
        .map(|f| f.student_id)
        .collect();
    let mut effects = Vec::new();
    let mut protected = BTreeSet::new();
    for fact in &day.facts {
        let previous = existing.iter().find(|e| {
            e.student_id == fact.student_id
                && e.source == fact.kind
                && e.source_key == fact.source_key
                && e.source_status == fact.status
        });
        let (rule_action, raw_points, rule_version) = if let Some(old) = previous {
            (
                match old.side {
                    BehaviorSide::Merit => BehaviorScoreAction::Merit,
                    BehaviorSide::Deduction => BehaviorScoreAction::Deduction,
                },
                old.raw_points,
                old.rule_version_id,
            )
        } else if settings.enabled {
            select_rule(&rules, fact, day.date).unwrap_or((BehaviorScoreAction::None, 0, None))
        } else {
            (BehaviorScoreAction::None, 0, None)
        };
        let Some(side) = action_side(rule_action) else {
            continue;
        };
        let cycle = if side == BehaviorSide::Merit {
            &merit
        } else {
            &deduction
        };
        if day.date < cycle.starts_on || blocked.contains(&(side, fact.kind)) {
            // Closed/purged history is immutable, including corrections and reset
            // callbacks. Keep any surviving old contribution under its old owner.
            if let Some(old) = previous {
                protected.insert(old.id);
            }
            continue;
        }
        if !fact.counted
            || fact.status == "unchecked"
            || (!policy.count_special && fact.kind == BehaviorSource::Special)
            || (!policy.count_absent_lessons
                && fact.kind == BehaviorSource::Lesson
                && fact.status == "absent"
                && daily_absent.contains(&fact.student_id))
        {
            continue;
        }
        effects.push(Effect {
            source_key: fact.source_key.clone(),
            source_session_id: fact.source_session_id,
            student_id: fact.student_id,
            grade_level_id: fact.grade_level_id,
            homeroom_id: fact.homeroom_id,
            side,
            source: fact.kind,
            status: fact.status.clone(),
            points: raw_points,
            raw_points,
            rule_version_id: rule_version,
            cycle_id: cycle.id,
            item_name: format!("เช็กชื่อ {}: {}", fact.kind.code(), fact.status),
        });
    }
    apply_caps(&mut effects, &policy);
    let mut reverse = Vec::new();
    let mut additions = Vec::new();
    for old in existing.iter().filter(|e| e.active) {
        let cycle = if old.side == BehaviorSide::Merit {
            &merit
        } else {
            &deduction
        };
        if old.cycle_id != cycle.id
            || day.date < cycle.starts_on
            || protected.contains(&old.id)
            || blocked.contains(&(old.side, old.source))
        {
            continue;
        }
        let unchanged = effects.iter().any(|e| {
            e.points > 0
                && e.student_id == old.student_id
                && e.source == old.source
                && e.source_key == old.source_key
                && e.status == old.source_status
                && e.side == old.side
                && e.points == old.points
        });
        if !unchanged {
            reverse.push(old.id);
        }
    }
    for effect in effects.into_iter().filter(|e| e.points > 0) {
        let unchanged = existing.iter().any(|old| {
            old.active
                && !reverse.contains(&old.id)
                && effect.student_id == old.student_id
                && effect.source == old.source
                && effect.source_key == old.source_key
                && effect.status == old.source_status
                && effect.side == old.side
                && effect.points == old.points
                && effect.cycle_id == old.cycle_id
        });
        if !unchanged {
            additions.push(effect);
        }
    }
    if !reverse.is_empty() {
        sqlx::query("UPDATE behavior_daily_totals t SET points=t.points-e.points,row_version=t.row_version+1 FROM(SELECT total_id,sum(points)points FROM behavior_entries WHERE id=ANY($1)AND cancelled_at IS NULL GROUP BY total_id)e WHERE e.total_id=t.id").bind(&reverse).execute(&mut *connection).await?;
        sqlx::query("UPDATE behavior_entries SET cancelled_at=now(),cancellation_reason='ผลเช็กชื่อหรือการนับคะแนนเปลี่ยน' WHERE id=ANY($1)AND cancelled_at IS NULL").bind(&reverse).execute(&mut *connection).await?;
    }
    if !additions.is_empty() {
        sqlx::query(r#"INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,homeroom_id,side,source,cycle_id,points)
          SELECT x."studentId",$2,$3,x."gradeLevelId",x."homeroomId",x.side,x.source,x."cycleId",sum(x.points) FROM jsonb_to_recordset($1)x("studentId" uuid,"gradeLevelId" uuid,"homeroomId" uuid,side text,source text,"cycleId" uuid,points integer)
          GROUP BY x."studentId",x."gradeLevelId",x."homeroomId",x.side,x.source,x."cycleId"
          ON CONFLICT(student_id,date,academic_term_id,grade_level_id,homeroom_id,side,source,cycle_id)DO UPDATE SET points=behavior_daily_totals.points+EXCLUDED.points,row_version=behavior_daily_totals.row_version+1"#).bind(Json(&additions)).bind(day.date).bind(day.academic_term_id).execute(&mut *connection).await?;
        sqlx::query(r#"INSERT INTO behavior_entries(total_id,source_key,source_session_id,source_generation,source_revision,source_status,raw_points,rule_version_id,item_name,points)
          SELECT t.id,x."sourceKey",x."sourceSessionId",$2,$3,x.status,x."rawPoints",x."ruleVersionId",x."itemName",x.points FROM jsonb_to_recordset($1)x("studentId" uuid,"gradeLevelId" uuid,"homeroomId" uuid,side text,source text,"cycleId" uuid,points integer,"sourceKey" text,"sourceSessionId" uuid,status text,"rawPoints" integer,"ruleVersionId" uuid,"itemName" text)
          JOIN behavior_daily_totals t ON t.student_id=x."studentId" AND t.grade_level_id=x."gradeLevelId" AND t.homeroom_id IS NOT DISTINCT FROM x."homeroomId" AND t.side=x.side AND t.source=x.source AND t.cycle_id=x."cycleId" AND t.date=$4 AND t.academic_term_id=$5"#).bind(Json(&additions)).bind(day.generation).bind(day.revision).bind(day.date).bind(day.academic_term_id).execute(&mut *connection).await?;
    }
    sqlx::query("INSERT INTO behavior_source_days(generation,academic_term_id,date,revision,policy)VALUES($1,$2,$3,$4,$5)ON CONFLICT(generation,academic_term_id,date)DO UPDATE SET revision=EXCLUDED.revision").bind(day.generation).bind(day.academic_term_id).bind(day.date).bind(day.revision).bind(Json(&policy)).execute(&mut *connection).await?;
    if !reverse.is_empty() || !additions.is_empty() {
        bump(connection).await?;
    }
    let activation: NaiveDate =
        sqlx::query_scalar("SELECT activation_date FROM behavior_state WHERE singleton")
            .fetch_one(&mut *connection)
            .await?;
    Ok(if day.date < activation {
        BehaviorDeliveryOutcome::Suppressed
    } else {
        BehaviorDeliveryOutcome::Applied
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caps_follow_stable_source_order() {
        let student = Uuid::new_v4();
        let effect = |key: &str| Effect {
            source_key: key.into(),
            source_session_id: None,
            student_id: student,
            grade_level_id: Uuid::new_v4(),
            homeroom_id: None,
            side: BehaviorSide::Merit,
            source: BehaviorSource::Lesson,
            status: "present".into(),
            points: 4,
            raw_points: 4,
            rule_version_id: None,
            cycle_id: Uuid::new_v4(),
            item_name: String::new(),
        };
        let mut effects = vec![effect("b"), effect("a")];
        apply_caps(
            &mut effects,
            &DayPolicy {
                count_absent_lessons: false,
                count_special: true,
                daily_merit_cap: Some(5),
                daily_deduction_cap: None,
            },
        );
        assert_eq!(
            effects
                .iter()
                .map(|e| (e.source_key.as_str(), e.points))
                .collect::<Vec<_>>(),
            vec![("a", 4), ("b", 1)]
        );
    }
}
