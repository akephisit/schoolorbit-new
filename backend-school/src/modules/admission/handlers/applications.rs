use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Datelike, FixedOffset, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::file_url::FileUrlBuilder;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;
use crate::modules::admission::models::applications::*;
use crate::services::r2_client::R2Client;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}


// ==========================================
// Submit Application (Public — no auth required)
// ==========================================

/// POST /api/admission/apply/:round_id
pub async fn submit_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<SubmitApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // 1. ตรวจว่ารอบเปิดรับสมัครอยู่ไหม
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM admission_rounds WHERE id = $1"
    )
    .bind(round_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    match status.as_deref() {
        None => return Err(AppError::NotFound("ไม่พบรอบรับสมัคร".to_string())),
        Some("open") => {}
        Some(s) => return Err(AppError::BadRequest(format!("รอบรับสมัครไม่ได้เปิดรับ (สถานะ: {})", s))),
    }

    // 2. ตรวจ track อยู่ในรอบนี้ไหม
    let track_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM admission_tracks WHERE id = $1 AND admission_round_id = $2)"
    )
    .bind(payload.admission_track_id)
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(false);

    if !track_exists {
        return Err(AppError::BadRequest("สายการเรียนไม่ถูกต้อง".to_string()));
    }

    // 3–5. Lock per round → ตรวจซ้ำ → generate number → insert (atomic)
    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // ดึง academic year + ลำดับรอบในปีนั้น (ใช้เป็น lock key + prefix เลขใบสมัคร)
    let (year, round_number): (i32, i64) = sqlx::query_as(
        r#"SELECT ay.year,
                  ROW_NUMBER() OVER (PARTITION BY ar.academic_year_id ORDER BY ar.created_at ASC)
           FROM admission_rounds ar
           JOIN academic_years ay ON ar.academic_year_id = ay.id
           WHERE ar.id = $1"#
    )
    .bind(round_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to get academic year".to_string()))?;

    // Advisory lock ต่อปีการศึกษา (ไม่ใช่ต่อ round)
    // เพราะ application_number มี unique constraint ระดับโรงเรียน ข้ามทุก round
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(year as i64)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError("Lock failed".to_string()))?;

    // ตรวจสอบ national_id ไม่ซ้ำในรอบนี้
    let already_applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM admission_applications WHERE national_id = $1 AND admission_round_id = $2)"
    )
    .bind(&payload.national_id)
    .bind(round_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(false);

    if already_applied {
        return Err(AppError::BadRequest("เลขบัตรประชาชนนี้ได้สมัครรอบนี้ไปแล้ว".to_string()));
    }

    // คำนวณ prefix เลขที่ใบสมัคร: YYMMDDRR (พ.ศ.)
    // เช่น 14/03/2569 รอบ 1 → "6903140100001", รอบ 2 → "6903140200001"
    let _ = year; // ยังคงใช้ year สำหรับ advisory lock ข้างบน
    let bangkok = FixedOffset::east_opt(7 * 3600).unwrap();
    let now = Utc::now().with_timezone(&bangkok);
    let be_year = now.year() + 543;
    let app_prefix = format!("{:02}{:02}{:02}{:02}", be_year % 100, now.month(), now.day(), round_number);
    let app_pattern = format!("{}%", app_prefix);

    // หา MAX sequence ของวันนี้ในรอบนี้ (format ใหม่: YYMMDDRRNNNNN = 13 หลัก)
    let max_seq: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(MAX(
            CASE WHEN application_number ~ '^[0-9]{13}$'
            THEN CAST(SUBSTRING(application_number, 9, 5) AS BIGINT)
            ELSE 0::BIGINT END
        ), 0::BIGINT) FROM admission_applications WHERE application_number LIKE $1"#
    )
    .bind(&app_pattern)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to compute application number".to_string()))?;

    let application_number = format!("{}{:05}", app_prefix, max_seq + 1);

    // Insert
    let application = sqlx::query_as::<_, AdmissionApplication>(
        r#"
        INSERT INTO admission_applications (
            admission_round_id, admission_track_id, application_number,
            national_id, title, first_name, last_name, gender, date_of_birth, phone, email,
            address_line, sub_district, district, province, postal_code,
            previous_school, previous_grade, previous_gpa,
            father_name, father_phone, father_occupation, father_national_id,
            mother_name, mother_phone, mother_occupation, mother_national_id,
            guardian_name, guardian_phone, guardian_relation, guardian_national_id,
            guardian_occupation, guardian_income, guardian_is,
            religion, ethnicity, nationality,
            home_house_no, home_moo, home_soi, home_road, home_phone,
            current_house_no, current_moo, current_soi, current_road,
            current_sub_district, current_district, current_province, current_postal_code, current_phone,
            previous_study_year, previous_school_province,
            father_income, mother_income,
            parent_status, parent_status_other
        )
        VALUES (
            $1, $2, $3,
            $4, $5, $6, $7, $8, $9, $10, $11,
            $12, $13, $14, $15, $16,
            $17, $18, $19,
            $20, $21, $22, $23,
            $24, $25, $26, $27,
            $28, $29, $30, $31,
            $32, $33, $34,
            $35, $36, $37,
            $38, $39, $40, $41, $42,
            $43, $44, $45, $46,
            $47, $48, $49, $50, $51,
            $52, $53,
            $54, $55,
            $56, $57
        )
        RETURNING *,
            (SELECT name FROM admission_tracks WHERE id = $2) AS track_name,
            (SELECT name FROM admission_rounds WHERE id = $1) AS round_name
        "#
    )
    .bind(round_id)
    .bind(payload.admission_track_id)
    .bind(&application_number)
    .bind(&payload.national_id)
    .bind(&payload.title)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.gender)
    .bind(payload.date_of_birth)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(&payload.address_line)
    .bind(&payload.sub_district)
    .bind(&payload.district)
    .bind(&payload.province)
    .bind(&payload.postal_code)
    .bind(&payload.previous_school)
    .bind(&payload.previous_grade)
    .bind(payload.previous_gpa)
    .bind(&payload.father_name)
    .bind(&payload.father_phone)
    .bind(&payload.father_occupation)
    .bind(&payload.father_national_id)
    .bind(&payload.mother_name)
    .bind(&payload.mother_phone)
    .bind(&payload.mother_occupation)
    .bind(&payload.mother_national_id)
    .bind(&payload.guardian_name)
    .bind(&payload.guardian_phone)
    .bind(&payload.guardian_relation)
    .bind(&payload.guardian_national_id)
    .bind(&payload.guardian_occupation)
    .bind(payload.guardian_income)
    .bind(&payload.guardian_is)
    .bind(&payload.religion)
    .bind(&payload.ethnicity)
    .bind(&payload.nationality)
    .bind(&payload.home_house_no)
    .bind(&payload.home_moo)
    .bind(&payload.home_soi)
    .bind(&payload.home_road)
    .bind(&payload.home_phone)
    .bind(&payload.current_house_no)
    .bind(&payload.current_moo)
    .bind(&payload.current_soi)
    .bind(&payload.current_road)
    .bind(&payload.current_sub_district)
    .bind(&payload.current_district)
    .bind(&payload.current_province)
    .bind(&payload.current_postal_code)
    .bind(&payload.current_phone)
    .bind(&payload.previous_study_year)
    .bind(&payload.previous_school_province)
    .bind(payload.father_income)
    .bind(payload.mother_income)
    .bind(&payload.parent_status)
    .bind(&payload.parent_status_other)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to submit application: {}", e);
        AppError::InternalServerError("ไม่สามารถยื่นใบสมัครได้".to_string())
    })?;

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok((StatusCode::CREATED, Json(json!({
        "success": true,
        "message": "ยื่นใบสมัครสำเร็จ",
        "data": {
            "application_number": application_number,
            "application": application,
        }
    }))).into_response())
}

