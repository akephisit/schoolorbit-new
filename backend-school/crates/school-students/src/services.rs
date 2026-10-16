use bcrypt::{hash, DEFAULT_COST};
use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use school_crypto as field_encryption;
use school_errors::AppError;

use super::models::{
    CreateParentRequest, CreateStudentRequest, CreateStudentResponse, ListStudentsQuery, ParentDto,
    StudentDbRow, StudentListItem, StudentListResponse, StudentProfile, StudentRegisterInfo,
    StudentRegisterProfile, StudentRegistrationStatus, UpdateOwnProfileRequest,
    UpdateStudentRequest,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StudentListAccess {
    Own(Uuid),
    Assigned(Uuid),
    OwnAndAssigned(Uuid),
    School,
}

pub async fn get_own_profile(
    pool: &PgPool,
    user_id: Uuid,
    academic_year_id: Uuid,
    include_pii: bool,
) -> Result<StudentProfile, AppError> {
    ensure_student_user(pool, user_id).await?;

    let mut student_row = sqlx::query_as::<_, StudentDbRow>(
        r#"
        SELECT
            u.id, u.username, u.national_id as national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_file_id,
            s.student_id,
            CASE gl.level_type
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level,
            h.name as homeroom,
            placement.class_number as student_number,
            s.blood_type, s.allergies, s.medical_conditions as medical_conditions,
            u.status
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        INNER JOIN student_academic_years student_year
          ON student_year.student_id = u.id
         AND student_year.academic_year_id = $2
        LEFT JOIN LATERAL (
            SELECT homeroom_id, class_number
            FROM homeroom_placements
            WHERE student_academic_year_id = student_year.id
            ORDER BY CASE status WHEN 'current' THEN 1 WHEN 'planned' THEN 2 ELSE 3 END,
                     start_date DESC, created_at DESC
            LIMIT 1
        ) placement ON true
        LEFT JOIN homerooms h ON placement.homeroom_id = h.id
        LEFT JOIN grade_levels gl ON student_year.grade_level_id = gl.id
        WHERE u.id = $1 AND u.user_type = 'student' AND u.status = 'active'
        "#,
    )
    .bind(user_id)
    .bind(academic_year_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get own student profile: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลนักเรียนได้".to_string())
    })?
    .ok_or(AppError::NotFound("Student not found".to_string()))?;

    apply_student_pii_visibility(&mut student_row, include_pii);
    let parents = list_student_parents(pool, user_id).await?;

    Ok(StudentProfile {
        info: student_row,
        parents,
    })
}

pub async fn update_own_profile(
    pool: &PgPool,
    user_id: Uuid,
    payload: UpdateOwnProfileRequest,
) -> Result<(), AppError> {
    ensure_student_user(pool, user_id).await?;

    sqlx::query(
        r#"
        UPDATE users
        SET
            phone = COALESCE($2, phone),
            address = COALESCE($3, address),
            nickname = COALESCE($4, nickname),
            updated_at = NOW()
        WHERE id = $1 AND user_type = 'student'
        "#,
    )
    .bind(user_id)
    .bind(&payload.phone)
    .bind(&payload.address)
    .bind(&payload.nickname)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update own student profile: {}", e);
        AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลได้".to_string())
    })?;

    Ok(())
}

