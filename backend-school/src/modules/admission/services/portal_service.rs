use crate::error::AppError;
use crate::modules::admission::models::applications::*;
use crate::utils::file_url::FileUrlBuilder;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

pub const VALID_DOC_TYPES: &[&str] = &[
    "photo_1_5inch", "transcript_por", "certificate_por7",
    "id_card_student", "id_card_father", "id_card_mother", "id_card_guardian",
    "house_reg_student", "house_reg_father", "house_reg_mother", "house_reg_guardian",
    "name_change_doc", "birth_cert",
];

pub const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "pdf", "webp"];

pub async fn verify_credentials(pool: &PgPool, national_id: &str, date_of_birth: &str) -> Result<Uuid, AppError> {
    if date_of_birth.len() != 8 {
        return Err(AppError::BadRequest("รูปแบบวันเกิดไม่ถูกต้อง (ต้องกรอก 8 หลัก ววดดปปปป เช่น 20082543)".to_string()));
    }
    let year_be: i32 = date_of_birth[4..].parse().unwrap_or(0);
    let year_ce = year_be - 543;
    let dob = chrono::NaiveDate::parse_from_str(
        &format!("{}/{}/{}", &date_of_birth[0..2], &date_of_birth[2..4], year_ce),
        "%d/%m/%Y"
    ).ok();
    let Some(dob) = dob else {
        return Err(AppError::BadRequest("รูปแบบวันเกิดไม่ถูกต้อง (กรอก ววดดปปปป พ.ศ. เช่น 20082543)".to_string()));
    };
    sqlx::query_scalar(
        "SELECT id FROM admission_applications WHERE national_id = $1 AND date_of_birth = $2 ORDER BY created_at DESC LIMIT 1"
    )
    .bind(national_id).bind(dob)
    .fetch_optional(pool).await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?
    .ok_or_else(|| AppError::AuthError("ไม่พบข้อมูลผู้สมัคร กรุณาตรวจสอบเลขบัตรประชาชนและวันเกิด".to_string()))
}

pub async fn get_round_status(pool: &PgPool, application_id: Uuid) -> Result<String, AppError> {
    sqlx::query_scalar(
        r#"SELECT ar.status FROM admission_applications aa
           JOIN admission_rounds ar ON ar.id = aa.admission_round_id
           WHERE aa.id = $1"#,
    )
    .bind(application_id).fetch_one(pool).await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))
}

#[derive(sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckStatusRow {
    pub id: Uuid,
    pub application_number: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub status: String,
    pub track_name: Option<String>,
    pub round_name: Option<String>,
    pub round_status: Option<String>,
}

pub async fn check_application(pool: &PgPool, payload: PortalCredentials) -> Result<CheckStatusRow, AppError> {
    let application_id = verify_credentials(pool, &payload.national_id, &payload.date_of_birth).await?;
    let round_status = get_round_status(pool, application_id).await?;
    if round_status == "draft" {
        return Err(AppError::BadRequest("รอบการสมัครยังไม่เปิดเผยข้อมูล".to_string()));
    }
    sqlx::query_as::<_, CheckStatusRow>(
        r#"SELECT aa.id, aa.application_number, aa.first_name, aa.last_name, aa.status,
                  at2.name AS track_name, ar.name AS round_name, ar.status AS round_status
           FROM admission_applications aa
           LEFT JOIN admission_tracks at2 ON aa.admission_track_id = at2.id
           LEFT JOIN admission_rounds ar ON aa.admission_round_id = ar.id
           WHERE aa.id = $1"#,
    )
    .bind(application_id).fetch_one(pool).await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))
}