// ==========================================
// Staff: List & Get Applications
// ==========================================

/// GET /api/admission/rounds/:id/applications
pub async fn list_applications(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Query(filter): Query<ApplicationFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let mut query = sqlx::QueryBuilder::new(
        r#"
        SELECT
            aa.id,
            aa.application_number,
            aa.national_id,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            at2.name AS track_name,
            aa.status,
            aa.phone,
            aa.previous_school,
            aa.previous_gpa,
            aa.created_at
        FROM admission_applications aa
        LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
        WHERE aa.admission_round_id = "#
    );
    query.push_bind(round_id);

    if let Some(ref s) = filter.status {
        query.push(" AND aa.status = ");
        query.push_bind(s);
    }
    if let Some(tid) = filter.track_id {
        query.push(" AND aa.admission_track_id = ");
        query.push_bind(tid);
    }
    if let Some(ref search) = filter.search {
        if !search.is_empty() {
            let like_term = format!("%{}%", search);
            query.push(" AND (aa.national_id ILIKE ");
            query.push_bind(like_term.clone());
            query.push(" OR aa.first_name ILIKE ");
            query.push_bind(like_term.clone());
            query.push(" OR aa.last_name ILIKE ");
            query.push_bind(like_term.clone());
            query.push(" OR aa.application_number ILIKE ");
            query.push_bind(like_term);
            query.push(")");
        }
    }
    query.push(" ORDER BY aa.created_at ASC");

    #[derive(sqlx::FromRow, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AppListRow {
        id: Uuid,
        application_number: Option<String>,
        national_id: String,
        full_name: String,
        track_name: Option<String>,
        status: String,
        phone: Option<String>,
        previous_school: Option<String>,
        previous_gpa: Option<f64>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let applications = query.build_query_as::<AppListRow>()
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to list applications: {}", e);
            AppError::InternalServerError("Failed to fetch applications".to_string())
        })?;

    Ok(Json(json!({ "success": true, "data": applications })).into_response())
}

