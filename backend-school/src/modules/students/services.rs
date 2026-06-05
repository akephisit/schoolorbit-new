use bcrypt::{hash, DEFAULT_COST};
use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::utils::field_encryption;

use super::models::{
    CreateParentRequest, CreateStudentRequest, CreateStudentResponse, ListStudentsQuery, ParentDto,
    StudentDbRow, StudentListItem, StudentListResponse, StudentProfile, UpdateOwnProfileRequest,
    UpdateStudentRequest,
};

pub async fn get_own_profile(pool: &PgPool, user_id: Uuid) -> Result<StudentProfile, AppError> {
    ensure_student_user(pool, user_id).await?;

    let mut student_row = sqlx::query_as::<_, StudentDbRow>(
        r#"
        SELECT
            u.id, u.username, u.national_id as national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_url,
            s.student_id,
            CASE gl.level_type
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level,
            c.name as class_room,
            sce.class_number as student_number,
            s.blood_type, s.allergies, s.medical_conditions as medical_conditions,
            u.status
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        LEFT JOIN LATERAL (
            SELECT student_id, class_room_id, class_number
            FROM student_class_enrollments
            WHERE student_id = u.id
            ORDER BY created_at DESC
            LIMIT 1
        ) sce ON true
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE u.id = $1 AND u.user_type = 'student' AND u.status = 'active'
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to get own student profile: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลนักเรียนได้".to_string())
    })?
    .ok_or(AppError::NotFound("Student not found".to_string()))?;

    decrypt_student_row_fields(&mut student_row);
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
        eprintln!("Failed to update own student profile: {}", e);
        AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลได้".to_string())
    })?;

    Ok(())
}