pub async fn get_status(pool: &PgPool, payload: PortalCredentials) -> Result<serde_json::Value, AppError> {
    let application_id = verify_credentials(pool, &payload.national_id, &payload.date_of_birth).await?;
    let round_status = get_round_status(pool, application_id).await?;
    if round_status == "draft" {
        return Err(AppError::BadRequest("รอบการสมัครยังไม่เปิดเผยข้อมูล".to_string()));
    }

    let selection_settings: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT selection_settings FROM admission_rounds WHERE id = (SELECT admission_round_id FROM admission_applications WHERE id = $1)"
    ).bind(application_id).fetch_optional(pool).await.unwrap_or(None).flatten();

    let show_scores = selection_settings.as_ref().and_then(|s| s.get("showScores")).and_then(|v| v.as_bool()).unwrap_or(false);
    let assignment_mode = selection_settings.as_ref().and_then(|s| s.get("assignmentMode")).and_then(|v| v.as_str()).unwrap_or("per_track").to_string();
    let show_assignment = ["announced", "enrolling", "closed"].contains(&round_status.as_str());
    let show_form = ["enrolling", "closed"].contains(&round_status.as_str());

    let application = sqlx::query_as::<_, AdmissionApplication>(
        r#"SELECT aa.*, at2.name AS track_name, at_asgn.name AS assigned_track_name, ar.name AS round_name
           FROM admission_applications aa
           LEFT JOIN admission_tracks at2 ON at2.id = aa.admission_track_id
           LEFT JOIN admission_tracks at_asgn ON at_asgn.id = aa.room_assignment_track_id
           LEFT JOIN admission_rounds ar ON aa.admission_round_id = ar.id
           WHERE aa.id = $1"#,
    )
    .bind(application_id).fetch_one(pool).await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

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
        r#"SELECT ara.rank_in_track, ara.rank_in_room, ara.total_score,
                  cr.name AS room_name, ara.student_confirmed
           FROM admission_room_assignments ara
           LEFT JOIN class_rooms cr ON ara.class_room_id = cr.id
           WHERE ara.application_id = $1"#,
    )
    .bind(application_id).fetch_optional(pool).await.unwrap_or(None);

    let scores: Vec<ExamScore> = if show_scores {
        sqlx::query_as::<_, ExamScore>(
            r#"SELECT esc.id, esc.application_id, esc.exam_subject_id, esc.score,
                      esc.entered_by, esc.entered_at, esc.updated_at,
                      aes.name AS subject_name, aes.code AS subject_code,
                      aes.max_score::FLOAT8 AS max_score
               FROM admission_exam_subjects aes
               LEFT JOIN admission_exam_scores esc ON esc.exam_subject_id = aes.id AND esc.application_id = $1
               WHERE aes.admission_round_id = $2
               ORDER BY aes.display_order ASC"#,
        )
        .bind(application_id).bind(application.admission_round_id)
        .fetch_all(pool).await.unwrap_or_default()
    } else { vec![] };

    let form = sqlx::query_as::<_, EnrollmentForm>(
        "SELECT * FROM admission_enrollment_forms WHERE application_id = $1"
    ).bind(application_id).fetch_optional(pool).await.unwrap_or(None);

    let documents = sqlx::query_as::<_, ApplicationDocument>(
        r#"SELECT d.id, d.application_id, d.file_id, d.doc_type, d.created_at, d.deleted_at,
                  f.storage_path AS file_url, f.original_filename, f.file_size, f.mime_type
           FROM admission_application_documents d
           JOIN files f ON f.id = d.file_id
           WHERE d.application_id = $1 AND d.deleted_at IS NULL
           ORDER BY d.created_at ASC"#,
    )
    .bind(application_id).fetch_all(pool).await.unwrap_or_default();

    let url_builder = FileUrlBuilder::new().unwrap_or_default();
    let documents: Vec<ApplicationDocument> = documents.into_iter().map(|mut doc| {
        if let Some(path) = doc.file_url.as_deref() {
            doc.file_url = Some(url_builder.build_url(path));
        }
        doc
    }).collect();

    Ok(json!({
        "application": application,
        "roundStatus": round_status,
        "assignmentMode": assignment_mode,
        "assignment": if show_assignment { json!(assignment) } else { json!(null) },
        "scores": if show_scores { json!(scores) } else { json!(null) },
        "enrollmentForm": if show_form { json!(form) } else { json!(null) },
        "documents": documents,
    }))
}

