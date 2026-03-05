use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;
use crate::modules::admission::models::applications::*;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// GET /api/admission/rounds/:id/scores — ดูคะแนนทุกคนในรอบ
pub async fn get_all_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES).await {
        return Ok(r);
    }

    #[derive(sqlx::FromRow, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ScoreRow {
        application_id: Uuid,
        application_number: Option<String>,
        full_name: String,
        track_name: Option<String>,
        subject_id: Uuid,
        subject_name: String,
        subject_code: Option<String>,
        max_score: f64,
        score: Option<f64>,
    }

    let scores = sqlx::query_as::<_, ScoreRow>(
        r#"
        SELECT
            aa.id AS application_id,
            aa.application_number,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            at2.name AS track_name,
            aes.id AS subject_id,
            aes.name AS subject_name,
            aes.code AS subject_code,
            aes.max_score::FLOAT8 AS max_score,
            esc.score
        FROM admission_applications aa
        JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
        CROSS JOIN admission_exam_subjects aes
        LEFT JOIN admission_exam_scores esc ON esc.application_id = aa.id AND esc.exam_subject_id = aes.id
        WHERE aa.admission_round_id = $1
          AND aes.admission_round_id = $1
          AND aa.status NOT IN ('rejected', 'withdrawn')
        ORDER BY at2.display_order ASC, aa.application_number ASC, aes.display_order ASC
        "#
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch scores: {}", e);
        AppError::InternalServerError("Failed to fetch scores".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": scores })).into_response())
}

/// GET /api/admission/applications/:id/scores — คะแนนของผู้สมัครคนหนึ่ง (ใช้ใน Portal ด้วย)
pub async fn get_application_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES).await {
        return Ok(r);
    }

    let scores = sqlx::query_as::<_, ExamScore>(
        r#"
        SELECT
            esc.id,
            esc.application_id,
            esc.exam_subject_id,
            esc.score,
            esc.entered_by,
            esc.entered_at,
            esc.updated_at,
            aes.name AS subject_name,
            aes.code AS subject_code,
            aes.max_score::FLOAT8 AS max_score
        FROM admission_exam_subjects aes
        LEFT JOIN admission_exam_scores esc ON esc.exam_subject_id = aes.id AND esc.application_id = $1
        WHERE aes.admission_round_id = (
            SELECT admission_round_id FROM admission_applications WHERE id = $1
        )
        ORDER BY aes.display_order ASC
        "#
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch application scores: {}", e);
        AppError::InternalServerError("Failed to fetch scores".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": scores })).into_response())
}

/// PUT /api/admission/applications/:id/scores — อัปเดตคะแนนของผู้สมัครคนหนึ่ง
pub async fn update_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user = match check_permission(&headers, &pool, codes::ADMISSION_SCORES).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    for entry in &payload.scores {
        sqlx::query(
            r#"
            INSERT INTO admission_exam_scores (application_id, exam_subject_id, score, entered_by, entered_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            ON CONFLICT (application_id, exam_subject_id)
            DO UPDATE SET score = $3, entered_by = $4, updated_at = NOW()
            "#
        )
        .bind(id)
        .bind(entry.exam_subject_id)
        .bind(entry.score)
        .bind(user.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to upsert score: {}", e);
            AppError::InternalServerError("Failed to update score".to_string())
        })?;
    }

    // อัปเดต application status เป็น scored ถ้ากรอกครบ
    let total_subjects: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admission_exam_subjects WHERE admission_round_id = (SELECT admission_round_id FROM admission_applications WHERE id = $1)"
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(0);

    let scored_subjects: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admission_exam_scores WHERE application_id = $1 AND score IS NOT NULL"
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(0);

    if total_subjects > 0 && scored_subjects >= total_subjects {
        sqlx::query(
            "UPDATE admission_applications SET status = 'scored', updated_at = NOW() WHERE id = $1 AND status = 'verified'"
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .ok();
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({ "success": true, "message": "อัปเดตคะแนนแล้ว" })).into_response())
}

/// PUT /api/admission/rounds/:id/scores/bulk — อัปเดตคะแนนหลายคนพร้อมกัน
pub async fn bulk_update_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<BulkUpdateScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user = match check_permission(&headers, &pool, codes::ADMISSION_SCORES).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut updated = 0usize;

    for entry in &payload.entries {
        for score in &entry.scores {
            sqlx::query(
                r#"
                INSERT INTO admission_exam_scores (application_id, exam_subject_id, score, entered_by, entered_at, updated_at)
                VALUES ($1, $2, $3, $4, NOW(), NOW())
                ON CONFLICT (application_id, exam_subject_id)
                DO UPDATE SET score = $3, entered_by = $4, updated_at = NOW()
                "#
            )
            .bind(entry.application_id)
            .bind(score.exam_subject_id)
            .bind(score.score)
            .bind(user.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("Bulk score error: {}", e);
                AppError::InternalServerError("Failed to bulk update scores".to_string())
            })?;

            updated += 1;
        }
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": format!("อัปเดต {} รายการ", updated),
        "data": { "updated_count": updated }
    })).into_response())
}