pub async fn list_students(
    pool: &PgPool,
    filter: ListStudentsQuery,
) -> Result<StudentListResponse, AppError> {
    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    let mut query = String::from(
        r#"
        SELECT
            u.id, u.username, u.title, u.first_name, u.last_name,
            s.student_id,
            CASE gl.level_type
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level,
            c.name as class_room,
            u.status
        FROM users u
        INNER JOIN student_info s ON u.id = s.user_id
        LEFT JOIN student_class_enrollments sce ON u.id = sce.student_id AND sce.status = 'active'
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE u.user_type = 'student'
        "#,
    );

    let mut idx = 0u32;

    if filter.status.is_some() {
        idx += 1;
        query.push_str(&format!(" AND u.status = ${idx}"));
    }
    if filter
        .search
        .as_ref()
        .is_some_and(|search| !search.is_empty())
    {
        idx += 1;
        query.push_str(&format!(" AND (u.first_name ILIKE ${idx} OR u.last_name ILIKE ${idx} OR s.student_id ILIKE ${idx} OR u.username ILIKE ${idx})"));
    }

    idx += 1;
    let limit_idx = idx;
    idx += 1;
    let offset_idx = idx;
    query.push_str(" ORDER BY CASE gl.level_type WHEN 'kindergarten' THEN 1 WHEN 'primary' THEN 2 WHEN 'secondary' THEN 3 ELSE 4 END, gl.year NULLS LAST, c.name NULLS LAST, s.student_number");
    query.push_str(&format!(" LIMIT ${limit_idx} OFFSET ${offset_idx}"));

    let mut q = sqlx::query_as::<_, StudentListItem>(&query);
    if let Some(status) = &filter.status {
        q = q.bind(status);
    }
    if let Some(search) = &filter.search {
        if !search.is_empty() {
            q = q.bind(format!("%{search}%"));
        }
    }
    q = q.bind(page_size);
    q = q.bind(offset);

    let items = q.fetch_all(pool).await.map_err(|e| {
        eprintln!("Failed to list students: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?;

    Ok(StudentListResponse {
        items,
        page,
        page_size,
    })
}

pub async fn create_student(
    pool: &PgPool,
    payload: CreateStudentRequest,
) -> Result<CreateStudentResponse, AppError> {
    let password_hash = hash(&payload.password, DEFAULT_COST).map_err(|e| {
        eprintln!("Student password hashing failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่าน".to_string())
    })?;
    let date_of_birth = parse_optional_date(payload.date_of_birth.as_deref())?;
    let encrypted_national_id = field_encryption::encrypt_optional(payload.national_id.as_deref())
        .map_err(|e| {
            eprintln!("Student national_id encryption failed: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูล".to_string())
        })?;
    let national_id_hash = field_encryption::hash_optional_for_search(
        payload.national_id.as_deref(),
    )
    .map_err(|e| {
        eprintln!("Student national_id blind index failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูล".to_string())
    })?;
    let username = payload
        .username
        .as_ref()
        .filter(|username| !username.is_empty())
        .cloned()
        .unwrap_or_else(|| payload.student_id.clone());

    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("Failed to start create student transaction: {}", e);
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

    insert_student_info(&mut tx, user_id, &payload).await?;
    link_initial_parents(&mut tx, user_id, payload.parents.as_deref()).await?;
    assign_active_role_if_available(
        &mut tx,
        user_id,
        "STUDENT",
        "ไม่สามารถตรวจสอบบทบาทนักเรียนได้",
        "ไม่สามารถกำหนดบทบาทนักเรียนได้",
    )
    .await?;

    tx.commit().await.map_err(|e| {
        eprintln!("Failed to commit create student transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok(CreateStudentResponse {
        id: user_id,
        username,
    })
}

pub async fn get_student(pool: &PgPool, student_id: Uuid) -> Result<StudentProfile, AppError> {
    let mut student_row = sqlx::query_as::<_, StudentDbRow>(
        r#"
        SELECT
            u.id, u.username, u.national_id as national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_url, u.status,
            s.student_id,
            CASE gl.level_type
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level,
            c.name as class_room,
            sce.class_number as student_number,
            s.blood_type, s.allergies, s.medical_conditions as medical_conditions
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        LEFT JOIN student_class_enrollments sce ON u.id = sce.student_id AND sce.status = 'active'
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE u.id = $1 AND u.user_type = 'student'
        "#,
    )
    .bind(student_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to get student: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?
    .ok_or(AppError::NotFound("Student not found".to_string()))?;

    decrypt_student_row_fields(&mut student_row);
    let parents = list_student_parents(pool, student_id).await?;

    Ok(StudentProfile {
        info: student_row,
        parents,
    })
}

pub async fn update_student(
    pool: &PgPool,
    student_id: Uuid,
    payload: UpdateStudentRequest,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("Failed to start update student transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการเริ่ม Transaction".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE users
        SET
            email = COALESCE($2, email),
            first_name = COALESCE($3, first_name),
            last_name = COALESCE($4, last_name),
            phone = COALESCE($5, phone),
            address = COALESCE($6, address),
            updated_at = NOW()
        WHERE id = $1 AND user_type = 'student'
        "#,
    )
    .bind(student_id)
    .bind(&payload.email)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.phone)
    .bind(&payload.address)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to update student user: {}", e);
        AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลได้".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE student_info
        SET
            student_number = COALESCE($2, student_number),
            updated_at = NOW()
        WHERE user_id = $1
        "#,
    )
    .bind(student_id)
    .bind(payload.student_number)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to update student_info: {}", e);
        AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลได้".to_string())
    })?;

    tx.commit().await.map_err(|e| {
        eprintln!("Failed to commit update student transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok(())
}

pub async fn delete_student(pool: &PgPool, student_id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("Failed to begin delete student transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;

    sqlx::query(
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
        eprintln!("Failed to delete student: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE student_class_enrollments
        SET status = 'dropped', updated_at = NOW()
        WHERE student_id = $1 AND status = 'active'
        "#,
    )
    .bind(student_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to drop student enrollments: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;

    tx.commit().await.map_err(|e| {
        eprintln!("Failed to commit delete student transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;

    Ok(())
}

pub async fn add_parent_to_student(
    pool: &PgPool,
    student_id: Uuid,
    payload: CreateParentRequest,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("Failed to begin add parent transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการเริ่มต้น transaction".to_string())
    })?;

    let parent_id = get_or_create_parent_user(&mut tx, &payload).await?;
    link_parent_to_student(&mut tx, student_id, parent_id, &payload.relationship).await?;

    tx.commit().await.map_err(|e| {
        eprintln!("Failed to commit add parent transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok(())
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
        eprintln!("Failed to remove parent link: {}", e);
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
            eprintln!("Failed to load current user type: {}", e);
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
        eprintln!("Failed to list student parents: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้ปกครองได้".to_string())
    })
}

fn decrypt_student_row_fields(row: &mut StudentDbRow) {
    if let Some(national_id) = row.national_id.clone() {
        match field_encryption::decrypt(&national_id) {
            Ok(decrypted) => row.national_id = Some(decrypted),
            Err(error) => eprintln!("Failed to decrypt student national_id: {}", error),
        }
    }

    if let Some(medical_conditions) = row.medical_conditions.clone() {
        match field_encryption::decrypt(&medical_conditions) {
            Ok(decrypted) => row.medical_conditions = Some(decrypted),
            Err(error) => eprintln!("Failed to decrypt student medical_conditions: {}", error),
        }
    }
}

fn parse_optional_date(value: Option<&str>) -> Result<Option<NaiveDate>, AppError> {
    match value {
        Some(date) if !date.is_empty() => NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map(Some)
            .map_err(|e| {
                eprintln!("Invalid student date_of_birth format: {}", e);
                AppError::BadRequest("รูปแบบวันเกิดไม่ถูกต้อง (ต้องเป็น YYYY-MM-DD)".to_string())
            }),
        _ => Ok(None),
    }
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
            user_type, status, date_of_birth, gender
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'student', 'active', $9, $10)
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
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to create student user: {}", e);
        map_duplicate_student_error(e, "ไม่สามารถสร้างผู้ใช้งานได้")
    })
}

async fn insert_student_info(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    payload: &CreateStudentRequest,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO student_info (
            user_id, student_id, student_number
        ) VALUES ($1, $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(payload.student_id.as_str())
    .bind(payload.student_number)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        eprintln!("Failed to create student_info: {}", e);
        AppError::InternalServerError("ไม่สามารถสร้างข้อมูลนักเรียนได้".to_string())
    })?;

    Ok(())
}

async fn link_initial_parents(
    tx: &mut Transaction<'_, Postgres>,
    student_id: Uuid,
    parents: Option<&[CreateParentRequest]>,
) -> Result<(), AppError> {
    let Some(parents) = parents else {
        return Ok(());
    };

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
    }

    Ok(())
}

async fn get_or_create_parent_user(
    tx: &mut Transaction<'_, Postgres>,
    payload: &CreateParentRequest,
) -> Result<Uuid, AppError> {
    let existing_parent = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE username = $1")
        .bind(&payload.phone)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| {
            eprintln!("Failed to check for existing parent: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการตรวจสอบผู้ปกครอง".to_string())
        })?;

    match existing_parent {
        Some(parent_id) => Ok(parent_id),
        None => create_parent_user(tx, payload).await,
    }
}

async fn create_parent_user(
    tx: &mut Transaction<'_, Postgres>,
    payload: &CreateParentRequest,
) -> Result<Uuid, AppError> {
    let password_hash = hash(&payload.phone, DEFAULT_COST).map_err(|e| {
        eprintln!("Parent password hashing failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่านผู้ปกครอง".to_string())
    })?;

    let encrypted_national_id = field_encryption::encrypt_optional(payload.national_id.as_deref())
        .map_err(|e| {
            eprintln!("Parent national_id encryption failed: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูลผู้ปกครอง".to_string())
        })?;
    let national_id_hash = field_encryption::hash_optional_for_search(
        payload.national_id.as_deref(),
    )
    .map_err(|e| {
        eprintln!("Parent national_id blind index failed: {}", e);
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
    .map_err(|e| {
        eprintln!("Failed to create parent: {}", e);
        AppError::InternalServerError("ไม่สามารถสร้างบัญชีผู้ปกครองได้".to_string())
    })?;

    assign_parent_role_if_available(tx, parent_id).await?;

    Ok(parent_id)
}

async fn assign_active_role_if_available(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    role_code: &str,
    load_error_message: &str,
    assign_error_message: &str,
) -> Result<(), AppError> {
    let role_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM roles WHERE code = $1 AND is_active = true")
            .bind(role_code)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| {
                eprintln!("Failed to load {role_code} role: {}", e);
                AppError::InternalServerError(load_error_message.to_string())
            })?;

    if let Some(role_id) = role_id {
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
            eprintln!("Failed to assign {role_code} role: {}", e);
            AppError::InternalServerError(assign_error_message.to_string())
        })?;
    }

    Ok(())
}

async fn assign_parent_role_if_available(
    tx: &mut Transaction<'_, Postgres>,
    parent_id: Uuid,
) -> Result<(), AppError> {
    assign_active_role_if_available(
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
        eprintln!("Failed to link parent: {}", e);
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
        eprintln!("Failed to link parent: {}", e);
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
