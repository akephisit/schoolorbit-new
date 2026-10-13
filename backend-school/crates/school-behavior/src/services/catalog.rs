use super::shared::{bump, lock, raw_lock, receipt, save_receipt, today};
use crate::{
    access::BehaviorAccess,
    models::*,
    rules::{invalid, validate_score, validate_status},
};
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub async fn list_items(
    pool: &PgPool,
    query: BehaviorItemsQuery,
) -> Result<Vec<BehaviorItem>, AppError> {
    Ok(sqlx::query_as("SELECT i.id,i.name,i.side,i.enabled,i.row_version,v.id version_id,v.effective_from,v.points FROM behavior_items i LEFT JOIN LATERAL(SELECT id,effective_from,points FROM behavior_item_versions WHERE item_id=i.id AND effective_from<=$1 ORDER BY effective_from DESC,id DESC LIMIT 1)v ON true WHERE($2 OR i.enabled)ORDER BY i.side,i.name,i.id LIMIT 1001").bind(query.date.unwrap_or_else(today)).bind(query.include_disabled.unwrap_or(false)).fetch_all(pool).await?)
}
pub async fn save_item(
    pool: &PgPool,
    actor: Uuid,
    mut p: SaveBehaviorItem,
) -> Result<BehaviorItem, AppError> {
    if p.name.trim().is_empty() || p.name.chars().count() > 200 || !(1..=100000).contains(&p.points)
    {
        return Err(invalid("ชื่อรายการหรือคะแนนไม่ถูกต้อง"));
    }
    p.name = p.name.trim().to_string();
    let mut tx = pool.begin().await?;
    raw_lock(&mut tx).await?;
    if let Some(previous) =
        receipt::<_, BehaviorItem>(&mut tx, actor, p.request_id, "item", &p).await?
    {
        tx.commit().await?;
        return Ok(previous);
    }
    lock(&mut tx).await?;
    let id = if let Some(id) = p.id {
        let old: Option<(BehaviorSide, i64)> =
            sqlx::query_as("SELECT side,row_version FROM behavior_items WHERE id=$1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let (old_side, version) =
            old.ok_or_else(|| AppError::NotFound("ไม่พบรายการพฤติกรรม".into()))?;
        if Some(version) != p.row_version || old_side != p.side {
            return Err(AppError::Conflict(
                "รายการเปลี่ยนแล้ว หรือไม่สามารถเปลี่ยนฝั่งคะแนนเดิมได้".into(),
            ));
        }
        sqlx::query(
            "UPDATE behavior_items SET name=$2,enabled=$3,row_version=row_version+1 WHERE id=$1",
        )
        .bind(id)
        .bind(p.name.trim())
        .bind(p.enabled)
        .execute(&mut *tx)
        .await?;
        id
    } else {
        if p.row_version.is_some() {
            return Err(invalid("รายการใหม่ไม่มีเวอร์ชันเดิม"));
        }
        sqlx::query_scalar(
            "INSERT INTO behavior_items(name,side,enabled)VALUES($1,$2,$3)RETURNING id",
        )
        .bind(p.name.trim())
        .bind(p.side)
        .bind(p.enabled)
        .fetch_one(&mut *tx)
        .await?
    };
    let existing: Option<i32> = sqlx::query_scalar(
        "SELECT points FROM behavior_item_versions WHERE item_id=$1 AND effective_from=$2",
    )
    .bind(id)
    .bind(p.effective_from)
    .fetch_optional(&mut *tx)
    .await?;
    match existing {
        Some(points) if points != p.points => {
            return Err(AppError::Conflict(
                "เกณฑ์วันที่นี้มีแล้ว ใช้วันเริ่มใช้ใหม่เพื่อรักษาประวัติ".into(),
            ))
        }
        Some(_) => {}
        None => {
            sqlx::query("INSERT INTO behavior_item_versions(item_id,effective_from,points,created_by)VALUES($1,$2,$3,$4)").bind(id).bind(p.effective_from).bind(p.points).bind(actor).execute(&mut *tx).await?;
        }
    }
    bump(&mut tx).await?;
    let item=sqlx::query_as("SELECT i.id,i.name,i.side,i.enabled,i.row_version,v.id version_id,v.effective_from,v.points FROM behavior_items i JOIN behavior_item_versions v ON v.item_id=i.id WHERE i.id=$1 AND v.effective_from=$2").bind(id).bind(p.effective_from).fetch_one(&mut *tx).await?;
    save_receipt(&mut tx, actor, p.request_id, "item", &p, &item).await?;
    tx.commit().await?;
    Ok(item)
}
pub async fn list_rules(pool: &PgPool) -> Result<Vec<BehaviorAttendanceRule>, AppError> {
    Ok(sqlx::query_as("SELECT id,kind,status,grade_level_id,effective_from,action,points FROM behavior_attendance_rules ORDER BY kind,status,grade_level_id NULLS FIRST,effective_from DESC,id LIMIT 10001").fetch_all(pool).await?)
}
pub async fn save_rule(
    pool: &PgPool,
    actor: Uuid,
    p: SaveBehaviorAttendanceRule,
) -> Result<BehaviorAttendanceRule, AppError> {
    if p.kind == BehaviorSource::Manual {
        return Err(invalid("เกณฑ์นี้ใช้เฉพาะเช็กชื่อ"));
    }
    validate_status(&p.status)?;
    validate_score(p.action, p.points)?;
    if p.status == "unchecked" && p.action != BehaviorScoreAction::None {
        return Err(invalid("ยังไม่ได้เช็คไม่คิดคะแนนจนกว่าจะปิดยอดเป็นขาด"));
    }
    let mut tx = pool.begin().await?;
    lock(&mut tx).await?;
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM behavior_attendance_rules WHERE kind=$1 AND status=$2 AND grade_level_id IS NOT DISTINCT FROM $3 AND effective_from=$4)").bind(p.kind).bind(&p.status).bind(p.grade_level_id).bind(p.effective_from).fetch_one(&mut *tx).await?;
    if exists {
        return Err(AppError::Conflict(
            "เกณฑ์นี้มีเวอร์ชันวันเริ่มใช้แล้ว กรุณาเลือกวันใหม่".into(),
        ));
    }
    let row=sqlx::query_as("INSERT INTO behavior_attendance_rules(kind,status,grade_level_id,effective_from,action,points,created_by)VALUES($1,$2,$3,$4,$5,$6,$7)RETURNING id,kind,status,grade_level_id,effective_from,action,points").bind(p.kind).bind(p.status).bind(p.grade_level_id).bind(p.effective_from).bind(p.action).bind(p.points).bind(actor).fetch_one(&mut *tx).await?;
    bump(&mut tx).await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn settings_on_connection(
    connection: &mut PgConnection,
) -> Result<BehaviorSettings, AppError> {
    Ok(sqlx::query_as("SELECT enabled,count_absent_lessons,count_special,daily_merit_cap,daily_deduction_cap,merit_reset,deduction_reset,row_version FROM behavior_settings WHERE singleton").fetch_one(connection).await?)
}
pub async fn settings(pool: &PgPool) -> Result<BehaviorSettings, AppError> {
    settings_on_connection(&mut *pool.acquire().await?).await
}
pub async fn save_settings(
    pool: &PgPool,
    p: SaveBehaviorSettings,
) -> Result<BehaviorSettings, AppError> {
    let s = p.settings;
    if s.daily_merit_cap
        .is_some_and(|p| !(0..=100000).contains(&p))
        || s.daily_deduction_cap
            .is_some_and(|p| !(0..=100000).contains(&p))
    {
        return Err(invalid("เพดานคะแนนไม่ถูกต้อง"));
    }
    let mut tx = pool.begin().await?;
    lock(&mut tx).await?;
    let affected=sqlx::query("UPDATE behavior_settings SET enabled=$1,count_absent_lessons=$2,count_special=$3,daily_merit_cap=$4,daily_deduction_cap=$5,merit_reset=$6,deduction_reset=$7,row_version=row_version+1 WHERE singleton AND row_version=$8").bind(s.enabled).bind(s.count_absent_lessons).bind(s.count_special).bind(s.daily_merit_cap).bind(s.daily_deduction_cap).bind(s.merit_reset).bind(s.deduction_reset).bind(s.row_version).execute(&mut *tx).await?.rows_affected();
    if affected != 1 {
        return Err(AppError::Conflict("การตั้งค่าเปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
    }
    bump(&mut tx).await?;
    let result = settings_on_connection(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn cycles(pool: &PgPool) -> Result<Vec<BehaviorCycle>, AppError> {
    Ok(sqlx::query_as("SELECT id,side,starts_on,starts_at,ends_at,ends_on,state,label FROM behavior_cycles ORDER BY starts_on DESC,side,id LIMIT 10001").fetch_all(pool).await?)
}
pub async fn overview(pool: &PgPool, access: BehaviorAccess) -> Result<BehaviorOverview, AppError> {
    access.require_selector()?;
    let cycles = cycles(pool).await?;
    let activation_date =
        sqlx::query_scalar("SELECT activation_date FROM behavior_state WHERE singleton")
            .fetch_one(pool)
            .await?;
    Ok(BehaviorOverview {
        cycles,
        activation_date,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unchecked_is_explicitly_unscored() {
        assert!(validate_score(BehaviorScoreAction::None, 0).is_ok());
        assert!(validate_status("unchecked").is_ok());
    }
}
