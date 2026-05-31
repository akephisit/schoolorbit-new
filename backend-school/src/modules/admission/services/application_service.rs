use crate::error::AppError;
use crate::modules::admission::models::applications::*;
use crate::modules::admission::services::pii;
use crate::utils::file_url::FileUrlBuilder;
use chrono::{Datelike, FixedOffset, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

fn pii_error(context: &str, error: String) -> AppError {
    eprintln!("Admission PII {} failed: {}", context, error);
    AppError::InternalServerError("ไม่สามารถประมวลผลข้อมูลส่วนบุคคลได้".to_string())
}

fn decrypt_application(
    mut application: AdmissionApplication,
) -> Result<AdmissionApplication, AppError> {
    pii::decrypt_application_pii(&mut application)
        .map_err(|error| pii_error("decrypt application", error))?;
    Ok(application)
}

fn decrypt_national_id(value: &mut String) -> Result<(), AppError> {
    *value =
        pii::decrypt_required(value).map_err(|error| pii_error("decrypt national_id", error))?;
    Ok(())
}

fn decrypt_optional_national_id(value: &mut Option<String>) -> Result<(), AppError> {
    *value = pii::decrypt_optional(value.as_deref())
        .map_err(|error| pii_error("decrypt optional national_id", error))?;
    Ok(())
}

// ==========================================
// Public submit
// ==========================================

pub async fn submit_application(
    pool: &PgPool,
    round_id: Uuid,
    payload: SubmitApplicationRequest,
) -> Result<(String, AdmissionApplication), AppError> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM admission_rounds WHERE id = $1")
            .bind(round_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    match status.as_deref() {
        None => return Err(AppError::NotFound("ไม่พบรอบรับสมัคร".to_string())),
        Some("open") => {}
        Some(s) => {
            return Err(AppError::BadRequest(format!(
                "รอบรับสมัครไม่ได้เปิดรับ (สถานะ: {})",
                s
            )))
        }
    }

    let track_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM admission_tracks WHERE id = $1 AND admission_round_id = $2)",
    )
    .bind(payload.admission_track_id)
    .bind(round_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !track_exists {
        return Err(AppError::BadRequest("สายการเรียนไม่ถูกต้อง".to_string()));
    }

    let encrypted_pii = pii::encrypt_application_pii(
        &payload.national_id,
        payload.father_national_id.as_deref(),
        payload.mother_national_id.as_deref(),
        payload.guardian_national_id.as_deref(),
    )
    .map_err(|error| pii_error("encrypt submit application", error))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let (year, round_number): (i32, i64) = sqlx::query_as(
        r#"SELECT ay.year,
                  ROW_NUMBER() OVER (PARTITION BY ar.academic_year_id ORDER BY ar.created_at ASC)
           FROM admission_rounds ar
           JOIN academic_years ay ON ar.academic_year_id = ay.id
           WHERE ar.id = $1"#,
    )
    .bind(round_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to get academic year".to_string()))?;

    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(year as i64)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::InternalServerError("Lock failed".to_string()))?;

    let already_applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM admission_applications WHERE national_id_hash = $1 AND admission_round_id = $2)",
    )
    .bind(&encrypted_pii.national_id_hash)
    .bind(round_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(false);

    if already_applied {
        return Err(AppError::BadRequest(
            "เลขบัตรประชาชนนี้ได้สมัครรอบนี้ไปแล้ว".to_string(),
        ));
    }

    let _ = year;
    let bangkok = FixedOffset::east_opt(7 * 3600).ok_or_else(|| {
        AppError::InternalServerError("ไม่สามารถสร้าง timezone offset ได้".to_string())
    })?;
    let now = Utc::now().with_timezone(&bangkok);
    let be_year = now.year() + 543;
    let app_prefix = format!(
        "{:02}{:02}{:02}{:02}",
        be_year % 100,
        now.month(),
        now.day(),
        round_number
    );
    let app_pattern = format!("{}%", app_prefix);

    let max_seq: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(MAX(
            CASE WHEN application_number ~ '^[0-9]{13}$'
            THEN CAST(SUBSTRING(application_number, 9, 5) AS BIGINT)
            ELSE 0::BIGINT END
        ), 0::BIGINT) FROM admission_applications WHERE application_number LIKE $1"#,
    )
    .bind(&app_pattern)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| {
        AppError::InternalServerError("Failed to compute application number".to_string())
    })?;

    let application_number = format!("{}{:05}", app_prefix, max_seq + 1);

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
            parent_status, parent_status_other,
            national_id_hash, father_national_id_hash, mother_national_id_hash, guardian_national_id_hash
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
            $56, $57,
            $58, $59, $60, $61
        )
        RETURNING *,
            (SELECT name FROM admission_tracks WHERE id = $2) AS track_name,
            NULL::text AS assigned_track_name,
            (SELECT name FROM admission_rounds WHERE id = $1) AS round_name
        "#,
    )
    .bind(round_id)
    .bind(payload.admission_track_id)
    .bind(&application_number)
    .bind(&encrypted_pii.national_id)
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
    .bind(&encrypted_pii.father_national_id)
    .bind(&payload.mother_name)
    .bind(&payload.mother_phone)
    .bind(&payload.mother_occupation)
    .bind(&encrypted_pii.mother_national_id)
    .bind(&payload.guardian_name)
    .bind(&payload.guardian_phone)
    .bind(&payload.guardian_relation)
    .bind(&encrypted_pii.guardian_national_id)
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
    .bind(&encrypted_pii.national_id_hash)
    .bind(&encrypted_pii.father_national_id_hash)
    .bind(&encrypted_pii.mother_national_id_hash)
    .bind(&encrypted_pii.guardian_national_id_hash)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to submit application: {}", e);
        AppError::InternalServerError("ไม่สามารถยื่นใบสมัครได้".to_string())
    })?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    let application = decrypt_application(application)?;

    Ok((application_number, application))
}

