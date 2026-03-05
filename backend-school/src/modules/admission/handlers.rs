use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use super::models::*;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc, NaiveDate};
use serde_json::json;
use uuid::Uuid;

// ==========================================
// Helper: get pool from headers
// ==========================================
macro_rules! get_pool {
    ($state:expr, $headers:expr) => {{
        let subdomain = extract_subdomain_from_request(&$headers)
            .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
        let db_url = get_school_database_url(&$state.admin_pool, &subdomain)
            .await
            .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        $state.pool_manager.get_pool(&db_url, &subdomain)
            .await
            .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?
    }};
}

// ==========================================
// Admission Periods Handlers
// ==========================================

pub async fn list_periods(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListAdmissionPeriodsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let mut query = String::from(
        "SELECT p.*,
                ay.name as academic_year_name,
                COUNT(a.id) as application_count,
                COUNT(a.id) FILTER (WHERE a.status = 'pending') as pending_count,
                COUNT(a.id) FILTER (WHERE a.status = 'accepted') as accepted_count,
                COUNT(a.id) FILTER (WHERE a.status = 'confirmed') as confirmed_count
         FROM admission_periods p
         JOIN academic_years ay ON p.academic_year_id = ay.id
         LEFT JOIN admission_applications a ON a.admission_period_id = p.id
         WHERE 1=1"
    );

    if let Some(year_id) = params.academic_year_id {
        query.push_str(&format!(" AND p.academic_year_id = '{}'", year_id));
    }
    if let Some(status) = &params.status {
        query.push_str(&format!(" AND p.status = '{}'", status));
    }

    query.push_str(" GROUP BY p.id, ay.name ORDER BY p.open_date DESC");

    let periods = sqlx::query_as::<_, AdmissionPeriod>(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("list_periods error: {e}");
            AppError::InternalServerError("ไม่สามารถโหลดรายการรอบรับสมัครได้".to_string())
        })?;

    Ok(Json(json!({ "success": true, "data": periods })))
}

pub async fn get_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let period = sqlx::query_as::<_, AdmissionPeriod>(
        "SELECT p.*,
                ay.name as academic_year_name,
                COUNT(a.id) as application_count,
                COUNT(a.id) FILTER (WHERE a.status = 'pending') as pending_count,
                COUNT(a.id) FILTER (WHERE a.status = 'accepted') as accepted_count,
                COUNT(a.id) FILTER (WHERE a.status = 'confirmed') as confirmed_count
         FROM admission_periods p
         JOIN academic_years ay ON p.academic_year_id = ay.id
         LEFT JOIN admission_applications a ON a.admission_period_id = p.id
         WHERE p.id = $1
         GROUP BY p.id, ay.name"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("get_period error: {e}");
        AppError::InternalServerError("ไม่สามารถโหลดข้อมูลรอบรับสมัครได้".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบรอบรับสมัคร".to_string()))?;

    Ok(Json(json!({ "success": true, "data": period })))
}

pub async fn create_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAdmissionPeriodRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    // Validate dates
    if payload.close_date <= payload.open_date {
        return Err(AppError::BadRequest("วันปิดรับสมัครต้องหลังวันเปิดรับสมัคร".to_string()));
    }

    let required_docs = payload.required_documents.unwrap_or_else(|| json!([]));
    let target_ids: Vec<String> = payload.target_grade_level_ids
        .unwrap_or_default()
        .iter()
        .map(|id| id.to_string())
        .collect();

    let period = sqlx::query_as::<_, AdmissionPeriod>(
        "INSERT INTO admission_periods 
            (academic_year_id, name, description, open_date, close_date, 
             announcement_date, confirmation_deadline, target_grade_level_ids,
             capacity_per_class, total_capacity, waitlist_capacity, 
             required_documents, application_fee, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8::uuid[],$9,$10,$11,$12,$13,'draft')
         RETURNING *,
            (SELECT name FROM academic_years WHERE id = $1) as academic_year_name,
            0::bigint as application_count,
            0::bigint as pending_count,
            0::bigint as accepted_count,
            0::bigint as confirmed_count"
    )
    .bind(payload.academic_year_id)
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(payload.open_date)
    .bind(payload.close_date)
    .bind(payload.announcement_date)
    .bind(payload.confirmation_deadline)
    .bind(&target_ids)
    .bind(payload.capacity_per_class)
    .bind(payload.total_capacity)
    .bind(payload.waitlist_capacity)
    .bind(&required_docs)
    .bind(payload.application_fee)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("create_period error: {e}");
        AppError::InternalServerError("ไม่สามารถสร้างรอบรับสมัครได้".to_string())
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": period }))))
}

pub async fn update_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAdmissionPeriodRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    // Validate status transition
    if let Some(ref new_status) = payload.status {
        let valid = ["draft", "open", "closed", "announced", "done"];
        if !valid.contains(&new_status.as_str()) {
            return Err(AppError::BadRequest(format!("สถานะ '{new_status}' ไม่ถูกต้อง")));
        }
    }

    let result = sqlx::query_as::<_, AdmissionPeriod>(
        "UPDATE admission_periods SET
            name                  = COALESCE($1, name),
            description           = COALESCE($2, description),
            open_date             = COALESCE($3, open_date),
            close_date            = COALESCE($4, close_date),
            announcement_date     = COALESCE($5, announcement_date),
            confirmation_deadline = COALESCE($6, confirmation_deadline),
            status                = COALESCE($7, status),
            capacity_per_class    = COALESCE($8, capacity_per_class),
            total_capacity        = COALESCE($9, total_capacity),
            waitlist_capacity     = COALESCE($10, waitlist_capacity),
            required_documents    = COALESCE($11, required_documents),
            updated_at            = NOW()
         WHERE id = $12
         RETURNING *,
            (SELECT name FROM academic_years WHERE id = academic_year_id) as academic_year_name,
            (SELECT COUNT(*) FROM admission_applications WHERE admission_period_id = $12) as application_count,
            (SELECT COUNT(*) FROM admission_applications WHERE admission_period_id = $12 AND status = 'pending') as pending_count,
            (SELECT COUNT(*) FROM admission_applications WHERE admission_period_id = $12 AND status = 'accepted') as accepted_count,
            (SELECT COUNT(*) FROM admission_applications WHERE admission_period_id = $12 AND status = 'confirmed') as confirmed_count"
    )
    .bind(&payload.name)
    .bind(&payload.description)
    .bind(payload.open_date)
    .bind(payload.close_date)
    .bind(payload.announcement_date)
    .bind(payload.confirmation_deadline)
    .bind(&payload.status)
    .bind(payload.capacity_per_class)
    .bind(payload.total_capacity)
    .bind(payload.waitlist_capacity)
    .bind(&payload.required_documents)
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_period error: {e}");
        AppError::InternalServerError("ไม่สามารถอัปเดตรอบรับสมัครได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": result })))
}