/// GET /api/admission/applications/:id
pub async fn get_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let application = sqlx::query_as::<_, AdmissionApplication>(
        r#"
        SELECT aa.*,
               at2.name AS track_name,
               ar.name  AS round_name
        FROM admission_applications aa
        LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
        LEFT JOIN admission_rounds ar ON aa.admission_round_id = ar.id
        WHERE aa.id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch application {}: {}", id, e);
        AppError::InternalServerError("Failed to fetch application".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;

    // Fetch linked documents
    let documents = sqlx::query_as::<_, ApplicationDocument>(
        r#"
        SELECT d.id, d.application_id, d.file_id, d.doc_type, d.created_at, d.deleted_at,
               f.storage_path AS file_url,
               f.original_filename, f.file_size, f.mime_type
        FROM admission_application_documents d
        JOIN files f ON f.id = d.file_id
        WHERE d.application_id = $1 AND d.deleted_at IS NULL
        ORDER BY d.created_at ASC
        "#
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let url_builder = FileUrlBuilder::new().unwrap_or_default();
    let documents: Vec<ApplicationDocument> = documents
        .into_iter()
        .map(|mut doc| {
            if let Some(path) = doc.file_url.as_deref() {
                doc.file_url = Some(url_builder.build_url(path));
            }
            doc
        })
        .collect();

    Ok(Json(json!({ "success": true, "data": application, "documents": documents })).into_response())
}

// ==========================================
// Staff: Verify / Reject
// ==========================================

/// PUT /api/admission/applications/:id/verify
pub async fn verify_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let verifier_id = match check_permission(&headers, &pool, codes::ADMISSION_VERIFY, &state.permission_cache).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };

    let result = sqlx::query(
        r#"
        UPDATE admission_applications
        SET status = 'verified',
            verified_by = $1,
            verified_at = NOW(),
            rejection_reason = NULL,
            updated_at = NOW()
        WHERE id = $2 AND status = 'submitted'
        "#
    )
    .bind(verifier_id)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to verify application: {}", e);
        AppError::InternalServerError("ไม่สามารถยืนยันใบสมัครได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "ไม่พบใบสมัคร หรือสถานะไม่ใช่ 'รอตรวจสอบ'".to_string()
        ));
    }

    Ok(Json(json!({ "success": true, "message": "ยืนยันใบสมัครแล้ว" })).into_response())
}

/// PUT /api/admission/applications/:id/reject
pub async fn reject_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<RejectApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_VERIFY, &state.permission_cache).await {
        return Ok(r);
    }

    sqlx::query(
        r#"
        UPDATE admission_applications
        SET status = 'rejected',
            rejection_reason = $1,
            updated_at = NOW()
        WHERE id = $2
          AND status NOT IN ('enrolled', 'withdrawn')
        "#
    )
    .bind(&payload.rejection_reason)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to reject application: {}", e);
        AppError::InternalServerError("ไม่สามารถปฏิเสธใบสมัครได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "message": "ปฏิเสธใบสมัครแล้ว" })).into_response())
}

/// PUT /api/admission/applications/:id/absent — ทำเครื่องหมายขาดสอบ / ยกเลิก
pub async fn mark_absent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<MarkAbsentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }

    if payload.absent {
        sqlx::query(
            "UPDATE admission_applications SET status = 'absent', updated_at = NOW() WHERE id = $1 AND status IN ('verified', 'scored')"
        )
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถทำเครื่องหมายขาดสอบได้".to_string()))?;
        Ok(Json(json!({ "success": true, "message": "ทำเครื่องหมายขาดสอบแล้ว" })).into_response())
    } else {
        sqlx::query(
            "UPDATE admission_applications SET status = 'verified', updated_at = NOW() WHERE id = $1 AND status = 'absent'"
        )
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถยกเลิกขาดสอบได้".to_string()))?;
        Ok(Json(json!({ "success": true, "message": "ยกเลิกขาดสอบแล้ว" })).into_response())
    }
}

// ==========================================
// Staff: Update Application (submitted only)
// ==========================================

/// PUT /api/admission/applications/:id
pub async fn update_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_VERIFY, &state.permission_cache).await {
        return Ok(r);
    }

    let result = sqlx::query(
        r#"
        UPDATE admission_applications
        SET title = $1, first_name = $2, last_name = $3, gender = $4, date_of_birth = $5,
            phone = $6, email = $7, religion = $8, ethnicity = $9, nationality = $10,
            address_line = $11, sub_district = $12, district = $13, province = $14, postal_code = $15,
            home_house_no = $16, home_moo = $17, home_soi = $18, home_road = $19, home_phone = $20,
            current_house_no = $21, current_moo = $22, current_soi = $23, current_road = $24,
            current_sub_district = $25, current_district = $26, current_province = $27,
            current_postal_code = $28, current_phone = $29,
            previous_school = $30, previous_grade = $31, previous_gpa = $32,
            previous_study_year = $33, previous_school_province = $34,
            father_name = $35, father_phone = $36, father_occupation = $37, father_national_id = $38, father_income = $39,
            mother_name = $40, mother_phone = $41, mother_occupation = $42, mother_national_id = $43, mother_income = $44,
            guardian_name = $45, guardian_phone = $46, guardian_relation = $47, guardian_national_id = $48,
            guardian_occupation = $49, guardian_income = $50, guardian_is = $51,
            parent_status = $52, parent_status_other = $53,
            updated_at = NOW()
        WHERE id = $54 AND status NOT IN ('enrolled', 'withdrawn')
        "#
    )
    .bind(&payload.title)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.gender)
    .bind(payload.date_of_birth)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(&payload.religion)
    .bind(&payload.ethnicity)
    .bind(&payload.nationality)
    .bind(&payload.address_line)
    .bind(&payload.sub_district)
    .bind(&payload.district)
    .bind(&payload.province)
    .bind(&payload.postal_code)
    .bind(&payload.home_house_no)
    .bind(&payload.home_moo)
    .bind(&payload.home_soi)
    .bind(&payload.home_road)
    .bind(&payload.home_phone)
    .bind(&payload.current_house_no)
    .bind(&payload.current_moo)
    .bind(&payload.current_soi)
    .bind(&payload.current_road)
    .bind(&payload.current_sub_district)
    .bind(&payload.current_district)
    .bind(&payload.current_province)
    .bind(&payload.current_postal_code)
    .bind(&payload.current_phone)
    .bind(&payload.previous_school)
    .bind(&payload.previous_grade)
    .bind(payload.previous_gpa)
    .bind(&payload.previous_study_year)
    .bind(&payload.previous_school_province)
    .bind(&payload.father_name)
    .bind(&payload.father_phone)
    .bind(&payload.father_occupation)
    .bind(&payload.father_national_id)
    .bind(payload.father_income)
    .bind(&payload.mother_name)
    .bind(&payload.mother_phone)
    .bind(&payload.mother_occupation)
    .bind(&payload.mother_national_id)
    .bind(payload.mother_income)
    .bind(&payload.guardian_name)
    .bind(&payload.guardian_phone)
    .bind(&payload.guardian_relation)
    .bind(&payload.guardian_national_id)
    .bind(&payload.guardian_occupation)
    .bind(payload.guardian_income)
    .bind(&payload.guardian_is)
    .bind(&payload.parent_status)
    .bind(&payload.parent_status_other)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update application {}: {}", id, e);
        AppError::InternalServerError("ไม่สามารถแก้ไขใบสมัครได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "ไม่พบใบสมัคร หรือไม่สามารถแก้ไขได้ (สถานะเป็น enrolled หรือ withdrawn)".to_string()
        ));
    }

    Ok(Json(json!({ "success": true, "message": "แก้ไขใบสมัครแล้ว" })).into_response())
}

