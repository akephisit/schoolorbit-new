//! Versioned policy and resumable orchestration. Domain providers own retained facts and deletion.
use crate::models::*;
use chrono::{DateTime, Duration, Utc};
use school_academic_core::services::{lifecycle_guard, years_terms};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::{types::Json, PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[async_trait::async_trait]
pub trait AcademicRetentionPort: Send + Sync {
    async fn impact(
        &self,
        connection: &mut PgConnection,
        term: Uuid,
    ) -> Result<AcademicRetentionImpact, AppError>;
    async fn advance(&self, pool: &PgPool, job: &RetentionJob) -> Result<(), AppError>;
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RetentionJob {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub close_request_id: Uuid,
    pub policy_revision: i64,
    pub authorized_by: Uuid,
    pub status: String,
    pub evidence_file_ids: Vec<Uuid>,
    pub lease_token: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct TermRetentionSource {
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    term_status: String,
    year_status: String,
    close_request_id: Option<Uuid>,
    closed_at: Option<DateTime<Utc>>,
    year_closed_at: Option<DateTime<Utc>>,
    level_types: Vec<String>,
}

const POLICY_COLUMNS: &str =
    "enabled,primary_boundary,secondary_boundary,grace_days,effective_from,row_version,updated_by";
const SOURCE_SQL: &str = r#"
 SELECT t.id academic_term_id,t.academic_year_id,t.status term_status,y.status year_status,
   closed.request_id close_request_id,closed.created_at closed_at,year_closed.created_at year_closed_at,
   ARRAY(SELECT DISTINCT COALESCE(g.level_type,'unknown')::text FROM student_academic_years s
     JOIN grade_levels g ON g.id=s.grade_level_id WHERE s.academic_year_id=y.id ORDER BY 1) level_types
 FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id
 LEFT JOIN LATERAL(SELECT request_id,created_at FROM academic_term_transition_receipts
   WHERE academic_term_id=t.id AND action='close' ORDER BY created_at DESC,request_id DESC LIMIT 1) closed ON true
 LEFT JOIN LATERAL(SELECT created_at FROM academic_year_transition_receipts
   WHERE academic_year_id=y.id AND action='close' ORDER BY created_at DESC,request_id DESC LIMIT 1) year_closed ON true
"#;

pub async fn policy(connection: &mut PgConnection) -> Result<AcademicRetentionPolicy, AppError> {
    Ok(DbSession::new(connection)
        .query_as(
            "academic_lifecycle.retention.policy",
            sqlx::AssertSqlSafe(format!(
                "SELECT {POLICY_COLUMNS} FROM academic_retention_policy WHERE singleton"
            )),
        )
        .fetch_one()
        .await?)
}

pub fn require_read(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_LIFECYCLE_READ_SCHOOL,
        codes::ATTENDANCE_DELETE_SCHOOL,
        codes::BEHAVIOR_CLEANUP_EXECUTE_SCHOOL,
    ])
}

pub async fn update_policy(
    pool: &PgPool,
    actor: &ActorContext,
    input: UpdateAcademicRetentionPolicy,
) -> Result<AcademicRetentionPolicy, AppError> {
    require_read(actor)?;
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL)?;
    if input.expected_row_version < 1 || !(1..=3650).contains(&input.grace_days) {
        return Err(AppError::ValidationError(
            "รุ่นการตั้งค่าหรือจำนวนวันเก็บรายละเอียดไม่ถูกต้อง".into(),
        ));
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "academic_lifecycle.retention.update")
            .await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    let before = policy(&mut tx).await?;
    let result: AcademicRetentionPolicy = DbSession::new(&mut *tx).query_as("academic_lifecycle.retention.update",
        sqlx::AssertSqlSafe(format!("UPDATE academic_retention_policy SET enabled=$1,primary_boundary=$2,secondary_boundary=$3,grace_days=$4,effective_from=CASE WHEN $1 AND NOT enabled THEN clock_timestamp() ELSE effective_from END,row_version=row_version+1,updated_by=$5,updated_at=now() WHERE singleton AND row_version=$6 RETURNING {POLICY_COLUMNS}")))
        .bind(input.enabled).bind(input.primary_boundary).bind(input.secondary_boundary).bind(input.grace_days)
        .bind(actor.user_id).bind(input.expected_row_version).fetch_optional().await?
        .ok_or_else(|| AppError::Conflict("นโยบายเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".into()))?;
    if !result.enabled {
        DbSession::new(&mut *tx).query("academic_lifecycle.retention.cancel_unstarted",
            "UPDATE academic_retention_jobs SET status='cancelled',lease_token=NULL,leased_until=NULL WHERE started_at IS NULL AND status='awaiting_attendance'")
            .execute().await?;
    }
    years_terms::append_audit(
        &mut tx,
        "academic.retention_policy.updated",
        "academic_retention_policy",
        Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            b"schoolorbit/academic-retention-policy",
        ),
        None,
        None,
        actor.user_id,
        (&before, &result),
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

fn eligibility(
    policy: &AcademicRetentionPolicy,
    source: &TermRetentionSource,
    now: DateTime<Utc>,
) -> (
    AcademicRetentionBoundary,
    Option<DateTime<Utc>>,
    Vec<String>,
) {
    let mut reasons = Vec::new();
    if !policy.enabled {
        reasons.push("policy_disabled".into());
    }
    if source.term_status != "closed" {
        reasons.push("term_not_closed".into());
    }
    if source.closed_at.is_none() {
        reasons.push("close_receipt_missing".into());
    }
    if source.level_types.is_empty()
        || source
            .level_types
            .iter()
            .any(|level| !matches!(level.as_str(), "primary" | "secondary"))
    {
        reasons.push("education_level_unknown".into());
    }
    let required = if source.level_types.iter().any(|level| match level.as_str() {
        "primary" => policy.primary_boundary == AcademicRetentionBoundary::Year,
        "secondary" => policy.secondary_boundary == AcademicRetentionBoundary::Year,
        _ => true,
    }) {
        AcademicRetentionBoundary::Year
    } else {
        AcademicRetentionBoundary::Term
    };
    if source
        .closed_at
        .zip(policy.effective_from)
        .is_none_or(|(closed, effective)| closed <= effective)
    {
        reasons.push("outside_policy_effective_period".into());
    }
    let boundary = if required == AcademicRetentionBoundary::Year {
        if !matches!(source.year_status.as_str(), "closed" | "archived")
            || source.year_closed_at.is_none()
        {
            reasons.push("year_not_closed".into());
            None
        } else {
            source
                .year_closed_at
                .zip(source.closed_at)
                .map(|(year, term)| year.max(term))
        }
    } else {
        source.closed_at
    };
    let eligible_at = boundary
        .and_then(|date| date.checked_add_signed(Duration::days(i64::from(policy.grace_days))));
    if eligible_at.is_some_and(|date| date > now) {
        reasons.push("grace_period".into());
    }
    (required, eligible_at, reasons)
}

pub async fn get_workspace(
    external: &dyn AcademicRetentionPort,
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
) -> Result<AcademicRetentionWorkspace, AppError> {
    require_read(actor)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "academic_lifecycle.retention.workspace")
            .await?;
    DbSession::new(&mut *tx)
        .query(
            "academic_lifecycle.retention.snapshot",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let policy = policy(&mut tx).await?;
    let sources: Vec<TermRetentionSource> = DbSession::new(&mut *tx)
        .query_as(
            "academic_lifecycle.retention.sources",
            sqlx::AssertSqlSafe(format!(
                "{SOURCE_SQL} WHERE y.id=$1 ORDER BY t.sequence_no,t.id LIMIT 101"
            )),
        )
        .bind(year)
        .fetch_all()
        .await?;
    if sources.len() > 100 {
        return Err(AppError::Conflict("จำนวนภาคเรียนเกินขอบเขตการแสดงผล".into()));
    }
    let now = Utc::now();
    let mut terms = Vec::with_capacity(sources.len());
    for source in sources {
        let (required_boundary, eligible_at, mut blocked_reasons) =
            eligibility(&policy, &source, now);
        let impact = external.impact(&mut tx, source.academic_term_id).await?;
        let job:Option<AcademicRetentionJobStatus> = DbSession::new(&mut *tx).query_as("academic_lifecycle.retention.job_status",
            "SELECT id,status,status IN('awaiting_files','completed') logical_cleanup_complete,status='completed' physical_cleanup_complete,pending_files,failed_files,last_error_code FROM academic_retention_jobs WHERE academic_term_id=$1 ORDER BY created_at DESC,id DESC LIMIT 1")
            .bind(source.academic_term_id).fetch_optional().await?;
        if impact.attendance_archived
            && job.as_ref().is_none_or(|job| {
                matches!(job.status.as_str(), "awaiting_attendance" | "cancelled")
            })
        {
            blocked_reasons.push("attendance_already_archived".into());
        }
        terms.push(AcademicRetentionTermPreview {
            academic_term_id: source.academic_term_id,
            academic_year_id: source.academic_year_id,
            level_types: source.level_types,
            required_boundary,
            closed_at: source.closed_at,
            eligible_at,
            eligible: blocked_reasons.is_empty(),
            blocked_reasons,
            impact,
            preserves_attendance_summaries: true,
            preserves_behavior_totals: true,
            job,
        });
    }
    tx.commit().await?;
    Ok(AcademicRetentionWorkspace {
        policy,
        terms,
        whole_term_cleanup: true,
    })
}

/// Called by the existing bounded tenant background turn. At most one period advances.
pub async fn run_due(
    external: &dyn AcademicRetentionPort,
    pool: &PgPool,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let mut enqueue_tx =
        school_tenancy::TenantPool::begin_observed(pool, "academic_lifecycle.retention.enqueue")
            .await?;
    enqueue_due(external, &mut enqueue_tx, now).await?;
    enqueue_tx.commit().await?;
    let lease = Uuid::now_v7();
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "academic_lifecycle.retention.claim")
            .await?;
    let job:Option<RetentionJob> = DbSession::new(&mut *tx).query_as("academic_lifecycle.retention.claim", r#"
        UPDATE academic_retention_jobs SET lease_token=$1,leased_until=now()+interval '5 minutes',attempt_count=attempt_count+1
        WHERE id=(SELECT id FROM academic_retention_jobs WHERE status NOT IN('completed','cancelled')
            AND next_attempt_at<=$2 AND (leased_until IS NULL OR leased_until<now())
            ORDER BY next_attempt_at,created_at,id LIMIT 1 FOR UPDATE SKIP LOCKED)
        RETURNING id,academic_term_id,academic_year_id,close_request_id,policy_revision,authorized_by,status,evidence_file_ids,lease_token
    "#).bind(lease).bind(now).fetch_optional().await?;
    tx.commit().await?;
    let Some(job) = job else {
        return Ok(());
    };
    if let Err(error) = external.advance(pool, &job).await {
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "academic_lifecycle.retention.retry",
        )
        .await?;
        DbSession::new(&mut connection).query("academic_lifecycle.retention.retry",
            "UPDATE academic_retention_jobs SET lease_token=NULL,leased_until=NULL,next_attempt_at=now()+interval '5 minutes',last_error_code='academic_retention_retry' WHERE id=$1 AND lease_token=$2")
            .bind(job.id).bind(job.lease_token).execute().await?;
        return Err(error);
    }
    Ok(())
}

