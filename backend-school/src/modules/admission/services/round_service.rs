use crate::error::AppError;
use crate::modules::admission::models::rounds::*;
use crate::services::r2_client::R2Client;
use sqlx::PgPool;
use uuid::Uuid;

const ROUND_GRADE_LEVEL_CASE: &str = r#"CASE gl.level_type
    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
    WHEN 'primary'      THEN CONCAT('ป.', gl.year)
    WHEN 'secondary'    THEN CONCAT('ม.', gl.year)
    ELSE CONCAT('?.', gl.year)
END"#;

pub async fn list_public_rounds(pool: &PgPool) -> Result<Vec<AdmissionRound>, AppError> {
    sqlx::query_as::<_, AdmissionRound>(&format!(
        r#"SELECT ar.*, ay.name AS academic_year_name,
                  {grade_case} AS grade_level_name,
                  0::bigint AS application_count
           FROM admission_rounds ar
           JOIN academic_years ay ON ar.academic_year_id = ay.id
           JOIN grade_levels gl ON ar.grade_level_id = gl.id
           WHERE ar.is_visible = true
           ORDER BY ar.apply_start_date ASC"#,
        grade_case = ROUND_GRADE_LEVEL_CASE
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch public rounds: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })
}

pub async fn get_public_round_info(
    pool: &PgPool,
    id: Uuid,
) -> Result<(AdmissionRound, Vec<AdmissionTrack>), AppError> {
    let round = sqlx::query_as::<_, AdmissionRound>(&format!(
        r#"SELECT ar.*, ay.name AS academic_year_name,
                  {grade_case} AS grade_level_name,
                  0::bigint AS application_count
           FROM admission_rounds ar
           JOIN academic_years ay ON ar.academic_year_id = ay.id
           JOIN grade_levels gl ON ar.grade_level_id = gl.id
           WHERE ar.id = $1 AND ar.is_visible = true"#,
        grade_case = ROUND_GRADE_LEVEL_CASE
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch public round: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบรับสมัคร หรือไม่ได้เปิดรับสมัครในขณะนี้".to_string()))?;

    let tracks = sqlx::query_as::<_, AdmissionTrack>(
        r#"SELECT at2.*, sp.name_th AS study_plan_name,
                  0::bigint AS computed_capacity, 0::bigint AS room_count, 0::bigint AS application_count
           FROM admission_tracks at2
           JOIN study_plans sp ON at2.study_plan_id = sp.id
           WHERE at2.admission_round_id = $1
           ORDER BY at2.display_order ASC"#
    )
    .bind(id).fetch_all(pool).await.unwrap_or_default();

    Ok((round, tracks))
}

pub async fn list_rounds(pool: &PgPool) -> Result<Vec<AdmissionRound>, AppError> {
    sqlx::query_as::<_, AdmissionRound>(&format!(
        r#"SELECT ar.*, ay.name AS academic_year_name,
                  {grade_case} AS grade_level_name,
                  (SELECT COUNT(*) FROM admission_applications aa WHERE aa.admission_round_id = ar.id) AS application_count
           FROM admission_rounds ar
           JOIN academic_years ay ON ar.academic_year_id = ay.id
           JOIN grade_levels gl ON ar.grade_level_id = gl.id
           ORDER BY ar.created_at DESC"#,
        grade_case = ROUND_GRADE_LEVEL_CASE
    ))
    .fetch_all(pool).await
    .map_err(|e| {
        eprintln!("Failed to fetch admission rounds: {}", e);
        AppError::InternalServerError("Failed to fetch rounds".to_string())
    })
}

pub async fn get_round(pool: &PgPool, id: Uuid) -> Result<AdmissionRound, AppError> {
    sqlx::query_as::<_, AdmissionRound>(&format!(
        r#"SELECT ar.*, ay.name AS academic_year_name,
                  {grade_case} AS grade_level_name,
                  (SELECT COUNT(*) FROM admission_applications aa WHERE aa.admission_round_id = ar.id) AS application_count
           FROM admission_rounds ar
           JOIN academic_years ay ON ar.academic_year_id = ay.id
           JOIN grade_levels gl ON ar.grade_level_id = gl.id
           WHERE ar.id = $1"#,
        grade_case = ROUND_GRADE_LEVEL_CASE
    ))
    .bind(id).fetch_optional(pool).await
    .map_err(|e| {
        eprintln!("Failed to fetch round {}: {}", id, e);
        AppError::InternalServerError("Failed to fetch round".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบรับสมัคร".to_string()))
}

pub async fn create_round(
    pool: &PgPool,
    payload: CreateAdmissionRoundRequest,
) -> Result<AdmissionRound, AppError> {
    sqlx::query_as::<_, AdmissionRound>(
        r#"INSERT INTO admission_rounds (
               academic_year_id, grade_level_id, name, description,
               apply_start_date, apply_end_date, exam_date,
               result_announce_date, enrollment_start_date, enrollment_end_date
           )
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING *,
               (SELECT name FROM academic_years WHERE id = $1) AS academic_year_name,
               (SELECT CASE level_type
                           WHEN 'kindergarten' THEN CONCAT('อ.', year)
                           WHEN 'primary'      THEN CONCAT('ป.', year)
                           WHEN 'secondary'    THEN CONCAT('ม.', year)
                           ELSE CONCAT('?.', year)
                       END FROM grade_levels WHERE id = $2) AS grade_level_name,
               0::bigint AS application_count"#,
    )
    .bind(payload.academic_year_id)
    .bind(payload.grade_level_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(payload.apply_start_date)
    .bind(payload.apply_end_date)
    .bind(payload.exam_date)
    .bind(payload.result_announce_date)
    .bind(payload.enrollment_start_date)
    .bind(payload.enrollment_end_date)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to create round: {}", e);
        AppError::InternalServerError("Failed to create round".to_string())
    })
}

pub async fn update_round(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateAdmissionRoundRequest,
) -> Result<AdmissionRound, AppError> {
    sqlx::query_as::<_, AdmissionRound>(
        r#"UPDATE admission_rounds SET
               name = COALESCE($1, name), description = COALESCE($2, description),
               apply_start_date = COALESCE($3, apply_start_date),
               apply_end_date = COALESCE($4, apply_end_date),
               exam_date = COALESCE($5, exam_date),
               result_announce_date = COALESCE($6, result_announce_date),
               enrollment_start_date = COALESCE($7, enrollment_start_date),
               enrollment_end_date = COALESCE($8, enrollment_end_date),
               report_config = COALESCE($9, report_config),
               updated_at = NOW()
           WHERE id = $10
           RETURNING *,
               (SELECT name FROM academic_years WHERE id = academic_year_id) AS academic_year_name,
               (SELECT CASE level_type
                           WHEN 'kindergarten' THEN CONCAT('อ.', year)
                           WHEN 'primary'      THEN CONCAT('ป.', year)
                           WHEN 'secondary'    THEN CONCAT('ม.', year)
                           ELSE CONCAT('?.', year)
                       END FROM grade_levels WHERE id = grade_level_id) AS grade_level_name,
               (SELECT COUNT(*) FROM admission_applications WHERE admission_round_id = $10) AS application_count"#
    )
    .bind(&payload.name).bind(&payload.description)
    .bind(payload.apply_start_date).bind(payload.apply_end_date)
    .bind(payload.exam_date).bind(payload.result_announce_date)
    .bind(payload.enrollment_start_date).bind(payload.enrollment_end_date)
    .bind(payload.report_config.map(sqlx::types::Json))
    .bind(id).fetch_one(pool).await
    .map_err(|e| {
        eprintln!("Failed to update round {}: {}", id, e);
        AppError::InternalServerError("Failed to update round".to_string())
    })
}

pub async fn update_round_status(pool: &PgPool, id: Uuid, status: &str) -> Result<(), AppError> {
    let valid = [
        "draft",
        "open",
        "exam_announced",
        "announced",
        "enrolling",
        "closed",
    ];
    if !valid.contains(&status) {
        return Err(AppError::BadRequest(format!("สถานะ '{}' ไม่ถูกต้อง", status)));
    }
    if status == "draft" {
        return Err(AppError::BadRequest(
            "ไม่สามารถเปลี่ยนกลับไปสถานะ 'ร่าง' ได้".to_string(),
        ));
    }

    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admission_rounds WHERE id = $1)")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);
    if !exists {
        return Err(AppError::NotFound("ไม่พบรอบรับสมัคร".to_string()));
    }

    sqlx::query("UPDATE admission_rounds SET status = $1, updated_at = NOW() WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to update round status: {}", e);
            AppError::InternalServerError("Failed to update status".to_string())
        })?;
    Ok(())
}

