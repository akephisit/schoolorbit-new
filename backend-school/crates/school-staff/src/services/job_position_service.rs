use crate::personnel::{JobPositionListQuery, JobPositionPage, StaffJobPositionSummary};
use school_errors::AppError;
use sqlx::{types::Json, PgPool};

pub async fn list_job_positions(
    pool: &PgPool,
    query: JobPositionListQuery,
) -> Result<JobPositionPage, AppError> {
    let page = query.page.unwrap_or(1).clamp(1, 1_000_000);
    let page_size = query.page_size.unwrap_or(25).clamp(1, 50);
    let (total,Json(items)): (i64,Json<Vec<StaffJobPositionSummary>>) = sqlx::query_as(
        "WITH matched AS (SELECT id,code,name,is_active,is_selectable,display_order FROM staff_job_positions WHERE (NOT $1 OR (is_active AND is_selectable)) AND ($2::text IS NULL OR name ILIKE '%' || $2 || '%' OR code ILIKE '%' || $2 || '%')), paged AS (SELECT * FROM matched ORDER BY display_order,name,id LIMIT $3 OFFSET $4) SELECT (SELECT count(*) FROM matched),coalesce((SELECT jsonb_agg(jsonb_build_object('id',id,'code',code,'name',name,'isActive',is_active,'isSelectable',is_selectable) ORDER BY display_order,name,id) FROM paged),'[]'::jsonb)"
    ).bind(query.selectable_only.unwrap_or(false)).bind(query.search.as_deref().map(str::trim).filter(|s|!s.is_empty())).bind(page_size).bind((page-1)*page_size).fetch_one(pool).await?;
    Ok(JobPositionPage {
        items,
        total,
        page,
        page_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn personnel_job_query_rejects_retired_kind() {
        assert!(serde_json::from_str::<JobPositionListQuery>(r#"{"kind":"major"}"#).is_err());
    }
}
