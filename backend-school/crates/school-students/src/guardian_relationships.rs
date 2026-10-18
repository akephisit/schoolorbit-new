//! Minimal relationship authority for confidential response workflows.
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;
/// Canonical current guardian authority, for set-based consumer scope predicates.
///
/// This server-owned relation exposes only relationship and subject IDs. Keep the
/// active account/type criteria here so lists, aggregates and direct saves agree.
pub const ACTIVE_GUARDIAN_RELATIONSHIPS_SQL: &str = "SELECT p.id AS relationship_id,p.student_user_id AS student_id,p.parent_user_id AS guardian_id FROM student_parents p JOIN users parent ON parent.id=p.parent_user_id AND parent.user_type='parent' AND parent.status='active' JOIN users child ON child.id=p.student_user_id AND child.user_type='student' AND child.status='active'";

/// Lock the accepted relationship row until the caller's business save commits.
pub async fn require_active_guardian_on_connection(
    connection: &mut PgConnection,
    student: Uuid,
    guardian: Uuid,
) -> Result<(), AppError> {
    let query = format!("SELECT relationship_id FROM ({ACTIVE_GUARDIAN_RELATIONSHIPS_SQL} WHERE p.student_user_id=$1 AND p.parent_user_id=$2 FOR SHARE OF p,parent,child) current_relationship");
    let row = sqlx::query_scalar::<_, Uuid>(sqlx::AssertSqlSafe(query))
        .bind(student)
        .bind(guardian)
        .fetch_optional(connection)
        .await?;
    if row.is_some() {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่พบความสัมพันธ์ผู้ปกครองที่ใช้งานอยู่".into()))
    }
}