pub async fn delete_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    // Check if there are applications
    let app_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM admission_applications WHERE admission_period_id = $1"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    if app_count > 0 {
        return Err(AppError::BadRequest(
            format!("ไม่สามารถลบรอบรับสมัครได้ เนื่องจากมีใบสมัคร {app_count} ใบ")
        ));
    }

    sqlx::query("DELETE FROM admission_periods WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถลบรอบรับสมัครได้".to_string()))?;

    Ok(Json(json!({ "success": true, "message": "ลบรอบรับสมัครเรียบร้อยแล้ว" })))
}

pub async fn get_period_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let stats = sqlx::query_as::<_, AdmissionStats>(
        "SELECT 
            $1::uuid as period_id,
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE status = 'pending') as pending,
            COUNT(*) FILTER (WHERE status = 'reviewing') as reviewing,
            COUNT(*) FILTER (WHERE status = 'accepted') as accepted,
            COUNT(*) FILTER (WHERE status = 'rejected') as rejected,
            COUNT(*) FILTER (WHERE status = 'waitlisted') as waitlisted,
            COUNT(*) FILTER (WHERE status = 'confirmed') as confirmed,
            COUNT(*) FILTER (WHERE status = 'cancelled') as cancelled
         FROM admission_applications
         WHERE admission_period_id = $1"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("get_period_stats error: {e}");
        AppError::InternalServerError("ไม่สามารถโหลดสถิติได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": stats })))
}

// ==========================================
// Application Handlers
// ==========================================

pub async fn list_applications(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListApplicationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    let mut where_clause = String::from("WHERE 1=1");
    if let Some(period_id) = params.admission_period_id {
        where_clause.push_str(&format!(" AND a.admission_period_id = '{period_id}'"));
    }
    if let Some(ref status) = params.status {
        where_clause.push_str(&format!(" AND a.status = '{status}'"));
    }
    if let Some(ref search) = params.search {
        let s = search.replace('\'', "''");
        where_clause.push_str(&format!(
            " AND (a.applicant_first_name ILIKE '%{s}%' OR a.applicant_last_name ILIKE '%{s}%' OR a.application_number ILIKE '%{s}%' OR a.applicant_national_id ILIKE '%{s}%')"
        ));
    }

    let total: i64 = sqlx::query_scalar(
        &format!("SELECT COUNT(*) FROM admission_applications a {where_clause}")
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let applications = sqlx::query_as::<_, AdmissionApplication>(
        &format!(
            "SELECT a.*,
                    CASE gl.level_type 
                        WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                        WHEN 'primary' THEN CONCAT('ป.', gl.year)
                        WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                        ELSE CONCAT('?.', gl.year)
                    END as grade_level_name,
                    p.name as period_name,
                    CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name) as reviewer_name
             FROM admission_applications a
             LEFT JOIN grade_levels gl ON a.applying_grade_level_id = gl.id
             LEFT JOIN admission_periods p ON a.admission_period_id = p.id
             LEFT JOIN users u ON a.reviewed_by = u.id
             {where_clause}
             ORDER BY a.submitted_at DESC NULLS LAST, a.created_at DESC
             LIMIT $1 OFFSET $2"
        )
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_applications error: {e}");
        AppError::InternalServerError("ไม่สามารถโหลดรายการใบสมัครได้".to_string())
    })?;

    Ok(Json(json!({
        "success": true,
        "data": applications,
        "total": total,
        "page": page,
        "page_size": page_size,
        "total_pages": (total + page_size - 1) / page_size
    })))
}

pub async fn get_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let app = sqlx::query_as::<_, AdmissionApplication>(
        "SELECT a.*,
                CASE gl.level_type 
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name,
                p.name as period_name,
                CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name) as reviewer_name
         FROM admission_applications a
         LEFT JOIN grade_levels gl ON a.applying_grade_level_id = gl.id
         LEFT JOIN admission_periods p ON a.admission_period_id = p.id
         LEFT JOIN users u ON a.reviewed_by = u.id
         WHERE a.id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("get_application error: {e}");
        AppError::InternalServerError("ไม่สามารถโหลดใบสมัครได้".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;

    // Fetch documents as JSON values
    let documents: Vec<serde_json::Value> = sqlx::query(
        "SELECT id, application_id, document_key, document_label, file_url, file_name, file_size_bytes, mime_type, uploaded_at
         FROM admission_documents WHERE application_id = $1 ORDER BY uploaded_at"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "id": row.try_get::<Uuid, _>("id").ok().map(|u| u.to_string()),
            "document_key": row.try_get::<String, _>("document_key").unwrap_or_default(),
            "document_label": row.try_get::<Option<String>, _>("document_label").unwrap_or(None),
            "file_url": row.try_get::<String, _>("file_url").unwrap_or_default(),
            "file_name": row.try_get::<Option<String>, _>("file_name").unwrap_or(None)
        })
    })
    .collect();

    // Fetch interviews
    let interviews = sqlx::query_as::<_, AdmissionInterview>(
        "SELECT i.*,
                CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name) as interviewer_name,
                CONCAT(COALESCE($2::text, a.applicant_title, ''), a.applicant_first_name, ' ', a.applicant_last_name) as applicant_name
         FROM admission_interviews i
         LEFT JOIN users u ON i.interviewer_id = u.id
         LEFT JOIN admission_applications a ON i.application_id = a.id
         WHERE i.application_id = $1
         ORDER BY i.scheduled_at"
    )
    .bind(id)
    .bind(Option::<String>::None)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "success": true,
        "data": app,
        "documents": documents,
        "interviews": interviews
    })))
}

