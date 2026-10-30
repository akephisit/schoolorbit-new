use super::shared::{bump, lock, receipt, revision, save_receipt, today};
use crate::{
    models::*,
    rules::{invalid, normalize_scope, totals, validate_reason},
};
use chrono::{DateTime, NaiveDate, Utc};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool, Postgres, QueryBuilder, types::Json};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StoredPreview {
    pub request: PreviewBehaviorCleanup,
    pub reset: bool,
    pub totals_digest: String,
}
pub(super) struct Snapshot {
    pub checksum: String,
    pub current: BehaviorScoreTotals,
    pub history: BehaviorScoreTotals,
    pub students: i64,
    pub entries: i64,
    pub records: i64,
}
#[derive(sqlx::FromRow)]
struct Stats {
    checksum: String,
    students: i64,
    entries: i64,
    records: i64,
    current_merit: i64,
    current_deduction: i64,
    history_merit: i64,
    history_deduction: i64,
    invalid: bool,
}
fn selection(
    scope: &BehaviorCleanupScope,
    current_only: bool,
    prefix: &str,
    suffix: &str,
) -> QueryBuilder<Postgres> {
    selection_builder(
        scope,
        current_only,
        QueryBuilder::new(prefix.to_owned()),
        suffix,
        false,
    )
}
fn selection_builder(
    scope: &BehaviorCleanupScope,
    current_only: bool,
    mut b: QueryBuilder<Postgres>,
    suffix: &str,
    detail_join: bool,
) -> QueryBuilder<Postgres> {
    b.push(" FROM behavior_daily_totals t JOIN behavior_cycles c ON c.id=t.cycle_id JOIN academic_terms term ON term.id=t.academic_term_id");
    if detail_join {
        b.push(" LEFT JOIN detail ON detail.total_id=t.id");
    }
    b.push(" WHERE true");
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
    b.push(suffix.to_owned());
    b
}
pub(super) async fn snapshot(
    connection: &mut PgConnection,
    scope: &BehaviorCleanupScope,
    current_only: bool,
) -> Result<Snapshot, AppError> {
    let mut q = selection_builder(
        scope,
        current_only,
        QueryBuilder::new(
            "WITH detail AS(SELECT total_id,count(*)::bigint n,COALESCE(sum(points)FILTER(WHERE cancelled_at IS NULL),0)::bigint p FROM behavior_entries GROUP BY total_id)SELECT COALESCE(md5(string_agg(md5(concat_ws(':',t.id,t.row_version,t.points,t.cycle_id)),''ORDER BY t.id)),'')checksum,count(DISTINCT t.student_id)::bigint students,count(*)::bigint records,COALESCE(sum(detail.n),0)::bigint entries,COALESCE(sum(t.points)FILTER(WHERE c.state='current' AND t.side='merit'),0)::bigint current_merit,COALESCE(sum(t.points)FILTER(WHERE c.state='current' AND t.side='deduction'),0)::bigint current_deduction,COALESCE(sum(t.points)FILTER(WHERE c.state='history' AND t.side='merit'),0)::bigint history_merit,COALESCE(sum(t.points)FILTER(WHERE c.state='history' AND t.side='deduction'),0)::bigint history_deduction,COALESCE(bool_or(COALESCE(detail.p,0)>t.points),false)invalid",
        ),
        "",
        true,
    );
    let stats: Stats = DbSession::new(connection)
        .prepared_query_as("behavior.services.cleanup.snapshot", q.build_query_as())
        .fetch_one()
        .await?;
    if stats.invalid {
        return Err(AppError::Conflict(
            "ยอดคะแนนกับรายละเอียดไม่ตรงกัน จึงยังไม่ล้างข้อมูล".into(),
        ));
    }
    Ok(Snapshot {
        checksum: stats.checksum,
        current: totals(stats.current_merit, stats.current_deduction),
        history: totals(stats.history_merit, stats.history_deduction),
        students: stats.students,
        entries: stats.entries,
        records: stats.records,
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
    let digest = snapshot.checksum.clone();
    let token:Uuid=DbSession::new(&mut *connection).scalar("behavior.services.cleanup.make_preview", "INSERT INTO behavior_cleanup_previews(actor_id,revision,request)VALUES($1,$2,$3)RETURNING token").bind(actor).bind(revision).bind(Json(StoredPreview{request:request.clone(),reset,totals_digest:digest})).fetch_one().await?;
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
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "behavior.cleanup.preview").await?;
    lock(&mut tx).await?;
    DbSession::new(&mut *tx)
        .query(
            "behavior.services.cleanup.preview",
            "DELETE FROM behavior_cleanup_previews WHERE created_at<now()-interval '1 day'",
        )
        .execute()
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
    let row:Option<(i64,Json<StoredPreview>)>=DbSession::new(&mut *connection).query_as("behavior.services.cleanup.validate_preview", "SELECT revision,request FROM behavior_cleanup_previews WHERE token=$1 AND actor_id=$2 AND created_at>now()-interval '1 day' FOR UPDATE").bind(token).bind(actor).fetch_optional().await?;
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
    if snapshot.checksum != stored.0.totals_digest {
        return Err(AppError::Conflict("ผลคะแนนเปลี่ยนแล้ว กรุณาตรวจใหม่".into()));
    }
    Ok(snapshot)
}
async fn suppress_scope(
    connection: &mut PgConnection,
    scope: &BehaviorCleanupScope,
) -> Result<(), AppError> {
    let mut checkpoints = QueryBuilder::<Postgres>::new(
        "UPDATE behavior_source_effect_snapshots snap SET suppressed=true WHERE snap.kind=ANY(",
    );
    checkpoints
        .push_bind(scope.sources.clone())
        .push(") AND snap.action=ANY(")
        .push_bind(
            scope
                .sides
                .iter()
                .map(|s| s.code().to_string())
                .collect::<Vec<_>>(),
        )
        .push(")");
    if let Some(d) = scope.start {
        checkpoints.push(" AND snap.date>=").push_bind(d);
    }
    if let Some(d) = scope.end {
        checkpoints.push(" AND snap.date<=").push_bind(d);
    }
    if let Some(id) = scope.academic_term_id {
        checkpoints
            .push(" AND snap.academic_term_id=")
            .push_bind(id);
    }
    if let Some(id) = scope.academic_year_id {
        checkpoints.push(" AND snap.academic_term_id IN(SELECT id FROM academic_terms WHERE academic_year_id=").push_bind(id).push(")");
    }
    DbSession::new(&mut *connection)
        .prepared_query(
            "behavior.services.cleanup.suppress_scope",
            checkpoints.build(),
        )
        .execute()
        .await?;
    let (start, end) = if scope.academic_term_id.is_some() || scope.academic_year_id.is_some() {
        (scope.start, scope.end)
    } else {
        let activation: NaiveDate = DbSession::new(&mut *connection)
            .scalar(
                "behavior.services.cleanup.activation",
                "SELECT activation_date FROM behavior_state WHERE singleton",
            )
            .fetch_one()
            .await?;
        (
            Some(scope.start.unwrap_or(activation)),
            Some(scope.end.unwrap_or_else(today)),
        )
    };
    for side in &scope.sides {
        for source in &scope.sources {
            DbSession::new(&mut *connection).query("behavior.services.cleanup.suppress_scope", "INSERT INTO behavior_suppressions(start_date,end_date,side,source,academic_term_id,academic_year_id)VALUES($1,$2,$3,$4,$5,$6)")
                .bind(start).bind(end).bind(side).bind(source).bind(scope.academic_term_id).bind(scope.academic_year_id).execute().await?;
        }
    }
    Ok(())
}
async fn finish_totals(
    connection: &mut PgConnection,
    scope: &BehaviorCleanupScope,
    mode: BehaviorCleanupMode,
    reset: bool,
) -> Result<(), AppError> {
    let set = match mode {
        BehaviorCleanupMode::DeleteEffects => {
            "points=0,details_purged=true,row_version=row_version+1"
        }
        _ => "details_purged=true,row_version=row_version+1",
    };
    let prefix = format!("UPDATE behavior_daily_totals SET {set} WHERE id IN(SELECT t.id");
    DbSession::new(&mut *connection)
        .prepared_query(
            "behavior.services.cleanup.finish_totals",
            selection(scope, reset, &prefix, ")").build(),
        )
        .execute()
        .await?;
    if mode == BehaviorCleanupMode::Archive && !reset {
        for side in &scope.sides {
            let mut part = scope.clone();
            part.sides = vec![*side];
            let bounds: Option<(Option<NaiveDate>, Option<NaiveDate>)> =
                DbSession::new(&mut *connection)
                    .prepared_query_as(
                        "behavior.services.cleanup.finish_totals",
                        selection(&part, true, "SELECT min(t.date),max(t.date)", "")
                            .build_query_as(),
                    )
                    .fetch_optional()
                    .await?;
            if let Some((Some(start), Some(end))) = bounds {
                let cycle:Uuid=DbSession::new(&mut *connection).scalar("behavior.services.cleanup.finish_totals", "INSERT INTO behavior_cycles(side,starts_on,starts_at,ends_on,ends_at,state,label)VALUES($1,$2,$3,$4,$5,'history','ประวัติจากการล้างรายละเอียด')RETURNING id").bind(side).bind(start).bind(local_midnight(start)?).bind(end).bind(local_midnight(end.succ_opt().ok_or_else(||invalid("วันสิ้นสุดเกินช่วง"))?)?).fetch_one().await?;
                let mut q =
                    QueryBuilder::<Postgres>::new("UPDATE behavior_daily_totals SET cycle_id=");
                q.push_bind(cycle).push(" WHERE id IN(SELECT t.id");
                DbSession::new(&mut *connection)
                    .prepared_query(
                        "behavior.services.cleanup.finish_totals",
                        selection_builder(&part, true, q, ")", false).build(),
                    )
                    .execute()
                    .await?;
            }
        }
    }
    DbSession::new(connection).query("behavior.services.cleanup.finish_totals", "DELETE FROM behavior_batches b WHERE NOT EXISTS(SELECT 1 FROM behavior_entries e WHERE e.batch_id=b.id)").execute().await?;
    Ok(())
}
async fn audit_completion(
    connection: &mut PgConnection,
    id: Uuid,
    actor: Option<Uuid>,
    scope: &BehaviorCleanupScope,
    mode: BehaviorCleanupMode,
    reset: bool,
    snapshot: &Snapshot,
    reason: &str,
    deleted: i64,
) -> Result<BehaviorCleanupOutcome, AppError> {
    let merit = snapshot.current.merit + snapshot.history.merit;
    let deduction = snapshot.current.deduction + snapshot.history.deduction;
    DbSession::new(&mut *connection).query("behavior.services.cleanup.audit_completion", "INSERT INTO behavior_cleanup_audits(id,actor_id,scope,mode,reason,students,entries,merit,deduction)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(id).bind(actor).bind(Json(scope)).bind(if reset{"reset"}else{mode.code()}).bind(reason).bind(snapshot.students).bind(deleted).bind(merit).bind(deduction).execute().await?;
    bump(connection).await?;
    Ok(BehaviorCleanupOutcome {
        id,
        status: "completed".into(),
        students: snapshot.students,
        entries_deleted: deleted,
        merit,
        deduction,
    })
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
    suppress_scope(connection, scope).await?;
    let id = Uuid::now_v7();
    let merit = snapshot.current.merit + snapshot.history.merit;
    let deduction = snapshot.current.deduction + snapshot.history.deduction;
    if snapshot.entries > 20000 || snapshot.records > 10000 {
        DbSession::new(&mut *connection).query("behavior.services.cleanup.apply", "INSERT INTO behavior_cleanup_jobs(id,actor_id,scope,mode,reset,reason,students,entries_expected,merit,deduction)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)").bind(id).bind(actor).bind(Json(scope)).bind(mode).bind(reset).bind(reason).bind(snapshot.students).bind(snapshot.entries).bind(merit).bind(deduction).execute().await?;
        bump(connection).await?;
        return Ok(BehaviorCleanupOutcome {
            id,
            status: "pending".into(),
            students: snapshot.students,
            entries_deleted: 0,
            merit,
            deduction,
        });
    }
    let deleted = DbSession::new(&mut *connection)
        .prepared_query(
            "behavior.services.cleanup.apply",
            selection(
                scope,
                reset,
                "DELETE FROM behavior_entries WHERE total_id IN(SELECT t.id",
                ")",
            )
            .build(),
        )
        .execute()
        .await?
        .rows_affected() as i64;
    finish_totals(connection, scope, mode, reset).await?;
    audit_completion(
        connection, id, actor, scope, mode, reset, &snapshot, reason, deleted,
    )
    .await
}
#[derive(sqlx::FromRow)]
struct CleanupJob {
    id: Uuid,
    actor_id: Option<Uuid>,
    scope: Json<BehaviorCleanupScope>,
    mode: BehaviorCleanupMode,
    reset: bool,
    reason: String,
    students: i64,
    entries_expected: i64,
    entries_deleted: i64,
    merit: i64,
    deduction: i64,
}
/// Each transaction removes one bounded chunk and commits durable progress. All
/// normal Behavior mutations pause while the job owns the score scope.
pub async fn run_cleanup_jobs(pool: &PgPool, limit: u32) -> Result<bool, AppError> {
    if limit == 0 || limit > 32 {
        return Err(invalid("จำนวนชุดล้างต้องอยู่ระหว่าง 1 ถึง 32"));
    }
    for _ in 0..limit {
        let mut tx =
            school_tenancy::TenantPool::begin_observed(&pool, "behavior.cleanup.run_cleanup_jobs")
                .await?;
        super::shared::raw_lock(&mut tx).await?;
        let job:Option<CleanupJob>=DbSession::new(&mut *tx).query_as("behavior.services.cleanup.run_cleanup_jobs", "SELECT id,actor_id,scope,mode,reset,reason,students,entries_expected,entries_deleted,merit,deduction FROM behavior_cleanup_jobs WHERE status='pending' ORDER BY created_at,id LIMIT 1 FOR UPDATE").fetch_optional().await?;
        let Some(job) = job else {
            tx.commit().await?;
            return Ok(false);
        };
        let prefix = "DELETE FROM behavior_entries WHERE id IN(SELECT e.id FROM behavior_entries e WHERE e.total_id IN(SELECT t.id";
        let deleted = DbSession::new(&mut *tx)
            .prepared_query(
                "behavior.services.cleanup.run_cleanup_jobs",
                selection(
                    &job.scope.0,
                    job.reset,
                    prefix,
                    ") ORDER BY e.id LIMIT 5000)",
                )
                .build(),
            )
            .execute()
            .await?
            .rows_affected() as i64;
        let total_deleted = job.entries_deleted + deleted;
        DbSession::new(&mut *tx)
            .query(
                "behavior.services.cleanup.run_cleanup_jobs",
                "UPDATE behavior_cleanup_jobs SET entries_deleted=$2 WHERE id=$1",
            )
            .bind(job.id)
            .bind(total_deleted)
            .execute()
            .await?;
        if deleted < 5000 {
            if total_deleted != job.entries_expected {
                return Err(AppError::Conflict(
                    "จำนวนรายละเอียดที่ล้างไม่ตรงกับผลตรวจ จึงยังไม่ปิดงาน".into(),
                ));
            }
            let snapshot = Snapshot {
                checksum: String::new(),
                current: totals(job.merit, job.deduction),
                history: totals(0, 0),
                students: job.students,
                entries: job.entries_expected,
                records: 0,
            };
            finish_totals(&mut tx, &job.scope.0, job.mode, job.reset).await?;
            if job.reset {
                super::schedules::rotate(
                    &mut tx,
                    &job.scope.0.sides,
                    job.scope.0.end.ok_or_else(|| invalid("งานรีเซ็ตไม่มีวันสิ้นสุด"))?,
                    &job.reason,
                )
                .await?;
            }
            audit_completion(
                &mut tx,
                job.id,
                job.actor_id,
                &job.scope.0,
                job.mode,
                job.reset,
                &snapshot,
                &job.reason,
                total_deleted,
            )
            .await?;
            DbSession::new(&mut *tx).query("behavior.services.cleanup.run_cleanup_jobs", "UPDATE behavior_cleanup_jobs SET status='completed',completed_at=now() WHERE id=$1").bind(job.id).execute().await?;
        }
        tx.commit().await?;
    }
    let mut database_stage_0 = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "behavior.cleanup.run_cleanup_jobs",
    )
    .await?;
    Ok(DbSession::new(&mut *database_stage_0)
        .scalar(
            "behavior.services.cleanup.run_cleanup_jobs",
            "SELECT EXISTS(SELECT 1 FROM behavior_cleanup_jobs WHERE status='pending')",
        )
        .fetch_one()
        .await?)
}
pub async fn require_drained(connection: &mut PgConnection) -> Result<(), AppError> {
    super::shared::raw_lock(connection).await?;
    let busy: bool = DbSession::new(connection)
        .scalar(
            "behavior.services.cleanup.require_drained",
            "SELECT EXISTS(SELECT 1 FROM behavior_cleanup_jobs WHERE status='pending')",
        )
        .fetch_one()
        .await?;
    if busy {
        return Err(AppError::Conflict(
            "รอให้งานรีเซ็ตหรือล้างคะแนนเสร็จก่อนปิดภาคเรียน".into(),
        ));
    }
    Ok(())
}
pub async fn job(pool: &PgPool, id: Uuid) -> Result<BehaviorCleanupOutcome, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "behavior.job").await?;
    job_on_connection(&mut connection, id).await
}

