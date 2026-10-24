//! Tenant-local module reset. The authenticated adapter owns tenant selection.
use crate::{models::*, rules::invalid};
use school_database::DbSession;
use school_errors::AppError;
use school_file_platform::repository::{DeleteWork, SqlFileRepository};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
const TABLES: &[&str] = &[
    "attendance_scan_events",
    "attendance_records",
    "attendance_sessions",
    "attendance_notifications",
    "attendance_term_teacher_summaries",
    "attendance_term_summaries",
    "attendance_term_archives",
    "attendance_special_rounds",
    "attendance_special_templates",
    "attendance_audience_groups",
    "attendance_days",
    "attendance_settings",
    "attendance_face_enrollments",
    "attendance_devices",
    "attendance_finalized_dates",
];
pub struct AttendanceResetOutcome {
    pub impact: AttendanceResetImpact,
    pub prepared_deletes: Vec<DeleteWork>,
}
async fn snapshot(tx: &mut Transaction<'_, Postgres>) -> Result<AttendanceResetImpact, AppError> {
    let (generation, coverage_start): (Uuid, chrono::NaiveDate) = DbSession::new(&mut **tx)
        .query_as(
            "attendance.services.reset.snapshot",
            "SELECT generation,coverage_start FROM attendance_module_state WHERE singleton",
        )
        .fetch_one()
        .await?;
    let mut counts = Vec::new();
    let mut checksums = Vec::new();
    for table in TABLES {
        let sql = format!(
            "SELECT count(*)::bigint,COALESCE(md5(string_agg(md5(to_jsonb(t)::text),'' ORDER BY md5(to_jsonb(t)::text))),'') FROM {table} t"
        );
        let (count, checksum): (i64, String) = DbSession::new(&mut **tx)
            .query_as(
                "attendance.services.reset.snapshot",
                sqlx::AssertSqlSafe(sql),
            )
            .fetch_one()
            .await?;
        counts.push(AttendanceResetCount {
            resource: (*table).into(),
            count,
        });
        checksums.push(checksum);
    }
    let (evidence_files,pending_file_deletions,evidence_bytes,file_checksum):(i64,i64,i64,String)=DbSession::new(&mut **tx).query_as("attendance.services.reset.snapshot", "SELECT count(*) FILTER(WHERE f.lifecycle_status NOT IN ('deleted','delete_requested')),count(*) FILTER(WHERE f.lifecycle_status='delete_requested'),COALESCE(sum(v.byte_size),0)::bigint,COALESCE(md5(string_agg(md5(concat_ws(':',f.id,f.lifecycle_status,f.updated_at)),'' ORDER BY f.id)),'') FROM files f LEFT JOIN LATERAL (SELECT sum(byte_size)::bigint byte_size FROM file_versions WHERE file_id=f.id AND storage_status<>'deleted') v ON true WHERE f.purpose_code='attendance_evidence' AND f.lifecycle_status<>'deleted'").fetch_one().await?;
    let bytes = serde_json::to_vec(&(
        generation,
        coverage_start,
        &counts,
        checksums,
        file_checksum,
    ))
    .map_err(|_| invalid("อ่านผลกระทบการล้างไม่ได้"))?;
    Ok(AttendanceResetImpact {
        generation,
        coverage_start,
        counts,
        evidence_files,
        evidence_bytes,
        pending_file_deletions,
        token: hex::encode(Sha256::digest(bytes)),
    })
}
pub async fn impact(pool: &PgPool) -> Result<AttendanceResetImpact, AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.reset.transaction").await?;
    DbSession::new(&mut *tx)
        .query(
            "attendance.services.reset.impact",
            "SELECT pg_advisory_xact_lock(17010000)",
        )
        .execute()
        .await?;
    let impact = snapshot(&mut tx).await?;
    tx.commit().await?;
    Ok(impact)
}
pub async fn reset_in_transaction(
    pool: &PgPool,
    mut tx: school_tenancy::ObservedTransaction,
    actor: Uuid,
    p: ResetAttendanceModule,
) -> Result<AttendanceResetOutcome, AppError> {
    if p.reason.trim().is_empty() || p.reason.len() > 1000 {
        return Err(invalid("ระบุเหตุผลการล้างระบบเช็คชื่อ"));
    }
    DbSession::new(&mut *tx)
        .query(
            "attendance.services.reset.reset_in_transaction",
            "SELECT pg_advisory_xact_lock(17010000)",
        )
        .execute()
        .await?;
    let before = snapshot(&mut tx).await?;
    if before.generation != p.generation || before.token != p.token {
        return Err(AppError::Conflict("ข้อมูลเปลี่ยนแล้ว กรุณาตรวจผลกระทบใหม่".into()));
    }
    // Keep old-generation reset intents until the consumer reconciles its prior
    // effects. Finalized dates also cover source details already retained as archives.
    DbSession::new(&mut *tx).query("attendance.services.reset.reset_in_transaction", "INSERT INTO attendance_score_changes(generation,academic_term_id,date,reset) SELECT $1,academic_term_id,date,true FROM (SELECT academic_term_id,date FROM attendance_sessions UNION SELECT academic_term_id,date FROM attendance_finalized_dates WHERE generation=$1 UNION SELECT academic_term_id,date FROM attendance_score_changes WHERE generation=$1) days ON CONFLICT(generation,academic_term_id,date) DO UPDATE SET row_version=nextval('attendance_score_revision_seq'),queued_at=now(),reset=true")
        .bind(before.generation).execute().await?;
    let ids:Vec<Uuid>=DbSession::new(&mut *tx).scalar("attendance.services.reset.reset_in_transaction", "SELECT id FROM files WHERE purpose_code='attendance_evidence' AND lifecycle_status<>'deleted' ORDER BY id FOR UPDATE").fetch_all().await?;
    let files = SqlFileRepository::new(pool.clone());
    let mut prepared_deletes = Vec::new();
    for id in ids {
        let work = files
            .request_delete_in_transaction(&mut tx, id)
            .await
            .map_err(|_| AppError::ServiceUnavailable("เตรียมล้างภาพไม่ได้ จึงยังไม่ล้างข้อมูล".into()))?;
        // Bound immediate provider work; every remaining object is already queued durably.
        prepared_deletes.extend(work.into_iter().take(50 - prepared_deletes.len()));
    }
    for table in TABLES {
        let sql = format!("DELETE FROM {table}");
        DbSession::new(&mut *tx)
            .query(
                "attendance.services.reset.reset_in_transaction",
                sqlx::AssertSqlSafe(sql),
            )
            .execute()
            .await?;
    }
    let generation = Uuid::new_v4();
    DbSession::new(&mut *tx).query("attendance.services.reset.reset_in_transaction", "UPDATE attendance_module_state SET generation=$1,coverage_start=(now() AT TIME ZONE 'Asia/Bangkok')::date,auto_finalize_from=(now() AT TIME ZONE 'Asia/Bangkok')::date WHERE singleton").bind(generation).execute().await?;
    DbSession::new(&mut *tx).query("attendance.services.reset.reset_in_transaction", "INSERT INTO audit_logs(user_id,action,entity_type,entity_id,old_values,new_values,description) VALUES($1,'delete','attendance',$2,$3,$4,$5)").bind(actor).bind(generation).bind(sqlx::types::Json(&before)).bind(sqlx::types::Json(serde_json::json!({"generation":generation}))).bind(p.reason.trim()).execute().await?;
    let after = snapshot(&mut tx).await?;
    tx.commit().await?;
    Ok(AttendanceResetOutcome {
        impact: after,
        prepared_deletes,
    })
}
