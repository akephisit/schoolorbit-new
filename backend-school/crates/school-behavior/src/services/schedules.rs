use super::{
    catalog::settings_on_connection,
    cleanup::{apply, local_midnight, make_preview, reset_scope, snapshot, validate_preview},
    shared::{current_cycle, lock, receipt, save_receipt, today},
};
use crate::{
    models::*,
    rules::{invalid, reset_boundary, validate_reason},
};
use chrono::{NaiveDate, Utc};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub async fn preview_reset(
    pool: &PgPool,
    actor: Uuid,
    p: PreviewBehaviorReset,
) -> Result<BehaviorCleanupPreview, AppError> {
    let scope = reset_scope(p.sides, p.through)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "behavior.schedules.preview_reset")
            .await?;
    lock(&mut tx).await?;
    for side in &scope.sides {
        if current_cycle(&mut tx, *side).await?.starts_on > p.through {
            return Err(invalid("วันสิ้นสุดก่อนรอบปัจจุบัน"));
        }
    }
    let result = make_preview(
        &mut tx,
        actor,
        PreviewBehaviorCleanup {
            scope,
            mode: BehaviorCleanupMode::Archive,
        },
        true,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}
pub(super) async fn rotate(
    connection: &mut PgConnection,
    sides: &[BehaviorSide],
    through: NaiveDate,
    reason: &str,
) -> Result<(), AppError> {
    validate_reason(reason)?;
    let next_day = through.succ_opt().ok_or_else(|| invalid("วันสิ้นสุดเกินช่วง"))?;
    let (at, starts) = if through == today() {
        (Utc::now(), through)
    } else {
        (local_midnight(next_day)?, next_day)
    };
    for side in sides {
        let old = current_cycle(connection, *side).await?;
        if old.starts_on > through {
            return Err(AppError::Conflict("รอบนี้ถูกรีเซ็ตแล้ว".into()));
        }
        DbSession::new(&mut *connection).query("behavior.services.schedules.rotate",
            "UPDATE behavior_cycles SET state='history',ends_on=$2,ends_at=$3,label=$4 WHERE id=$1",
        )
        .bind(old.id)
        .bind(through)
        .bind(at)
        .bind("ประวัติรอบคะแนน")
        .execute()
        .await?;
        let new:Uuid=DbSession::new(&mut *connection).scalar("behavior.services.schedules.rotate", "INSERT INTO behavior_cycles(side,starts_on,starts_at,state,label)VALUES($1,$2,$3,'current','รอบปัจจุบัน')RETURNING id").bind(side).bind(starts).bind(at).fetch_one().await?;
        // A delayed month-end job may run after new-month facts arrived. Their
        // existing contributions move to the new cycle without copying effects.
        DbSession::new(&mut *connection).query("behavior.services.schedules.rotate", "UPDATE behavior_daily_totals SET cycle_id=$2,row_version=row_version+1 WHERE cycle_id=$1 AND date>$3").bind(old.id).bind(new).bind(through).execute().await?;
    }
    Ok(())
}
pub async fn reset(
    pool: &PgPool,
    actor: Uuid,
    mut p: ResetBehaviorScores,
    readiness: BehaviorResetReadiness,
) -> Result<BehaviorCleanupOutcome, AppError> {
    validate_reason(&p.reason)?;
    p.reason = p.reason.trim().into();
    let scope = reset_scope(p.sides.clone(), p.through)?;
    p.sides = scope.sides.clone();
    if p.through < today()
        && (readiness.finalized_through < p.through || readiness.delivered_through < p.through)
    {
        return Err(AppError::Conflict(
            "รอปิดยอดเช็กชื่อและส่งคะแนนครบก่อนรีเซ็ต".into(),
        ));
    }
    // Same-day explicit reset requires the caller to flush all source delivery
    // currently committed; future unchecked results are finalized next midnight.
    if readiness.delivered_through < p.through {
        return Err(AppError::Conflict("มีผลเช็กชื่อที่ยังส่งคะแนนไม่ครบ".into()));
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "behavior.schedules.reset").await?;
    let result = reset_on_connection(&mut tx, actor, p, readiness).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn reset_on_connection(
    connection: &mut PgConnection,
    actor: Uuid,
    mut p: ResetBehaviorScores,
    readiness: BehaviorResetReadiness,
) -> Result<BehaviorCleanupOutcome, AppError> {
    validate_reason(&p.reason)?;
    p.reason = p.reason.trim().into();
    let scope = reset_scope(p.sides.clone(), p.through)?;
    p.sides = scope.sides.clone();
    if (p.through < today() && readiness.finalized_through < p.through)
        || readiness.delivered_through < p.through
    {
        return Err(AppError::Conflict(
            "รอปิดยอดเช็กชื่อและส่งคะแนนครบก่อนรีเซ็ต".into(),
        ));
    }
    super::shared::raw_lock(connection).await?;
    if let Some(result) = receipt(connection, actor, p.request_id, "reset", &p).await? {
        return Ok(result);
    }
    lock(connection).await?;
    let request = PreviewBehaviorCleanup {
        scope: scope.clone(),
        mode: BehaviorCleanupMode::Archive,
    };
    let snapshot = validate_preview(connection, actor, p.token, p.revision, &request, true).await?;
    let result = apply(
        connection,
        Some(actor),
        &scope,
        BehaviorCleanupMode::Archive,
        snapshot,
        &p.reason,
        true,
    )
    .await?;
    if result.status == "completed" {
        rotate(connection, &p.sides, p.through, &p.reason).await?;
    }
    save_receipt(connection, actor, p.request_id, "reset", &p, &result).await?;
    DbSession::new(connection)
        .query(
            "behavior.services.schedules.reset_on_connection",
            "DELETE FROM behavior_cleanup_previews WHERE token=$1",
        )
        .bind(p.token)
        .execute()
        .await?;
    Ok(result)
}

/// Caller supplies verified finalization/delivery boundaries from Attendance.
/// Each iteration commits one completed boundary; restart resumes from the new
/// cycle's starts_on rather than a volatile scheduler timestamp.
pub async fn run_due_resets(
    pool: &PgPool,
    now: NaiveDate,
    readiness: BehaviorResetReadiness,
    limit: u32,
) -> Result<u32, AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "behavior.schedules.run_due_resets")
            .await?;
    let result = run_due_resets_on_connection(&mut tx, now, readiness, limit).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn run_due_resets_on_connection(
    connection: &mut PgConnection,
    now: NaiveDate,
    readiness: BehaviorResetReadiness,
    limit: u32,
) -> Result<u32, AppError> {
    if limit == 0 || limit > 24 {
        return Err(invalid("จำนวนรอบรีเซ็ตต่อชุดต้องอยู่ระหว่าง 1 ถึง 24"));
    }
    lock(connection).await?;
    let mut completed = 0;
    for _ in 0..limit {
        let settings = settings_on_connection(connection).await?;
        let mut due = None;
        for (side, schedule) in [
            (BehaviorSide::Merit, settings.merit_reset),
            (BehaviorSide::Deduction, settings.deduction_reset),
        ] {
            let cycle = current_cycle(connection, side).await?;
            if let Some(through) = reset_boundary(
                schedule,
                cycle.starts_on,
                now,
                &readiness,
                match side {
                    BehaviorSide::Merit => &settings.merit_reset_dates,
                    BehaviorSide::Deduction => &settings.deduction_reset_dates,
                },
            ) {
                due = Some((side, through));
                break;
            }
        }
        let Some((side, through)) = due else {
            break;
        };
        let scope = reset_scope(vec![side], through)?;
        let snapshot = snapshot(connection, &scope, true).await?;
        let result = apply(
            connection,
            None,
            &scope,
            BehaviorCleanupMode::Archive,
            snapshot,
            "รีเซ็ตคะแนนตามรอบอัตโนมัติ",
            true,
        )
        .await?;
        if result.status == "pending" {
            break;
        }
        rotate(connection, &[side], through, "ประวัติจากรอบอัตโนมัติ").await?;
        completed += 1;
    }
    Ok(completed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_reset_allows_same_civil_day() {
        assert!(reset_scope(vec![BehaviorSide::Merit], today()).is_ok());
    }
}