pub async fn create_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAdmissionApplicationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    // Verify period exists and is open
    let period_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM admission_periods WHERE id = $1"
    )
    .bind(payload.admission_period_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    match period_status.as_deref() {
        None => return Err(AppError::NotFound("ไม่พบรอบรับสมัคร".to_string())),
        Some("open") => {},
        Some(s) => return Err(AppError::BadRequest(format!("รอบรับสมัครมีสถานะ '{s}' ไม่สามารถยื่นสมัครได้"))),
    }

    // Generate application number: ADM-YYYY-XXXXX
    let year: i32 = sqlx::query_scalar(
        "SELECT EXTRACT(YEAR FROM open_date)::int FROM admission_periods WHERE id = $1"
    )
    .bind(payload.admission_period_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(2568);

    let seq: i64 = sqlx::query_scalar("SELECT nextval('admission_application_seq')")
        .fetch_one(&pool)
        .await
        .unwrap_or(1);

    let app_number = format!("ADM-{year}-{seq:05}");

    let app = sqlx::query_as::<_, AdmissionApplication>(
        "INSERT INTO admission_applications 
            (admission_period_id, application_number,
             applicant_first_name, applicant_last_name, applicant_title,
             applicant_national_id, applicant_date_of_birth, applicant_gender,
             applicant_nationality, applicant_religion, applicant_blood_type,
             applicant_phone, applicant_email, applicant_address,
             previous_school, previous_grade, previous_gpa,
             applying_grade_level_id, applying_classroom_preference,
             guardian_name, guardian_relationship, guardian_phone,
             guardian_email, guardian_occupation, guardian_national_id,
             status, submitted_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,'pending',NOW())
         RETURNING *,
            NULL::text as grade_level_name,
            (SELECT name FROM admission_periods WHERE id = $1) as period_name,
            NULL::text as reviewer_name"
    )
    .bind(payload.admission_period_id)
    .bind(&app_number)
    .bind(&payload.applicant_first_name)
    .bind(&payload.applicant_last_name)
    .bind(&payload.applicant_title)
    .bind(&payload.applicant_national_id)
    .bind(payload.applicant_date_of_birth)
    .bind(&payload.applicant_gender)
    .bind(&payload.applicant_nationality)
    .bind(&payload.applicant_religion)
    .bind(&payload.applicant_blood_type)
    .bind(&payload.applicant_phone)
    .bind(&payload.applicant_email)
    .bind(&payload.applicant_address)
    .bind(&payload.previous_school)
    .bind(&payload.previous_grade)
    .bind(payload.previous_gpa)
    .bind(payload.applying_grade_level_id)
    .bind(&payload.applying_classroom_preference)
    .bind(&payload.guardian_name)
    .bind(&payload.guardian_relationship)
    .bind(&payload.guardian_phone)
    .bind(&payload.guardian_email)
    .bind(&payload.guardian_occupation)
    .bind(&payload.guardian_national_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("create_application error: {e}");
        AppError::InternalServerError("ไม่สามารถสร้างใบสมัครได้".to_string())
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": app }))))
}

pub async fn update_application_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationStatusRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    // Get current status for audit
    let (old_status, _): (String, String) = sqlx::query_as(
        "SELECT status, application_number FROM admission_applications WHERE id = $1"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;

    let valid_statuses = ["pending", "reviewing", "interview_scheduled", "accepted", "rejected", "waitlisted", "confirmed", "cancelled"];
    if !valid_statuses.contains(&payload.status.as_str()) {
        return Err(AppError::BadRequest(format!("สถานะ '{}' ไม่ถูกต้อง", payload.status)));
    }

    let app = sqlx::query_as::<_, AdmissionApplication>(
        "UPDATE admission_applications SET
            status            = $1,
            staff_notes       = COALESCE($2, staff_notes),
            rejection_reason  = COALESCE($3, rejection_reason),
            interview_score   = COALESCE($4, interview_score),
            exam_score        = COALESCE($5, exam_score),
            total_score       = COALESCE($6, total_score),
            reviewed_by       = $7,
            reviewed_at       = NOW(),
            updated_at        = NOW()
         WHERE id = $8
         RETURNING *,
            NULL::text as grade_level_name,
            NULL::text as period_name,
            NULL::text as reviewer_name"
    )
    .bind(&payload.status)
    .bind(&payload.staff_notes)
    .bind(&payload.rejection_reason)
    .bind(payload.interview_score)
    .bind(payload.exam_score)
    .bind(payload.total_score)
    .bind(Option::<Uuid>::None) // TODO: get from auth middleware
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_application_status error: {e}");
        AppError::InternalServerError("ไม่สามารถอัปเดตสถานะใบสมัครได้".to_string())
    })?;

    // Audit log
    let _ = sqlx::query(
        "INSERT INTO admission_audit_logs (application_id, action, old_value, new_value, note)
         VALUES ($1, 'status_changed', $2, $3, $4)"
    )
    .bind(id)
    .bind(&old_status)
    .bind(&payload.status)
    .bind(&payload.staff_notes)
    .execute(&pool)
    .await;

    Ok(Json(json!({ "success": true, "data": app })))
}

pub async fn get_application_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let logs = sqlx::query_as::<_, AdmissionAuditLog>(
        "SELECT l.*,
                CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name) as performer_name
         FROM admission_audit_logs l
         LEFT JOIN users u ON l.performed_by = u.id
         WHERE l.application_id = $1
         ORDER BY l.performed_at DESC"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("get_application_logs error: {e}");
        AppError::InternalServerError("ไม่สามารถโหลดประวัติได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": logs })))
}

// ==========================================
// Interview Handlers
// ==========================================

pub async fn create_interview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateInterviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let interview = sqlx::query_as::<_, AdmissionInterview>(
        "INSERT INTO admission_interviews
            (application_id, interview_type, scheduled_at, location, interviewer_id, max_score, status)
         VALUES ($1, $2, $3, $4, $5, $6, 'scheduled')
         RETURNING *,
            NULL::text as interviewer_name,
            NULL::text as applicant_name"
    )
    .bind(payload.application_id)
    .bind(payload.interview_type.as_deref().unwrap_or("interview"))
    .bind(payload.scheduled_at)
    .bind(&payload.location)
    .bind(payload.interviewer_id)
    .bind(payload.max_score.unwrap_or(100.0))
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("create_interview error: {e}");
        AppError::InternalServerError("ไม่สามารถสร้างรายการสัมภาษณ์ได้".to_string())
    })?;

    // Update application status to interview_scheduled
    let _ = sqlx::query(
        "UPDATE admission_applications SET status = 'interview_scheduled', updated_at = NOW()
         WHERE id = $1 AND status = 'reviewing'"
    )
    .bind(payload.application_id)
    .execute(&pool)
    .await;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": interview }))))
}