// ==========================================
// Staff: List & Get
// ==========================================

#[derive(sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppListRow {
    pub id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub track_name: Option<String>,
    pub status: String,
    pub phone: Option<String>,
    pub previous_school: Option<String>,
    pub previous_gpa: Option<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_applications(
    pool: &PgPool,
    round_id: Uuid,
    filter: ApplicationFilter,
) -> Result<Vec<AppListRow>, AppError> {
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
        WHERE aa.admission_round_id = "#,
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
            let national_id_hash = pii::hash_required(search)
                .map_err(|error| pii_error("hash application list search national_id", error))?;
            query.push(" AND (");
            query.push("aa.national_id_hash = ");
            query.push_bind(national_id_hash);
            query.push(" OR ");
            query.push("aa.first_name ILIKE ");
            query.push_bind(like_term.clone());
            query.push(" OR aa.last_name ILIKE ");
            query.push_bind(like_term.clone());
            query.push(" OR aa.application_number ILIKE ");
            query.push_bind(like_term);
            query.push(")");
        }
    }
    query.push(" ORDER BY aa.created_at ASC");

    let mut rows = query
        .build_query_as::<AppListRow>()
        .fetch_all(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to list applications: {}", e);
            AppError::InternalServerError("Failed to fetch applications".to_string())
        })?;

    for row in &mut rows {
        decrypt_national_id(&mut row.national_id)?;
    }

    Ok(rows)
}

pub async fn get_application_with_documents(
    pool: &PgPool,
    id: Uuid,
) -> Result<(AdmissionApplication, Vec<ApplicationDocument>), AppError> {
    let application = sqlx::query_as::<_, AdmissionApplication>(
        r#"
        SELECT aa.*,
               at2.name     AS track_name,
               NULL::text   AS assigned_track_name,
               ar.name      AS round_name
        FROM admission_applications aa
        LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
        LEFT JOIN admission_rounds ar ON aa.admission_round_id = ar.id
        WHERE aa.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch application {}: {}", id, e);
        AppError::InternalServerError("Failed to fetch application".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;
    let application = decrypt_application(application)?;

    let documents = sqlx::query_as::<_, ApplicationDocument>(
        r#"
        SELECT d.id, d.application_id, d.file_id, d.doc_type, d.created_at, d.deleted_at,
               f.storage_path AS file_url,
               f.original_filename, f.file_size, f.mime_type
        FROM admission_application_documents d
        JOIN files f ON f.id = d.file_id
        WHERE d.application_id = $1 AND d.deleted_at IS NULL
        ORDER BY d.created_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(pool)
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

    Ok((application, documents))
}

// ==========================================
// Verify / Reject / Absent / Update / Unverify / Delete
// ==========================================

pub async fn verify_application(
    pool: &PgPool,
    id: Uuid,
    verifier_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"
        UPDATE admission_applications
        SET status = 'verified', verified_by = $1, verified_at = NOW(),
            rejection_reason = NULL, updated_at = NOW()
        WHERE id = $2 AND status = 'submitted'
        "#,
    )
    .bind(verifier_id)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to verify application: {}", e);
        AppError::InternalServerError("ไม่สามารถยืนยันใบสมัครได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "ไม่พบใบสมัคร หรือสถานะไม่ใช่ 'รอตรวจสอบ'".to_string(),
        ));
    }
    Ok(())
}

pub async fn reject_application(pool: &PgPool, id: Uuid, reason: &str) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE admission_applications
        SET status = 'rejected', rejection_reason = $1, updated_at = NOW()
        WHERE id = $2 AND status NOT IN ('enrolled', 'withdrawn')
        "#,
    )
    .bind(reason)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to reject application: {}", e);
        AppError::InternalServerError("ไม่สามารถปฏิเสธใบสมัครได้".to_string())
    })?;
    Ok(())
}