// ==========================================
// Staff: Unverify Application (verified → submitted)
// ==========================================

/// PUT /api/admission/applications/:id/unverify
pub async fn unverify_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_VERIFY, &state.permission_cache).await {
        return Ok(r);
    }

    let result = sqlx::query(
        r#"
        UPDATE admission_applications
        SET status = 'submitted',
            verified_by = NULL,
            verified_at = NULL,
            updated_at = NOW()
        WHERE id = $1 AND status = 'verified'
        "#
    )
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to unverify application {}: {}", id, e);
        AppError::InternalServerError("ไม่สามารถยกเลิกการอนุมัติได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "ไม่พบใบสมัคร หรือสถานะไม่ใช่ 'ผ่านการตรวจสอบ'".to_string()
        ));
    }

    Ok(Json(json!({ "success": true, "message": "ยกเลิกการอนุมัติแล้ว" })).into_response())
}

// ==========================================
// Staff: Delete Application
// ==========================================

/// DELETE /api/admission/applications/:id
pub async fn delete_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    // ดึง application_number ก่อน (เพื่อตรวจสอบว่ามีอยู่จริง)
    let app_number: Option<String> = sqlx::query_scalar(
        "SELECT application_number FROM admission_applications WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch application {}: {}", id, e);
        AppError::InternalServerError("ไม่สามารถลบใบสมัครได้".to_string())
    })?;

    let Some(_app_number) = app_number else {
        return Err(AppError::NotFound("ไม่พบใบสมัคร".to_string()));
    };

    // ดึง file records ทั้งหมดที่เชื่อมกับใบสมัครนี้
    let file_rows: Vec<(Uuid, String)> = sqlx::query_as(
        r#"SELECT f.id, f.storage_path
           FROM admission_application_documents aad
           JOIN files f ON f.id = aad.file_id
           WHERE aad.application_id = $1 AND aad.deleted_at IS NULL"#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // ลบไฟล์ใน R2
    if !file_rows.is_empty() {
        if let Ok(r2) = R2Client::new().await {
            for (_, storage_path) in &file_rows {
                r2.delete_file(storage_path).await.ok();
            }
        }

        // ลบ files records (ต้องลบก่อน admission_applications เพราะ FK)
        let file_ids: Vec<Uuid> = file_rows.into_iter().map(|(fid, _)| fid).collect();
        sqlx::query("DELETE FROM files WHERE id = ANY($1)")
            .bind(&file_ids)
            .execute(&pool)
            .await
            .ok();
    }

    // ลบ application (CASCADE ลบ admission_application_documents)
    let result = sqlx::query("DELETE FROM admission_applications WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete application {}: {}", id, e);
            AppError::InternalServerError("ไม่สามารถลบใบสมัครได้".to_string())
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบใบสมัคร".to_string()));
    }

    Ok(Json(json!({ "success": true, "message": "ลบใบสมัครแล้ว" })).into_response())
}

// ==========================================
// Staff: Enrollment
// ==========================================

/// GET /api/admission/rounds/:id/enrollment — รายชื่อที่รอมอบตัว
pub async fn list_enrollment_pending(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_ENROLL, &state.permission_cache).await {
        return Ok(r);
    }

    #[derive(sqlx::FromRow, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct EnrollmentPendingRow {
        id: Uuid,
        application_number: Option<String>,
        national_id: String,
        full_name: String,
        track_name: Option<String>,
        room_name: Option<String>,
        status: String,
        student_confirmed: Option<bool>,
        pre_submitted: bool,
        assigned_student_id: Option<String>,
    }

    let list = sqlx::query_as::<_, EnrollmentPendingRow>(
        r#"
        SELECT
            aa.id,
            aa.application_number,
            aa.national_id,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            at2.name AS track_name,
            cr.name AS room_name,
            aa.status,
            ara.student_confirmed,
            (aef.id IS NOT NULL AND aef.pre_submitted_at IS NOT NULL) AS pre_submitted,
            aa.assigned_student_id
        FROM admission_applications aa
        LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
        LEFT JOIN admission_room_assignments ara ON aa.id = ara.application_id
        LEFT JOIN class_rooms cr ON ara.class_room_id = cr.id
        LEFT JOIN admission_enrollment_forms aef ON aa.id = aef.application_id
        WHERE aa.admission_round_id = $1
          AND aa.status = 'accepted'
        ORDER BY at2.name ASC, ara.rank_in_room ASC
        "#
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({ "success": true, "data": list })).into_response())
}