pub async fn update_interview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateInterviewRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let interview = sqlx::query_as::<_, AdmissionInterview>(
        "UPDATE admission_interviews SET
            scheduled_at   = COALESCE($1, scheduled_at),
            location       = COALESCE($2, location),
            interviewer_id = COALESCE($3, interviewer_id),
            score          = COALESCE($4, score),
            notes          = COALESCE($5, notes),
            status         = COALESCE($6, status),
            updated_at     = NOW()
         WHERE id = $7
         RETURNING *, NULL::text as interviewer_name, NULL::text as applicant_name"
    )
    .bind(payload.scheduled_at)
    .bind(&payload.location)
    .bind(payload.interviewer_id)
    .bind(payload.score)
    .bind(&payload.notes)
    .bind(&payload.status)
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_interview error: {e}");
        AppError::InternalServerError("ไม่สามารถอัปเดตรายการสัมภาษณ์ได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": interview })))
}

// ==========================================
// Selection Handlers
// ==========================================

pub async fn list_selections(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let selections = sqlx::query_as::<_, AdmissionSelection>(
        "SELECT s.*,
                CONCAT(COALESCE(a.applicant_title, ''), a.applicant_first_name, ' ', a.applicant_last_name) as applicant_name,
                a.application_number,
                CASE gl.level_type 
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END as grade_level_name,
                a.total_score,
                CASE agl.level_type 
                    WHEN 'kindergarten' THEN CONCAT('อ.', agl.year)
                    WHEN 'primary' THEN CONCAT('ป.', agl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', agl.year)
                    ELSE CONCAT('?.', agl.year)
                END as applying_grade_level_name
         FROM admission_selections s
         JOIN admission_applications a ON s.application_id = a.id
         LEFT JOIN grade_levels gl ON s.assigned_grade_level_id = gl.id
         LEFT JOIN grade_levels agl ON a.applying_grade_level_id = agl.id
         WHERE s.admission_period_id = $1
         ORDER BY s.selection_type, s.rank NULLS LAST, a.total_score DESC NULLS LAST"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_selections error: {e}");
        AppError::InternalServerError("ไม่สามารถโหลดรายชื่อผู้ผ่านคัดเลือกได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": selections })))
}

pub async fn create_selections(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(period_id): Path<Uuid>,
    Json(payload): Json<CreateSelectionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let selection_type = payload.selection_type.as_deref().unwrap_or("main");
    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut created = 0i32;
    for (rank, app_id) in payload.application_ids.iter().enumerate() {
        let result = sqlx::query(
            "INSERT INTO admission_selections (application_id, admission_period_id, selection_type, rank, confirmation_deadline)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (application_id) DO UPDATE SET
                selection_type = EXCLUDED.selection_type,
                rank = EXCLUDED.rank,
                confirmation_deadline = EXCLUDED.confirmation_deadline,
                updated_at = NOW()"
        )
        .bind(app_id)
        .bind(period_id)
        .bind(selection_type)
        .bind((rank + 1) as i32)
        .bind(payload.confirmation_deadline)
        .execute(&mut *tx)
        .await;

        if result.is_ok() {
            // Update application status to accepted/waitlisted
            let new_status = if selection_type == "main" { "accepted" } else { "waitlisted" };
            let _ = sqlx::query(
                "UPDATE admission_applications SET status = $1, updated_at = NOW() WHERE id = $2"
            )
            .bind(new_status)
            .bind(app_id)
            .execute(&mut *tx)
            .await;
            created += 1;
        }
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok((StatusCode::CREATED, Json(json!({
        "success": true,
        "message": format!("เพิ่มรายชื่อผู้ผ่านคัดเลือก {created} ราย")
    }))))
}

pub async fn confirm_selection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let selection = sqlx::query_as::<_, AdmissionSelection>(
        "UPDATE admission_selections SET
            is_confirmed  = true,
            confirmed_at  = NOW(),
            updated_at    = NOW()
         WHERE id = $1
         RETURNING *,
            NULL::text as applicant_name, NULL::text as application_number,
            NULL::text as grade_level_name, NULL::text as applying_grade_level_name,
            NULL::text as classroom_name, NULL::text as classroom_code,
            NULL::text as study_plan_name, NULL::text as study_plan_version_name,
            NULL::numeric as app_total_score, NULL::text as checked_in_by_name,
            NULL::text as student_username, NULL::text as applicant_national_id,
            NULL::text as applicant_gender, NULL::date as applicant_date_of_birth,
            NULL::text as guardian_phone, NULL::text as guardian_name"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("confirm_selection error: {e}");
        AppError::InternalServerError("ไม่สามารถยืนยันสิทธิ์ได้".to_string())
    })?;

    // Update application status
    let _ = sqlx::query(
        "UPDATE admission_applications SET status = 'confirmed', updated_at = NOW()
         WHERE id = $1"
    )
    .bind(selection.application_id)
    .execute(&pool)
    .await;

    Ok(Json(json!({ "success": true, "data": selection })))
}

// ==========================================
// Generate Students from Confirmed Selections
// ==========================================

