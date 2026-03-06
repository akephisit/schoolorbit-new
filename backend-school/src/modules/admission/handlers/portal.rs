use axum::{
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::modules::admission::models::applications::*;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// Helper: ตรวจสอบ credentials และคืน application id
/// พารามิเตอร์: national_id + date_of_birth (DDMMYYYY เช่น "20082543")
async fn verify_credentials(
    pool: &sqlx::PgPool,
    national_id: &str,
    date_of_birth: &str,
) -> Result<uuid::Uuid, AppError> {
    if date_of_birth.len() != 8 {
        return Err(AppError::BadRequest("รูปแบบวันเกิดไม่ถูกต้อง (ต้องกรอก 8 หลัก ววดดปปปป เช่น 20082543)".to_string()));
    }

    let year_be: i32 = date_of_birth[4..].parse().unwrap_or(0);
    // แปลง พ.ศ. เป็น ค.ศ.
    let year_ce = year_be - 543;

    // แปลง DDMMYYYY (พ.ศ.) เป็น NaiveDate
    let dob = chrono::NaiveDate::parse_from_str(
        &format!("{}/{}/{}", &date_of_birth[0..2], &date_of_birth[2..4], year_ce), 
        "%d/%m/%Y"
    ).ok();
    
    let Some(dob) = dob else {
        return Err(AppError::BadRequest("รูปแบบวันเกิดไม่ถูกต้อง (กรอก ววดดปปปป พ.ศ. เช่น 20082543)".to_string()));
    };

    let application_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM admission_applications WHERE national_id = $1 AND date_of_birth = $2 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(national_id)
    .bind(dob)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    application_id.ok_or_else(|| AppError::AuthError("ไม่พบข้อมูลผู้สมัคร กรุณาตรวจสอบเลขบัตรประชาชนและวันเกิด".to_string()))
}

// ==========================================
// Portal Endpoints (Stateless - Credentials ทุก request)
// ==========================================

/// POST /api/admission/portal/check
/// ตรวจสอบว่า national_id + application_number ถูกต้องหรือเปล่า
pub async fn check_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let application_id = verify_credentials(&pool, &payload.national_id, &payload.date_of_birth).await?;

    #[derive(sqlx::FromRow, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct StatusRow {
        id: uuid::Uuid,
        application_number: Option<String>,
        first_name: String,
        last_name: String,
        status: String,
        track_name: Option<String>,
        round_name: Option<String>,
        round_status: Option<String>,
    }

    let info = sqlx::query_as::<_, StatusRow>(
        r#"
        SELECT
            aa.id,
            aa.application_number,
            aa.first_name,
            aa.last_name,
            aa.status,
            at2.name AS track_name,
            ar.name  AS round_name,
            ar.status AS round_status
        FROM admission_applications aa
        LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
        LEFT JOIN admission_rounds ar ON aa.admission_round_id = ar.id
        WHERE aa.id = $1
        "#
    )
    .bind(application_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": "ตรวจสอบสำเร็จ",
        "data": info,
    })).into_response())
}

/// POST /api/admission/portal/status
/// ดูสถานะ + ผลการสมัคร (คะแนน, ห้อง) พร้อม credentials
pub async fn get_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let application_id = verify_credentials(&pool, &payload.national_id, &payload.date_of_birth).await?;

    // ดึงข้อมูล application
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
    .bind(application_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    // ดึงผลการจัดห้อง (ถ้ามี)
    #[derive(sqlx::FromRow, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct AssignmentRow {
        rank_in_track: Option<i32>,
        rank_in_room: Option<i32>,
        total_score: Option<f64>,
        room_name: Option<String>,
        student_confirmed: bool,
    }

    let assignment = sqlx::query_as::<_, AssignmentRow>(
        r#"
        SELECT
            ara.rank_in_track,
            ara.rank_in_room,
            ara.total_score,
            cr.name AS room_name,
            ara.student_confirmed
        FROM admission_room_assignments ara
        LEFT JOIN class_rooms cr ON ara.class_room_id = cr.id
        WHERE ara.application_id = $1
        "#
    )
    .bind(application_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    // ดึงคะแนน (ถ้ากรอกแล้ว)
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
        WHERE aes.admission_round_id = $2
        ORDER BY aes.display_order ASC
        "#
    )
    .bind(application_id)
    .bind(application.admission_round_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // form ที่กรอกไว้
    let form = sqlx::query_as::<_, EnrollmentForm>(
        "SELECT * FROM admission_enrollment_forms WHERE application_id = $1"
    )
    .bind(application_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    Ok(Json(json!({
        "success": true,
        "data": {
            "application": application,
            "assignment": assignment,
            "scores": scores,
            "enrollmentForm": form,
        }
    })).into_response())
}

/// POST /api/admission/portal/confirm
/// ยืนยันเข้าเรียน (student_confirmed = true)
pub async fn confirm_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalConfirmRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let application_id = verify_credentials(&pool, &payload.national_id, &payload.date_of_birth).await?;

    // ตรวจสอบสถานะ
    let status: String = sqlx::query_scalar(
        "SELECT status FROM admission_applications WHERE id = $1"
    )
    .bind(application_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    if status != "accepted" {
        return Err(AppError::BadRequest(
            format!("ไม่สามารถยืนยันได้ (สถานะปัจจุบัน: {})", status)
        ));
    }

    sqlx::query(
        "UPDATE admission_room_assignments SET student_confirmed = true, student_confirmed_at = NOW() WHERE application_id = $1"
    )
    .bind(application_id)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to confirm".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": "ยืนยันเข้าเรียนแล้ว กรุณากรอกแบบฟอร์มมอบตัวด้านล่าง",
    })).into_response())
}