pub async fn mark_absent(pool: &PgPool, id: Uuid, absent: bool) -> Result<(), AppError> {
    if absent {
        sqlx::query(
            "UPDATE admission_applications SET status = 'absent', updated_at = NOW() WHERE id = $1 AND status IN ('verified', 'scored')"
        )
        .bind(id)
        .execute(pool)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถทำเครื่องหมายขาดสอบได้".to_string()))?;
    } else {
        sqlx::query(
            "UPDATE admission_applications SET status = 'verified', updated_at = NOW() WHERE id = $1 AND status = 'absent'"
        )
        .bind(id)
        .execute(pool)
        .await
        .map_err(|_| AppError::InternalServerError("ไม่สามารถยกเลิกขาดสอบได้".to_string()))?;
    }
    Ok(())
}

pub async fn update_application(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateApplicationRequest,
) -> Result<(), AppError> {
    let father_national_id = pii::encrypt_optional(payload.father_national_id.as_deref())
        .map_err(|error| pii_error("encrypt father national_id", error))?;
    let father_national_id_hash = pii::hash_optional(payload.father_national_id.as_deref());
    let mother_national_id = pii::encrypt_optional(payload.mother_national_id.as_deref())
        .map_err(|error| pii_error("encrypt mother national_id", error))?;
    let mother_national_id_hash = pii::hash_optional(payload.mother_national_id.as_deref());
    let guardian_national_id = pii::encrypt_optional(payload.guardian_national_id.as_deref())
        .map_err(|error| pii_error("encrypt guardian national_id", error))?;
    let guardian_national_id_hash = pii::hash_optional(payload.guardian_national_id.as_deref());

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
            father_national_id_hash = $54, mother_national_id_hash = $55, guardian_national_id_hash = $56,
            updated_at = NOW()
        WHERE id = $57 AND status NOT IN ('enrolled', 'withdrawn')
        "#,
    )
    .bind(&payload.title).bind(&payload.first_name).bind(&payload.last_name)
    .bind(&payload.gender).bind(payload.date_of_birth).bind(&payload.phone).bind(&payload.email)
    .bind(&payload.religion).bind(&payload.ethnicity).bind(&payload.nationality)
    .bind(&payload.address_line).bind(&payload.sub_district).bind(&payload.district)
    .bind(&payload.province).bind(&payload.postal_code)
    .bind(&payload.home_house_no).bind(&payload.home_moo).bind(&payload.home_soi)
    .bind(&payload.home_road).bind(&payload.home_phone)
    .bind(&payload.current_house_no).bind(&payload.current_moo).bind(&payload.current_soi)
    .bind(&payload.current_road).bind(&payload.current_sub_district).bind(&payload.current_district)
    .bind(&payload.current_province).bind(&payload.current_postal_code).bind(&payload.current_phone)
    .bind(&payload.previous_school).bind(&payload.previous_grade).bind(payload.previous_gpa)
    .bind(&payload.previous_study_year).bind(&payload.previous_school_province)
    .bind(&payload.father_name).bind(&payload.father_phone).bind(&payload.father_occupation)
    .bind(&father_national_id).bind(payload.father_income)
    .bind(&payload.mother_name).bind(&payload.mother_phone).bind(&payload.mother_occupation)
    .bind(&mother_national_id).bind(payload.mother_income)
    .bind(&payload.guardian_name).bind(&payload.guardian_phone).bind(&payload.guardian_relation)
    .bind(&guardian_national_id).bind(&payload.guardian_occupation).bind(payload.guardian_income)
    .bind(&payload.guardian_is).bind(&payload.parent_status).bind(&payload.parent_status_other)
    .bind(&father_national_id_hash).bind(&mother_national_id_hash).bind(&guardian_national_id_hash)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to update application {}: {}", id, e);
        AppError::InternalServerError("ไม่สามารถแก้ไขใบสมัครได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "ไม่พบใบสมัคร หรือไม่สามารถแก้ไขได้ (สถานะเป็น enrolled หรือ withdrawn)".to_string(),
        ));
    }
    Ok(())
}

pub async fn unverify_application(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"
        UPDATE admission_applications
        SET status = 'submitted', verified_by = NULL, verified_at = NULL, updated_at = NOW()
        WHERE id = $1 AND status = 'verified'
        "#,
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to unverify application {}: {}", id, e);
        AppError::InternalServerError("ไม่สามารถยกเลิกการอนุมัติได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::BadRequest(
            "ไม่พบใบสมัคร หรือสถานะไม่ใช่ 'ผ่านการตรวจสอบ'".to_string(),
        ));
    }
    Ok(())
}