pub async fn list_students(
    pool: &PgPool,
    filter: ListStudentsQuery,
    access: StudentListAccess,
) -> Result<StudentListResponse, AppError> {
    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20);
    if page < 1 || !(1..=100).contains(&page_size) {
        return Err(AppError::BadRequest(
            "เลขหน้าและจำนวนรายการต่อหน้าไม่ถูกต้อง".into(),
        ));
    }
    let offset = (page - 1)
        .checked_mul(page_size)
        .ok_or_else(|| AppError::BadRequest("เลขหน้ามากเกินไป".into()))?;
    if filter.account_status.as_deref().is_some_and(|status| {
        !matches!(
            status,
            "active" | "inactive" | "suspended" | "resigned" | "retired"
        )
    }) {
        return Err(AppError::BadRequest("สถานะบัญชีไม่ถูกต้อง".into()));
    }
    let search = filter
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if search.is_some_and(|value| value.chars().count() > 200) {
        return Err(AppError::BadRequest("คำค้นหาต้องไม่เกิน 200 ตัวอักษร".into()));
    }
    let (school, own, assigned, actor) = match access {
        StudentListAccess::School => (true, false, false, None),
        StudentListAccess::Own(id) => (false, true, false, Some(id)),
        StudentListAccess::Assigned(id) => (false, false, true, Some(id)),
        StudentListAccess::OwnAndAssigned(id) => (false, true, true, Some(id)),
    };
    let items = sqlx::query_as::<_, StudentListItem>(
        r#"
        SELECT u.id, u.username, u.title, u.first_name, u.last_name,
               s.student_id, s.enrollment_date,
               COALESCE(s.registration_status, 'unrecorded') AS registration_status,
               u.status AS account_status
        FROM users u
        LEFT JOIN student_info s ON s.user_id = u.id
        WHERE u.user_type = 'student'
          AND ($1::text IS NULL OR COALESCE(s.registration_status, 'unrecorded') = $1)
          AND ($2::text IS NULL OR u.status = $2)
          AND ($3::text IS NULL OR u.first_name ILIKE $3 OR u.last_name ILIKE $3
               OR s.student_id ILIKE $3 OR u.username ILIKE $3)
          AND ($4::boolean OR ($5::boolean AND u.id = $7)
               OR ($6::boolean AND EXISTS (
                   SELECT 1 FROM student_academic_years student_year
                   JOIN homeroom_placements placement
                     ON placement.student_academic_year_id = student_year.id
                    AND placement.status = 'current'
                   JOIN homeroom_advisors advisor ON advisor.homeroom_id = placement.homeroom_id
                   WHERE student_year.student_id = u.id AND student_year.status = 'active'
                     AND advisor.user_id = $7
               )))
        ORDER BY u.first_name, u.last_name, u.id
        LIMIT $8 OFFSET $9
    "#,
    )
    .bind(filter.registration_status)
    .bind(filter.account_status)
    .bind(search.map(|value| format!("%{value}%")))
    .bind(school)
    .bind(own)
    .bind(assigned)
    .bind(actor)
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to list student register: {}", error);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".into())
    })?;
    Ok(StudentListResponse {
        items,
        page,
        page_size,
    })
}

pub struct CreatedStudent {
    pub student: CreateStudentResponse,
    pub parent_ids: Vec<Uuid>,
}

pub async fn create_student(
    pool: &PgPool,
    actor_user_id: Uuid,
    mut payload: CreateStudentRequest,
) -> Result<CreatedStudent, AppError> {
    crate::validation::normalize_create(&mut payload)?;
    let password_hash = hash(&payload.password, DEFAULT_COST).map_err(|e| {
        tracing::error!("Student password hashing failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่าน".to_string())
    })?;
    let date_of_birth = parse_optional_date(payload.date_of_birth.as_deref())?;
    let encrypted_national_id = field_encryption::encrypt_optional(payload.national_id.as_deref())
        .map_err(|e| {
            tracing::error!("Student national_id encryption failed: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูล".to_string())
        })?;
    let national_id_hash = field_encryption::hash_optional_for_search(
        payload.national_id.as_deref(),
    )
    .map_err(|e| {
        tracing::error!("Student national_id blind index failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูล".to_string())
    })?;
    let username = student_username_or_default(payload.username.clone(), &payload.student_id);

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Failed to start create student transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการเริ่ม Transaction".to_string())
    })?;

    let user_id = insert_student_user(
        &mut tx,
        &payload,
        &username,
        &password_hash,
        encrypted_national_id,
        national_id_hash,
        date_of_birth,
    )
    .await?;

    insert_student_info(&mut tx, user_id, actor_user_id, &payload).await?;
    let parent_ids = link_initial_parents(&mut tx, user_id, payload.parents.as_deref()).await?;
    assign_required_role(
        &mut tx,
        user_id,
        "STUDENT",
        "ไม่สามารถตรวจสอบบทบาทนักเรียนได้",
        "ไม่สามารถกำหนดบทบาทนักเรียนได้",
    )
    .await?;

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit create student transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok(CreatedStudent {
        student: CreateStudentResponse {
            id: user_id,
            username,
        },
        parent_ids,
    })
}