/// POST /api/admission/applications/:id/enroll — ยืนยันมอบตัว + สร้าง account
pub async fn complete_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<CompleteEnrollmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let enroller_id = match check_permission(&headers, &pool, codes::ADMISSION_ENROLL, &state.permission_cache).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // 1. ดึงข้อมูล application + room
    let application = sqlx::query_as::<_, AdmissionApplication>(
        "SELECT aa.*, at2.name AS track_name, ar.name AS round_name FROM admission_applications aa LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id LEFT JOIN admission_rounds ar ON aa.admission_round_id = ar.id WHERE aa.id = $1"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or_else(|| AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;

    if application.status != "accepted" {
        return Err(AppError::BadRequest(
            format!("ใบสมัครมีสถานะ '{}' ไม่สามารถมอบตัวได้", application.status)
        ));
    }

    let class_room_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT class_room_id FROM admission_room_assignments WHERE application_id = $1"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or(None);

    let class_room_id = class_room_id
        .ok_or_else(|| AppError::BadRequest("ไม่พบข้อมูลห้องเรียน กรุณาตรวจสอบการจัดห้อง".to_string()))?;

    // 2. สร้าง User account
    // Priority: payload.student_code > application.assigned_student_id > auto-generate
    let student_code = if let Some(code) = payload.student_code.filter(|c| !c.is_empty()) {
        code
    } else if let Some(pre) = application.assigned_student_id.filter(|c| !c.is_empty()) {
        pre
    } else {
        // Auto-increment: หาเลข MAX ที่เป็น numeric แล้ว +1
        let max_id: i64 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(student_id::bigint), 0)
            FROM student_info
            WHERE student_id ~ '^\d+$'
            "#
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);

        (max_id + 1).to_string()
    };

    let username = student_code.clone();
    let password_hash = bcrypt::hash(&student_code, 8)
        .map_err(|_| AppError::InternalServerError("Password hash failed".to_string()))?;

    let gender_normalized: Option<String> = application.gender.as_deref().map(|g| {
        match g.to_lowercase().as_str() {
            "male" | "m" => "male",
            "female" | "f" => "female",
            _ => "other",
        }.to_string()
    });

    let new_user_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (
            username, national_id, password_hash,
            first_name, last_name, user_type, status,
            phone, date_of_birth, gender, address
        )
        VALUES ($1, $2, $3, $4, $5, 'student', 'active', $6, $7, $8, $9)
        RETURNING id
        "#
    )
    .bind(&username)
    .bind(&application.national_id)
    .bind(&password_hash)
    .bind(&application.first_name)
    .bind(&application.last_name)
    .bind(&application.phone)
    .bind(application.date_of_birth)
    .bind(&gender_normalized)
    .bind(&application.address_line)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to create user: {}", e);
        if e.to_string().contains("unique") {
            AppError::BadRequest("มี account นี้อยู่แล้วในระบบ".to_string())
        } else {
            AppError::InternalServerError("ไม่สามารถสร้าง account ได้".to_string())
        }
    })?;

    // 3. สร้าง student_info

    sqlx::query(
        r#"
        INSERT INTO student_info (user_id, student_id, enrollment_date)
        VALUES ($1, $2, CURRENT_DATE)
        ON CONFLICT (user_id) DO NOTHING
        "#
    )
    .bind(new_user_id)
    .bind(&student_code)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to create student_info: {}", e);
        AppError::InternalServerError("ไม่สามารถสร้างข้อมูลนักเรียนได้".to_string())
    })?;

    // 4. Enroll เข้า class_room
    sqlx::query(
        r#"
        INSERT INTO student_class_enrollments (student_id, class_room_id, enrollment_date, status)
        VALUES ($1, $2, CURRENT_DATE, 'active')
        ON CONFLICT (student_id, class_room_id) DO UPDATE SET status = 'active', updated_at = NOW()
        "#
    )
    .bind(new_user_id)
    .bind(class_room_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to enroll student: {}", e);
        AppError::InternalServerError("ไม่สามารถลงทะเบียนเข้าห้องเรียนได้".to_string())
    })?;

    // 5. ยืนยัน enrollment form (upsert ถ้า staff กรอกข้อมูลแทน)
    if let Some(fd) = payload.form_data {
        sqlx::query(
            r#"
            INSERT INTO admission_enrollment_forms (application_id, form_data, pre_submitted_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (application_id) DO UPDATE SET
                form_data = $2,
                pre_submitted_at = NOW()
            "#
        )
        .bind(id)
        .bind(fd)
        .execute(&mut *tx)
        .await
        .ok();
    }
    sqlx::query(
        r#"
        UPDATE admission_enrollment_forms
        SET completed_at = NOW(), completed_by = $1
        WHERE application_id = $2
        "#
    )
    .bind(enroller_id)
    .bind(id)
    .execute(&mut *tx)
    .await
    .ok(); // ไม่ error ถ้าไม่มี form

    // 6. อัปเดต application status
    sqlx::query(
        r#"
        UPDATE admission_applications
        SET status = 'enrolled',
            enrolled_by = $1,
            enrolled_at = NOW(),
            created_user_id = $2,
            updated_at = NOW()
        WHERE id = $3
        "#
    )
    .bind(enroller_id)
    .bind(new_user_id)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to update application status: {}", e);
        AppError::InternalServerError("ไม่สามารถอัปเดตสถานะได้".to_string())
    })?;

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": "มอบตัวสำเร็จ สร้าง account แล้ว",
        "data": {
            "user_id": new_user_id,
            "username": username,
            "student_code": student_code,
        }
    })).into_response())
}

