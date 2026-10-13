use super::shared::{bump, lock, receipt, revision, save_receipt, today};
use crate::{
    models::*,
    rules::{digest, invalid, normalize_scope, totals, validate_reason},
};
use chrono::{DateTime, NaiveDate, Utc};
use school_errors::AppError;
use sqlx::{types::Json, PgConnection, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StoredPreview {
    pub request: PreviewBehaviorCleanup,
    pub reset: bool,
    pub totals_digest: String,
}
pub(super) struct Snapshot {
    pub rows: Vec<BehaviorDailyTotal>,
    pub current: BehaviorScoreTotals,
    pub history: BehaviorScoreTotals,
    pub students: i64,
    pub entries: i64,
}
fn selected(scope: &BehaviorCleanupScope, current_only: bool) -> QueryBuilder<Postgres> {
    let mut b=QueryBuilder::new("SELECT t.id,t.student_id,t.date,t.academic_term_id,t.grade_level_id,t.homeroom_id,t.side,t.source,t.cycle_id,t.points,t.row_version FROM behavior_daily_totals t JOIN behavior_cycles c ON c.id=t.cycle_id JOIN academic_terms term ON term.id=t.academic_term_id WHERE true");
    if let Some(d) = scope.start {
        b.push(" AND t.date>=").push_bind(d);
    }
    if let Some(d) = scope.end {
        b.push(" AND t.date<=").push_bind(d);
    }
    if let Some(id) = scope.academic_term_id {
        b.push(" AND t.academic_term_id=").push_bind(id);
    }
    if let Some(id) = scope.academic_year_id {
        b.push(" AND term.academic_year_id=").push_bind(id);
    }
    b.push(" AND t.side=ANY(")
        .push_bind(scope.sides.clone())
        .push(") AND t.source=ANY(")
        .push_bind(scope.sources.clone())
        .push(")");
    if current_only {
        b.push(" AND c.state='current'");
    }
    b.push(" ORDER BY t.id LIMIT 100001 FOR UPDATE OF t");
    b
}
pub(super) async fn snapshot(
    connection: &mut PgConnection,
    scope: &BehaviorCleanupScope,
    current_only: bool,
) -> Result<Snapshot, AppError> {
    let rows: Vec<BehaviorDailyTotal> = selected(scope, current_only)
        .build_query_as()
        .fetch_all(&mut *connection)
        .await?;
    if rows.len() > 100000 {
        return Err(invalid("ขอบเขตเกิน 100,000 ยอดรายวัน กรุณาแบ่งช่วงวันที่หรือที่มา"));
    }
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let current_cycles: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM behavior_cycles WHERE state='current'")
            .fetch_all(&mut *connection)
            .await?;
    let mut current = totals(0, 0);
    let mut history = totals(0, 0);
    let mut students = std::collections::BTreeSet::new();
    for row in &rows {
        students.insert(row.student_id);
        let target = if current_cycles.contains(&row.cycle_id) {
            &mut current
        } else {
            &mut history
        };
        match row.side {
            BehaviorSide::Merit => target.merit += row.points,
            BehaviorSide::Deduction => target.deduction += row.points,
        }
    }
    let entries: i64 =
        sqlx::query_scalar("SELECT count(*)::bigint FROM behavior_entries WHERE total_id=ANY($1)")
            .bind(&ids)
            .fetch_one(&mut *connection)
            .await?;
    // A compressed total may exceed remaining detail contributions, but never the
    // other way round. Refuse deletion if score reconciliation is already broken.
    let invalid_total:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM behavior_daily_totals t JOIN(SELECT total_id,sum(points)p FROM behavior_entries WHERE total_id=ANY($1) AND cancelled_at IS NULL GROUP BY total_id)e ON e.total_id=t.id WHERE e.p>t.points)").bind(&ids).fetch_one(&mut *connection).await?;
    if invalid_total {
        return Err(AppError::Conflict(
            "ยอดคะแนนกับรายละเอียดไม่ตรงกัน จึงยังไม่ล้างข้อมูล".into(),
        ));
    }
    Ok(Snapshot {
        rows,
        current: totals(current.merit, current.deduction),
        history: totals(history.merit, history.deduction),
        students: students.len() as i64,
        entries,
    })
}
pub(super) async fn make_preview(
    connection: &mut PgConnection,
    actor: Uuid,
    request: PreviewBehaviorCleanup,
    reset: bool,
) -> Result<BehaviorCleanupPreview, AppError> {
    let snapshot = snapshot(connection, &request.scope, reset).await?;
    let revision = revision(connection).await?;
    let digest = digest(&snapshot.rows)?;
    let token:Uuid=sqlx::query_scalar("INSERT INTO behavior_cleanup_previews(actor_id,revision,request)VALUES($1,$2,$3)RETURNING token").bind(actor).bind(revision).bind(Json(StoredPreview{request:request.clone(),reset,totals_digest:digest})).fetch_one(&mut *connection).await?;
    let total = totals(
        snapshot.current.merit + snapshot.history.merit,
        snapshot.current.deduction + snapshot.history.deduction,
    );
    let (current, history, lifetime) = match request.mode {
        BehaviorCleanupMode::DetailsOnly => (totals(0, 0), totals(0, 0), totals(0, 0)),
        BehaviorCleanupMode::Archive => (
            totals(-snapshot.current.merit, -snapshot.current.deduction),
            snapshot.current,
            totals(0, 0),
        ),
        BehaviorCleanupMode::DeleteEffects => (
            totals(-snapshot.current.merit, -snapshot.current.deduction),
            totals(-snapshot.history.merit, -snapshot.history.deduction),
            totals(-total.merit, -total.deduction),
        ),
    };
    Ok(BehaviorCleanupPreview {
        token,
        revision,
        students: snapshot.students,
        entries: snapshot.entries,
        merit: total.merit,
        deduction: total.deduction,
        current,
        history,
        lifetime,
    })
}
pub async fn preview(
    pool: &PgPool,
    actor: Uuid,
    mut p: PreviewBehaviorCleanup,
) -> Result<BehaviorCleanupPreview, AppError> {
    p.scope = normalize_scope(p.scope)?;
    let mut tx = pool.begin().await?;
    lock(&mut tx).await?;
    sqlx::query("DELETE FROM behavior_cleanup_previews WHERE created_at<now()-interval '1 day'")
        .execute(&mut *tx)
        .await?;
    let result = make_preview(&mut tx, actor, p, false).await?;
    tx.commit().await?;
    Ok(result)
}
pub(super) async fn validate_preview(
    connection: &mut PgConnection,
    actor: Uuid,
    token: Uuid,
    expected_revision: i64,
    p: &PreviewBehaviorCleanup,
    reset: bool,
) -> Result<Snapshot, AppError> {
    let row:Option<(i64,Json<StoredPreview>)>=sqlx::query_as("SELECT revision,request FROM behavior_cleanup_previews WHERE token=$1 AND actor_id=$2 AND created_at>now()-interval '1 day' FOR UPDATE").bind(token).bind(actor).fetch_optional(&mut *connection).await?;
    let (r, stored) =
        row.ok_or_else(|| AppError::Conflict("ผลตรวจหมดอายุหรือไม่ใช่ของผู้ใช้นี้ กรุณาตรวจใหม่".into()))?;
    if r != expected_revision
        || r != revision(connection).await?
        || stored.0.request != *p
        || stored.0.reset != reset
    {
        return Err(AppError::Conflict(
            "ขอบเขตหรือข้อมูลเปลี่ยนแล้ว กรุณาตรวจผลกระทบใหม่".into(),
        ));
    }
    let snapshot = snapshot(connection, &p.scope, reset).await?;
    if digest(&snapshot.rows)? != stored.0.totals_digest {
        return Err(AppError::Conflict("ผลคะแนนเปลี่ยนแล้ว กรุณาตรวจใหม่".into()));
    }
    Ok(snapshot)
}
pub(super) async fn apply(
    connection: &mut PgConnection,
    actor: Option<Uuid>,
    scope: &BehaviorCleanupScope,
    mode: BehaviorCleanupMode,
    snapshot: Snapshot,
    reason: &str,
    reset: bool,
) -> Result<BehaviorCleanupOutcome, AppError> {
    let ids: Vec<_> = snapshot.rows.iter().map(|r| r.id).collect();
    // Store suppression before removing source-bearing entries, in this same
    // serialized transaction. Preserve no event text in the purge audit.
    let (context_start, context_end): (NaiveDate, NaiveDate) = if let Some(term) =
        scope.academic_term_id
    {
        sqlx::query_as("SELECT t.start_date,COALESCE(t.closed_on,t.planned_end_date,y.end_date)FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1").bind(term).fetch_one(&mut *connection).await?
    } else if let Some(year) = scope.academic_year_id {
        sqlx::query_as("SELECT start_date,end_date FROM academic_years WHERE id=$1")
            .bind(year)
            .fetch_one(&mut *connection)
            .await?
    } else {
        let activation: NaiveDate =
            sqlx::query_scalar("SELECT activation_date FROM behavior_state WHERE singleton")
                .fetch_one(&mut *connection)
                .await?;
        (
            snapshot
                .rows
                .iter()
                .map(|r| r.date)
                .min()
                .unwrap_or(activation),
            today(),
        )
    };
    let start = scope.start.unwrap_or(context_start).max(context_start);
    let end = scope.end.unwrap_or(context_end).min(context_end);
    if start <= end {
        for side in &scope.sides {
            for source in &scope.sources {
                sqlx::query("INSERT INTO behavior_suppressions(start_date,end_date,side,source,academic_term_id,academic_year_id)VALUES($1,$2,$3,$4,$5,$6)").bind(start).bind(end).bind(side).bind(source).bind(scope.academic_term_id).bind(scope.academic_year_id).execute(&mut *connection).await?;
            }
        }
    }
    let entries_deleted = sqlx::query("DELETE FROM behavior_entries WHERE total_id=ANY($1)")
        .bind(&ids)
        .execute(&mut *connection)
        .await?
        .rows_affected() as i64;
    sqlx::query("DELETE FROM behavior_batches b WHERE NOT EXISTS(SELECT 1 FROM behavior_entries e WHERE e.batch_id=b.id)").execute(&mut *connection).await?;
    match mode {
        BehaviorCleanupMode::DetailsOnly => {
            sqlx::query("UPDATE behavior_daily_totals SET details_purged=true,row_version=row_version+1 WHERE id=ANY($1)").bind(&ids).execute(&mut *connection).await?;
        }
        BehaviorCleanupMode::DeleteEffects => {
            sqlx::query("UPDATE behavior_daily_totals SET points=0,details_purged=true,row_version=row_version+1 WHERE id=ANY($1)").bind(&ids).execute(&mut *connection).await?;
        }
        BehaviorCleanupMode::Archive => {
            sqlx::query("UPDATE behavior_daily_totals SET details_purged=true,row_version=row_version+1 WHERE id=ANY($1)").bind(&ids).execute(&mut *connection).await?;
            let current_ids: Vec<Uuid> =
                sqlx::query_scalar("SELECT id FROM behavior_cycles WHERE state='current'")
                    .fetch_all(&mut *connection)
                    .await?;
            for side in &scope.sides {
                if reset {
                    continue;
                }
                let selected: Vec<_> = snapshot
                    .rows
                    .iter()
                    .filter(|r| r.side == *side && current_ids.contains(&r.cycle_id))
                    .collect();
                if let (Some(start), Some(end)) = (
                    selected.iter().map(|r| r.date).min(),
                    selected.iter().map(|r| r.date).max(),
                ) {
                    let start_at = local_midnight(start)?;
                    let end_at =
                        local_midnight(end.succ_opt().ok_or_else(|| invalid("วันสิ้นสุดเกินช่วง"))?)?;
                    let cycle:Uuid=sqlx::query_scalar("INSERT INTO behavior_cycles(side,starts_on,starts_at,ends_on,ends_at,state,label)VALUES($1,$2,$3,$4,$5,'history','ประวัติจากการล้างรายละเอียด')RETURNING id").bind(side).bind(start).bind(start_at).bind(end).bind(end_at).fetch_one(&mut *connection).await?;
                    let side_ids: Vec<_> = selected.iter().map(|r| r.id).collect();
                    sqlx::query("UPDATE behavior_daily_totals SET cycle_id=$2,details_purged=true,row_version=row_version+1 WHERE id=ANY($1)").bind(side_ids).bind(cycle).execute(&mut *connection).await?;
                }
            }
        }
    }
    let merit = snapshot.current.merit + snapshot.history.merit;
    let deduction = snapshot.current.deduction + snapshot.history.deduction;
    let id:Uuid=sqlx::query_scalar("INSERT INTO behavior_cleanup_audits(actor_id,scope,mode,reason,students,entries,merit,deduction)VALUES($1,$2,$3,$4,$5,$6,$7,$8)RETURNING id").bind(actor).bind(Json(scope)).bind(if reset{"reset"}else{mode.code()}).bind(reason).bind(snapshot.students).bind(snapshot.entries).bind(merit).bind(deduction).fetch_one(&mut *connection).await?;
    bump(connection).await?;
    Ok(BehaviorCleanupOutcome {
        id,
        students: snapshot.students,
        entries_deleted,
        merit,
        deduction,
    })
}
pub async fn execute(
    pool: &PgPool,
    actor: Uuid,
    mut p: ExecuteBehaviorCleanup,
) -> Result<BehaviorCleanupOutcome, AppError> {
    p.scope = normalize_scope(p.scope)?;
    validate_reason(&p.reason)?;
    p.reason = p.reason.trim().into();
    let mut tx = pool.begin().await?;
    lock(&mut tx).await?;
    if let Some(result) = receipt(&mut tx, actor, p.request_id, "cleanup", &p).await? {
        tx.commit().await?;
        return Ok(result);
    }
    let request = PreviewBehaviorCleanup {
        scope: p.scope.clone(),
        mode: p.mode,
    };
    let snapshot = validate_preview(&mut tx, actor, p.token, p.revision, &request, false).await?;
    let result = apply(
        &mut tx,
        Some(actor),
        &p.scope,
        p.mode,
        snapshot,
        &p.reason,
        false,
    )
    .await?;
    save_receipt(&mut tx, actor, p.request_id, "cleanup", &p, &result).await?;
    sqlx::query("DELETE FROM behavior_cleanup_previews WHERE token=$1")
        .bind(p.token)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn adjust_history(
    pool: &PgPool,
    actor: Uuid,
    mut p: AdjustBehaviorHistory,
) -> Result<BehaviorHistoryAdjustment, AppError> {
    validate_reason(&p.reason)?;
    p.reason = p.reason.trim().into();
    if p.delta == 0 || p.delta.unsigned_abs() > 100000000 {
        return Err(invalid("จำนวนปรับยอดไม่ถูกต้อง"));
    }
    let mut tx = pool.begin().await?;
    lock(&mut tx).await?;
    if let Some(result) = receipt(&mut tx, actor, p.request_id, "history", &p).await? {
        tx.commit().await?;
        return Ok(result);
    }
    let old:Option<(i64,i64,String)>=sqlx::query_as("SELECT t.points,t.row_version,c.state FROM behavior_daily_totals t JOIN behavior_cycles c ON c.id=t.cycle_id WHERE t.id=$1 FOR UPDATE OF t").bind(p.total_id).fetch_optional(&mut *tx).await?;
    let (points, version, state) = old.ok_or_else(|| AppError::NotFound("ไม่พบยอดประวัติ".into()))?;
    if state != "history" || version != p.row_version {
        return Err(AppError::Conflict("ยอดนี้ไม่ใช่ประวัติหรือเปลี่ยนแล้ว".into()));
    }
    let points = points
        .checked_add(p.delta)
        .filter(|n| *n >= 0)
        .ok_or_else(|| invalid("ปรับยอดแล้วคะแนนต้องไม่ติดลบ"))?;
    sqlx::query("UPDATE behavior_daily_totals SET points=$2,row_version=row_version+1 WHERE id=$1")
        .bind(p.total_id)
        .bind(points)
        .execute(&mut *tx)
        .await?;
    let id:Uuid=sqlx::query_scalar("INSERT INTO behavior_history_adjustments(total_id,delta,reason,actor_id)VALUES($1,$2,$3,$4)RETURNING id").bind(p.total_id).bind(p.delta).bind(&p.reason).bind(actor).fetch_one(&mut *tx).await?;
    let result = BehaviorHistoryAdjustment {
        id,
        total_id: p.total_id,
        points,
        row_version: version + 1,
    };
    save_receipt(&mut tx, actor, p.request_id, "history", &p, &result).await?;
    bump(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}
pub(super) fn local_midnight(date: NaiveDate) -> Result<DateTime<Utc>, AppError> {
    use chrono::TimeZone;
    chrono_tz::Asia::Bangkok
        .from_local_datetime(&date.and_time(chrono::NaiveTime::MIN))
        .single()
        .map(|v| v.with_timezone(&Utc))
        .ok_or_else(|| invalid("แปลงเวลาโรงเรียนไม่ได้"))
}
pub(super) fn reset_scope(
    sides: Vec<BehaviorSide>,
    through: NaiveDate,
) -> Result<BehaviorCleanupScope, AppError> {
    if through > today() {
        return Err(invalid("ไม่สามารถรีเซ็ตวันในอนาคต"));
    }
    let mut sides = sides;
    sides.sort_unstable();
    sides.dedup();
    if sides.is_empty() {
        return Err(invalid("เลือกฝั่งคะแนน"));
    }
    Ok(BehaviorCleanupScope {
        start: None,
        end: Some(through),
        academic_year_id: None,
        academic_term_id: None,
        sides,
        sources: vec![
            BehaviorSource::Manual,
            BehaviorSource::Arrival,
            BehaviorSource::Lesson,
            BehaviorSource::Special,
        ],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reason_and_scope_validation() {
        assert!(validate_reason("").is_err());
        assert!(normalize_scope(BehaviorCleanupScope {
            start: None,
            end: None,
            academic_year_id: None,
            academic_term_id: None,
            sides: vec![],
            sources: vec![BehaviorSource::Manual]
        })
        .is_err());
    }
}
