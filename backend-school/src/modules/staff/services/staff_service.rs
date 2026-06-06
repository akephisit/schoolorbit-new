use crate::error::AppError;
use crate::modules::staff::models::*;
use crate::utils::field_encryption;
use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// Helper structs for query results
#[derive(Debug, FromRow)]
struct UserBasicRow {
    id: Uuid,
    username: String,
    national_id: Option<String>,
    email: Option<String>,
    title: Option<String>,
    first_name: String,
    last_name: String,
    nickname: Option<String>,
    phone: Option<String>,
    emergency_contact: Option<String>,
    line_id: Option<String>,
    date_of_birth: Option<NaiveDate>,
    gender: Option<String>,
    address: Option<String>,
    hired_date: Option<NaiveDate>,
    user_type: String,
    status: String,
    profile_image_url: Option<String>,
}

#[derive(Debug, FromRow)]
struct StaffInfoRow {
    education_level: Option<String>,
    major: Option<String>,
    university: Option<String>,
}

#[derive(Debug, FromRow)]
struct RoleRow {
    id: Uuid,
    code: String,
    name: String,
    name_en: Option<String>,
    user_type: String,
    level: i32,
    is_primary: bool,
}

#[derive(Debug, FromRow)]
struct DepartmentRow {
    id: Uuid,
    code: String,
    name: String,
    position: String,
    is_primary_department: bool,
    category: Option<String>,
    org_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PublicStaffRole {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub level: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct PublicStaffDepartment {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub position: String,
}

#[derive(Debug, Serialize)]
pub struct PublicStaffProfile {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub nickname: Option<String>,
    pub title: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub hired_date: Option<NaiveDate>,
    pub profile_image_url: Option<String>,
    pub user_type: String,
    pub status: String,
    pub roles: Vec<PublicStaffRole>,
    pub departments: Vec<PublicStaffDepartment>,
}

#[derive(Debug, FromRow)]
struct TeachingCourseRow {
    classroom_course_id: Uuid,
    subject_code: String,
    subject_name: String,
    hours_per_semester: Option<i32>,
    classroom_name: String,
    classroom_code: String,
    academic_year: i32,
    academic_year_label: String,
    term: String,
    role: String,
}

#[derive(Debug, FromRow)]
struct AdvisorClassroomRow {
    classroom_id: Uuid,
    classroom_name: String,
    classroom_code: String,
    academic_year: i32,
    academic_year_label: String,
    role: String,
}

/// List staff (paginated, filterable)
pub async fn list_staff(
    pool: &PgPool,
    filter: StaffListFilter,
) -> Result<(Vec<StaffListItem>, i64, i64, i64), AppError> {
    let page_params = staff_page_params(&filter);

    let mut query = String::from(
        "SELECT DISTINCT u.id, u.username, u.title, u.first_name, u.last_name, u.status
         FROM users u
         WHERE u.user_type = 'staff'",
    );

    let mut idx = 0u32;

    if filter.status.is_some() {
        idx += 1;
        query.push_str(&format!(" AND u.status = ${idx}"));
    } else {
        query.push_str(" AND u.status = 'active'");
    }

    let search_pattern = staff_search_pattern(filter.search.clone());
    if search_pattern.is_some() {
        idx += 1;
        query.push_str(&format!(
            " AND (u.first_name ILIKE ${idx} OR u.last_name ILIKE ${idx} OR u.username ILIKE ${idx})"
        ));
    }

    idx += 1;
    let limit_idx = idx;
    idx += 1;
    let offset_idx = idx;
    query.push_str(&format!(
        " ORDER BY u.first_name LIMIT ${limit_idx} OFFSET ${offset_idx}"
    ));

    let mut q = sqlx::query_as::<_, (Uuid, String, Option<String>, String, String, String)>(&query);
    if let Some(ref status) = filter.status {
        q = q.bind(status);
    }
    if let Some(ref pattern) = search_pattern {
        q = q.bind(pattern);
    }
    q = q.bind(page_params.page_size);
    q = q.bind(page_params.offset);

    let staff_rows = q.fetch_all(pool).await.map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?;

    let count_query = "SELECT COUNT(DISTINCT u.id) FROM users u WHERE u.user_type = 'staff'";
    let total: i64 = sqlx::query_scalar(count_query)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let items: Vec<StaffListItem> = staff_rows
        .into_iter()
        .map(
            |(id, username, title, first_name, last_name, status)| StaffListItem {
                id,
                username,
                title: staff_title_or_default(title),
                first_name,
                last_name,
                roles: vec![],
                departments: vec![],
                status,
            },
        )
        .collect();

    Ok((items, total, page_params.page, page_params.page_size))
}

/// Get staff full profile with parallel queries
pub async fn get_staff_profile(
    pool: &PgPool,
    staff_id: Uuid,
) -> Result<StaffProfileResponse, AppError> {
    let mut user = sqlx::query_as::<_, UserBasicRow>(
        "SELECT id, username, national_id, email, title, first_name, last_name, nickname, phone,
                emergency_contact, line_id, date_of_birth, gender, address, hired_date,
                user_type, status, profile_image_url
         FROM users
         WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบบุคลากร".to_string()))?;

    if let Some(nid) = &user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    // 5 independent queries — run in parallel
    let staff_info_fut = sqlx::query_as::<_, StaffInfoRow>(
        "SELECT education_level, major, university FROM staff_info WHERE user_id = $1",
    )
    .bind(staff_id)
    .fetch_optional(pool);

    let roles_fut = sqlx::query_as::<_, RoleRow>(
        "SELECT r.id, r.code, r.name, r.name_en, r.user_type, r.level, ur.is_primary
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL
         ORDER BY ur.is_primary DESC, r.level DESC",
    )
    .bind(staff_id)
    .fetch_all(pool);

    let departments_fut = sqlx::query_as::<_, DepartmentRow>(
        "SELECT d.id, d.code, d.name, d.category, d.org_type, dm.position, dm.is_primary_department
         FROM department_members dm
         JOIN departments d ON dm.department_id = d.id
         WHERE dm.user_id = $1 AND dm.ended_at IS NULL
         ORDER BY dm.is_primary_department DESC",
    )
    .bind(staff_id)
    .fetch_all(pool);

    let teaching_fut = sqlx::query_as::<_, TeachingCourseRow>(
        r#"WITH teacher_cc AS (
            SELECT cc.id AS classroom_course_id,
                   cc.subject_id, cc.classroom_id, cc.academic_semester_id,
                   'primary'::text AS role
            FROM classroom_courses cc
            WHERE cc.primary_instructor_id = $1
            UNION
            SELECT cc.id AS classroom_course_id,
                   cc.subject_id, cc.classroom_id, cc.academic_semester_id,
                   cci.role
            FROM classroom_course_instructors cci
            JOIN classroom_courses cc ON cc.id = cci.classroom_course_id
            WHERE cci.instructor_id = $1
        )
        SELECT tc.classroom_course_id,
               s.code AS subject_code,
               s.name_th AS subject_name,
               s.hours_per_semester,
               cr.name AS classroom_name,
               cr.code AS classroom_code,
               ay.year AS academic_year,
               ay.name AS academic_year_label,
               sem.term,
               tc.role
        FROM teacher_cc tc
        JOIN subjects s ON s.id = tc.subject_id
        JOIN class_rooms cr ON cr.id = tc.classroom_id
        JOIN academic_semesters sem ON sem.id = tc.academic_semester_id
        JOIN academic_years ay ON ay.id = sem.academic_year_id
        ORDER BY ay.year DESC, sem.term ASC, s.code ASC"#,
    )
    .bind(staff_id)
    .fetch_all(pool);

    let advisor_fut = sqlx::query_as::<_, AdvisorClassroomRow>(
        r#"SELECT cr.id AS classroom_id,
                  cr.name AS classroom_name,
                  cr.code AS classroom_code,
                  ay.year AS academic_year,
                  ay.name AS academic_year_label,
                  ca.role
           FROM classroom_advisors ca
           JOIN class_rooms cr ON cr.id = ca.classroom_id
           JOIN academic_years ay ON ay.id = cr.academic_year_id
           WHERE ca.user_id = $1
           ORDER BY ay.year DESC, cr.name ASC"#,
    )
    .bind(staff_id)
    .fetch_all(pool);

    let (staff_info_res, roles_res, departments_res, teaching_res, advisor_res) = tokio::join!(
        staff_info_fut,
        roles_fut,
        departments_fut,
        teaching_fut,
        advisor_fut
    );

    let staff_info = staff_info_res.ok().flatten();

    let roles: Vec<RoleResponse> = roles_res
        .unwrap_or_default()
        .into_iter()
        .map(|row| RoleResponse {
            id: row.id,
            code: row.code,
            name: row.name,
            name_en: row.name_en,
            user_type: row.user_type,
            level: row.level,
            is_primary: Some(row.is_primary),
        })
        .collect();

    let departments: Vec<DepartmentResponse> = departments_res
        .unwrap_or_default()
        .into_iter()
        .map(|row| DepartmentResponse {
            id: row.id,
            code: row.code,
            name: row.name,
            position: Some(row.position),
            is_primary_department: Some(row.is_primary_department),
            category: row.category,
            org_type: row.org_type,
        })
        .collect();

    let teaching_courses: Vec<TeachingCourseItem> = teaching_res
        .unwrap_or_default()
        .into_iter()
        .map(|r| TeachingCourseItem {
            classroom_course_id: r.classroom_course_id,
            subject_code: r.subject_code,
            subject_name: r.subject_name,
            hours_per_semester: r.hours_per_semester,
            classroom_name: r.classroom_name,
            classroom_code: r.classroom_code,
            academic_year: r.academic_year,
            academic_year_label: r.academic_year_label,
            term: r.term,
            role: r.role,
        })
        .collect();

    let advisor_classrooms: Vec<AdvisorClassroomItem> = advisor_res
        .unwrap_or_default()
        .into_iter()
        .map(|r| AdvisorClassroomItem {
            classroom_id: r.classroom_id,
            classroom_name: r.classroom_name,
            classroom_code: r.classroom_code,
            academic_year: r.academic_year,
            academic_year_label: r.academic_year_label,
            role: r.role,
        })
        .collect();

    Ok(StaffProfileResponse {
        id: user.id,
        username: user.username,
        national_id: user.national_id,
        email: user.email,
        title: user.title,
        first_name: user.first_name,
        last_name: user.last_name,
        nickname: user.nickname,
        phone: user.phone,
        emergency_contact: user.emergency_contact,
        line_id: user.line_id,
        date_of_birth: user.date_of_birth.map(|d| d.to_string()),
        gender: user.gender,
        address: user.address,
        hired_date: user.hired_date.map(|d| d.to_string()),
        user_type: user.user_type,
        status: user.status,
        profile_image_url: user.profile_image_url,
        staff_info: staff_info.map(|si| StaffInfoResponse {
            education_level: si.education_level,
            major: si.major,
            university: si.university,
        }),
        roles,
        departments,
        teaching_courses,
        advisor_classrooms,
        permissions: vec![],
    })
}

/// Create staff — encrypt national_id, insert user + staff_info + roles + departments in transaction
pub async fn create_staff(pool: &PgPool, payload: CreateStaffRequest) -> Result<Uuid, AppError> {
    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST).map_err(|e| {
        eprintln!("❌ Password hashing failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่าน".to_string())
    })?;

    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let encrypted_national_id = field_encryption::encrypt_optional(payload.national_id.as_deref())
        .map_err(|e| {
            eprintln!("Encryption failed: {}", e);
            AppError::InternalServerError("Encryption error".to_string())
        })?;

    let national_id_hash = field_encryption::hash_optional_for_search(
        payload.national_id.as_deref(),
    )
    .map_err(|e| {
        eprintln!("Blind index failed: {}", e);
        AppError::InternalServerError("Encryption error".to_string())
    })?;

    // Generate username if not provided — T0001 pattern, first available slot
    let username = match payload.username.as_deref() {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => {
            let next_num: i64 = sqlx::query_scalar(
                r#"SELECT MIN(n)::bigint FROM generate_series(1, 9999) AS n
                   WHERE NOT EXISTS (
                       SELECT 1 FROM users WHERE username = 'T' || LPAD(n::text, 4, '0')
                   )"#,
            )
            .fetch_one(pool)
            .await
            .unwrap_or(Some(1))
            .unwrap_or(1);
            let generated = format!("T{:04}", next_num);
            println!("🔑 Generated staff username: {}", generated);
            generated
        }
    };

    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (
            username, national_id, national_id_hash, email, password_hash, title, first_name, last_name, nickname,
            phone, emergency_contact, line_id, date_of_birth, gender, address,
            user_type, hired_date, status, profile_image_url
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 'staff', $16, 'active', $17)
        RETURNING id",
    )
    .bind(&username)
    .bind(&encrypted_national_id)
    .bind(&national_id_hash)
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(&payload.title)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.nickname)
    .bind(&payload.phone)
    .bind(&payload.emergency_contact)
    .bind(&payload.line_id)
    .bind(payload.date_of_birth)
    .bind(&payload.gender)
    .bind(&payload.address)
    .bind(payload.hired_date)
    .bind(&payload.profile_image_url)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to create user: {}", e);
        let msg = e.to_string();
        if msg.contains("duplicate key value violates unique constraint") {
            if msg.contains("users_username_key") {
                AppError::BadRequest("รหัสผู้ใช้งาน (Username) นี้มีอยู่ในระบบแล้ว กรุณาใช้รหัสอื่น".to_string())
            } else if msg.contains("users_national_id_hash_key") {
                AppError::BadRequest("รหัสบัตรประชาชนนี้มีอยู่ในระบบแล้ว".to_string())
            } else if msg.contains("users_email_key") {
                AppError::BadRequest("อีเมลนี้มีอยู่ในระบบแล้ว".to_string())
            } else {
                AppError::BadRequest("ข้อมูลบางอย่างซ้ำกับที่มีในระบบ".to_string())
            }
        } else {
            AppError::InternalServerError("ไม่สามารถสร้างบุคลากรได้".to_string())
        }
    })?;

    if let Some(staff_info) = &payload.staff_info {
        sqlx::query(
            "INSERT INTO staff_info (
                user_id, education_level, major, university,
                teaching_license_number, teaching_license_expiry, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, '{}'::jsonb)",
        )
        .bind(user_id)
        .bind(&staff_info.education_level)
        .bind(&staff_info.major)
        .bind(&staff_info.university)
        .bind(&staff_info.teaching_license_number)
        .bind(staff_info.teaching_license_expiry)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to create staff info: {}", e);
            AppError::InternalServerError("ไม่สามารถสร้างข้อมูลบุคลากรได้".to_string())
        })?;
    }

    // Validate: all roles must have user_type = 'staff'
    if !payload.role_ids.is_empty() {
        let invalid_roles: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT code FROM roles
             WHERE id = ANY($1)
               AND (user_type != 'staff' OR is_active = false)",
        )
        .bind(&payload.role_ids)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(code,)| code)
        .collect();

        if !invalid_roles.is_empty() {
            eprintln!(
                "❌ Role validation failed for staff: invalid roles = {:?}",
                invalid_roles
            );
            return Err(AppError::BadRequest(format!(
                "มีบทบาทที่ไม่ถูกต้องสำหรับบุคลากร: {:?}",
                invalid_roles
            )));
        }
    }

    for role_id in &payload.role_ids {
        let is_primary = payload.primary_role_id.as_ref() == Some(role_id);
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
             VALUES ($1, $2, $3, CURRENT_DATE)",
        )
        .bind(user_id)
        .bind(role_id)
        .bind(is_primary)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to assign role: {}", e);
            AppError::InternalServerError("ไม่สามารถบันทึกบทบาทได้".to_string())
        })?;
    }

    if let Some(dept_assignments) = &payload.department_assignments {
        for assignment in dept_assignments {
            sqlx::query(
                "INSERT INTO department_members (
                    user_id, department_id, position, is_primary_department,
                    responsibilities, started_at
                ) VALUES ($1, $2, $3, $4, $5, CURRENT_DATE)",
            )
            .bind(user_id)
            .bind(assignment.department_id)
            .bind(&assignment.position)
            .bind(assignment.is_primary.unwrap_or(false))
            .bind(&assignment.responsibilities)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to assign department: {}", e);
                AppError::InternalServerError("ไม่สามารถบันทึกแผนกได้".to_string())
            })?;
        }
    }

    tx.commit().await.map_err(|e| {
        eprintln!("❌ Failed to commit transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการบันทึกข้อมูล".to_string())
    })?;

    println!("✅ Staff created successfully: {}", user_id);
    Ok(user_id)
}