pub async fn confirm_enrollment(pool: &PgPool, payload: PortalConfirmRequest) -> Result<(), AppError> {
    let application_id = verify_credentials(pool, &payload.national_id, &payload.date_of_birth).await?;
    let round_status = get_round_status(pool, application_id).await?;
    if round_status != "enrolling" {
        return Err(AppError::BadRequest("ยังไม่ถึงช่วงเวลารายงานตัว".to_string()));
    }
    let status: String = sqlx::query_scalar("SELECT status FROM admission_applications WHERE id = $1")
        .bind(application_id).fetch_one(pool).await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;
    if status != "accepted" {
        return Err(AppError::BadRequest(format!("ไม่สามารถยืนยันได้ (สถานะปัจจุบัน: {})", status)));
    }
    sqlx::query("UPDATE admission_room_assignments SET student_confirmed = true, student_confirmed_at = NOW() WHERE application_id = $1")
        .bind(application_id).execute(pool).await
        .map_err(|_| AppError::InternalServerError("Failed to confirm".to_string()))?;
    Ok(())
}

pub async fn get_enrollment_form(pool: &PgPool, payload: PortalCredentials) -> Result<Option<EnrollmentForm>, AppError> {
    let application_id = verify_credentials(pool, &payload.national_id, &payload.date_of_birth).await?;
    let round_status = get_round_status(pool, application_id).await?;
    if !["enrolling", "closed"].contains(&round_status.as_str()) {
        return Err(AppError::BadRequest("ยังไม่ถึงช่วงเวลารายงานตัว".to_string()));
    }
    sqlx::query_as::<_, EnrollmentForm>("SELECT * FROM admission_enrollment_forms WHERE application_id = $1")
        .bind(application_id).fetch_optional(pool).await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))
}

pub async fn submit_enrollment_form(pool: &PgPool, payload: PortalFormRequest) -> Result<(), AppError> {
    let application_id = verify_credentials(pool, &payload.national_id, &payload.date_of_birth).await?;
    let round_status = get_round_status(pool, application_id).await?;
    if round_status != "enrolling" {
        return Err(AppError::BadRequest("ยังไม่ถึงช่วงเวลารายงานตัว หรือหมดเขตแล้ว".to_string()));
    }
    let status: String = sqlx::query_scalar("SELECT status FROM admission_applications WHERE id = $1")
        .bind(application_id).fetch_one(pool).await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;
    if status != "accepted" {
        return Err(AppError::BadRequest(format!("ไม่สามารถยืนยันได้ (สถานะปัจจุบัน: {})", status)));
    }
    sqlx::query("UPDATE admission_room_assignments SET student_confirmed = true, student_confirmed_at = NOW() WHERE application_id = $1")
        .bind(application_id).execute(pool).await
        .map_err(|_| AppError::InternalServerError("Failed to confirm".to_string()))?;

    let form_data = payload.form_data.unwrap_or(json!({}));
    sqlx::query(
        r#"INSERT INTO admission_enrollment_forms (application_id, form_data, pre_submitted_at)
           VALUES ($1, $2, NOW())
           ON CONFLICT (application_id) DO UPDATE SET form_data = $2, pre_submitted_at = NOW()"#,
    )
    .bind(application_id).bind(form_data).execute(pool).await
    .map_err(|e| {
        eprintln!("Failed to submit enrollment form: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกแบบฟอร์มได้".to_string())
    })?;
    Ok(())
}