pub async fn get_student(
    pool: &PgPool,
    student_id: Uuid,
    academic_year_id: Option<Uuid>,
    include_pii: bool,
) -> Result<StudentRegisterProfile, AppError> {
    let mut student_row = sqlx::query_as::<_, StudentRegisterInfo>(
        r#"
        SELECT
            u.id, u.username, u.national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_file_id, u.status AS account_status,
            s.student_id, s.enrollment_date,
            COALESCE(s.registration_status, 'unrecorded') AS registration_status,
            s.registration_status_updated_at, s.registration_status_updated_by,
            CASE WHEN student_year.id IS NULL THEN NULL ELSE
                CASE gl.level_type
                    WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                    WHEN 'primary' THEN CONCAT('ป.', gl.year)
                    WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                    ELSE CONCAT('?.', gl.year)
                END
            END AS grade_level,
            h.name AS homeroom,
            placement.class_number AS student_number,
            s.blood_type, s.allergies, s.medical_conditions
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        LEFT JOIN student_academic_years student_year
          ON student_year.student_id = u.id AND student_year.academic_year_id = $2
        LEFT JOIN LATERAL (
            SELECT homeroom_id, class_number
            FROM homeroom_placements
            WHERE student_academic_year_id = student_year.id
            ORDER BY CASE status WHEN 'current' THEN 1 WHEN 'planned' THEN 2 ELSE 3 END,
                     start_date DESC, created_at DESC, id
            LIMIT 1
        ) placement ON true
        LEFT JOIN homerooms h ON placement.homeroom_id = h.id
        LEFT JOIN grade_levels gl ON student_year.grade_level_id = gl.id
        WHERE u.id = $1 AND u.user_type = 'student'
        "#,
    )
    .bind(student_id)
    .bind(academic_year_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to get student register profile: {}", error);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".into())
    })?
    .ok_or_else(|| AppError::NotFound("ไม่พบนักเรียน".into()))?;

    if include_pii {
        decrypt_student_field(&mut student_row.national_id);
        decrypt_student_field(&mut student_row.medical_conditions);
    } else {
        student_row.national_id = None;
        student_row.blood_type = None;
        student_row.allergies = None;
        student_row.medical_conditions = None;
    }
    let parents = list_student_parents(pool, student_id).await?;
    Ok(StudentRegisterProfile {
        info: student_row,
        parents,
    })
}