/// PATCH /api/admission/applications/:id/track — ย้ายนักเรียนไปสายการเรียนอื่น
pub async fn change_application_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(payload): Json<ChangeTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }

    // อัปเดต room_assignment_track_id (override) — ไม่แตะ admission_track_id ที่นักเรียนสมัคร
    // None = ย้อนกลับสายที่สมัคร
    sqlx::query(
        "UPDATE admission_applications SET room_assignment_track_id = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(payload.track_id)
    .bind(application_id)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("ย้ายสายไม่สำเร็จ".to_string()))?;

    // ลบ room assignment เดิม เพราะต้องจัดใหม่
    sqlx::query(
        "DELETE FROM admission_room_assignments WHERE application_id = $1"
    )
    .bind(application_id)
    .execute(&pool)
    .await
    .ok();

    Ok(Json(json!({ "success": true })).into_response())
}

/// PATCH /api/admission/applications/:id/admission-track — แก้ไขสายที่สมัครจริง
pub async fn update_admission_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(payload): Json<UpdateAdmissionTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_VERIFY, &state.permission_cache).await {
        return Ok(r);
    }

    // เปลี่ยน admission_track_id (สายที่สมัครจริง) และล้าง override
    let result = sqlx::query(
        "UPDATE admission_applications SET admission_track_id = $1, room_assignment_track_id = NULL, updated_at = NOW() WHERE id = $2"
    )
    .bind(payload.track_id)
    .bind(application_id)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("แก้ไขสายการเรียนไม่สำเร็จ".to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบใบสมัคร".to_string()));
    }

    // ลบ room assignment เดิม เพราะสายเปลี่ยน
    sqlx::query("DELETE FROM admission_room_assignments WHERE application_id = $1")
        .bind(application_id)
        .execute(&pool)
        .await
        .ok();

    Ok(Json(json!({ "success": true })).into_response())
}

// ==========================================
// Staff Document Upload / Delete
// ==========================================

const VALID_DOC_TYPES: &[&str] = &[
    "photo_1_5inch", "transcript_por", "certificate_por7",
    "id_card_student", "id_card_father", "id_card_mother", "id_card_guardian",
    "house_reg_student", "house_reg_father", "house_reg_mother", "house_reg_guardian",
    "name_change_doc", "birth_cert",
];

const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "pdf", "webp"];

/// POST /api/admission/applications/{id}/documents
/// Staff upload/replace a document for an application (requires JWT auth)
pub async fn staff_upload_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    // Require authentication
    if let Err(r) = check_permission(&headers, &pool, "admission.update", &state.permission_cache).await {
        return Ok(r);
    }

    // Parse multipart
    let mut doc_type: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut original_filename: Option<String> = None;
    let mut mime_type = "application/octet-stream".to_string();

    while let Some(field) = multipart.next_field().await
        .map_err(|_| AppError::BadRequest("Invalid multipart data".to_string()))? {
        match field.name().unwrap_or("") {
            "doc_type" => {
                doc_type = Some(String::from_utf8_lossy(&field.bytes().await.unwrap_or_default()).to_string());
            }
            "file" => {
                original_filename = field.file_name().map(|s| s.to_string()).or(Some("document".to_string()));
                if let Some(ct) = field.content_type() {
                    mime_type = ct.to_string();
                }
                file_data = Some(field.bytes().await
                    .map_err(|_| AppError::BadRequest("Failed to read file".to_string()))?.to_vec());
            }
            _ => { let _ = field.bytes().await; }
        }
    }

    let doc_type = doc_type.ok_or_else(|| AppError::BadRequest("Missing doc_type".to_string()))?;
    let file_data = file_data.ok_or_else(|| AppError::BadRequest("Missing file".to_string()))?;
    let original_filename = original_filename.unwrap_or_else(|| "document".to_string());

    if !VALID_DOC_TYPES.contains(&doc_type.as_str()) {
        return Err(AppError::BadRequest(format!("Invalid doc_type: {}", doc_type)));
    }

    // Validate extension
    let ext = std::path::Path::new(&original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_lowercase();
    if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AppError::BadRequest(format!("File extension .{} not allowed. Use: jpg, png, pdf", ext)));
    }

    if file_data.len() > 20 * 1024 * 1024 {
        return Err(AppError::BadRequest("File size exceeds 20MB".to_string()));
    }

    // Verify application exists and fetch application_number + round_id
    let app_info: Option<(String, Uuid)> = sqlx::query_as(
        "SELECT application_number, admission_round_id FROM admission_applications WHERE id = $1"
    )
    .bind(application_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let (app_number, round_id) = app_info.ok_or(AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;

    // Fetch existing doc's storage_path before upload (for cleanup after success)
    let old_doc: Option<(String, Uuid)> = sqlx::query_as(
        r#"SELECT f.storage_path, f.id
           FROM admission_application_documents aad
           JOIN files f ON f.id = aad.file_id
           WHERE aad.application_id = $1 AND aad.doc_type = $2 AND aad.deleted_at IS NULL
           LIMIT 1"#
    )
    .bind(application_id)
    .bind(&doc_type)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    // Upload to R2
    let file_id = Uuid::new_v4();
    let storage_path = format!(
        "school-{}/admission/{}/{}/{}.{}",
        subdomain, round_id, app_number, file_id, ext
    );

    let r2_client = R2Client::new().await
        .map_err(|_| AppError::InternalServerError("Storage service unavailable".to_string()))?;

    // Upload new file first — if this fails, old file is still intact
    r2_client.upload_file(&storage_path, file_data.clone(), &mime_type).await
        .map_err(|_| AppError::InternalServerError("Failed to upload file".to_string()))?;

    // Save file metadata (permanent)
    let file_size = file_data.len() as i64;
    sqlx::query(
        r#"
        INSERT INTO files (id, user_id, school_id, filename, original_filename,
            file_size, mime_type, storage_path, file_type,
            is_temporary, is_public, uploaded_by)
        VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, 'document',
            false, false, NULL)
        "#
    )
    .bind(file_id)
    .bind(&subdomain)
    .bind(format!("{}.{}", file_id, ext))
    .bind(&original_filename)
    .bind(file_size)
    .bind(&mime_type)
    .bind(&storage_path)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to save file metadata: {}", e);
        AppError::InternalServerError("Failed to save file metadata".to_string())
    })?;

    // Soft-delete existing document record of same type
    sqlx::query(
        "UPDATE admission_application_documents SET deleted_at = NOW() WHERE application_id = $1 AND doc_type = $2 AND deleted_at IS NULL"
    )
    .bind(application_id)
    .bind(&doc_type)
    .execute(&pool)
    .await
    .ok();

    // Link new document to application
    let doc_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO admission_application_documents (id, application_id, file_id, doc_type)
        VALUES ($1, $2, $3, $4)
        "#
    )
    .bind(doc_id)
    .bind(application_id)
    .bind(file_id)
    .bind(&doc_type)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to link document: {}", e);
        AppError::InternalServerError("Failed to link document".to_string())
    })?;

    // DB update succeeded — now safe to delete old file from R2
    if let Some((old_path, _)) = old_doc {
        r2_client.delete_file(&old_path).await.ok();
    }

    let url_builder = FileUrlBuilder::new()
        .map_err(|_| AppError::InternalServerError("Configuration error".to_string()))?;
    let file_url = format!("{}/{}", url_builder.base_url(), storage_path);

    Ok(Json(json!({
        "success": true,
        "data": {
            "id": doc_id,
            "fileId": file_id,
            "docType": doc_type,
            "fileUrl": file_url,
            "fileSize": file_size,
        }
    })).into_response())
}