pub async fn update_application(pool: &PgPool, payload: UpdatePortalApplicationRequest) -> Result<(), AppError> {
    let application_id = verify_credentials(pool, &payload.auth_national_id, &payload.auth_date_of_birth).await?;
    let round_status = get_round_status(pool, application_id).await?;
    if round_status != "open" {
        return Err(AppError::BadRequest("ไม่สามารถแก้ไขใบสมัครได้ในช่วงเวลานี้".to_string()));
    }
    let status: String = sqlx::query_scalar("SELECT status FROM admission_applications WHERE id = $1")
        .bind(application_id).fetch_one(pool).await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;
    if status == "enrolled" || status == "withdrawn" {
        return Err(AppError::BadRequest(format!("ไม่สามารถแก้ไขใบสมัครได้เนื่องจากอยู่ในสถานะ '{}'", status)));
    }

    if payload.data.national_id != payload.auth_national_id {
        let round_id: Uuid = sqlx::query_scalar("SELECT admission_round_id FROM admission_applications WHERE id = $1")
            .bind(application_id).fetch_one(pool).await.unwrap_or_default();
        let already: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM admission_applications WHERE national_id = $1 AND admission_round_id = $2 AND id != $3)"
        ).bind(&payload.data.national_id).bind(round_id).bind(application_id)
        .fetch_one(pool).await.unwrap_or(false);
        if already {
            return Err(AppError::BadRequest("เลขบัตรประชาชนใหม่ที่กรอกได้สมัครรอบนี้ไปแล้ว (ซ้ำ)".to_string()));
        }
    }

    sqlx::query(
        r#"UPDATE admission_applications SET
            admission_track_id = $1, title = $2, first_name = $3, last_name = $4,
            gender = $5, phone = $6, email = $7,
            address_line = $8, sub_district = $9, district = $10, province = $11, postal_code = $12,
            previous_school = $13, previous_grade = $14, previous_gpa = $15,
            father_name = $16, father_phone = $17, father_occupation = $18, father_national_id = $19,
            mother_name = $20, mother_phone = $21, mother_occupation = $22, mother_national_id = $23,
            guardian_name = $24, guardian_phone = $25, guardian_relation = $26, guardian_national_id = $27,
            national_id = $28, date_of_birth = $29,
            guardian_occupation = $30, guardian_income = $31, guardian_is = $32,
            religion = $33, ethnicity = $34, nationality = $35,
            home_house_no = $36, home_moo = $37, home_soi = $38, home_road = $39, home_phone = $40,
            current_house_no = $41, current_moo = $42, current_soi = $43, current_road = $44,
            current_sub_district = $45, current_district = $46, current_province = $47,
            current_postal_code = $48, current_phone = $49,
            previous_study_year = $50, previous_school_province = $51,
            father_income = $52, mother_income = $53,
            parent_status = $54, parent_status_other = $55,
            status = 'submitted', rejection_reason = NULL, updated_at = NOW()
           WHERE id = $56"#,
    )
    .bind(payload.data.admission_track_id).bind(&payload.data.title).bind(&payload.data.first_name).bind(&payload.data.last_name)
    .bind(&payload.data.gender).bind(&payload.data.phone).bind(&payload.data.email)
    .bind(&payload.data.address_line).bind(&payload.data.sub_district).bind(&payload.data.district)
    .bind(&payload.data.province).bind(&payload.data.postal_code)
    .bind(&payload.data.previous_school).bind(&payload.data.previous_grade).bind(payload.data.previous_gpa)
    .bind(&payload.data.father_name).bind(&payload.data.father_phone).bind(&payload.data.father_occupation).bind(&payload.data.father_national_id)
    .bind(&payload.data.mother_name).bind(&payload.data.mother_phone).bind(&payload.data.mother_occupation).bind(&payload.data.mother_national_id)
    .bind(&payload.data.guardian_name).bind(&payload.data.guardian_phone).bind(&payload.data.guardian_relation).bind(&payload.data.guardian_national_id)
    .bind(&payload.data.national_id).bind(payload.data.date_of_birth)
    .bind(&payload.data.guardian_occupation).bind(payload.data.guardian_income).bind(&payload.data.guardian_is)
    .bind(&payload.data.religion).bind(&payload.data.ethnicity).bind(&payload.data.nationality)
    .bind(&payload.data.home_house_no).bind(&payload.data.home_moo).bind(&payload.data.home_soi).bind(&payload.data.home_road).bind(&payload.data.home_phone)
    .bind(&payload.data.current_house_no).bind(&payload.data.current_moo).bind(&payload.data.current_soi).bind(&payload.data.current_road)
    .bind(&payload.data.current_sub_district).bind(&payload.data.current_district).bind(&payload.data.current_province)
    .bind(&payload.data.current_postal_code).bind(&payload.data.current_phone)
    .bind(&payload.data.previous_study_year).bind(&payload.data.previous_school_province)
    .bind(payload.data.father_income).bind(payload.data.mother_income)
    .bind(&payload.data.parent_status).bind(&payload.data.parent_status_other)
    .bind(application_id)
    .execute(pool).await
    .map_err(|e| {
        eprintln!("Failed to update application: {}", e);
        AppError::InternalServerError("ไม่สามารถแก้ไขใบสมัครได้".to_string())
    })?;
    Ok(())
}