pub async fn generate_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(period_id): Path<Uuid>,
    Json(payload): Json<GenerateStudentsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    #[derive(sqlx::FromRow)]
    struct SelectionApp {
        selection_id: Uuid,
        applicant_first_name: String,
        applicant_last_name: String,
        applicant_title: Option<String>,
        applicant_national_id: Option<String>,
        applicant_date_of_birth: Option<NaiveDate>,
        applicant_gender: Option<String>,
        guardian_phone: Option<String>,
        guardian_email: Option<String>,
    }

    // Load confirmed selections without a student yet
    let selections = sqlx::query_as::<_, SelectionApp>(
        "SELECT s.id as selection_id,
                a.applicant_first_name, a.applicant_last_name, a.applicant_title,
                a.applicant_national_id, a.applicant_date_of_birth, a.applicant_gender,
                a.guardian_phone, a.guardian_email
         FROM admission_selections s
         JOIN admission_applications a ON s.application_id = a.id
         WHERE s.admission_period_id = $1
           AND s.is_confirmed = true
           AND s.student_user_id IS NULL
           AND ($2::uuid[] IS NULL OR s.id = ANY($2::uuid[]))"
    )
    .bind(period_id)
    .bind(&payload.selection_ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("generate_students query error: {e}");
        AppError::InternalServerError("ไม่สามารถโหลดรายชื่อได้".to_string())
    })?;

    if selections.is_empty() {
        return Ok(Json(json!({
            "success": true,
            "message": "ไม่มีรายชื่อที่ต้องสร้าง account",
            "created_count": 0
        })));
    }

    let password_prefix = payload.password_prefix.as_deref().unwrap_or("school");
    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut created_count = 0i32;
    let mut skipped_count = 0i32;

    // Get year for student ID generation
    let year: i32 = sqlx::query_scalar(
        "SELECT EXTRACT(YEAR FROM open_date)::int + 543 FROM admission_periods WHERE id = $1"
    )
    .bind(period_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(2568);

    let short_year = year % 100;

    for sel in &selections {
        // Generate username: first 2+2 chars of name + random
        let username_base = format!(
            "{}{}{}",
            short_year,
            sel.applicant_first_name.chars().take(3).collect::<String>(),
            sel.applicant_last_name.chars().take(2).collect::<String>()
        );
        let username = username_base.to_lowercase().replace(' ', "");

        // Make username unique by appending random digits
        let seq: i64 = sqlx::query_scalar("SELECT nextval('admission_application_seq')")
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(1);
        let final_username = format!("{username}{seq:03}");

        // Hash password (bcrypt)
        let plain_password = format!("{password_prefix}{seq:04}");
        let hashed = match bcrypt::hash(&plain_password, bcrypt::DEFAULT_COST) {
            Ok(h) => h,
            Err(_) => {
                skipped_count += 1;
                continue;
            }
        };

        // Check if username already exists (skip national_id check since it may be encrypted)
        let username_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)"
        )
        .bind(&final_username)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(false);

        if username_exists {
            skipped_count += 1;
            continue;
        }

        // Create user
        let user_id: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO users (username, password_hash, first_name, last_name, title,
                                national_id, date_of_birth, gender, phone, email, user_type, status)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'student', 'active')
             ON CONFLICT (username) DO NOTHING
             RETURNING id"
        )
        .bind(&final_username)
        .bind(&hashed)
        .bind(&sel.applicant_first_name)
        .bind(&sel.applicant_last_name)
        .bind(&sel.applicant_title)
        .bind(&sel.applicant_national_id)
        .bind(sel.applicant_date_of_birth)
        .bind(&sel.applicant_gender)
        .bind(&sel.guardian_phone)
        .bind(&sel.guardian_email)
        .fetch_optional(&mut *tx)
        .await
        .unwrap_or(None);

        if let Some(uid) = user_id {
            // Generate student_id
            let student_id = format!("{short_year}{seq:04}");

            // Create student_info
            let _ = sqlx::query(
                "INSERT INTO student_info (user_id, student_id) VALUES ($1, $2)"
            )
            .bind(uid)
            .bind(&student_id)
            .execute(&mut *tx)
            .await;

            // Assign STUDENT role
            let _ = sqlx::query(
                "INSERT INTO user_roles (user_id, role_id)
                 SELECT $1, id FROM roles WHERE code = 'STUDENT'
                 ON CONFLICT DO NOTHING"
            )
            .bind(uid)
            .execute(&mut *tx)
            .await;

            // Link selection to student
            let _ = sqlx::query(
                "UPDATE admission_selections SET student_user_id = $1, updated_at = NOW() WHERE id = $2"
            )
            .bind(uid)
            .bind(sel.selection_id)
            .execute(&mut *tx)
            .await;

            // If classroom_id provided, enroll student
            if let Some(classroom_id) = payload.classroom_id {
                let _ = sqlx::query(
                    "INSERT INTO student_class_enrollments (student_id, class_room_id, enrollment_date, status)
                     VALUES ($1, $2, NOW(), 'active')
                     ON CONFLICT DO NOTHING"
                )
                .bind(uid)
                .bind(classroom_id)
                .execute(&mut *tx)
                .await;
            }

            created_count += 1;
        } else {
            skipped_count += 1;
        }
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": format!("สร้าง account สำเร็จ {created_count} ราย, ข้ามไป {skipped_count} ราย"),
        "created_count": created_count,
        "skipped_count": skipped_count
    })))
}

// ==========================================
// Exam Subjects Handlers (NEW)
// ==========================================

pub async fn list_exam_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(period_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let subjects = sqlx::query_as::<_, AdmissionExamSubject>(
        "SELECT * FROM admission_exam_subjects
         WHERE admission_period_id = $1
         ORDER BY display_order, created_at"
    )
    .bind(period_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_exam_subjects error: {e}");
        AppError::InternalServerError("ไม่สามารถโหลดรายวิชาสอบได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": subjects })))
}

pub async fn create_exam_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(period_id): Path<Uuid>,
    Json(payload): Json<UpsertExamSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let subject = sqlx::query_as::<_, AdmissionExamSubject>(
        "INSERT INTO admission_exam_subjects
            (admission_period_id, subject_name, subject_code, max_score, display_order, is_active)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *"
    )
    .bind(period_id)
    .bind(&payload.subject_name)
    .bind(&payload.subject_code)
    .bind(payload.max_score.unwrap_or(100.0))
    .bind(payload.display_order.unwrap_or(0))
    .bind(payload.is_active.unwrap_or(true))
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("create_exam_subject error: {e}");
        AppError::InternalServerError("ไม่สามารถสร้างรายวิชาได้".to_string())
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": subject }))))
}

pub async fn update_exam_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((_period_id, subject_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpsertExamSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let subject = sqlx::query_as::<_, AdmissionExamSubject>(
        "UPDATE admission_exam_subjects SET
            subject_name  = $1,
            subject_code  = COALESCE($2, subject_code),
            max_score     = COALESCE($3, max_score),
            display_order = COALESCE($4, display_order),
            is_active     = COALESCE($5, is_active),
            updated_at    = NOW()
         WHERE id = $6
         RETURNING *"
    )
    .bind(&payload.subject_name)
    .bind(&payload.subject_code)
    .bind(payload.max_score)
    .bind(payload.display_order)
    .bind(payload.is_active)
    .bind(subject_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_exam_subject error: {e}");
        AppError::InternalServerError("ไม่สามารถอัปเดตรายวิชาได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": subject })))
}