/// Return files (id, storage_path) ที่ต้องลบ R2 — handler รับผิดชอบ R2 cleanup
pub async fn fetch_application_files_then_delete(
    pool: &PgPool,
    id: Uuid,
) -> Result<Vec<(Uuid, String)>, AppError> {
    let app_number: Option<String> =
        sqlx::query_scalar("SELECT application_number FROM admission_applications WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                eprintln!("Failed to fetch application {}: {}", id, e);
                AppError::InternalServerError("ไม่สามารถลบใบสมัครได้".to_string())
            })?;

    if app_number.is_none() {
        return Err(AppError::NotFound("ไม่พบใบสมัคร".to_string()));
    }

    let file_rows: Vec<(Uuid, String)> = sqlx::query_as(
        r#"SELECT f.id, f.storage_path
           FROM admission_application_documents aad
           JOIN files f ON f.id = aad.file_id
           WHERE aad.application_id = $1 AND aad.deleted_at IS NULL"#,
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if !file_rows.is_empty() {
        let file_ids: Vec<Uuid> = file_rows.iter().map(|(fid, _)| *fid).collect();
        sqlx::query("DELETE FROM files WHERE id = ANY($1)")
            .bind(&file_ids)
            .execute(pool)
            .await
            .ok();
    }

    let result = sqlx::query("DELETE FROM admission_applications WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            eprintln!("Failed to delete application {}: {}", id, e);
            AppError::InternalServerError("ไม่สามารถลบใบสมัครได้".to_string())
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบใบสมัคร".to_string()));
    }

    Ok(file_rows)
}

// ==========================================
// Enrollment
// ==========================================

#[derive(sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollmentPendingRow {
    pub id: Uuid,
    pub application_number: Option<String>,
    pub national_id: String,
    pub full_name: String,
    pub track_name: Option<String>,
    pub room_name: Option<String>,
    pub status: String,
    pub student_confirmed: Option<bool>,
    pub pre_submitted: bool,
    pub assigned_student_id: Option<String>,
    pub form_data: Option<serde_json::Value>,
}