pub struct PortalUploadInput {
    pub doc_type: String,
    pub file_data: Vec<u8>,
    pub original_filename: String,
    pub mime_type: String,
    pub ext: String,
    pub national_id: Option<String>,
    pub date_of_birth: Option<String>,
}

pub struct PortalUploadDbResult {
    pub file_id: Uuid,
    pub storage_path: String,
    pub file_size: i64,
    pub old_storage_path: Option<String>,
    pub linked_to_application: bool,
}

/// DB-side ของ portal upload — R2 client อยู่ที่ handler
pub async fn save_portal_upload(
    pool: &PgPool,
    subdomain: &str,
    input: &PortalUploadInput,
) -> Result<(PortalUploadDbResult, String /* storage_path to upload */), AppError> {
    let file_id = Uuid::new_v4();
    let file_size = input.file_data.len() as i64;

    if let (Some(nid), Some(dob)) = (&input.national_id, &input.date_of_birth) {
        let application_id = verify_credentials(pool, nid, dob).await?;
        let round_status = get_round_status(pool, application_id).await?;
        if !["open", "enrolling"].contains(&round_status.as_str()) {
            return Err(AppError::BadRequest("ไม่สามารถอัปโหลดเอกสารได้ในช่วงเวลานี้".to_string()));
        }
        let (app_number, round_id): (String, Uuid) = sqlx::query_as(
            "SELECT application_number, admission_round_id FROM admission_applications WHERE id = $1"
        )
        .bind(application_id).fetch_one(pool).await
        .map_err(|_| AppError::NotFound("ไม่พบใบสมัคร".to_string()))?;

        let storage_path = format!(
            "school-{}/admission/{}/{}/{}.{}",
            subdomain, round_id, app_number, file_id, input.ext
        );

        // Find old doc
        let old_doc = sqlx::query_as::<_, (Uuid, Uuid, String)>(
            r#"SELECT aad.id, aad.file_id, f.storage_path
               FROM admission_application_documents aad
               JOIN files f ON f.id = aad.file_id
               WHERE aad.application_id = $1 AND aad.doc_type = $2 AND aad.deleted_at IS NULL
               LIMIT 1"#,
        )
        .bind(application_id).bind(&input.doc_type)
        .fetch_optional(pool).await.ok().flatten();

        let old_storage = old_doc.as_ref().map(|(_, _, p)| p.clone());

        // Insert file metadata (after R2 upload caller will do)
        sqlx::query(
            r#"INSERT INTO files (id, user_id, school_id, filename, original_filename,
                file_size, mime_type, storage_path, file_type,
                is_temporary, is_public, expires_at, uploaded_by)
               VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, 'document', false, false, NULL, NULL)"#,
        )
        .bind(file_id).bind(subdomain).bind(format!("{}.{}", file_id, input.ext))
        .bind(&input.original_filename).bind(file_size).bind(&input.mime_type).bind(&storage_path)
        .execute(pool).await
        .map_err(|e| {
            eprintln!("Failed to save file metadata: {}", e);
            AppError::InternalServerError("Failed to save file metadata".to_string())
        })?;

        // Delete old DB records
        if let Some((old_aad_id, old_file_id, _)) = old_doc {
            let _ = sqlx::query("DELETE FROM admission_application_documents WHERE id = $1")
                .bind(old_aad_id).execute(pool).await;
            let _ = sqlx::query("DELETE FROM files WHERE id = $1")
                .bind(old_file_id).execute(pool).await;
        }

        // Link new
        let _ = sqlx::query(
            "INSERT INTO admission_application_documents (application_id, file_id, doc_type) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
        )
        .bind(application_id).bind(file_id).bind(&input.doc_type).execute(pool).await;

        Ok((PortalUploadDbResult {
            file_id, storage_path: storage_path.clone(), file_size,
            old_storage_path: old_storage, linked_to_application: true,
        }, storage_path))
    } else {
        // Anonymous upload — flat path
        let storage_path = format!("school-{}/admission/documents/{}.{}", subdomain, file_id, input.ext);
        sqlx::query(
            r#"INSERT INTO files (id, user_id, school_id, filename, original_filename,
                file_size, mime_type, storage_path, file_type,
                is_temporary, is_public, expires_at, uploaded_by)
               VALUES ($1, NULL, $2, $3, $4, $5, $6, $7, 'document', false, false, NULL, NULL)"#,
        )
        .bind(file_id).bind(subdomain).bind(format!("{}.{}", file_id, input.ext))
        .bind(&input.original_filename).bind(file_size).bind(&input.mime_type).bind(&storage_path)
        .execute(pool).await
        .map_err(|e| {
            eprintln!("Failed to save file metadata: {}", e);
            AppError::InternalServerError("Failed to save file metadata".to_string())
        })?;

        Ok((PortalUploadDbResult {
            file_id, storage_path: storage_path.clone(), file_size,
            old_storage_path: None, linked_to_application: false,
        }, storage_path))
    }
}