async fn enqueue_due(
    external: &dyn AcademicRetentionPort,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    lifecycle_guard::lock_transition_shared(tx).await?;
    let policy = policy(tx).await?;
    DbSession::new(&mut **tx).query("academic_lifecycle.retention.cancel_stale",r#"
        UPDATE academic_retention_jobs SET status='cancelled',lease_token=NULL,leased_until=NULL
        WHERE id IN(SELECT j.id FROM academic_retention_jobs j JOIN academic_terms t ON t.id=j.academic_term_id
            WHERE j.started_at IS NULL AND j.status='awaiting_attendance' AND (t.status<>'closed'
                OR j.close_request_id IS DISTINCT FROM(SELECT r.request_id FROM academic_term_transition_receipts r
                    WHERE r.academic_term_id=t.id AND r.action='close' ORDER BY r.created_at DESC,r.request_id DESC LIMIT 1))
            ORDER BY j.created_at,j.id LIMIT 64)
    "#).execute().await?;
    if !policy.enabled {
        return Ok(());
    }
    if !authorizer_is_current(tx, policy.updated_by).await? {
        return Ok(());
    }
    // A persisted keyset advances past blocked/not-yet-due terms. Wrap only after
    // the end, so more than 64 blocked periods cannot starve a later eligible one.
    let (after_date,after_id):(Option<DateTime<Utc>>,Option<Uuid>)=DbSession::new(&mut **tx)
        .query_as("academic_lifecycle.retention.scan_cursor","SELECT after_closed_at,after_request_id FROM academic_retention_scan_state WHERE singleton FOR UPDATE")
        .fetch_one().await?;
    let boundaries:Vec<(Uuid,DateTime<Utc>,Uuid)>=DbSession::new(&mut **tx)
        .query_as("academic_lifecycle.retention.scan_boundaries",r#"
            SELECT r.academic_term_id,r.created_at,r.request_id FROM academic_term_transition_receipts r
            WHERE r.action='close' AND r.created_at>$1
                AND ($2::timestamptz IS NULL OR (r.created_at,r.request_id)>($2,$3::uuid))
            ORDER BY r.created_at,r.request_id LIMIT 64
        "#).bind(policy.effective_from).bind(after_date).bind(after_id).fetch_all().await?;
    let next = if boundaries.len() == 64 {
        boundaries.last().map(|(_, date, id)| (*date, *id))
    } else {
        None
    };
    DbSession::new(&mut **tx).query("academic_lifecycle.retention.advance_cursor",
        "UPDATE academic_retention_scan_state SET after_closed_at=$1,after_request_id=$2 WHERE singleton")
        .bind(next.map(|(date,_)|date)).bind(next.map(|(_,id)|id)).execute().await?;
    let term_ids: Vec<Uuid> = boundaries.iter().map(|(term, _, _)| *term).collect();
    let sources:Vec<TermRetentionSource>=DbSession::new(&mut **tx).query_as("academic_lifecycle.retention.due_sources",
        sqlx::AssertSqlSafe(format!("{SOURCE_SQL} WHERE t.id=ANY($1) AND t.status='closed' AND NOT EXISTS(SELECT 1 FROM academic_retention_jobs j WHERE j.academic_term_id=t.id AND (j.started_at IS NOT NULL OR j.status='completed' OR (j.close_request_id=closed.request_id AND j.policy_revision=$2 AND j.status<>'cancelled'))) ORDER BY closed.created_at,t.id")))
        .bind(&term_ids).bind(policy.row_version).fetch_all().await?;
    for source in sources {
        let (_, eligible_at, reasons) = eligibility(&policy, &source, now);
        if !reasons.is_empty() {
            continue;
        }
        if external
            .impact(tx, source.academic_term_id)
            .await?
            .attendance_archived
        {
            continue;
        }
        let (Some(close), Some(eligible), Some(actor)) =
            (source.close_request_id, eligible_at, policy.updated_by)
        else {
            continue;
        };
        DbSession::new(&mut **tx).query("academic_lifecycle.retention.enqueue_job",r#"
            INSERT INTO academic_retention_jobs(id,academic_term_id,academic_year_id,close_request_id,policy_revision,policy_snapshot,authorized_by,eligible_at,next_attempt_at)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)
            ON CONFLICT(academic_term_id,close_request_id) DO UPDATE SET policy_revision=EXCLUDED.policy_revision,
                policy_snapshot=EXCLUDED.policy_snapshot,authorized_by=EXCLUDED.authorized_by,eligible_at=EXCLUDED.eligible_at,
                status='awaiting_attendance',next_attempt_at=EXCLUDED.next_attempt_at,last_error_code=NULL
            WHERE academic_retention_jobs.started_at IS NULL AND academic_retention_jobs.lease_token IS NULL
        "#).bind(Uuid::now_v7()).bind(source.academic_term_id).bind(source.academic_year_id).bind(close)
            .bind(policy.row_version).bind(Json(&policy)).bind(actor).bind(eligible).bind(now).execute().await?;
    }
    Ok(())
}

/// Application takes module -> lifecycle -> term locks before this row lock.
/// Before the first deletion, recheck policy, classification, source receipt and deadline.
pub async fn validate_job_in_transaction(
    connection: &mut PgConnection,
    job: &RetentionJob,
) -> Result<(), AppError> {
    let row:Option<(Option<DateTime<Utc>>,String)>=DbSession::new(connection).query_as("academic_lifecycle.retention.validate_job",
        "SELECT started_at,status FROM academic_retention_jobs WHERE id=$1 AND lease_token=$2 FOR UPDATE")
        .bind(job.id).bind(job.lease_token).fetch_optional().await?;
    let Some((started, status)) = row else {
        return Err(AppError::Conflict("งานเก็บสรุปเปลี่ยนแล้ว".into()));
    };
    if status != job.status {
        return Err(AppError::Conflict("ขั้นตอนเก็บสรุปเปลี่ยนแล้ว".into()));
    }
    let source: TermRetentionSource = DbSession::new(connection)
        .query_as(
            "academic_lifecycle.retention.recheck_source",
            sqlx::AssertSqlSafe(format!("{SOURCE_SQL} WHERE t.id=$1")),
        )
        .bind(job.academic_term_id)
        .fetch_one()
        .await?;
    if source.term_status != "closed" || source.close_request_id != Some(job.close_request_id) {
        return Err(AppError::Conflict(
            "ภาคเรียนเปิดกลับหรือปิดใหม่แล้ว ต้องตรวจอายุรายละเอียดอีกครั้ง".into(),
        ));
    }
    if started.is_none() {
        let current = policy(connection).await?;
        let (_, _, reasons) = eligibility(&current, &source, Utc::now());
        if current.row_version != job.policy_revision || !reasons.is_empty() {
            return Err(AppError::Conflict(
                "นโยบายหรือกำหนดเก็บรายละเอียดเปลี่ยนแล้ว".into(),
            ));
        }
        if !authorizer_is_current(connection, Some(job.authorized_by)).await? {
            return Err(AppError::Forbidden(
                "ผู้อนุมัตินโยบายไม่มีสิทธิ์ปัจจุบันสำหรับเริ่มเก็บสรุป".into(),
            ));
        }
    }
    Ok(())
}

pub async fn attendance_accepted(
    connection: &mut PgConnection,
    job: &RetentionJob,
    files: &[Uuid],
) -> Result<(), AppError> {
    DbSession::new(connection).query("academic_lifecycle.retention.attendance_accepted",
        "UPDATE academic_retention_jobs SET started_at=COALESCE(started_at,now()),evidence_file_ids=$3,pending_files=cardinality($3),status='awaiting_behavior',lease_token=NULL,leased_until=NULL,next_attempt_at=now(),last_error_code=NULL WHERE id=$1 AND lease_token=$2")
        .bind(job.id).bind(job.lease_token).bind(files).execute().await?;
    Ok(())
}

pub async fn behavior_accepted(
    connection: &mut PgConnection,
    job: &RetentionJob,
    outcome_id: Uuid,
    completed: bool,
) -> Result<(), AppError> {
    DbSession::new(connection).query("academic_lifecycle.retention.behavior_accepted",
        "UPDATE academic_retention_jobs SET behavior_job_id=$3,status=CASE WHEN $4 THEN 'awaiting_files' ELSE 'awaiting_behavior' END,lease_token=NULL,leased_until=NULL,next_attempt_at=now()+interval '15 seconds',last_error_code=NULL WHERE id=$1 AND lease_token=$2")
        .bind(job.id).bind(job.lease_token).bind(outcome_id).bind(completed).execute().await?;
    Ok(())
}

pub async fn files_reconciled(
    connection: &mut PgConnection,
    job: &RetentionJob,
    pending: i64,
    failed: i64,
) -> Result<(), AppError> {
    let complete = pending == 0 && failed == 0;
    DbSession::new(connection).query("academic_lifecycle.retention.files_reconciled",
        "UPDATE academic_retention_jobs SET pending_files=$3,failed_files=$4,status=CASE WHEN $5 THEN 'completed' ELSE 'awaiting_files' END,completed_at=CASE WHEN $5 THEN now() ELSE NULL END,lease_token=NULL,leased_until=NULL,next_attempt_at=now()+interval '5 minutes',last_error_code=CASE WHEN $4>0 THEN 'file_deletion_failed' ELSE NULL END WHERE id=$1 AND lease_token=$2")
        .bind(job.id).bind(job.lease_token).bind(pending).bind(failed).bind(complete).execute().await?;
    Ok(())
}

pub async fn require_reopen_allowed(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    term: Option<Uuid>,
) -> Result<(), AppError> {
    let started:bool=DbSession::new(&mut **tx).scalar("academic_lifecycle.retention.reopen_guard",
        "SELECT EXISTS(SELECT 1 FROM academic_retention_jobs WHERE academic_year_id=$1 AND ($2::uuid IS NULL OR academic_term_id=$2) AND started_at IS NOT NULL)")
        .bind(year).bind(term).fetch_one().await?;
    if started {
        return Err(AppError::Conflict(
            "รายละเอียดของช่วงนี้เริ่มเก็บเป็นสรุปแล้ว จึงเปิดกลับเพื่อแก้รายละเอียดไม่ได้".into(),
        ));
    }
    Ok(())
}

async fn authorizer_is_current(
    connection: &mut PgConnection,
    actor: Option<Uuid>,
) -> Result<bool, AppError> {
    let Some(actor) = actor else {
        return Ok(false);
    };
    let users = school_authorization::staff_users_with_all_permissions_on_connection(
        connection,
        &[
            codes::ACADEMIC_LIFECYCLE_READ_SCHOOL,
            codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
            codes::ATTENDANCE_DELETE_SCHOOL,
            codes::BEHAVIOR_CLEANUP_EXECUTE_SCHOOL,
        ],
    )
    .await?;
    Ok(users.contains(&actor))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (AcademicRetentionPolicy, TermRetentionSource, DateTime<Utc>) {
        let closed = DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        (
            AcademicRetentionPolicy {
                enabled: true,
                primary_boundary: AcademicRetentionBoundary::Year,
                secondary_boundary: AcademicRetentionBoundary::Term,
                grace_days: 90,
                effective_from: Some(closed - Duration::days(1)),
                row_version: 1,
                updated_by: Some(Uuid::nil()),
            },
            TermRetentionSource {
                academic_term_id: Uuid::nil(),
                academic_year_id: Uuid::nil(),
                term_status: "closed".into(),
                year_status: "active".into(),
                close_request_id: Some(Uuid::nil()),
                closed_at: Some(closed),
                year_closed_at: None,
                level_types: vec!["secondary".into()],
            },
            closed,
        )
    }
    #[test]
    fn secondary_waits_full_grace_and_historical_closures_are_excluded() {
        let (mut policy, source, closed) = fixture();
        assert_eq!(
            eligibility(&policy, &source, closed + Duration::days(89)).2,
            vec!["grace_period"]
        );
        assert!(eligibility(&policy, &source, closed + Duration::days(90))
            .2
            .is_empty());
        policy.effective_from = Some(closed);
        assert!(eligibility(&policy, &source, closed + Duration::days(100))
            .2
            .contains(&"outside_policy_effective_period".into()));
    }
    #[test]
    fn mixed_rosters_wait_for_actual_year_close_and_unknown_levels_block() {
        let (policy, mut source, closed) = fixture();
        source.level_types.push("primary".into());
        assert!(eligibility(&policy, &source, closed + Duration::days(300))
            .2
            .contains(&"year_not_closed".into()));
        source.year_status = "closed".into();
        source.year_closed_at = Some(closed + Duration::days(120));
        assert_eq!(
            eligibility(&policy, &source, closed + Duration::days(210)).1,
            Some(closed + Duration::days(210))
        );
        assert!(eligibility(&policy, &source, closed + Duration::days(210))
            .2
            .is_empty());
        source.level_types.push("other".into());
        assert!(eligibility(&policy, &source, closed + Duration::days(300))
            .2
            .contains(&"education_level_unknown".into()));
    }
    #[test]
    fn disabled_policy_never_becomes_eligible() {
        let (mut policy, source, closed) = fixture();
        policy.enabled = false;
        assert!(eligibility(&policy, &source, closed + Duration::days(100))
            .2
            .contains(&"policy_disabled".into()));
    }
}

/// A manual Attendance archive has no recoverable physical deletion manifest.
/// Cancel only this leased, unstarted job; never claim physical completion.
pub async fn cancel_unstarted_job(
    connection: &mut PgConnection,
    job: &RetentionJob,
) -> Result<(), AppError> {
    let changed=DbSession::new(connection).query("academic_lifecycle.retention.cancel_manual_archive",
        "UPDATE academic_retention_jobs SET status='cancelled',last_error_code='attendance_already_archived',lease_token=NULL,leased_until=NULL WHERE id=$1 AND lease_token=$2 AND started_at IS NULL")
        .bind(job.id).bind(job.lease_token).execute().await?;
    if changed.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "งานเก็บรายละเอียดเริ่มแล้วหรือเปลี่ยนรุ่นแล้ว".into(),
        ));
    }
    Ok(())
}