pub async fn update_student(
    pool: &PgPool,
    student_id: Uuid,
    actor_user_id: Uuid,
    mut payload: UpdateStudentRequest,
) -> Result<(), AppError> {
    crate::validation::normalize_update(&mut payload)?;
    let date_of_birth = payload
        .date_of_birth
        .as_ref()
        .map(|value| crate::validation::parse_date(value.as_deref()))
        .transpose()?
        .flatten();
    let enrollment_date = payload
        .enrollment_date
        .as_ref()
        .map(|value| crate::validation::parse_date(value.as_deref()))
        .transpose()?
        .flatten();
    let mut tx = pool.begin().await?;
    let user_update = sqlx::query(
        r#"
        UPDATE users SET
            email = CASE WHEN $2 THEN $3 ELSE email END,
            first_name = COALESCE($4, first_name), last_name = COALESCE($5, last_name),
            title = CASE WHEN $6 THEN $7 ELSE title END,
            phone = CASE WHEN $8 THEN $9 ELSE phone END,
            address = CASE WHEN $10 THEN $11 ELSE address END,
            date_of_birth = CASE WHEN $12 THEN $13 ELSE date_of_birth END,
            gender = CASE WHEN $14 THEN $15 ELSE gender END,
            updated_at = NOW()
        WHERE id = $1 AND user_type = 'student'
    "#,
    )
    .bind(student_id)
    .bind(payload.email.is_some())
    .bind(payload.email.flatten())
    .bind(payload.first_name)
    .bind(payload.last_name)
    .bind(payload.title.is_some())
    .bind(payload.title.flatten())
    .bind(payload.phone.is_some())
    .bind(payload.phone.flatten())
    .bind(payload.address.is_some())
    .bind(payload.address.flatten())
    .bind(payload.date_of_birth.is_some())
    .bind(date_of_birth)
    .bind(payload.gender.is_some())
    .bind(payload.gender.flatten())
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        // SQL errors can contain contact values. Return only a classified error.
        map_duplicate_student_error(error, "ไม่สามารถอัพเดตข้อมูลได้")
    })?;
    if user_update.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบนักเรียน".into()));
    }
    if payload.registration_status.is_some() || payload.enrollment_date.is_some() {
        let result = sqlx::query(
            r#"
            UPDATE student_info SET
                registration_status_updated_at = CASE
                    WHEN $2::text IS NOT NULL AND registration_status IS DISTINCT FROM $2
                    THEN NOW() ELSE registration_status_updated_at END,
                registration_status_updated_by = CASE
                    WHEN $2::text IS NOT NULL AND registration_status IS DISTINCT FROM $2
                    THEN $3 ELSE registration_status_updated_by END,
                registration_status = COALESCE($2, registration_status),
                enrollment_date = CASE WHEN $4 THEN $5 ELSE enrollment_date END,
                updated_at = NOW()
            WHERE user_id = $1
        "#,
        )
        .bind(student_id)
        .bind(payload.registration_status)
        .bind(actor_user_id)
        .bind(payload.enrollment_date.is_some())
        .bind(enrollment_date)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::Conflict(
                "ไม่พบทะเบียนประจำตัวนักเรียน กรุณาตรวจสอบข้อมูลก่อน".into(),
            ));
        }
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete_student(
    pool: &PgPool,
    student_id: Uuid,
    actor_user_id: Uuid,
    placement_rosters: &dyn school_academic_core::ports::PlacementRosterPort,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Failed to begin delete student transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;

    school_academic_core::services::student_years::withdraw_for_account_deactivation(
        &mut tx,
        placement_rosters,
        actor_user_id,
        student_id,
    )
    .await?;

    let user_update = sqlx::query(
        r#"
        UPDATE users
        SET status = 'inactive',
            username = username || '_del_' || CAST(EXTRACT(EPOCH FROM NOW()) AS BIGINT),
            updated_at = NOW()
        WHERE id = $1 AND user_type = 'student'
        "#,
    )
    .bind(student_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete student: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;
    if user_update.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบนักเรียน".to_string()));
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit delete student transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;

    Ok(())
}

pub async fn add_parent_to_student(
    pool: &PgPool,
    student_id: Uuid,
    mut payload: CreateParentRequest,
) -> Result<Uuid, AppError> {
    crate::validation::normalize_parent(&mut payload)?;
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Failed to begin add parent transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการเริ่มต้น transaction".to_string())
    })?;

    let student_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM users
            WHERE id = $1 AND user_type = 'student'
        )",
    )
    .bind(student_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to validate student before adding parent: {}", e);
        AppError::InternalServerError("ไม่สามารถตรวจสอบข้อมูลนักเรียนได้".to_string())
    })?;
    if !student_exists {
        return Err(AppError::NotFound("ไม่พบนักเรียน".to_string()));
    }

    let parent_id = get_or_create_parent_user(&mut tx, &payload).await?;
    link_parent_to_student(&mut tx, student_id, parent_id, &payload.relationship).await?;

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit add parent transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok(parent_id)
}

pub async fn remove_parent_from_student(
    pool: &PgPool,
    student_id: Uuid,
    parent_id: Uuid,
) -> Result<(), AppError> {
    let result = sqlx::query(
        "DELETE FROM student_parents WHERE student_user_id = $1 AND parent_user_id = $2",
    )
    .bind(student_id)
    .bind(parent_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to remove parent link: {}", e);
        AppError::InternalServerError("ไม่สามารถลบผู้ปกครองได้".to_string())
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ไม่พบข้อมูลความสัมพันธ์ผู้ปกครอง".to_string()));
    }

    Ok(())
}

async fn ensure_student_user(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let user_type: Option<String> = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to load current user type: {}", e);
            AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้ใช้ได้".to_string())
        })?;

    match user_type.as_deref() {
        Some("student") => Ok(()),
        Some(_) => Err(AppError::Forbidden("เฉพาะนักเรียนเท่านั้น".to_string())),
        None => Err(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string())),
    }
}