pub async fn delete_exam_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((_period_id, subject_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    sqlx::query("DELETE FROM admission_exam_subjects WHERE id = $1")
        .bind(subject_id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("ลบรายวิชาไม่สำเร็จ".to_string()))?;

    Ok(Json(json!({ "success": true })))
}

// ==========================================
// Exam Scores Handlers (NEW)
// ==========================================

/// โหลดคะแนนทั้งหมดของรอบนี้ (grouped by application)
pub async fn list_scores_by_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(period_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    // subjects
    let subjects = sqlx::query_as::<_, AdmissionExamSubject>(
        "SELECT * FROM admission_exam_subjects
         WHERE admission_period_id = $1 AND is_active = true
         ORDER BY display_order"
    )
    .bind(period_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // applications accepted/waitlisted/confirmed with their scores
    let rows = sqlx::query(
        "SELECT a.id as app_id,
                a.application_number,
                CONCAT(COALESCE(a.applicant_title,''), a.applicant_first_name, ' ', a.applicant_last_name) as name,
                a.status,
                a.applying_grade_level_id,
                CASE gl.level_type
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary'      THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary'    THEN CONCAT('ม.', gl.year)
                    ELSE gl.name
                END as grade_level_name,
                COALESCE(
                    jsonb_object_agg(es.exam_subject_id::text, es.score)
                    FILTER (WHERE es.exam_subject_id IS NOT NULL),
                    '{}'
                ) as score_map,
                COALESCE(SUM(es.score), 0) as computed_total
         FROM admission_applications a
         LEFT JOIN grade_levels gl ON a.applying_grade_level_id = gl.id
         LEFT JOIN admission_exam_scores es ON es.application_id = a.id
         WHERE a.admission_period_id = $1
           AND a.status IN ('accepted', 'waitlisted', 'confirmed', 'reviewing', 'interview_scheduled')
         GROUP BY a.id, a.application_number, a.applicant_title, a.applicant_first_name,
                  a.applicant_last_name, a.status, a.applying_grade_level_id,
                  gl.level_type, gl.year, gl.name
         ORDER BY computed_total DESC NULLS LAST, a.applicant_first_name"
    )
    .bind(period_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_scores_by_period error: {e}");
        AppError::InternalServerError("โหลดคะแนนไม่สำเร็จ".to_string())
    })?;

    use sqlx::Row;
    let applications: Vec<serde_json::Value> = rows.into_iter().map(|row| {
        json!({
            "app_id": row.try_get::<Uuid, _>("app_id").ok().map(|u| u.to_string()),
            "application_number": row.try_get::<String, _>("application_number").unwrap_or_default(),
            "name": row.try_get::<String, _>("name").unwrap_or_default(),
            "status": row.try_get::<String, _>("status").unwrap_or_default(),
            "grade_level_name": row.try_get::<Option<String>, _>("grade_level_name").unwrap_or(None),
            "score_map": row.try_get::<serde_json::Value, _>("score_map").unwrap_or(json!({})),
            "computed_total": row.try_get::<f64, _>("computed_total").unwrap_or(0.0),
        })
    }).collect();

    Ok(Json(json!({
        "success": true,
        "subjects": subjects,
        "applications": applications
    })))
}

/// Batch upsert scores + optional recalculate total
pub async fn batch_upsert_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<BatchUpsertScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut upserted = 0i32;
    for entry in &payload.scores {
        sqlx::query(
            "INSERT INTO admission_exam_scores (application_id, exam_subject_id, score)
             VALUES ($1, $2, $3)
             ON CONFLICT (application_id, exam_subject_id)
             DO UPDATE SET score = EXCLUDED.score, updated_at = NOW()"
        )
        .bind(entry.application_id)
        .bind(entry.exam_subject_id)
        .bind(entry.score)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("batch_upsert_scores error: {e}");
            AppError::InternalServerError("บันทึกคะแนนไม่สำเร็จ".to_string())
        })?;
        upserted += 1;
    }

    // Recalculate total_score for affected applications
    if payload.recalculate_total.unwrap_or(true) {
        let app_ids: Vec<Uuid> = payload.scores.iter()
            .map(|s| s.application_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter().collect();

        for app_id in &app_ids {
            // Build subject filter
            let subject_filter = if let Some(ref ids) = payload.total_subject_ids {
                let id_strs: Vec<String> = ids.iter().map(|u| format!("'{}'", u)).collect();
                format!("AND es.exam_subject_id IN ({})", id_strs.join(","))
            } else {
                String::new()
            };

            let _ = sqlx::query(
                &format!("UPDATE admission_applications SET
                    total_score = (
                        SELECT COALESCE(SUM(es.score), 0)
                        FROM admission_exam_scores es
                        WHERE es.application_id = $1 {subject_filter}
                    ),
                    updated_at = NOW()
                 WHERE id = $1")
            )
            .bind(app_id)
            .execute(&mut *tx)
            .await;
        }
    }

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({ "success": true, "upserted": upserted })))
}

// ==========================================
// Selection Update Handler (NEW)
// ==========================================

pub async fn update_selection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSelectionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let sel = sqlx::query_as::<_, AdmissionSelection>(
        "UPDATE admission_selections SET
            rank                   = COALESCE($1, rank),
            study_plan_version_id  = COALESCE($2, study_plan_version_id),
            assigned_classroom_id  = COALESCE($3, assigned_classroom_id),
            notes                  = COALESCE($4, notes),
            updated_at             = NOW()
         WHERE id = $5
         RETURNING *, NULL::text as applicant_name, NULL::text as application_number,
            NULL::text as grade_level_name, NULL::text as applying_grade_level_name,
            NULL::text as classroom_name, NULL::text as classroom_code,
            NULL::text as study_plan_name, NULL::text as study_plan_version_name,
            NULL::numeric as app_total_score, NULL::text as checked_in_by_name,
            NULL::text as student_username, NULL::text as applicant_national_id,
            NULL::text as applicant_gender, NULL::date as applicant_date_of_birth,
            NULL::text as guardian_phone, NULL::text as guardian_name"
    )
    .bind(payload.rank)
    .bind(payload.study_plan_version_id)
    .bind(payload.assigned_classroom_id)
    .bind(&payload.notes)
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_selection error: {e}");
        AppError::InternalServerError("ไม่สามารถอัปเดตรายชื่อผู้ผ่านได้".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": sel })))
}

// ==========================================
// Check-in Handlers (NEW)
// ==========================================

