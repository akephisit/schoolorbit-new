use crate::personnel::*;
use school_errors::AppError;
use sqlx::{types::Json, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

const ITEM_JSON: &str = "jsonb_build_object('id',id,'kind',kind,'code',code,'name',name,'isActive',is_active,'displayOrder',display_order,'createdAt',created_at,'updatedAt',updated_at)";
fn validate_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() || name.chars().count() > 200 || name.chars().any(char::is_control) {
        return Err(AppError::BadRequest(
            "กรุณาระบุชื่อรายการ 1–200 ตัวอักษร โดยไม่มีอักขระควบคุม".into(),
        ));
    }
    Ok(())
}
fn map_reference_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(ref db) = error {
        if db.is_unique_violation() {
            return AppError::Conflict("รายการชื่อนี้มีอยู่แล้ว".into());
        }
    }
    error.into()
}
pub async fn list_reference_items(
    pool: &PgPool,
    query: ReferenceListQuery,
) -> Result<ReferencePage, AppError> {
    let page = query.page.unwrap_or(1).clamp(1, 1_000_000);
    let page_size = query.page_size.unwrap_or(25).clamp(1, 50);
    let search = query.search.map(|s| format!("%{}%", s.trim()));
    let active = match query.status.unwrap_or(ReferenceStatusFilter::Active) {
        ReferenceStatusFilter::Active => Some(true),
        ReferenceStatusFilter::Inactive => Some(false),
        ReferenceStatusFilter::All => None,
    };
    // A single statement keeps items and total in the same snapshot, including empty pages.
    let mut sql = QueryBuilder::<Postgres>::new(
        "WITH matched AS (SELECT * FROM staff_reference_items WHERE kind = ",
    );
    sql.push_bind(query.kind.as_str());
    if let Some(active) = active {
        sql.push(" AND is_active = ").push_bind(active);
    }
    if let Some(search) = search {
        sql.push(" AND name ILIKE ").push_bind(search);
    }
    sql.push("), page AS (SELECT * FROM matched ORDER BY display_order,name,id LIMIT ")
        .push_bind(page_size)
        .push(" OFFSET ")
        .push_bind((page - 1) * page_size);
    sql.push(") SELECT (SELECT count(*) FROM matched), COALESCE((SELECT jsonb_agg(item ORDER BY display_order,name,id) FROM (SELECT ").push(ITEM_JSON).push(" AS item,display_order,name,id FROM page) p), '[]'::jsonb)");
    let (total, Json(items)): (i64, Json<Vec<StaffReferenceItem>>) =
        sql.build_query_as().fetch_one(pool).await?;
    Ok(ReferencePage {
        items,
        total,
        page,
        page_size,
    })
}
pub async fn create_reference_item(
    pool: &PgPool,
    input: CreateReferenceRequest,
) -> Result<StaffReferenceItem, AppError> {
    validate_name(&input.name)?;
    let id = Uuid::new_v4();
    let sql = format!("INSERT INTO staff_reference_items (id,kind,code,name,display_order) VALUES ($1,$2,$3,staff_reference_display_name($4),$5) RETURNING {ITEM_JSON}");
    let Json(item) = sqlx::query_scalar::<_, Json<StaffReferenceItem>>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(input.kind.as_str())
        .bind(format!("ref_{}", id.simple()))
        .bind(input.name)
        .bind(input.display_order.unwrap_or(0))
        .fetch_one(pool)
        .await
        .map_err(map_reference_error)?;
    Ok(item)
}
pub async fn update_reference_item(
    pool: &PgPool,
    id: Uuid,
    input: UpdateReferenceRequest,
) -> Result<StaffReferenceItem, AppError> {
    if let Some(name) = &input.name {
        validate_name(name)?;
    }
    let sql = format!("UPDATE staff_reference_items SET name=COALESCE(staff_reference_display_name($2),name),is_active=COALESCE($3,is_active),display_order=COALESCE($4,display_order),updated_at=NOW() WHERE id=$1 RETURNING {ITEM_JSON}");
    let row = sqlx::query_scalar::<_, Json<StaffReferenceItem>>(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(input.name)
        .bind(input.is_active)
        .bind(input.display_order)
        .fetch_optional(pool)
        .await
        .map_err(map_reference_error)?;
    row.map(|Json(item)| item)
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการกลาง".into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_name_validation_handles_thai_and_control_characters() {
        assert!(validate_name("ครูอัตราจ้าง").is_ok());
        assert!(validate_name(" \t").is_err());
        assert!(validate_name("\nครู").is_err());
        assert!(validate_name(&"ก".repeat(201)).is_err());
        assert!(serde_json::from_str::<UpdateReferenceRequest>(r#"{"kind":"major"}"#).is_err());
    }
}