pub async fn list_enrollment_pending(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<EnrollmentPendingRow>, AppError> {
    let mut list = sqlx::query_as::<_, EnrollmentPendingRow>(
        r#"
        SELECT
            aa.id, aa.application_number, aa.national_id,
            CONCAT(COALESCE(aa.title, ''), aa.first_name, ' ', aa.last_name) AS full_name,
            at2.name AS track_name,
            cr.name AS room_name,
            aa.status,
            ara.student_confirmed,
            (aef.id IS NOT NULL AND aef.pre_submitted_at IS NOT NULL) AS pre_submitted,
            aa.assigned_student_id,
            aef.form_data
        FROM admission_applications aa
        LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
        JOIN admission_room_assignments ara ON aa.id = ara.application_id
        LEFT JOIN class_rooms cr ON ara.class_room_id = cr.id
        LEFT JOIN admission_enrollment_forms aef ON aa.id = aef.application_id
        WHERE aa.admission_round_id = $1
          AND aa.status IN ('accepted', 'enrolled')
        ORDER BY at2.name ASC, ara.rank_in_room ASC
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for row in &mut list {
        decrypt_national_id(&mut row.national_id)?;
    }

    Ok(list)
}

pub struct EnrollmentResult {
    pub user_id: Uuid,
    pub username: String,
    pub student_code: String,
}

pub async fn complete_enrollment(
    pool: &PgPool,
    id: Uuid,
    payload: CompleteEnrollmentRequest,
    enroller_id: Uuid,
) -> Result<EnrollmentResult, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let application = sqlx::query_as::<_, AdmissionApplication>(
        "SELECT aa.*, at2.name AS track_name, NULL::text AS assigned_track_name, ar.name AS round_name FROM admission_applications aa LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id LEFT JOIN admission_rounds ar ON aa.admission_round_id = ar.id WHERE aa.id = $1"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or_else(|| AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;

    if application.status != "accepted" {
        return Err(AppError::BadRequest(format!(
            "ใบสมัครมีสถานะ '{}' ไม่สามารถมอบตัวได้",
            application.status
        )));
    }

    let class_room_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT class_room_id FROM admission_room_assignments WHERE application_id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .unwrap_or(None);

    let class_room_id = class_room_id
        .ok_or_else(|| AppError::BadRequest("ไม่พบข้อมูลห้องเรียน กรุณาตรวจสอบการจัดห้อง".to_string()))?;

    let student_code = if let Some(code) = payload.student_code.filter(|c| !c.is_empty()) {
        code
    } else if let Some(pre) = application
        .assigned_student_id
        .clone()
        .filter(|c| !c.is_empty())
    {
        pre
    } else {
        let max_id: i64 = sqlx::query_scalar(
            r#"SELECT COALESCE(MAX(student_id::bigint), 0) FROM student_info WHERE student_id ~ '^\d+$'"#,
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
        }
        .to_string()
    });

    let new_user_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (
            username, national_id, national_id_hash, password_hash,
            first_name, last_name, user_type, status,
            phone, date_of_birth, gender, address
        )
        VALUES ($1, $2, $3, $4, $5, $6, 'student', 'active', $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(&username)
    .bind(&application.national_id)
    .bind(&application.national_id_hash)
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

    sqlx::query(
        r#"
        INSERT INTO student_info (user_id, student_id, enrollment_date)
        VALUES ($1, $2, CURRENT_DATE)
        ON CONFLICT (user_id) DO NOTHING
        "#,
    )
    .bind(new_user_id)
    .bind(&student_code)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to create student_info: {}", e);
        AppError::InternalServerError("ไม่สามารถสร้างข้อมูลนักเรียนได้".to_string())
    })?;

    sqlx::query(
        r#"
        INSERT INTO student_class_enrollments (student_id, class_room_id, enrollment_date, status)
        VALUES ($1, $2, CURRENT_DATE, 'active')
        ON CONFLICT (student_id, class_room_id) DO UPDATE SET status = 'active', updated_at = NOW()
        "#,
    )
    .bind(new_user_id)
    .bind(class_room_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to enroll student: {}", e);
        AppError::InternalServerError("ไม่สามารถลงทะเบียนเข้าห้องเรียนได้".to_string())
    })?;

    if let Some(fd) = payload.form_data {
        sqlx::query(
            r#"
            INSERT INTO admission_enrollment_forms (application_id, form_data, pre_submitted_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (application_id) DO UPDATE SET form_data = $2, pre_submitted_at = NOW()
            "#,
        )
        .bind(id)
        .bind(fd)
        .execute(&mut *tx)
        .await
        .ok();
    }
    sqlx::query(
        "UPDATE admission_enrollment_forms SET completed_at = NOW(), completed_by = $1 WHERE application_id = $2",
    )
    .bind(enroller_id)
    .bind(id)
    .execute(&mut *tx)
    .await
    .ok();

    // สร้าง parent accounts (ครอบคลุม father / mother / guardians)
    {
        let form_data: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT form_data FROM admission_enrollment_forms WHERE application_id = $1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .unwrap_or(None);

        if let Some(fd) = form_data {
            let mut parent_entries: Vec<(String, String, String, String, String)> = Vec::new();

            if let Some(father) = fd.get("father") {
                let phone = father
                    .get("phone")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !phone.is_empty() {
                    parent_entries.push((
                        father
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("นาย")
                            .to_string(),
                        father
                            .get("firstName")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        father
                            .get("lastName")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        phone,
                        "father".to_string(),
                    ));
                }
            }
            if let Some(mother) = fd.get("mother") {
                let phone = mother
                    .get("phone")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !phone.is_empty() {
                    parent_entries.push((
                        mother
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("นาง")
                            .to_string(),
                        mother
                            .get("firstName")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        mother
                            .get("lastName")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        phone,
                        "mother".to_string(),
                    ));
                }
            }
            if let Some(guardians) = fd.get("guardians").and_then(|v| v.as_array()) {
                for g in guardians {
                    let phone = g
                        .get("phone")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !phone.is_empty() {
                        parent_entries.push((
                            g.get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            g.get("firstName")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            g.get("lastName")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            phone,
                            g.get("relationship")
                                .and_then(|v| v.as_str())
                                .unwrap_or("guardian")
                                .to_string(),
                        ));
                    }
                }
            }

            let mut seen_phones = std::collections::HashSet::new();
            let parent_entries: Vec<_> = parent_entries
                .into_iter()
                .filter(|(_, _, _, phone, _)| seen_phones.insert(phone.clone()))
                .collect();

            for (title, first_name, last_name, phone, relationship) in parent_entries {
                let existing_parent_id: Option<Uuid> =
                    sqlx::query_scalar("SELECT id FROM users WHERE username = $1")
                        .bind(&phone)
                        .fetch_optional(&mut *tx)
                        .await
                        .unwrap_or(None);

                let parent_id = if let Some(pid) = existing_parent_id {
                    pid
                } else {
                    let parent_password_hash = bcrypt::hash(&phone, 8).map_err(|_| {
                        AppError::InternalServerError("Parent password hash failed".to_string())
                    })?;

                    let pid: Uuid = sqlx::query_scalar(
                        r#"
                        INSERT INTO users (
                            username, password_hash, title, first_name, last_name, phone, user_type, status
                        ) VALUES ($1, $2, $3, $4, $5, $6, 'parent', 'active')
                        RETURNING id
                        "#,
                    )
                    .bind(&phone)
                    .bind(&parent_password_hash)
                    .bind(&title)
                    .bind(&first_name)
                    .bind(&last_name)
                    .bind(&phone)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| {
                        eprintln!("Failed to create parent user: {}", e);
                        AppError::InternalServerError("ไม่สามารถสร้าง account ผู้ปกครองได้".to_string())
                    })?;

                    let parent_role_id: Option<Uuid> = sqlx::query_scalar(
                        "SELECT id FROM roles WHERE code = 'PARENT' AND is_active = true",
                    )
                    .fetch_optional(&mut *tx)
                    .await
                    .ok()
                    .flatten();

                    if let Some(rid) = parent_role_id {
                        let _ = sqlx::query(
                            "INSERT INTO user_roles (user_id, role_id, is_primary) VALUES ($1, $2, true)",
                        )
                        .bind(pid)
                        .bind(rid)
                        .execute(&mut *tx)
                        .await;
                    }

                    pid
                };

                sqlx::query(
                    r#"
                    INSERT INTO student_parents (student_user_id, parent_user_id, relationship, is_primary)
                    VALUES ($1, $2, $3, false)
                    ON CONFLICT (student_user_id, parent_user_id)
                    DO UPDATE SET relationship = EXCLUDED.relationship
                    "#,
                )
                .bind(new_user_id)
                .bind(parent_id)
                .bind(&relationship)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    eprintln!("Failed to link parent to student: {}", e);
                    AppError::InternalServerError("ไม่สามารถเชื่อมโยงผู้ปกครองได้".to_string())
                })?;
            }
        }
    }

    sqlx::query(
        r#"
        UPDATE admission_applications
        SET status = 'enrolled', enrolled_by = $1, enrolled_at = NOW(), created_user_id = $2, updated_at = NOW()
        WHERE id = $3
        "#,
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

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(EnrollmentResult {
        user_id: new_user_id,
        username,
        student_code,
    })
}