/// List selections สำหรับหน้า checkin (ค้นหา + filter)
pub async fn list_checkins(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(period_id): Path<Uuid>,
    Query(params): Query<ListSelectionsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let mut where_parts = vec![
        "s.admission_period_id = $1".to_string(),
        "s.is_confirmed = true".to_string(),
    ];

    if let Some(ref status) = params.checkin_status {
        where_parts.push(format!("s.checkin_status = '{}'", status.replace('\'', "''")));
    }
    if let Some(ref search) = params.search {
        let s = search.replace('\'', "''");
        where_parts.push(format!(
            "(a.applicant_first_name ILIKE '%{s}%' OR a.applicant_last_name ILIKE '%{s}%' OR a.application_number ILIKE '%{s}%')"
        ));
    }

    // Sort by subset of subjects or by total_score
    let sort_col = if let Some(ref subject_csv) = params.sort_subject_ids {
        let ids: Vec<&str> = subject_csv.split(',').map(|s| s.trim()).collect();
        let ids_sql: Vec<String> = ids.iter().map(|id| format!("'{}'", id)).collect();
        format!(
            "(SELECT COALESCE(SUM(es.score),0) FROM admission_exam_scores es WHERE es.application_id = a.id AND es.exam_subject_id::text IN ({}))",
            ids_sql.join(",")
        )
    } else {
        "COALESCE(a.total_score, 0)".to_string()
    };

    let sort_dir = match params.sort_dir.as_deref() {
        Some("asc") => "ASC",
        _ => "DESC",
    };

    let where_clause = where_parts.join(" AND ");

    let rows = sqlx::query(
        &format!(
            "SELECT s.*,
                    CONCAT(COALESCE(a.applicant_title,''), a.applicant_first_name, ' ', a.applicant_last_name) as applicant_name,
                    a.application_number,
                    a.applicant_national_id,
                    a.applicant_gender,
                    a.applicant_date_of_birth,
                    a.guardian_phone,
                    a.guardian_name,
                    a.total_score as app_total_score,
                    CASE agl.level_type
                        WHEN 'kindergarten' THEN CONCAT('อ.', agl.year)
                        WHEN 'primary'      THEN CONCAT('ป.', agl.year)
                        WHEN 'secondary'    THEN CONCAT('ม.', agl.year)
                        ELSE agl.name
                    END as applying_grade_level_name,
                    cr.name  as classroom_name,
                    cr.code  as classroom_code,
                    sp.name  as study_plan_name,
                    spv.name as study_plan_version_name,
                    CONCAT(COALESCE(cu.title,''), cu.first_name, ' ', cu.last_name) as checked_in_by_name,
                    su.username as student_username,
                    NULL::text as grade_level_name
             FROM admission_selections s
             JOIN admission_applications a   ON s.application_id        = a.id
             LEFT JOIN grade_levels agl      ON a.applying_grade_level_id = agl.id
             LEFT JOIN class_rooms cr        ON s.assigned_classroom_id  = cr.id
             LEFT JOIN study_plan_versions spv ON s.study_plan_version_id = spv.id
             LEFT JOIN study_plans sp        ON spv.study_plan_id        = sp.id
             LEFT JOIN users cu              ON s.checked_in_by          = cu.id
             LEFT JOIN users su              ON s.student_user_id        = su.id
             WHERE {where_clause}
             ORDER BY {sort_col} {sort_dir} NULLS LAST, a.applicant_first_name"
        )
    )
    .bind(period_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("list_checkins error: {e}");
        AppError::InternalServerError("โหลดรายชื่อรายงานตัวไม่สำเร็จ".to_string())
    })?;

    use sqlx::Row as SRow;
    let data: Vec<serde_json::Value> = rows.into_iter().map(|row| {
        json!({
            "id":                      row.try_get::<Uuid,_>("id").ok().map(|u|u.to_string()),
            "application_id":         row.try_get::<Uuid,_>("application_id").ok().map(|u|u.to_string()),
            "selection_type":          row.try_get::<String,_>("selection_type").unwrap_or_default(),
            "rank":                    row.try_get::<Option<i32>,_>("rank").unwrap_or(None),
            "checkin_status":          row.try_get::<String,_>("checkin_status").unwrap_or_default(),
            "checked_in_at":           row.try_get::<Option<DateTime<Utc>>,_>("checked_in_at").unwrap_or(None),
            "checked_in_by_name":      row.try_get::<Option<String>,_>("checked_in_by_name").unwrap_or(None),
            "checkin_notes":           row.try_get::<Option<String>,_>("checkin_notes").unwrap_or(None),
            "student_user_id":         row.try_get::<Option<Uuid>,_>("student_user_id").ok().flatten().map(|u|u.to_string()),
            "student_username":        row.try_get::<Option<String>,_>("student_username").unwrap_or(None),
            "is_confirmed":            row.try_get::<bool,_>("is_confirmed").unwrap_or(false),
            "applicant_name":          row.try_get::<Option<String>,_>("applicant_name").unwrap_or(None),
            "application_number":      row.try_get::<Option<String>,_>("application_number").unwrap_or(None),
            "applicant_national_id":   row.try_get::<Option<String>,_>("applicant_national_id").unwrap_or(None),
            "applicant_gender":        row.try_get::<Option<String>,_>("applicant_gender").unwrap_or(None),
            "guardian_phone":          row.try_get::<Option<String>,_>("guardian_phone").unwrap_or(None),
            "guardian_name":           row.try_get::<Option<String>,_>("guardian_name").unwrap_or(None),
            "applying_grade_level_name": row.try_get::<Option<String>,_>("applying_grade_level_name").unwrap_or(None),
            "classroom_name":          row.try_get::<Option<String>,_>("classroom_name").unwrap_or(None),
            "classroom_code":          row.try_get::<Option<String>,_>("classroom_code").unwrap_or(None),
            "study_plan_name":         row.try_get::<Option<String>,_>("study_plan_name").unwrap_or(None),
            "study_plan_version_name": row.try_get::<Option<String>,_>("study_plan_version_name").unwrap_or(None),
            "app_total_score":         row.try_get::<Option<f64>,_>("app_total_score").unwrap_or(None),
        })
    }).collect();

    Ok(Json(json!({ "success": true, "data": data })))
}

/// สถิติรายงานตัว
pub async fn get_checkin_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(period_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    let stats = sqlx::query_as::<_, CheckinStats>(
        "SELECT
            $1::uuid as period_id,
            COUNT(*) FILTER (WHERE is_confirmed = true)                 as total_confirmed,
            COUNT(*) FILTER (WHERE is_confirmed = true AND checkin_status = 'pending')     as pending_checkin,
            COUNT(*) FILTER (WHERE is_confirmed = true AND checkin_status = 'checked_in')  as checked_in,
            COUNT(*) FILTER (WHERE is_confirmed = true AND checkin_status = 'absent')      as absent
         FROM admission_selections
         WHERE admission_period_id = $1"
    )
    .bind(period_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("get_checkin_stats error: {e}");
        AppError::InternalServerError("โหลดสถิติรายงานตัวไม่สำเร็จ".to_string())
    })?;

    Ok(Json(json!({ "success": true, "data": stats })))
}