async fn list_student_parents(pool: &PgPool, student_id: Uuid) -> Result<Vec<ParentDto>, AppError> {
    sqlx::query_as::<_, ParentDto>(
        r#"
        SELECT
            p.id, p.username, p.first_name, p.last_name, p.phone,
            sp.relationship, sp.is_primary
        FROM student_parents sp
        INNER JOIN users p ON sp.parent_user_id = p.id
        WHERE sp.student_user_id = $1
        ORDER BY sp.is_primary DESC, p.first_name ASC
        "#,
    )
    .bind(student_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list student parents: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้ปกครองได้".to_string())
    })
}

fn apply_student_pii_visibility(row: &mut StudentDbRow, include_pii: bool) {
    if !include_pii {
        hide_student_pii_fields(row);
        return;
    }

    decrypt_student_field(&mut row.national_id);
    decrypt_student_field(&mut row.medical_conditions);
}

fn decrypt_student_field(value: &mut Option<String>) {
    if let Some(encrypted) = value.take() {
        match field_encryption::decrypt(&encrypted) {
            Ok(decrypted) => *value = Some(decrypted),
            Err(_) => tracing::error!("Failed to decrypt protected student field"),
        }
    }
}

fn hide_student_pii_fields(row: &mut StudentDbRow) {
    row.national_id = None;
    row.blood_type = None;
    row.allergies = None;
    row.medical_conditions = None;
}

fn parse_optional_date(value: Option<&str>) -> Result<Option<NaiveDate>, AppError> {
    crate::validation::parse_date(value)
}

fn student_username_or_default(username: Option<String>, student_id: &str) -> String {
    username
        .filter(|username| !username.is_empty())
        .unwrap_or_else(|| student_id.to_string())
}

async fn insert_student_user(
    tx: &mut Transaction<'_, Postgres>,
    payload: &CreateStudentRequest,
    username: &str,
    password_hash: &str,
    encrypted_national_id: Option<String>,
    national_id_hash: Option<String>,
    date_of_birth: Option<NaiveDate>,
) -> Result<Uuid, AppError> {
    sqlx::query_scalar(
        r#"
        INSERT INTO users (
            username, national_id, national_id_hash, email, password_hash,
            first_name, last_name, title,
            user_type, status, date_of_birth, gender, phone, address
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'student', 'active', $9, $10, $11, $12)
        RETURNING id
        "#,
    )
    .bind(username)
    .bind(&encrypted_national_id)
    .bind(&national_id_hash)
    .bind(&payload.email)
    .bind(password_hash)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.title)
    .bind(date_of_birth)
    .bind(&payload.gender)
    .bind(&payload.phone)
    .bind(&payload.address)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| map_duplicate_student_error(e, "ไม่สามารถสร้างผู้ใช้งานได้"))
}

async fn insert_student_info(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    actor_user_id: Uuid,
    payload: &CreateStudentRequest,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO student_info (
            user_id, student_id, registration_status,
            registration_status_updated_at, registration_status_updated_by, enrollment_date
        ) VALUES ($1, $2, $3, NOW(), $4, $5)
        "#,
    )
    .bind(user_id)
    .bind(payload.student_id.as_str())
    .bind(
        payload
            .registration_status
            .unwrap_or(StudentRegistrationStatus::Pending),
    )
    .bind(actor_user_id)
    .bind(crate::validation::parse_date(
        payload.enrollment_date.as_deref(),
    )?)
    .execute(&mut **tx)
    .await
    .map_err(|e| map_duplicate_student_error(e, "ไม่สามารถสร้างข้อมูลนักเรียนได้"))?;

    Ok(())
}

async fn link_initial_parents(
    tx: &mut Transaction<'_, Postgres>,
    student_id: Uuid,
    parents: Option<&[CreateParentRequest]>,
) -> Result<Vec<Uuid>, AppError> {
    let Some(parents) = parents else {
        return Ok(vec![]);
    };

    let mut parent_ids = Vec::with_capacity(parents.len());
    for (index, parent) in parents.iter().enumerate() {
        let parent_id = get_or_create_parent_user(tx, parent).await?;
        link_parent_to_student_if_absent(
            tx,
            student_id,
            parent_id,
            &parent.relationship,
            index == 0,
        )
        .await?;
        parent_ids.push(parent_id);
    }

    Ok(parent_ids)
}