pub async fn change_application_track(
    pool: &PgPool,
    application_id: Uuid,
    track_id: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE admission_applications SET room_assignment_track_id = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(track_id)
    .bind(application_id)
    .execute(pool)
    .await
    .map_err(|_| AppError::InternalServerError("ย้ายสายไม่สำเร็จ".to_string()))?;

    sqlx::query("DELETE FROM admission_room_assignments WHERE application_id = $1")
        .bind(application_id)
        .execute(pool)
        .await
        .ok();

    Ok(())
}

pub async fn update_admission_track(
    pool: &PgPool,
    application_id: Uuid,
    track_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE admission_applications SET admission_track_id = $1, room_assignment_track_id = NULL, updated_at = NOW() WHERE id = $2",
    )
    .bind(track_id)
    .bind(application_id)
    .execute(pool)
    .await
    .map_err(|_| AppError::InternalServerError("แก้ไขสายการเรียนไม่สำเร็จ".to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบใบสมัคร".to_string()));
    }

    sqlx::query("DELETE FROM admission_room_assignments WHERE application_id = $1")
        .bind(application_id)
        .execute(pool)
        .await
        .ok();

    Ok(())
}

// ==========================================
// Documents — handler ส่ง file_data + parsed multipart มาเอง (Multipart ใส่ใน service ไม่ได้)
// ==========================================

pub const VALID_DOC_TYPES: &[&str] = &[
    "photo_1_5inch",
    "transcript_por",
    "certificate_por7",
    "id_card_student",
    "id_card_father",
    "id_card_mother",
    "id_card_guardian",
    "house_reg_student",
    "house_reg_father",
    "house_reg_mother",
    "house_reg_guardian",
    "name_change_doc",
    "birth_cert",
];

pub const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "pdf", "webp"];

pub struct DocumentUploadInput {
    pub doc_type: String,
    pub file_data: Vec<u8>,
    pub original_filename: String,
    pub mime_type: String,
    pub ext: String,
}

pub struct DocumentUploadResult {
    pub doc_id: Uuid,
    pub file_id: Uuid,
    pub storage_path: String,
    pub file_size: i64,
    pub old_storage_path: Option<String>,
}