/// ครูยืนยันรายงานตัว → สร้าง student account ทันที
pub async fn confirm_checkin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(selection_id): Path<Uuid>,
    Json(payload): Json<CheckinRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    // โหลดข้อมูล selection + application
    #[derive(sqlx::FromRow)]
    struct SelInfo {
        application_id: Uuid,
        admission_period_id: Uuid,
        is_confirmed: bool,
        checkin_status: String,
        student_user_id: Option<Uuid>,
        assigned_classroom_id: Option<Uuid>,
        applicant_first_name: String,
        applicant_last_name: String,
        applicant_title: Option<String>,
        applicant_national_id: Option<String>,
        applicant_date_of_birth: Option<NaiveDate>,
        applicant_gender: Option<String>,
        guardian_phone: Option<String>,
        guardian_email: Option<String>,
    }

    let info = sqlx::query_as::<_, SelInfo>(
        "SELECT s.id, s.application_id, s.admission_period_id,
                s.is_confirmed, s.checkin_status, s.student_user_id,
                s.assigned_classroom_id,
                a.applicant_first_name, a.applicant_last_name, a.applicant_title,
                a.applicant_national_id, a.applicant_date_of_birth, a.applicant_gender,
                a.guardian_phone, a.guardian_email
         FROM admission_selections s
         JOIN admission_applications a ON s.application_id = a.id
         WHERE s.id = $1"
    )
    .bind(selection_id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or(AppError::NotFound("ไม่พบรายชื่อผู้ผ่านคัดเลือก".to_string()))?;

    if !info.is_confirmed {
        return Err(AppError::BadRequest("นักเรียนยังไม่ได้ยืนยันสิทธิ์".to_string()));
    }
    if info.checkin_status == "checked_in" {
        return Err(AppError::BadRequest("รายงานตัวแล้ว".to_string()));
    }
    if info.student_user_id.is_some() {
        return Err(AppError::BadRequest("มี account อยู่แล้ว".to_string()));
    }

    let mut tx = pool.begin().await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    // Generate username & password
    let year: i32 = sqlx::query_scalar(
        "SELECT EXTRACT(YEAR FROM open_date)::int + 543 FROM admission_periods WHERE id = $1"
    )
    .bind(info.admission_period_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(2568);

    let short_year = year % 100;

    let seq: i64 = sqlx::query_scalar("SELECT nextval('admission_application_seq')")
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(1);

    let username_base = format!(
        "{}{}{}",
        short_year,
        info.applicant_first_name.chars().take(3).collect::<String>(),
        info.applicant_last_name.chars().take(2).collect::<String>()
    );
    let final_username = format!("{}{seq:03}", username_base.to_lowercase().replace(' ', ""));
    let plain_password = format!("school{seq:04}");
    let student_id_str = format!("{short_year}{seq:04}");

    let hashed = bcrypt::hash(&plain_password, bcrypt::DEFAULT_COST)
        .map_err(|_| AppError::InternalServerError("Password hash failed".to_string()))?;

    // Create user
    let user_id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, first_name, last_name, title,
                            national_id, date_of_birth, gender, phone, email, user_type, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'student','active')
         ON CONFLICT (username) DO NOTHING
         RETURNING id"
    )
    .bind(&final_username)
    .bind(&hashed)
    .bind(&info.applicant_first_name)
    .bind(&info.applicant_last_name)
    .bind(&info.applicant_title)
    .bind(&info.applicant_national_id)
    .bind(info.applicant_date_of_birth)
    .bind(&info.applicant_gender)
    .bind(&info.guardian_phone)
    .bind(&info.guardian_email)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("confirm_checkin create user error: {e}");
        AppError::InternalServerError("สร้างบัญชีผู้ใช้ไม่สำเร็จ".to_string())
    })?
    .flatten();

    let uid = user_id.ok_or_else(|| AppError::BadRequest(
        format!("ชื่อผู้ใช้ '{final_username}' ซ้ำกับในระบบ กรุณาลองใหม่")
    ))?;

    // student_info
    let _ = sqlx::query(
        "INSERT INTO student_info (user_id, student_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
    )
    .bind(uid)
    .bind(&student_id_str)
    .execute(&mut *tx)
    .await;

    // STUDENT role
    let _ = sqlx::query(
        "INSERT INTO user_roles (user_id, role_id)
         SELECT $1, id FROM roles WHERE code = 'STUDENT'
         ON CONFLICT DO NOTHING"
    )
    .bind(uid)
    .execute(&mut *tx)
    .await;

    // Enroll in classroom if set
    if let Some(cr_id) = info.assigned_classroom_id {
        let _ = sqlx::query(
            "INSERT INTO student_class_enrollments (student_id, class_room_id, enrollment_date, status)
             VALUES ($1, $2, CURRENT_DATE, 'active')
             ON CONFLICT DO NOTHING"
        )
        .bind(uid)
        .bind(cr_id)
        .execute(&mut *tx)
        .await;
    }

    // Update selection: checkin_status = checked_in, student_user_id = uid
    sqlx::query(
        "UPDATE admission_selections SET
            checkin_status  = 'checked_in',
            checked_in_at   = NOW(),
            student_user_id = $1,
            checkin_notes   = $2,
            updated_at      = NOW()
         WHERE id = $3"
    )
    .bind(uid)
    .bind(&payload.notes)
    .bind(selection_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError("อัปเดตสถานะรายงานตัวไม่สำเร็จ".to_string()))?;

    // Update application status
    let _ = sqlx::query(
        "UPDATE admission_applications SET status = 'confirmed', updated_at = NOW() WHERE id = $1"
    )
    .bind(info.application_id)
    .execute(&mut *tx)
    .await;

    tx.commit().await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(Json(json!({
        "success": true,
        "message": "รายงานตัวและสร้างบัญชีนักเรียนเรียบร้อยแล้ว",
        "student_user_id": uid.to_string(),
        "username": final_username,
        "password": plain_password,
        "student_id": student_id_str
    })))
}

/// Mark as absent
pub async fn mark_absent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(selection_id): Path<Uuid>,
    Json(payload): Json<CheckinRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool!(state, headers);

    sqlx::query(
        "UPDATE admission_selections SET
            checkin_status = 'absent',
            checked_in_at  = NOW(),
            checkin_notes  = $1,
            updated_at     = NOW()
         WHERE id = $2"
    )
    .bind(&payload.notes)
    .bind(selection_id)
    .execute(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("อัปเดตสถานะไม่สำเร็จ".to_string()))?;

    Ok(Json(json!({ "success": true, "message": "บันทึกไม่มารายงานตัวแล้ว" })))
}