pub async fn job_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<BehaviorCleanupOutcome, AppError> {
    let pending:Option<(String,i64,i64,i64,i64)>=DbSession::new(&mut *connection).query_as("behavior.services.cleanup.job_on_connection", "SELECT status,students,entries_deleted,merit,deduction FROM behavior_cleanup_jobs WHERE id=$1").bind(id).fetch_optional().await?;
    if let Some((status, students, entries_deleted, merit, deduction)) = pending {
        return Ok(BehaviorCleanupOutcome {
            id,
            status,
            students,
            entries_deleted,
            merit,
            deduction,
        });
    }
    let row: Option<(i64, i64, i64, i64)> = DbSession::new(&mut *connection)
        .query_as(
            "behavior.services.cleanup.job_on_connection",
            "SELECT students,entries,merit,deduction FROM behavior_cleanup_audits WHERE id=$1",
        )
        .bind(id)
        .fetch_optional()
        .await?;
    let (students, entries_deleted, merit, deduction) =
        row.ok_or_else(|| AppError::NotFound("ไม่พบงานล้างคะแนน".into()))?;
    Ok(BehaviorCleanupOutcome {
        id,
        status: "completed".into(),
        students,
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
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "behavior.cleanup.execute").await?;
    super::shared::raw_lock(&mut tx).await?;
    if let Some(result) = receipt(&mut tx, actor, p.request_id, "cleanup", &p).await? {
        tx.commit().await?;
        return Ok(result);
    }
    lock(&mut tx).await?;
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
    DbSession::new(&mut *tx)
        .query(
            "behavior.services.cleanup.execute",
            "DELETE FROM behavior_cleanup_previews WHERE token=$1",
        )
        .bind(p.token)
        .execute()
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
    let mut tx =
        school_tenancy::TenantPool::begin_observed(&pool, "behavior.cleanup.adjust_history")
            .await?;
    lock(&mut tx).await?;
    if let Some(result) = receipt(&mut tx, actor, p.request_id, "history", &p).await? {
        tx.commit().await?;
        return Ok(result);
    }
    let old:Option<(i64,i64,String)>=DbSession::new(&mut *tx).query_as("behavior.services.cleanup.adjust_history", "SELECT t.points,t.row_version,c.state FROM behavior_daily_totals t JOIN behavior_cycles c ON c.id=t.cycle_id WHERE t.id=$1 FOR UPDATE OF t").bind(p.total_id).fetch_optional().await?;
    let (points, version, state) = old.ok_or_else(|| AppError::NotFound("ไม่พบยอดประวัติ".into()))?;
    if state != "history" || version != p.row_version {
        return Err(AppError::Conflict("ยอดนี้ไม่ใช่ประวัติหรือเปลี่ยนแล้ว".into()));
    }
    let points = points
        .checked_add(p.delta)
        .filter(|n| *n >= 0)
        .ok_or_else(|| invalid("ปรับยอดแล้วคะแนนต้องไม่ติดลบ"))?;
    DbSession::new(&mut *tx)
        .query(
            "behavior.services.cleanup.adjust_history",
            "UPDATE behavior_daily_totals SET points=$2,row_version=row_version+1 WHERE id=$1",
        )
        .bind(p.total_id)
        .bind(points)
        .execute()
        .await?;
    let id:Uuid=DbSession::new(&mut *tx).scalar("behavior.services.cleanup.adjust_history", "INSERT INTO behavior_history_adjustments(total_id,delta,reason,actor_id)VALUES($1,$2,$3,$4)RETURNING id").bind(p.total_id).bind(p.delta).bind(&p.reason).bind(actor).fetch_one().await?;
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
        assert!(
            normalize_scope(BehaviorCleanupScope {
                start: None,
                end: None,
                academic_year_id: None,
                academic_term_id: None,
                sides: vec![],
                sources: vec![BehaviorSource::Manual]
            })
            .is_err()
        );
    }
}
