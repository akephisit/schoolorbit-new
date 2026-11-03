//! Composition of Lifecycle policy with owned Attendance, Behavior and Files providers.
use chrono::Utc;
use school_academic_lifecycle::{
    models::AcademicRetentionImpact,
    services::retention::{self, AcademicRetentionPort, RetentionJob},
};
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub struct ApplicationAcademicRetention;
pub static ACADEMIC_RETENTION: ApplicationAcademicRetention = ApplicationAcademicRetention;

#[async_trait::async_trait]
impl AcademicRetentionPort for ApplicationAcademicRetention {
    async fn impact(
        &self,
        connection: &mut PgConnection,
        term: Uuid,
    ) -> Result<AcademicRetentionImpact, AppError> {
        let attendance =
            school_attendance::services::reports::impact_on_connection(connection, term).await?;
        let behavior_entries =
            school_behavior::services::retention::entry_count(connection, term).await?;
        Ok(AcademicRetentionImpact {
            attendance_records: attendance.records,
            evidence_files: attendance.evidence,
            evidence_bytes: attendance.evidence_bytes,
            behavior_entries,
            attendance_archived: attendance.archived,
        })
    }

    async fn advance(&self, pool: &PgPool, job: &RetentionJob) -> Result<(), AppError> {
        let mut tx =
            school_tenancy::TenantPool::begin_observed(pool, "school.academic_retention.advance")
                .await?;
        // Existing cross-domain order: module -> academic lifecycle -> term -> Behavior.
        school_attendance::services::lock_term(&mut tx, job.academic_term_id, false, true).await?;
        retention::validate_job_in_transaction(&mut tx, job).await?;
        match job.status.as_str() {
            "awaiting_attendance" => {
                let impact = school_attendance::services::reports::impact_on_connection(
                    &mut tx,
                    job.academic_term_id,
                )
                .await?;
                if impact.archived {
                    retention::cancel_unstarted_job(&mut tx, job).await?;
                    tx.commit().await?;
                    return Ok(());
                }
                let files = school_attendance::services::retention::evidence_manifest(
                    &mut tx,
                    job.academic_term_id,
                )
                .await?;
                school_file_platform::retention::require_retention_allowed(&mut tx, &files).await?;
                retention::attendance_accepted(&mut tx, job, &files).await?;
                // Commits source-boundary receipt, manifest, frozen summaries and deletion together.
                school_attendance::services::retention::archive_in_transaction(
                    pool,
                    tx,
                    job.authorized_by,
                    job.academic_term_id,
                )
                .await?;
            }
            "awaiting_behavior" => {
                school_attendance::services::score_changes::require_drained(
                    &mut tx,
                    job.academic_term_id,
                )
                .await?;
                let outcome = school_behavior::services::retention::retain_in_transaction(
                    &mut tx,
                    job.authorized_by,
                    job.id,
                    job.academic_term_id,
                )
                .await?;
                retention::behavior_accepted(
                    &mut tx,
                    job,
                    outcome.id,
                    outcome.status == "completed",
                )
                .await?;
                tx.commit().await?;
            }
            "awaiting_files" => {
                let progress = school_file_platform::retention::deletion_progress(
                    &mut tx,
                    &job.evidence_file_ids,
                )
                .await?;
                retention::files_reconciled(&mut tx, job, progress.pending, progress.failed)
                    .await?;
                tx.commit().await?;
            }
            _ => return Err(AppError::Conflict("สถานะงานเก็บสรุปไม่รองรับการทำงานนี้".into())),
        }
        Ok(())
    }
}

/// Invoked from the existing Attendance/Behavior background turn, including idle schools.
pub async fn run(pool: &PgPool) -> Result<(), AppError> {
    retention::run_due(&ACADEMIC_RETENTION, pool, Utc::now()).await
}