/// Save file metadata + soft-delete old doc + link new doc.
/// Caller (handler) จัดการ R2 upload/delete จริง — return paths สำหรับ cleanup
pub async fn save_document_record(
    pool: &PgPool,
    subdomain: &str,
    application_id: Uuid,
    input: DocumentUploadInput,
) -> Result<DocumentUploadResult, AppError> {
    let app_info: Option<(String, Uuid)> = sqlx::query_as(
        "SELECT application_number, admission_round_id FROM admission_applications WHERE id = $1",
    )
    .bind(application_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let (app_number, round_id) = app_info.ok_or(AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;

    let old_doc: Option<(String, Uuid)> = sqlx::query_as(
        r#"SELECT f.storage_path, f.id
           FROM admission_application_documents aad
           JOIN files f ON f.id = aad.file_id
           WHERE aad.application_id = $1 AND aad.doc_type = $2 AND aad.deleted_at IS NULL
           LIMIT 1"#,
    )
    .bind(application_id)
    .bind(&input.doc_type)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    let file_id = Uuid::new_v4();
    let storage_path = format!(
        "school-{}/admission/{}/{}/{}.{}",
        subdomain, round_id, app_number, file_id, input.ext
    );

    let file_size = input.file_data.len() as i64;
    sqlx::query(
        r#"
        INSERT INTO files (id, user_id, school_id, filename, original_filename,
            file_size, mime_type, storage_path, file_type,
            is_temporary, is_public, uploaded_by)
        VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, 'document',
            false, false, NULL)
        "#,
    )
    .bind(file_id)
    .bind(subdomain)
    .bind(format!("{}.{}", file_id, input.ext))
    .bind(&input.original_filename)
    .bind(file_size)
    .bind(&input.mime_type)
    .bind(&storage_path)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to save file metadata: {}", e);
        AppError::InternalServerError("Failed to save file metadata".to_string())
    })?;

    sqlx::query(
        "UPDATE admission_application_documents SET deleted_at = NOW() WHERE application_id = $1 AND doc_type = $2 AND deleted_at IS NULL",
    )
    .bind(application_id)
    .bind(&input.doc_type)
    .execute(pool)
    .await
    .ok();

    let doc_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO admission_application_documents (id, application_id, file_id, doc_type)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(doc_id)
    .bind(application_id)
    .bind(file_id)
    .bind(&input.doc_type)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to link document: {}", e);
        AppError::InternalServerError("Failed to link document".to_string())
    })?;

    Ok(DocumentUploadResult {
        doc_id,
        file_id,
        storage_path,
        file_size,
        old_storage_path: old_doc.map(|(p, _)| p),
    })
}

/// Delete document record + return storage_path สำหรับ R2 cleanup
pub async fn delete_document_record(
    pool: &PgPool,
    application_id: Uuid,
    doc_type: &str,
) -> Result<String, AppError> {
    let doc_info: Option<(String, Uuid)> = sqlx::query_as(
        r#"SELECT f.storage_path, f.id
           FROM admission_application_documents aad
           JOIN files f ON f.id = aad.file_id
           WHERE aad.application_id = $1 AND aad.doc_type = $2 AND aad.deleted_at IS NULL
           LIMIT 1"#,
    )
    .bind(application_id)
    .bind(doc_type)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    let (storage_path, file_id) =
        doc_info.ok_or_else(|| AppError::NotFound("ไม่พบเอกสารที่ต้องการลบ".to_string()))?;

    sqlx::query(
        "DELETE FROM admission_application_documents WHERE application_id = $1 AND doc_type = $2 AND deleted_at IS NULL",
    )
    .bind(application_id)
    .bind(doc_type)
    .execute(pool)
    .await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    sqlx::query("DELETE FROM files WHERE id = $1")
        .bind(file_id)
        .execute(pool)
        .await
        .ok();

    Ok(storage_path)
}

// ==========================================
// Student ID Assignment
// ==========================================