/// Return storage_path สำหรับ R2 cleanup
pub async fn delete_portal_document(
    pool: &PgPool,
    doc_type: &str,
    query: PortalDeleteDocumentQuery,
) -> Result<String, AppError> {
    let application_id = verify_credentials(pool, &query.national_id, &query.date_of_birth).await?;
    let round_status = get_round_status(pool, application_id).await?;
    if !["open", "enrolling"].contains(&round_status.as_str()) {
        return Err(AppError::BadRequest("ไม่สามารถลบเอกสารได้ในช่วงเวลานี้".to_string()));
    }

    let doc_row = sqlx::query_as::<_, (Uuid, Uuid, String)>(
        r#"SELECT aad.id, aad.file_id, f.storage_path
           FROM admission_application_documents aad
           JOIN files f ON f.id = aad.file_id
           WHERE aad.application_id = $1 AND aad.doc_type = $2 AND aad.deleted_at IS NULL
           LIMIT 1"#,
    )
    .bind(application_id).bind(doc_type)
    .fetch_optional(pool).await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    let (doc_id, file_id, storage_path) = doc_row
        .ok_or_else(|| AppError::NotFound("ไม่พบเอกสารที่ต้องการลบ".to_string()))?;

    sqlx::query("DELETE FROM admission_application_documents WHERE id = $1")
        .bind(doc_id).execute(pool).await
        .map_err(|_| AppError::InternalServerError("Failed to delete document".to_string()))?;
    sqlx::query("DELETE FROM files WHERE id = $1").bind(file_id).execute(pool).await.ok();

    Ok(storage_path)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalExamSeatRequest {
    pub national_id: String,
    pub date_of_birth: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExamSeatInfo {
    pub seat_number: i32,
    pub exam_id: Option<String>,
    pub room_name: String,
    pub building_name: Option<String>,
    pub exam_date: Option<chrono::NaiveDate>,
}

pub async fn get_exam_seat(pool: &PgPool, national_id: &str, date_of_birth: &str) -> Result<Option<ExamSeatInfo>, AppError> {
    let application_id = verify_credentials(pool, national_id, date_of_birth).await?;
    let round_status = get_round_status(pool, application_id).await?;
    let allowed = ["exam_announced", "announced", "enrolling", "closed"];
    if !allowed.contains(&round_status.as_str()) {
        return Err(AppError::BadRequest("ยังไม่ถึงเวลาดูข้อมูลห้องสอบ".to_string()));
    }
    sqlx::query_as::<_, ExamSeatInfo>(
        r#"SELECT sa.seat_number, sa.exam_id,
                  COALESCE(er.custom_name, r.name_th, r.name_en, 'ห้องสอบ') AS room_name,
                  b.name_th AS building_name, ar.exam_date
           FROM admission_exam_seat_assignments sa
           JOIN admission_exam_rooms er ON er.id = sa.exam_room_id
           JOIN admission_rounds ar ON ar.id = er.admission_round_id
           LEFT JOIN rooms r ON r.id = er.room_id
           LEFT JOIN buildings b ON b.id = r.building_id
           WHERE sa.application_id = $1"#,
    )
    .bind(application_id).fetch_optional(pool).await
    .map_err(|_| AppError::InternalServerError("Database error".to_string()))
}

pub fn build_file_url_full(storage_path: &str) -> Result<String, AppError> {
    let url_builder = FileUrlBuilder::new()
        .map_err(|_| AppError::InternalServerError("Configuration error".to_string()))?;
    Ok(format!("{}/{}", url_builder.base_url(), storage_path))
}