/// DELETE /api/admission/applications/{id}/documents/{doc_type}
/// Staff soft-delete a document for an application (requires JWT auth)
pub async fn staff_delete_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((application_id, doc_type)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    if let Err(r) = check_permission(&headers, &pool, "admission.update", &state.permission_cache).await {
        return Ok(r);
    }

    if !VALID_DOC_TYPES.contains(&doc_type.as_str()) {
        return Err(AppError::BadRequest(format!("Invalid doc_type: {}", doc_type)));
    }

    // Fetch storage_path + file_id before deleting
    let doc_info: Option<(String, Uuid)> = sqlx::query_as(
        r#"SELECT f.storage_path, f.id
           FROM admission_application_documents aad
           JOIN files f ON f.id = aad.file_id
           WHERE aad.application_id = $1 AND aad.doc_type = $2 AND aad.deleted_at IS NULL
           LIMIT 1"#
    )
    .bind(application_id)
    .bind(&doc_type)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    let Some((storage_path, file_id)) = doc_info else {
        return Err(AppError::NotFound("ไม่พบเอกสารที่ต้องการลบ".to_string()));
    };

    // Hard delete admission_application_documents record
    sqlx::query(
        "DELETE FROM admission_application_documents WHERE application_id = $1 AND doc_type = $2 AND deleted_at IS NULL"
    )
    .bind(application_id)
    .bind(&doc_type)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    // Hard delete files record
    sqlx::query("DELETE FROM files WHERE id = $1")
        .bind(file_id)
        .execute(&pool)
        .await
        .ok();

    // Delete file from R2
    let r2_client = R2Client::new().await
        .map_err(|_| AppError::InternalServerError("Storage service unavailable".to_string()))?;
    r2_client.delete_file(&storage_path).await.ok();

    Ok(Json(json!({ "success": true })).into_response())
}

// ==========================================
// Student ID Pre-Assignment
// ==========================================

/// POST /rounds/:id/sort-room-students — เรียง rank_in_room ใหม่ทุกห้องในรอบ (ชาย→หญิง, ก-ฮ)
pub async fn sort_room_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let updated = sqlx::query_scalar::<_, i64>(
        r#"
        WITH ranked AS (
            SELECT ara.application_id,
                   ROW_NUMBER() OVER (
                       PARTITION BY ara.class_room_id
                       ORDER BY
                           CASE WHEN aa.gender ILIKE 'male' OR aa.gender = 'ชาย' THEN 0
                                WHEN aa.gender ILIKE 'female' OR aa.gender = 'หญิง' THEN 1
                                ELSE 2 END,
                           aa.first_name,
                           aa.last_name
                   )::int AS new_rank
            FROM admission_room_assignments ara
            JOIN admission_applications aa ON aa.id = ara.application_id
            WHERE aa.admission_round_id = $1
        ),
        updated_rows AS (
            UPDATE admission_room_assignments ara
            SET rank_in_room = ranked.new_rank
            FROM ranked
            WHERE ara.application_id = ranked.application_id
            RETURNING 1
        )
        SELECT COUNT(*) FROM updated_rows
        "#
    )
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    Ok(Json(json!({ "success": true, "updated": updated })).into_response())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoAssignStudentIdsRequest {
    pub start_number: i64,
}