/// Update staff — patch user + staff_info + replace roles + replace departments
pub async fn update_staff(
    pool: &PgPool,
    staff_id: Uuid,
    payload: UpdateStaffRequest,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let result = sqlx::query(
        "UPDATE users
         SET
            title = COALESCE($2, title),
            first_name = COALESCE($3, first_name),
            last_name = COALESCE($4, last_name),
            nickname = COALESCE($5, nickname),
            email = COALESCE($6, email),
            phone = COALESCE($7, phone),
            emergency_contact = COALESCE($8, emergency_contact),
            line_id = COALESCE($9, line_id),
            date_of_birth = COALESCE($10, date_of_birth),
            gender = COALESCE($11, gender),
            address = COALESCE($12, address),
            hired_date = COALESCE($13, hired_date),
            status = COALESCE($14, status),
            profile_image_url = COALESCE($15, profile_image_url),
            updated_at = NOW()
         WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
    .bind(&payload.title)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.nickname)
    .bind(&payload.email)
    .bind(&payload.phone)
    .bind(&payload.emergency_contact)
    .bind(&payload.line_id)
    .bind(payload.date_of_birth)
    .bind(&payload.gender)
    .bind(&payload.address)
    .bind(payload.hired_date)
    .bind(&payload.status)
    .bind(&payload.profile_image_url)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการอัปเดตข้อมูล".to_string())
    })?;

    if result.rows_affected() == 0 {
        if let Err(rb_err) = tx.rollback().await {
            eprintln!("⚠️ Transaction rollback failed: {}", rb_err);
        }
        return Err(AppError::NotFound("ไม่พบบุคลากร".to_string()));
    }

    if let Some(staff_info) = &payload.staff_info {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM staff_info WHERE user_id = $1)")
                .bind(staff_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

        if exists {
            sqlx::query(
                "UPDATE staff_info
                 SET
                    education_level = COALESCE($2, education_level),
                    major = COALESCE($3, major),
                    university = COALESCE($4, university),
                    updated_at = NOW()
                 WHERE user_id = $1",
            )
            .bind(staff_id)
            .bind(&staff_info.education_level)
            .bind(&staff_info.major)
            .bind(&staff_info.university)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to update staff_info: {}", e);
                AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลบุคลากรได้".to_string())
            })?;
        } else {
            sqlx::query(
                "INSERT INTO staff_info (user_id, education_level, major, university, metadata)
                 VALUES ($1, $2, $3, $4, '{}'::jsonb)",
            )
            .bind(staff_id)
            .bind(&staff_info.education_level)
            .bind(&staff_info.major)
            .bind(&staff_info.university)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to create staff_info: {}", e);
                AppError::InternalServerError("ไม่สามารถสร้างข้อมูลบุคลากรได้".to_string())
            })?;
        }
    }

    if let Some(role_ids) = &payload.role_ids {
        sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
            .bind(staff_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to delete roles: {}", e);
                AppError::InternalServerError("ไม่สามารถอัพเดตบทบาทได้".to_string())
            })?;

        for role_id in role_ids {
            let is_primary = payload.primary_role_id.as_ref() == Some(role_id);
            sqlx::query(
                "INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
                 VALUES ($1, $2, $3, CURRENT_DATE)",
            )
            .bind(staff_id)
            .bind(role_id)
            .bind(is_primary)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to insert role: {}", e);
                AppError::InternalServerError("ไม่สามารถเพิ่มบทบาทได้".to_string())
            })?;
        }
    }

    if let Some(dept_assignments) = &payload.department_assignments {
        sqlx::query("DELETE FROM department_members WHERE user_id = $1")
            .bind(staff_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to delete department members: {}", e);
                AppError::InternalServerError("ไม่สามารถอัพเดตแผนกได้".to_string())
            })?;

        for assignment in dept_assignments {
            sqlx::query(
                "INSERT INTO department_members (
                    user_id, department_id, position, is_primary_department,
                    responsibilities, started_at
                ) VALUES ($1, $2, $3, $4, $5, CURRENT_DATE)",
            )
            .bind(staff_id)
            .bind(assignment.department_id)
            .bind(&assignment.position)
            .bind(assignment.is_primary.unwrap_or(false))
            .bind(&assignment.responsibilities)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to insert department member: {}", e);
                AppError::InternalServerError("ไม่สามารถเพิ่มแผนกได้".to_string())
            })?;
        }
    }

    tx.commit().await.map_err(|e| {
        eprintln!("❌ Failed to commit transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการบันทึกข้อมูล".to_string())
    })?;

    Ok(())
}