pub async fn delete_round(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let file_rows: Vec<(Uuid, String)> = sqlx::query_as(
        r#"SELECT f.id, f.storage_path
           FROM admission_applications aa
           JOIN admission_application_documents aad ON aad.application_id = aa.id
           JOIN files f ON f.id = aad.file_id
           WHERE aa.admission_round_id = $1 AND aad.deleted_at IS NULL"#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if !file_rows.is_empty() {
        if let Ok(r2) = R2Client::new().await {
            for (_, storage_path) in &file_rows {
                r2.delete_file(storage_path).await.ok();
            }
        }
        let file_ids: Vec<Uuid> = file_rows.into_iter().map(|(fid, _)| fid).collect();
        sqlx::query("DELETE FROM files WHERE id = ANY($1)")
            .bind(&file_ids)
            .execute(pool)
            .await
            .ok();
    }

    sqlx::query("DELETE FROM admission_applications WHERE admission_round_id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete applications: {}", e);
            AppError::InternalServerError("Failed to delete applications".to_string())
        })?;

    sqlx::query("DELETE FROM admission_rounds WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete round: {}", e);
            AppError::InternalServerError("Failed to delete round".to_string())
        })?;
    Ok(())
}

pub async fn toggle_round_visibility(
    pool: &PgPool,
    id: Uuid,
    is_visible: bool,
) -> Result<bool, AppError> {
    sqlx::query_scalar::<_, bool>(
        "UPDATE admission_rounds SET is_visible = $1, updated_at = NOW() WHERE id = $2 RETURNING is_visible"
    )
    .bind(is_visible).bind(id).fetch_optional(pool).await
    .map_err(|e| {
        eprintln!("Failed to update round visibility: {}", e);
        AppError::InternalServerError("Failed to update visibility".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบรอบรับสมัคร".to_string()))
}

// --- Exam Subjects ---

pub async fn list_exam_subjects(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<AdmissionExamSubject>, AppError> {
    sqlx::query_as::<_, AdmissionExamSubject>(
        "SELECT id, admission_round_id, name, code, max_score::FLOAT8 AS max_score, display_order, created_at FROM admission_exam_subjects WHERE admission_round_id = $1 ORDER BY display_order ASC, created_at ASC"
    )
    .bind(round_id).fetch_all(pool).await
    .map_err(|e| {
        eprintln!("Failed to fetch subjects: {}", e);
        AppError::InternalServerError("Failed to fetch subjects".to_string())
    })
}

pub async fn create_exam_subject(
    pool: &PgPool,
    round_id: Uuid,
    payload: CreateExamSubjectRequest,
) -> Result<AdmissionExamSubject, AppError> {
    sqlx::query_as::<_, AdmissionExamSubject>(
        r#"INSERT INTO admission_exam_subjects (admission_round_id, name, code, max_score, display_order)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, admission_round_id, name, code, max_score::FLOAT8 AS max_score, display_order, created_at"#
    )
    .bind(round_id).bind(&payload.name).bind(&payload.code)
    .bind(payload.max_score.unwrap_or(100.0))
    .bind(payload.display_order.unwrap_or(0))
    .fetch_one(pool).await
    .map_err(|e| {
        eprintln!("Failed to create subject: {}", e);
        AppError::InternalServerError("Failed to create subject".to_string())
    })
}

pub async fn update_exam_subject(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateExamSubjectRequest,
) -> Result<AdmissionExamSubject, AppError> {
    sqlx::query_as::<_, AdmissionExamSubject>(
        r#"UPDATE admission_exam_subjects SET
               name = COALESCE($1, name), code = COALESCE($2, code),
               max_score = COALESCE($3, max_score), display_order = COALESCE($4, display_order)
           WHERE id = $5
           RETURNING id, admission_round_id, name, code, max_score::FLOAT8 AS max_score, display_order, created_at"#
    )
    .bind(&payload.name).bind(&payload.code)
    .bind(payload.max_score).bind(payload.display_order)
    .bind(id).fetch_one(pool).await
    .map_err(|e| {
        eprintln!("Failed to update subject: {}", e);
        AppError::InternalServerError("Failed to update subject".to_string())
    })
}

pub async fn delete_exam_subject(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM admission_exam_subjects WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to delete subject".to_string()))?;
    Ok(())
}

// --- Admission Tracks ---

pub async fn list_tracks(pool: &PgPool, round_id: Uuid) -> Result<Vec<AdmissionTrack>, AppError> {
    sqlx::query_as::<_, AdmissionTrack>(
        r#"SELECT t.*, sp.name_th AS study_plan_name,
               (SELECT COUNT(DISTINCT cr.id)
                FROM study_plan_versions spv
                JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
                WHERE spv.study_plan_id = t.study_plan_id
                  AND cr.academic_year_id = (SELECT academic_year_id FROM admission_rounds WHERE id = t.admission_round_id)
                  AND cr.grade_level_id   = (SELECT grade_level_id   FROM admission_rounds WHERE id = t.admission_round_id)
               ) AS room_count,
               COALESCE(
                   t.capacity_override::bigint,
                   (SELECT SUM(cr.capacity)
                    FROM study_plan_versions spv
                    JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
                    WHERE spv.study_plan_id = t.study_plan_id
                      AND cr.academic_year_id = (SELECT academic_year_id FROM admission_rounds WHERE id = t.admission_round_id)
                      AND cr.grade_level_id   = (SELECT grade_level_id   FROM admission_rounds WHERE id = t.admission_round_id)
                   )
               ) AS computed_capacity,
               (SELECT COUNT(*) FROM admission_applications aa WHERE aa.admission_track_id = t.id) AS application_count
           FROM admission_tracks t
           JOIN study_plans sp ON t.study_plan_id = sp.id
           WHERE t.admission_round_id = $1
           ORDER BY t.display_order ASC, t.created_at ASC"#
    )
    .bind(round_id).fetch_all(pool).await
    .map_err(|e| {
        eprintln!("Failed to fetch tracks: {}", e);
        AppError::InternalServerError("Failed to fetch tracks".to_string())
    })
}

pub async fn create_track(
    pool: &PgPool,
    round_id: Uuid,
    payload: CreateAdmissionTrackRequest,
) -> Result<AdmissionTrack, AppError> {
    let scoring_ids = serde_json::to_value(payload.scoring_subject_ids.unwrap_or_default())
        .unwrap_or(serde_json::json!([]));

    sqlx::query_as::<_, AdmissionTrack>(
        r#"INSERT INTO admission_tracks (
               admission_round_id, study_plan_id, name, capacity_override,
               scoring_subject_ids, tiebreak_method, display_order
           )
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *,
               (SELECT name_th FROM study_plans WHERE id = $2) AS study_plan_name,
               0::bigint AS room_count,
               NULL::bigint AS computed_capacity,
               0::bigint AS application_count"#,
    )
    .bind(round_id)
    .bind(payload.study_plan_id)
    .bind(&payload.name)
    .bind(payload.capacity_override)
    .bind(scoring_ids)
    .bind(
        payload
            .tiebreak_method
            .unwrap_or_else(|| "applied_at".to_string()),
    )
    .bind(payload.display_order.unwrap_or(0))
    .fetch_one(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to create track: {}", e);
        AppError::InternalServerError("Failed to create track".to_string())
    })
}

pub async fn update_track(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateAdmissionTrackRequest,
) -> Result<AdmissionTrack, AppError> {
    let scoring_ids = payload
        .scoring_subject_ids
        .map(|v| serde_json::to_value(v).unwrap_or(serde_json::json!([])));

    sqlx::query_as::<_, AdmissionTrack>(
        r#"UPDATE admission_tracks SET
               name = COALESCE($1, name),
               capacity_override = COALESCE($2, capacity_override),
               scoring_subject_ids = COALESCE($3, scoring_subject_ids),
               tiebreak_method = COALESCE($4, tiebreak_method),
               display_order = COALESCE($5, display_order)
           WHERE id = $6
           RETURNING *,
               (SELECT name_th FROM study_plans WHERE id = study_plan_id) AS study_plan_name,
               NULL::bigint AS room_count,
               NULL::bigint AS computed_capacity,
               (SELECT COUNT(*) FROM admission_applications WHERE admission_track_id = $6) AS application_count"#
    )
    .bind(&payload.name).bind(payload.capacity_override).bind(scoring_ids)
    .bind(&payload.tiebreak_method).bind(payload.display_order)
    .bind(id).fetch_one(pool).await
    .map_err(|e| {
        eprintln!("Failed to update track: {}", e);
        AppError::InternalServerError("Failed to update track".to_string())
    })
}

pub async fn delete_track(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admission_applications WHERE admission_track_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if count > 0 {
        return Err(AppError::BadRequest(format!(
            "ไม่สามารถลบสายที่มีใบสมัครอยู่แล้ว ({} ใบ)",
            count
        )));
    }

    sqlx::query("DELETE FROM admission_tracks WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to delete track".to_string()))?;
    Ok(())
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct RoomCapacityRow {
    pub room_id: Uuid,
    pub room_name: String,
    pub room_code: String,
}

pub async fn get_track_capacity(pool: &PgPool, id: Uuid) -> Result<Vec<RoomCapacityRow>, AppError> {
    sqlx::query_as::<_, RoomCapacityRow>(
        r#"SELECT cr.id AS room_id, cr.name AS room_name, cr.code AS room_code
           FROM admission_tracks t
           JOIN study_plans sp ON t.study_plan_id = sp.id
           JOIN study_plan_versions spv ON spv.study_plan_id = sp.id AND spv.is_active = true
           JOIN class_rooms cr ON cr.study_plan_version_id = spv.id
               AND cr.academic_year_id = (
                   SELECT academic_year_id FROM admission_rounds WHERE id = t.admission_round_id
               )
           WHERE t.id = $1
           ORDER BY cr.name ASC"#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch track capacity: {}", e);
        AppError::InternalServerError("Failed to fetch capacity".to_string())
    })
}