/// POST /rounds/:id/auto-assign-student-ids — กำหนดเลขประจำตัวอัตโนมัติ (เฉพาะที่ยังว่าง)
pub async fn auto_assign_student_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AutoAssignStudentIdsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    // 1. เก็บเลขที่ใช้ไปแล้วทั้งรอบ (สำหรับตรวจ collision)
    let existing: Vec<String> = sqlx::query_scalar(
        "SELECT assigned_student_id FROM admission_applications WHERE admission_round_id = $1 AND assigned_student_id IS NOT NULL AND assigned_student_id != ''"
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut occupied: std::collections::HashSet<i64> = existing
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();

    // 2. ดึง students ที่ยังไม่มีเลข เรียงตาม ห้อง → rank_in_room
    #[derive(sqlx::FromRow)]
    struct AppIdRow { application_id: Uuid }

    let students = sqlx::query_as::<_, AppIdRow>(
        r#"
        SELECT ara.application_id
        FROM admission_room_assignments ara
        JOIN admission_applications aa ON aa.id = ara.application_id
        LEFT JOIN class_rooms cr ON cr.id = ara.class_room_id
        WHERE aa.admission_round_id = $1
          AND (aa.assigned_student_id IS NULL OR aa.assigned_student_id = '')
          AND aa.status IN ('accepted', 'enrolled', 'scored')
        ORDER BY cr.name, ara.rank_in_room
        "#
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("auto_assign_student_ids fetch error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

    // 3. Assign เลขต่อเนื่อง หลีก collision
    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut next = payload.start_number;
    let mut assigned: i64 = 0;

    for student in &students {
        while occupied.contains(&next) {
            next += 1;
        }
        sqlx::query(
            "UPDATE admission_applications SET assigned_student_id = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(next.to_string())
        .bind(student.application_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

        occupied.insert(next);
        assigned += 1;
        next += 1;
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({ "success": true, "assigned": assigned })).into_response())
}

pub async fn list_student_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let rows = sqlx::query_as::<_, StudentIdRow>(
        r#"
        SELECT
            a.id AS application_id,
            a.application_number,
            a.assigned_student_id,
            concat(a.first_name, ' ', a.last_name) AS full_name,
            cr.name AS room_name,
            ra.rank_in_room,
            a.previous_school
        FROM admission_applications a
        JOIN admission_room_assignments ra ON ra.application_id = a.id
        LEFT JOIN class_rooms cr ON ra.class_room_id = cr.id
        WHERE a.admission_round_id = $1
          AND a.status IN ('accepted', 'enrolled')
        ORDER BY cr.name, ra.rank_in_room
        "#
    )
    .bind(round_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_student_ids error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}

/// PATCH /applications/:id/room — ย้ายห้องเรียนทีละคน (หลัง assign แล้ว)
pub async fn move_application_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<MoveRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }

    // ดึง old_room_id ก่อนย้าย
    let old_room_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT class_room_id FROM admission_room_assignments WHERE application_id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    if old_room_id.is_none() {
        return Err(AppError::BadRequest("ยังไม่มีการจัดห้อง กรุณาบันทึกการจัดห้องก่อน".to_string()));
    }

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // ย้ายห้อง
    sqlx::query(
        "UPDATE admission_room_assignments SET class_room_id = $1, assigned_at = NOW() WHERE application_id = $2"
    )
    .bind(payload.room_id)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("move_application_room error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

    // recalculate rank_in_room ของห้องใหม่ (เรียงตาม total_score DESC)
    sqlx::query(
        r#"
        UPDATE admission_room_assignments ara
        SET rank_in_room = ranked.new_rank
        FROM (
            SELECT application_id,
                   ROW_NUMBER() OVER (ORDER BY total_score DESC, rank_in_track ASC) AS new_rank
            FROM admission_room_assignments
            WHERE class_room_id = $1
        ) ranked
        WHERE ara.application_id = ranked.application_id
          AND ara.class_room_id = $1
        "#
    )
    .bind(payload.room_id)
    .execute(&mut *tx)
    .await.ok();

    // recalculate rank_in_room ของห้องเก่า (ถ้าต่างกัน)
    if let Some(old_id) = old_room_id {
        if old_id != payload.room_id {
            sqlx::query(
                r#"
                UPDATE admission_room_assignments ara
                SET rank_in_room = ranked.new_rank
                FROM (
                    SELECT application_id,
                           ROW_NUMBER() OVER (ORDER BY total_score DESC, rank_in_track ASC) AS new_rank
                    FROM admission_room_assignments
                    WHERE class_room_id = $1
                ) ranked
                WHERE ara.application_id = ranked.application_id
                  AND ara.class_room_id = $1
                "#
            )
            .bind(old_id)
            .execute(&mut *tx)
            .await.ok();
        }
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({ "success": true })).into_response())
}

pub async fn batch_update_student_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<Vec<UpdateStudentIdItem>>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut updated = 0i64;
    for item in &payload {
        let rows_affected = sqlx::query(
            r#"
            UPDATE admission_applications
            SET assigned_student_id = $1, updated_at = NOW()
            WHERE id = $2 AND admission_round_id = $3
            "#
        )
        .bind(&item.student_id)
        .bind(item.application_id)
        .bind(round_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("batch_update_student_ids error: {}", e);
            AppError::InternalServerError("Database error".to_string())
        })?
        .rows_affected();
        updated += rows_affected as i64;
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({ "success": true, "updated": updated })).into_response())
}