/// Soft-delete staff (set status='inactive')
pub async fn soft_delete_staff(pool: &PgPool, staff_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query(
        "UPDATE users
         SET status = 'inactive', updated_at = NOW()
         WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
    .execute(pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการลบบุคลากร".to_string())
    })?;

    if result.rows_affected() == 0 {
        Err(AppError::NotFound("ไม่พบบุคลากร".to_string()))
    } else {
        Ok(())
    }
}

/// Public staff profile — limited fields, no national_id, no permission required (auth only)
pub async fn get_public_staff_profile(
    pool: &PgPool,
    staff_id: Uuid,
) -> Result<PublicStaffProfile, AppError> {
    #[derive(sqlx::FromRow)]
    struct PublicUserRow {
        id: Uuid,
        username: String,
        first_name: String,
        last_name: String,
        nickname: Option<String>,
        email: Option<String>,
        user_type: String,
        status: String,
        profile_image_url: Option<String>,
        title: Option<String>,
        phone: Option<String>,
        hired_date: Option<NaiveDate>,
    }

    let user_rec = sqlx::query_as::<_, PublicUserRow>(
        "SELECT id, username, first_name, last_name, nickname, email, user_type, status, profile_image_url, title, phone, hired_date
         FROM users WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error (user): {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบบุคลากร".to_string()))?;

    #[derive(sqlx::FromRow)]
    struct PublicRoleRow {
        id: Uuid,
        code: String,
        name: String,
        level: Option<i32>,
    }

    let roles = sqlx::query_as::<_, PublicRoleRow>(
        "SELECT r.id, r.code, r.name, r.level
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1",
    )
    .bind(staff_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct PublicDeptRow {
        id: Uuid,
        code: String,
        name: String,
        position: String,
    }

    let departments = sqlx::query_as::<_, PublicDeptRow>(
        "SELECT d.id, d.code, d.name, dm.position
         FROM department_members dm
         JOIN departments d ON dm.department_id = d.id
         WHERE dm.user_id = $1",
    )
    .bind(staff_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    Ok(PublicStaffProfile {
        id: user_rec.id,
        username: user_rec.username,
        first_name: user_rec.first_name,
        last_name: user_rec.last_name,
        nickname: user_rec.nickname,
        title: user_rec.title,
        email: user_rec.email,
        phone: user_rec.phone,
        hired_date: user_rec.hired_date,
        profile_image_url: user_rec.profile_image_url,
        user_type: user_rec.user_type,
        status: user_rec.status,
        roles: roles
            .into_iter()
            .map(|r| PublicStaffRole {
                id: r.id,
                code: r.code,
                name: r.name,
                level: r.level,
            })
            .collect(),
        departments: departments
            .into_iter()
            .map(|d| PublicStaffDepartment {
                id: d.id,
                code: d.code,
                name: d.name,
                position: d.position,
            })
            .collect(),
    })
}

struct StaffPageParams {
    page: i64,
    page_size: i64,
    offset: i64,
}

fn staff_page_params(filter: &StaffListFilter) -> StaffPageParams {
    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20);
    StaffPageParams {
        page,
        page_size,
        offset: (page - 1) * page_size,
    }
}

fn staff_search_pattern(search: Option<String>) -> Option<String> {
    search
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"))
}

fn staff_title_or_default(title: Option<String>) -> String {
    title.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staff_filter(
        page: Option<i64>,
        page_size: Option<i64>,
        search: Option<String>,
    ) -> StaffListFilter {
        StaffListFilter {
            user_type: None,
            role_id: None,
            department_id: None,
            page,
            page_size,
            search,
            status: None,
        }
    }

    #[test]
    fn staff_page_params_default_to_first_page_and_twenty_items() {
        let params = staff_page_params(&staff_filter(None, None, None));

        assert_eq!(params.page, 1);
        assert_eq!(params.page_size, 20);
        assert_eq!(params.offset, 0);
    }

    #[test]
    fn staff_page_params_calculate_offset() {
        let params = staff_page_params(&staff_filter(Some(3), Some(15), None));

        assert_eq!(params.offset, 30);
    }

    #[test]
    fn staff_search_pattern_ignores_empty_values() {
        assert_eq!(staff_search_pattern(None), None);
        assert_eq!(staff_search_pattern(Some("".to_string())), None);
        assert_eq!(
            staff_search_pattern(Some("ครู".to_string())),
            Some("%ครู%".to_string())
        );
    }

    #[test]
    fn staff_title_or_default_uses_empty_string_when_missing() {
        assert_eq!(staff_title_or_default(None), "");
        assert_eq!(staff_title_or_default(Some("นาย".to_string())), "นาย");
    }
}
