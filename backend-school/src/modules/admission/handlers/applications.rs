use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
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

/// สร้างเลขที่ใบสมัคร running per round
/// Format: YYYY-NNNN  e.g. "2569-0001"
async fn generate_application_number(pool: &sqlx::PgPool, round_id: Uuid) -> Result<String, AppError> {
    // ดึง academic year บน round
    let year: i32 = sqlx::query_scalar(
        "SELECT ay.year FROM admission_rounds ar JOIN academic_years ay ON ar.academic_year_id = ay.id WHERE ar.id = $1"
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to get academic year".to_string()))?;

    // ใช้ MAX แทน COUNT เพื่อป้องกัน race condition
    let max_seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(CAST(SPLIT_PART(application_number, '-', 2) AS INT)), 0) FROM admission_applications WHERE admission_round_id = $1"
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    Ok(format!("{}-{:04}", year, max_seq + 1))
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

    // 3. ตรวจสอบ national_id ไม่ซ้ำในรอบนี้
    let already_applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM admission_applications WHERE national_id = $1 AND admission_round_id = $2)"
    )
    .bind(&payload.national_id)
    .bind(round_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(false);

    if already_applied {
        return Err(AppError::BadRequest("เลขบัตรประชาชนนี้ได้สมัครรอบนี้ไปแล้ว".to_string()));
    }

    // 4. สร้างเลขที่ใบสมัคร
    let application_number = generate_application_number(&pool, round_id).await?;

    // 5. Insert
    let application = sqlx::query_as::<_, AdmissionApplication>(
        r#"
        INSERT INTO admission_applications (
            admission_round_id, admission_track_id, application_number,
            national_id, title, first_name, last_name, gender, date_of_birth, phone, email,
            address_line, sub_district, district, province, postal_code,
            previous_school, previous_grade, previous_gpa,
            father_name, father_phone, father_occupation, father_national_id,
            mother_name, mother_phone, mother_occupation, mother_national_id,
            guardian_name, guardian_phone, guardian_relation, guardian_national_id
        )
        VALUES (
            $1, $2, $3,
            $4, $5, $6, $7, $8, $9, $10, $11,
            $12, $13, $14, $15, $16,
            $17, $18, $19,
            $20, $21, $22, $23,
            $24, $25, $26, $27,
            $28, $29, $30, $31
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
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to submit application: {}", e);
        AppError::InternalServerError("ไม่สามารถยื่นใบสมัครได้".to_string())
    })?;

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
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL).await {
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
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL).await {
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

    Ok(Json(json!({ "success": true, "data": application })).into_response())
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
    let verifier = match check_permission(&headers, &pool, codes::ADMISSION_VERIFY).await {
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
    .bind(verifier.id)
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
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_VERIFY).await {
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
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_ENROLL).await {
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
            (aef.id IS NOT NULL AND aef.pre_submitted_at IS NOT NULL) AS pre_submitted
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
    let enroller = match check_permission(&headers, &pool, codes::ADMISSION_ENROLL).await {
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
    let student_code = if let Some(code) = payload.student_code {
        code
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

    // 5. ยืนยัน enrollment form
    sqlx::query(
        r#"
        UPDATE admission_enrollment_forms
        SET completed_at = NOW(), completed_by = $1
        WHERE application_id = $2
        "#
    )
    .bind(enroller.id)
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
    .bind(enroller.id)
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