pub async fn sort_room_students(pool: &PgPool, round_id: Uuid) -> Result<i64, AppError> {
    let updated = sqlx::query_scalar::<_, i64>(
        r#"
        WITH ranked AS (
            SELECT ara.application_id,
                   ROW_NUMBER() OVER (
                       PARTITION BY ara.class_room_id
                       ORDER BY
                           CASE aa.title
                               WHEN 'เด็กชาย' THEN 0
                               WHEN 'นาย'     THEN 1
                               WHEN 'เด็กหญิง' THEN 2
                               WHEN 'นาง'     THEN 3
                               WHEN 'นางสาว'  THEN 4
                               ELSE 5
                           END,
                           aa.first_name, aa.last_name
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
        "#,
    )
    .bind(round_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    Ok(updated)
}

pub async fn auto_assign_student_ids(
    pool: &PgPool,
    round_id: Uuid,
    start_number: i64,
) -> Result<i64, AppError> {
    let existing: Vec<String> = sqlx::query_scalar(
        "SELECT assigned_student_id FROM admission_applications WHERE admission_round_id = $1 AND assigned_student_id IS NOT NULL AND assigned_student_id != ''"
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut occupied: std::collections::HashSet<i64> = existing
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();

    #[derive(sqlx::FromRow)]
    struct AppIdRow {
        application_id: Uuid,
    }

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
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("auto_assign_student_ids fetch error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut next = start_number;
    let mut assigned: i64 = 0;

    for student in &students {
        while occupied.contains(&next) {
            next += 1;
        }
        sqlx::query(
            "UPDATE admission_applications SET assigned_student_id = $1, updated_at = NOW() WHERE id = $2",
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

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    Ok(assigned)
}

pub async fn list_student_ids(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<StudentIdRow>, AppError> {
    let mut rows = sqlx::query_as::<_, StudentIdRow>(
        r#"
        SELECT
            a.id AS application_id, a.application_number, a.assigned_student_id,
            CONCAT(COALESCE(a.title, ''), a.first_name, ' ', a.last_name) AS full_name,
            a.title, a.first_name, a.last_name, a.national_id,
            cr.name AS room_name, ra.rank_in_room, ra.rank_in_track,
            a.previous_school, at_orig.name AS original_track_name,
            CASE WHEN a.room_assignment_track_id IS NOT NULL
                      AND a.room_assignment_track_id != a.admission_track_id
                 THEN at_assigned.name
                 ELSE NULL
            END AS assigned_track_name,
            esa.exam_id
        FROM admission_applications a
        JOIN admission_room_assignments ra ON ra.application_id = a.id
        LEFT JOIN class_rooms cr ON ra.class_room_id = cr.id
        LEFT JOIN admission_tracks at_orig ON at_orig.id = a.admission_track_id
        LEFT JOIN admission_tracks at_assigned ON at_assigned.id = a.room_assignment_track_id
        LEFT JOIN admission_exam_seat_assignments esa ON esa.application_id = a.id
        WHERE a.admission_round_id = $1
          AND a.status IN ('accepted', 'enrolled')
        ORDER BY cr.name, ra.rank_in_room
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("list_student_ids error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

    for row in &mut rows {
        decrypt_optional_national_id(&mut row.national_id)?;
    }

    Ok(rows)
}

pub async fn move_application_room(pool: &PgPool, id: Uuid, room_id: Uuid) -> Result<(), AppError> {
    let old_room_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT class_room_id FROM admission_room_assignments WHERE application_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if old_room_id.is_none() {
        return Err(AppError::BadRequest(
            "ยังไม่มีการจัดห้อง กรุณาบันทึกการจัดห้องก่อน".to_string(),
        ));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    sqlx::query(
        "UPDATE admission_room_assignments SET class_room_id = $1, assigned_at = NOW() WHERE application_id = $2",
    )
    .bind(room_id)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("move_application_room error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

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
        "#,
    )
    .bind(room_id)
    .execute(&mut *tx)
    .await
    .ok();

    if let Some(old_id) = old_room_id {
        if old_id != room_id {
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
                "#,
            )
            .bind(old_id)
            .execute(&mut *tx)
            .await.ok();
        }
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;

    sqlx::query(
        r#"
        UPDATE admission_applications aa
        SET room_assignment_track_id = (
            SELECT CASE WHEN t.id = aa.admission_track_id THEN NULL ELSE t.id END
            FROM class_rooms cr
            JOIN study_plan_versions spv ON spv.id = cr.study_plan_version_id
            JOIN study_plans sp ON sp.id = spv.study_plan_id
            JOIN admission_tracks t ON t.study_plan_id = sp.id
                AND t.admission_round_id = aa.admission_round_id
            WHERE cr.id = $2
            LIMIT 1
        ),
        updated_at = NOW()
        WHERE aa.id = $1
        "#,
    )
    .bind(id)
    .bind(room_id)
    .execute(pool)
    .await
    .ok();

    Ok(())
}

pub async fn batch_update_student_ids(
    pool: &PgPool,
    round_id: Uuid,
    payload: Vec<UpdateStudentIdItem>,
) -> Result<i64, AppError> {
    let app_ids: Vec<Uuid> = payload.iter().map(|i| i.application_id).collect();
    let student_ids: Vec<Option<String>> = payload.iter().map(|i| i.student_id.clone()).collect();

    sqlx::query_scalar::<_, i64>(
        r#"
        WITH updates AS (
            UPDATE admission_applications aa
            SET assigned_student_id = u.student_id, updated_at = NOW()
            FROM UNNEST($1::uuid[], $2::text[]) AS u(application_id, student_id)
            WHERE aa.id = u.application_id
              AND aa.admission_round_id = $3
            RETURNING 1
        )
        SELECT COUNT(*) FROM updates
        "#,
    )
    .bind(&app_ids)
    .bind(&student_ids)
    .bind(round_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        eprintln!("batch_update_student_ids error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })
}

pub fn build_full_file_url(storage_path: &str) -> Result<String, AppError> {
    let url_builder = FileUrlBuilder::new()
        .map_err(|_| AppError::InternalServerError("Configuration error".to_string()))?;
    Ok(format!("{}/{}", url_builder.base_url(), storage_path))
}

pub fn document_upload_response_json(
    result: &DocumentUploadResult,
    doc_type: &str,
) -> Result<serde_json::Value, AppError> {
    let file_url = build_full_file_url(&result.storage_path)?;
    Ok(json!({
        "id": result.doc_id,
        "fileId": result.file_id,
        "docType": doc_type,
        "fileUrl": file_url,
        "fileSize": result.file_size,
    }))
}