async fn get_or_create_parent_user(
    tx: &mut Transaction<'_, Postgres>,
    payload: &CreateParentRequest,
) -> Result<Uuid, AppError> {
    let existing_parent = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id,user_type,status FROM users WHERE username = $1 FOR UPDATE",
    )
    .bind(&payload.phone)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to check for existing parent: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการตรวจสอบผู้ปกครอง".to_string())
    })?;

    match existing_parent {
        Some((parent_id, user_type, status)) => {
            if user_type != "parent" || status != "active" {
                return Err(AppError::Conflict(
                    "บัญชีนี้ไม่ใช่ผู้ปกครองที่เปิดใช้งาน กรุณาตรวจสอบบัญชีก่อนเชื่อมโยง".into(),
                ));
            }
            let has_history: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_roles WHERE user_id=$1)")
                    .bind(parent_id)
                    .fetch_one(&mut **tx)
                    .await?;
            if !has_history {
                assign_parent_role(tx, parent_id).await?;
            }
            Ok(parent_id)
        }
        None => create_parent_user(tx, payload).await,
    }
}

async fn create_parent_user(
    tx: &mut Transaction<'_, Postgres>,
    payload: &CreateParentRequest,
) -> Result<Uuid, AppError> {
    let password_hash = hash(&payload.phone, DEFAULT_COST).map_err(|e| {
        tracing::error!("Parent password hashing failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่านผู้ปกครอง".to_string())
    })?;

    let encrypted_national_id = field_encryption::encrypt_optional(payload.national_id.as_deref())
        .map_err(|e| {
            tracing::error!("Parent national_id encryption failed: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูลผู้ปกครอง".to_string())
        })?;
    let national_id_hash = field_encryption::hash_optional_for_search(
        payload.national_id.as_deref(),
    )
    .map_err(|e| {
        tracing::error!("Parent national_id blind index failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูลผู้ปกครอง".to_string())
    })?;

    let parent_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (
            username, national_id, national_id_hash, email, password_hash,
            title, first_name, last_name, phone,
            user_type, status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'parent', 'active')
        RETURNING id
        "#,
    )
    .bind(&payload.phone)
    .bind(encrypted_national_id)
    .bind(national_id_hash)
    .bind(&payload.email)
    .bind(password_hash)
    .bind(&payload.title)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.phone)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| map_duplicate_student_error(error, "ไม่สามารถสร้างบัญชีผู้ปกครองได้"))?;

    assign_parent_role(tx, parent_id).await?;

    Ok(parent_id)
}

async fn assign_required_role(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    role_code: &str,
    load_error_message: &str,
    assign_error_message: &str,
) -> Result<(), AppError> {
    let role_id: Option<Uuid> =
        sqlx::query_scalar("SELECT r.id FROM roles r JOIN users u ON u.user_type=r.user_type WHERE r.code=$1 AND r.is_active=true AND u.id=$2 FOR SHARE OF r")
            .bind(role_code)
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to load {role_code} role: {}", e);
                AppError::InternalServerError(load_error_message.to_string())
            })?;

    let role_id = role_id.ok_or_else(|| {
        AppError::Conflict("บทบาทมาตรฐานยังไม่พร้อม กรุณาให้ผู้ดูแลตรวจสอบก่อนสร้างบัญชี".into())
    })?;
    {
        sqlx::query(
            r#"
            INSERT INTO user_roles (user_id, role_id, is_primary)
            VALUES ($1, $2, true)
            "#,
        )
        .bind(user_id)
        .bind(role_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to assign {role_code} role: {}", e);
            AppError::InternalServerError(assign_error_message.to_string())
        })?;
    }

    Ok(())
}

async fn assign_parent_role(
    tx: &mut Transaction<'_, Postgres>,
    parent_id: Uuid,
) -> Result<(), AppError> {
    assign_required_role(
        tx,
        parent_id,
        "PARENT",
        "ไม่สามารถตรวจสอบบทบาทผู้ปกครองได้",
        "ไม่สามารถกำหนดบทบาทผู้ปกครองได้",
    )
    .await
}