/// POST /api/admission/portal/form — ดูแบบฟอร์มมอบตัว
pub async fn get_enrollment_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalCredentials>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let application_id = verify_credentials(&pool, &payload.national_id, &payload.date_of_birth).await?;

    let form = sqlx::query_as::<_, EnrollmentForm>(
        "SELECT * FROM admission_enrollment_forms WHERE application_id = $1"
    )
    .bind(application_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    Ok(Json(json!({ "success": true, "data": form })).into_response())
}

/// PUT /api/admission/portal/form — กรอกแบบฟอร์มมอบตัว
pub async fn submit_enrollment_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PortalFormRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let application_id = verify_credentials(&pool, &payload.national_id, &payload.date_of_birth).await?;

    // ตรวจสอบว่ายืนยันแล้วหรือยัง (student_confirmed = true)
    let confirmed: bool = sqlx::query_scalar(
        "SELECT COALESCE(student_confirmed, false) FROM admission_room_assignments WHERE application_id = $1"
    )
    .bind(application_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None)
    .unwrap_or(false);

    if !confirmed {
        return Err(AppError::BadRequest("กรุณายืนยันเข้าเรียนก่อนกรอกแบบฟอร์ม".to_string()));
    }

    let form_data = payload.form_data.unwrap_or(json!({}));

    sqlx::query(
        r#"
        INSERT INTO admission_enrollment_forms (application_id, form_data, pre_submitted_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (application_id) DO UPDATE SET
            form_data = $2,
            pre_submitted_at = NOW()
        "#
    )
    .bind(application_id)
    .bind(form_data)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to submit enrollment form: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกแบบฟอร์มได้".to_string())
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "บันทึกแบบฟอร์มมอบตัวแล้ว",
    })).into_response())
}

/// PUT /api/admission/portal/application
/// แก้ไขใบสมัคร (เฉพาะสถานะ submitted หรือ rejected)
pub async fn update_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdatePortalApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let application_id = verify_credentials(&pool, &payload.auth_national_id, &payload.auth_date_of_birth).await?;

    // ตรวจสอบสถานะก่อนแก้
    let status: String = sqlx::query_scalar(
        "SELECT status FROM admission_applications WHERE id = $1"
    )
    .bind(application_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    if status != "submitted" && status != "rejected" {
        return Err(AppError::BadRequest(
            format!("ไม่สามารถแก้ไขใบสมัครได้เนื่องจากอยู่ในสถานะ '{}'", status)
        ));
    }

    // ตรวจสอบ national_id แอบเปลี่ยนไปซ้ำกับใบอื่นรอบนี้ไหม (ถ้ามีการแก้เลขบัตร)
    if payload.data.national_id != payload.auth_national_id {
        let round_id: uuid::Uuid = sqlx::query_scalar(
            "SELECT admission_round_id FROM admission_applications WHERE id = $1"
        )
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap_or_default();

        let already_applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM admission_applications WHERE national_id = $1 AND admission_round_id = $2 AND id != $3)"
        )
        .bind(&payload.data.national_id)
        .bind(round_id)
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(false);

        if already_applied {
            return Err(AppError::BadRequest("เลขบัตรประชาชนใหม่ที่กรอกได้สมัครรอบนี้ไปแล้ว (ซ้ำ)".to_string()));
        }
    }

    // อัปเดตข้อมูล
    sqlx::query(
        r#"
        UPDATE admission_applications SET
            admission_track_id = $1, title = $2, first_name = $3, last_name = $4,
            gender = $5, phone = $6, email = $7,
            address_line = $8, sub_district = $9, district = $10, province = $11, postal_code = $12,
            previous_school = $13, previous_grade = $14, previous_gpa = $15,
            father_name = $16, father_phone = $17, father_occupation = $18, father_national_id = $19,
            mother_name = $20, mother_phone = $21, mother_occupation = $22, mother_national_id = $23,
            guardian_name = $24, guardian_phone = $25, guardian_relation = $26, guardian_national_id = $27,
            national_id = $28, date_of_birth = $29,
            status = 'submitted', -- คืนสถานะกลับเป็น Submitted เพื่อให้ครูรอตรวจใหม่
            rejection_reason = NULL,
            updated_at = NOW()
        WHERE id = $30
        "#
    )
    .bind(payload.data.admission_track_id)
    .bind(&payload.data.title)
    .bind(&payload.data.first_name)
    .bind(&payload.data.last_name)
    .bind(&payload.data.gender)
    .bind(&payload.data.phone)
    .bind(&payload.data.email)
    .bind(&payload.data.address_line)
    .bind(&payload.data.sub_district)
    .bind(&payload.data.district)
    .bind(&payload.data.province)
    .bind(&payload.data.postal_code)
    .bind(&payload.data.previous_school)
    .bind(&payload.data.previous_grade)
    .bind(payload.data.previous_gpa)
    .bind(&payload.data.father_name)
    .bind(&payload.data.father_phone)
    .bind(&payload.data.father_occupation)
    .bind(&payload.data.father_national_id)
    .bind(&payload.data.mother_name)
    .bind(&payload.data.mother_phone)
    .bind(&payload.data.mother_occupation)
    .bind(&payload.data.mother_national_id)
    .bind(&payload.data.guardian_name)
    .bind(&payload.data.guardian_phone)
    .bind(&payload.data.guardian_relation)
    .bind(&payload.data.guardian_national_id)
    .bind(&payload.data.national_id)
    .bind(payload.data.date_of_birth)
    .bind(application_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update application: {}", e);
        AppError::InternalServerError("ไม่สามารถแก้ไขใบสมัครได้".to_string())
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "แก้ไขและอัปเดตใบสมัครเรียบร้อยแล้ว",
    })).into_response())
}

