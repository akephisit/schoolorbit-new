use super::super::models::{OpeningPolicy, UpdateOpeningPolicyInput};
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::core::services::{lifecycle_guard, years_terms},
    permissions::registry::codes,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const COLUMNS: &str = "row_version,require_homeroom_placements,require_published_offerings,require_published_timetable";

pub async fn get_opening_policy(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<OpeningPolicy, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    let mut tx = pool.begin().await?;
    let result = read_in_transaction(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) async fn read_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<OpeningPolicy, AppError> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM academic_opening_policy WHERE id=1"
    ))
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::InternalServerError("ไม่พบการตั้งค่าเงื่อนไขเปิดภาคเรียน".into()))
}

pub async fn update_opening_policy(
    pool: &PgPool,
    actor: &ActorContext,
    input: UpdateOpeningPolicyInput,
) -> Result<OpeningPolicy, AppError> {
    actor.require_all_permissions(&[
        codes::ACADEMIC_LIFECYCLE_READ_SCHOOL,
        codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
    ])?;
    if input.row_version <= 0 {
        return Err(AppError::ValidationError("รุ่นการตั้งค่าไม่ถูกต้อง".into()));
    }
    let mut tx = pool.begin().await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    let before = read_in_transaction(&mut tx).await?;
    let result: OpeningPolicy = sqlx::query_as(&format!(
        "UPDATE academic_opening_policy SET require_homeroom_placements=$1,require_published_offerings=$2,require_published_timetable=$3,row_version=row_version+1,updated_by=$4,updated_at=now() WHERE id=1 AND row_version=$5 RETURNING {COLUMNS}"
    )).bind(input.require_homeroom_placements).bind(input.require_published_offerings).bind(input.require_published_timetable)
        .bind(actor.user_id).bind(input.row_version).fetch_optional(&mut *tx).await?
        .ok_or_else(|| AppError::Conflict("เงื่อนไขเปิดภาคเรียนเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".into()))?;
    years_terms::append_audit(
        &mut tx,
        "academic.opening_policy.updated",
        "academic_opening_policy",
        Uuid::new_v5(&Uuid::NAMESPACE_URL, b"schoolorbit/academic-opening-policy"),
        None,
        None,
        actor.user_id,
        (&before, &result),
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}