async fn link_parent_to_student(
    tx: &mut Transaction<'_, Postgres>,
    student_id: Uuid,
    parent_id: Uuid,
    relationship: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO student_parents (student_user_id, parent_user_id, relationship, is_primary)
        VALUES ($1, $2, $3, false)
        ON CONFLICT (student_user_id, parent_user_id)
        DO UPDATE SET relationship = EXCLUDED.relationship
        "#,
    )
    .bind(student_id)
    .bind(parent_id)
    .bind(relationship)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to link parent: {}", e);
        AppError::InternalServerError("ไม่สามารถเชื่อมโยงผู้ปกครองได้".to_string())
    })?;

    Ok(())
}

async fn link_parent_to_student_if_absent(
    tx: &mut Transaction<'_, Postgres>,
    student_id: Uuid,
    parent_id: Uuid,
    relationship: &str,
    is_primary: bool,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO student_parents (student_user_id, parent_user_id, relationship, is_primary)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (student_user_id, parent_user_id) DO NOTHING
        "#,
    )
    .bind(student_id)
    .bind(parent_id)
    .bind(relationship)
    .bind(is_primary)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to link parent: {}", e);
        AppError::InternalServerError("ไม่สามารถเชื่อมโยงผู้ปกครองได้".to_string())
    })?;

    Ok(())
}

fn map_duplicate_student_error(error: sqlx::Error, fallback: &str) -> AppError {
    let message = error.to_string();
    if message.contains("duplicate key value violates unique constraint") {
        if message.contains("users_username_key") {
            AppError::BadRequest("รหัสผู้ใช้งาน (Username) นี้มีอยู่ในระบบแล้ว กรุณาใช้รหัสอื่น".to_string())
        } else if message.contains("users_national_id_hash_key") {
            AppError::BadRequest("รหัสบัตรประชาชนนี้มีอยู่ในระบบแล้ว".to_string())
        } else if message.contains("users_email_key") {
            AppError::BadRequest("อีเมลนี้มีอยู่ในระบบแล้ว".to_string())
        } else {
            AppError::BadRequest("ข้อมูลบางอย่างซ้ำกับที่มีในระบบ".to_string())
        }
    } else {
        AppError::InternalServerError(fallback.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn student_username_or_default_uses_student_id_when_username_missing_or_blank() {
        assert_eq!(student_username_or_default(None, "S001"), "S001");
        assert_eq!(
            student_username_or_default(Some("".to_string()), "S001"),
            "S001"
        );
    }

    #[test]
    fn student_username_or_default_preserves_explicit_username() {
        assert_eq!(
            student_username_or_default(Some("custom".to_string()), "S001"),
            "custom"
        );
    }

    #[test]
    fn parse_optional_date_accepts_iso_dates() {
        assert_eq!(
            parse_optional_date(Some("2026-06-06")).unwrap(),
            Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap())
        );
    }

    #[test]
    fn parse_optional_date_rejects_invalid_dates() {
        assert!(matches!(
            parse_optional_date(Some("06/06/2026")),
            Err(AppError::BadRequest(message)) if message.contains("YYYY-MM-DD")
        ));
    }

    #[test]
    fn hide_student_pii_fields_removes_sensitive_profile_values() {
        let mut row = StudentDbRow {
            id: Uuid::new_v4(),
            username: "s001".to_string(),
            national_id: Some("ciphertext".to_string()),
            email: Some("student@example.com".to_string()),
            first_name: "สมชาย".to_string(),
            last_name: "ใจดี".to_string(),
            title: Some("เด็กชาย".to_string()),
            nickname: Some("ชาย".to_string()),
            phone: Some("0800000000".to_string()),
            date_of_birth: None,
            gender: None,
            address: Some("address".to_string()),
            profile_image_file_id: None,
            student_id: Some("S001".to_string()),
            student_number: Some(1),
            blood_type: Some("O".to_string()),
            allergies: Some("ถั่ว".to_string()),
            medical_conditions: Some("ciphertext-medical".to_string()),
            status: Some("active".to_string()),
            grade_level: Some("ม.1".to_string()),
            homeroom: Some("1/1".to_string()),
        };

        hide_student_pii_fields(&mut row);

        assert_eq!(row.national_id, None);
        assert_eq!(row.blood_type, None);
        assert_eq!(row.allergies, None);
        assert_eq!(row.medical_conditions, None);
        assert_eq!(row.first_name, "สมชาย");
    }
}
