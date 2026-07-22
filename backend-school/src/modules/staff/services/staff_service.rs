use crate::error::AppError;
use crate::modules::staff::models::*;
use crate::policies::resource_access_policy::UserResourceListAccess;
use crate::utils::field_encryption;
use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use utoipa::ToSchema;
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
struct OrganizationUnitRow {
    id: Uuid,
    code: String,
    name: String,
    position_code: String,
    position_title: Option<String>,
    is_primary: bool,
    category: Option<String>,
    unit_type: Option<String>,
    subject_group_id: Option<Uuid>,
    responsibilities: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublicStaffRole {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub level: Option<i32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublicStaffOrganizationUnit {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub position_code: String,
    #[schema(required = true)]
    pub position_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserRoleBulkRow {
    role_id: Uuid,
    is_primary: bool,
}

fn user_role_bulk_rows(role_ids: &[Uuid], primary_role_id: Option<Uuid>) -> Vec<UserRoleBulkRow> {
    let mut seen = std::collections::HashSet::with_capacity(role_ids.len());
    role_ids
        .iter()
        .copied()
        .filter(|role_id| seen.insert(*role_id))
        .map(|role_id| UserRoleBulkRow {
            role_id,
            is_primary: Some(role_id) == primary_role_id,
        })
        .collect()
}

async fn insert_user_roles(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    rows: &[UserRoleBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let role_ids: Vec<Uuid> = rows.iter().map(|row| row.role_id).collect();
    let primary_flags: Vec<bool> = rows.iter().map(|row| row.is_primary).collect();

    sqlx::query(
        "INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
         SELECT $1, role_id, is_primary, CURRENT_DATE
         FROM UNNEST($2::uuid[], $3::bool[]) AS rows(role_id, is_primary)
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(&role_ids)
    .bind(&primary_flags)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        tracing::error!("❌ Failed to assign roles: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกบทบาทได้".to_string())
    })?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OrganizationMembershipBulkRow {
    organization_unit_id: Uuid,
    position_code: String,
    position_title: Option<String>,
    is_primary: bool,
    responsibilities: Option<String>,
}

fn organization_assignments_to_bulk_rows(
    assignments: &[OrganizationAssignment],
) -> Vec<OrganizationMembershipBulkRow> {
    let mut rows: Vec<OrganizationMembershipBulkRow> = Vec::with_capacity(assignments.len());
    let mut index_by_unit = std::collections::HashMap::with_capacity(assignments.len());

    for assignment in assignments {
        let row = OrganizationMembershipBulkRow {
            organization_unit_id: assignment.organization_unit_id,
            position_code: assignment.position_code.clone(),
            position_title: assignment.position_title.clone(),
            is_primary: assignment.is_primary.unwrap_or(false),
            responsibilities: assignment.responsibilities.clone(),
        };

        if let Some(index) = index_by_unit.get(&assignment.organization_unit_id).copied() {
            rows[index] = row;
        } else {
            index_by_unit.insert(assignment.organization_unit_id, rows.len());
            rows.push(row);
        }
    }

    rows
}

async fn insert_organization_memberships(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    rows: &[OrganizationMembershipBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let organization_unit_ids: Vec<Uuid> =
        rows.iter().map(|row| row.organization_unit_id).collect();
    let position_codes: Vec<String> = rows.iter().map(|row| row.position_code.clone()).collect();
    let position_titles: Vec<Option<String>> =
        rows.iter().map(|row| row.position_title.clone()).collect();
    let primary_flags: Vec<bool> = rows.iter().map(|row| row.is_primary).collect();
    let responsibilities: Vec<Option<String>> = rows
        .iter()
        .map(|row| row.responsibilities.clone())
        .collect();

    sqlx::query(
        "INSERT INTO organization_members (
            user_id, organization_unit_id, position_code, position_title,
            is_primary, responsibilities, started_at
        )
        SELECT $1, organization_unit_id, position_code, position_title,
               is_primary, responsibilities, CURRENT_DATE
        FROM UNNEST($2::uuid[], $3::text[], $4::text[], $5::bool[], $6::text[])
             AS rows(organization_unit_id, position_code, position_title, is_primary, responsibilities)",
    )
    .bind(user_id)
    .bind(&organization_unit_ids)
    .bind(&position_codes)
    .bind(&position_titles)
    .bind(&primary_flags)
    .bind(&responsibilities)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        tracing::error!("❌ Failed to assign organization units: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกหน่วยงานได้".to_string())
    })?;

    Ok(())
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublicStaffProfile {
    pub id: Uuid,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    #[schema(required = true)]
    pub nickname: Option<String>,
    #[schema(required = true)]
    pub title: Option<String>,
    #[schema(required = true)]
    pub email: Option<String>,
    #[schema(required = true)]
    pub phone: Option<String>,
    #[schema(required = true)]
    pub hired_date: Option<NaiveDate>,
    #[schema(required = true)]
    pub profile_image_url: Option<String>,
    pub user_type: String,
    pub status: String,
    pub roles: Vec<PublicStaffRole>,
    pub organization_units: Vec<PublicStaffOrganizationUnit>,
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
    access: UserResourceListAccess,
) -> Result<(Vec<StaffListItem>, i64, i64, i64), AppError> {
    let page_params = staff_page_params(&filter);

    let search_pattern = staff_search_pattern(filter.search.clone());
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT DISTINCT u.id, u.username, u.title, u.first_name, u.last_name, u.status
         FROM users u
         WHERE u.user_type = 'staff'",
    );
    push_staff_list_filters(&mut query, &filter, search_pattern.as_deref(), access);
    query
        .push(" ORDER BY u.first_name LIMIT ")
        .push_bind(page_params.page_size)
        .push(" OFFSET ")
        .push_bind(page_params.offset);

    let staff_rows = query
        .build_query_as::<(Uuid, String, Option<String>, String, String, String)>()
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
        })?;

    let mut count_query = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(DISTINCT u.id) FROM users u WHERE u.user_type = 'staff'",
    );
    push_staff_list_filters(&mut count_query, &filter, search_pattern.as_deref(), access);
    let total: i64 = count_query
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการนับข้อมูล".to_string())
        })?;

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
                organization_units: vec![],
                status,
            },
        )
        .collect();

    Ok((items, total, page_params.page, page_params.page_size))
}

fn push_staff_list_filters<'args>(
    query: &mut QueryBuilder<'args, Postgres>,
    filter: &'args StaffListFilter,
    search_pattern: Option<&'args str>,
    access: UserResourceListAccess,
) {
    match &filter.status {
        Some(status) => {
            query.push(" AND u.status = ").push_bind(status);
        }
        None => {
            query.push(" AND u.status = 'active'");
        }
    }

    if let Some(pattern) = search_pattern {
        query
            .push(" AND (u.first_name ILIKE ")
            .push_bind(pattern)
            .push(" OR u.last_name ILIKE ")
            .push_bind(pattern)
            .push(" OR u.username ILIKE ")
            .push_bind(pattern)
            .push(")");
    }

    push_staff_list_access_filter(query, access);
}

fn push_staff_list_access_filter(
    query: &mut QueryBuilder<'_, Postgres>,
    access: UserResourceListAccess,
) {
    match access {
        UserResourceListAccess::School => {}
        UserResourceListAccess::Own(actor_user_id)
        | UserResourceListAccess::Assigned(actor_user_id) => {
            query.push(" AND u.id = ").push_bind(actor_user_id);
        }
        UserResourceListAccess::OrganizationUnit(actor_user_id) => {
            query
                .push(
                    r#" AND EXISTS (
                        SELECT 1
                        FROM organization_members actor_member
                        JOIN organization_members target_member
                          ON target_member.organization_unit_id = actor_member.organization_unit_id
                        WHERE actor_member.user_id = "#,
                )
                .push_bind(actor_user_id)
                .push(
                    r#" AND target_member.user_id = u.id
                        AND (actor_member.ended_at IS NULL OR actor_member.ended_at > CURRENT_DATE)
                        AND (target_member.ended_at IS NULL OR target_member.ended_at > CURRENT_DATE)
                    )"#,
                );
        }
        UserResourceListAccess::OrganizationTree(actor_user_id) => {
            query
                .push(
                    r#" AND EXISTS (
                        WITH RECURSIVE actor_roots AS (
                            SELECT organization_unit_id
                            FROM organization_members
                            WHERE user_id = "#,
                )
                .push_bind(actor_user_id)
                .push(
                    r#"
                              AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
                        ),
                        organization_tree AS (
                            SELECT organization_unit_id
                            FROM actor_roots
                            UNION
                            SELECT child.id
                            FROM organization_units child
                            JOIN organization_tree parent_tree
                              ON child.parent_unit_id = parent_tree.organization_unit_id
                            WHERE child.is_active = true
                        )
                        SELECT 1
                        FROM organization_members target_member
                        WHERE target_member.user_id = u.id
                          AND target_member.organization_unit_id IN (
                              SELECT organization_unit_id FROM organization_tree
                          )
                          AND (target_member.ended_at IS NULL OR target_member.ended_at > CURRENT_DATE)
                    )"#,
                );
        }
    }
}

/// Get staff full profile with parallel queries
pub async fn get_staff_profile(
    pool: &PgPool,
    staff_id: Uuid,
    include_pii: bool,
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
        tracing::error!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบบุคลากร".to_string()))?;

    if include_pii {
        if let Some(nid) = &user.national_id {
            if let Ok(dec) = field_encryption::decrypt(nid) {
                user.national_id = Some(dec);
            }
        }
    } else {
        user.national_id = None;
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

    let organization_units_fut = sqlx::query_as::<_, OrganizationUnitRow>(
        "SELECT ou.id, ou.code, ou.name, ou.category, ou.unit_type, ou.subject_group_id,
                om.position_code, om.position_title, om.is_primary, om.responsibilities
         FROM organization_members om
         JOIN organization_units ou ON om.organization_unit_id = ou.id
         WHERE om.user_id = $1 AND om.ended_at IS NULL
         ORDER BY om.is_primary DESC",
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

    let (staff_info_res, roles_res, organization_units_res, teaching_res, advisor_res) = tokio::join!(
        staff_info_fut,
        roles_fut,
        organization_units_fut,
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

    let organization_units: Vec<OrganizationUnitResponse> = organization_units_res
        .unwrap_or_default()
        .into_iter()
        .map(|row| OrganizationUnitResponse {
            id: row.id,
            code: row.code,
            name: row.name,
            position_code: Some(row.position_code),
            position_title: row.position_title,
            is_primary: Some(row.is_primary),
            category: row.category,
            unit_type: row.unit_type,
            subject_group_id: row.subject_group_id,
            responsibilities: row.responsibilities,
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
        organization_units,
        teaching_courses,
        advisor_classrooms,
        permissions: vec![],
    })
}

/// Create staff — encrypt national_id, insert user + staff_info + roles + organization memberships in transaction
pub async fn create_staff(pool: &PgPool, payload: CreateStaffRequest) -> Result<Uuid, AppError> {
    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST).map_err(|e| {
        tracing::error!("❌ Password hashing failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่าน".to_string())
    })?;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let encrypted_national_id = field_encryption::encrypt_optional(payload.national_id.as_deref())
        .map_err(|e| {
            tracing::error!("Encryption failed: {}", e);
            AppError::InternalServerError("Encryption error".to_string())
        })?;

    let national_id_hash = field_encryption::hash_optional_for_search(
        payload.national_id.as_deref(),
    )
    .map_err(|e| {
        tracing::error!("Blind index failed: {}", e);
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
            tracing::info!("🔑 Generated staff username: {}", generated);
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
        tracing::error!("❌ Failed to create user: {}", e);
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
            tracing::error!("❌ Failed to create staff info: {}", e);
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
            tracing::error!(
                "❌ Role validation failed for staff: invalid roles = {:?}",
                invalid_roles
            );
            return Err(AppError::BadRequest(format!(
                "มีบทบาทที่ไม่ถูกต้องสำหรับบุคลากร: {:?}",
                invalid_roles
            )));
        }
    }

    let role_rows = user_role_bulk_rows(&payload.role_ids, payload.primary_role_id);
    insert_user_roles(&mut tx, user_id, &role_rows).await?;

    if let Some(organization_assignments) = &payload.organization_assignments {
        let membership_rows = organization_assignments_to_bulk_rows(organization_assignments);
        insert_organization_memberships(&mut tx, user_id, &membership_rows).await?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("❌ Failed to commit transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการบันทึกข้อมูล".to_string())
    })?;

    tracing::info!("✅ Staff created successfully: {}", user_id);
    Ok(user_id)
}

/// Update staff — patch user + staff_info + replace roles + replace organization memberships
pub async fn update_staff(
    pool: &PgPool,
    staff_id: Uuid,
    payload: UpdateStaffRequest,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("❌ Failed to start transaction: {}", e);
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
        tracing::error!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการอัปเดตข้อมูล".to_string())
    })?;

    if result.rows_affected() == 0 {
        if let Err(rb_err) = tx.rollback().await {
            tracing::error!("⚠️ Transaction rollback failed: {}", rb_err);
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
                tracing::error!("❌ Failed to update staff_info: {}", e);
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
                tracing::error!("❌ Failed to create staff_info: {}", e);
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
                tracing::error!("❌ Failed to delete roles: {}", e);
                AppError::InternalServerError("ไม่สามารถอัพเดตบทบาทได้".to_string())
            })?;

        let role_rows = user_role_bulk_rows(role_ids, payload.primary_role_id);
        insert_user_roles(&mut tx, staff_id, &role_rows).await?;
    }

    if let Some(organization_assignments) = &payload.organization_assignments {
        sqlx::query("DELETE FROM organization_members WHERE user_id = $1")
            .bind(staff_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!("❌ Failed to delete organization members: {}", e);
                AppError::InternalServerError("ไม่สามารถอัพเดตหน่วยงานได้".to_string())
            })?;

        let membership_rows = organization_assignments_to_bulk_rows(organization_assignments);
        insert_organization_memberships(&mut tx, staff_id, &membership_rows).await?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("❌ Failed to commit transaction: {}", e);
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
        tracing::error!("❌ Database error: {}", e);
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
        tracing::error!("❌ Database error (user): {}", e);
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
    struct PublicOrganizationUnitRow {
        id: Uuid,
        code: String,
        name: String,
        position_code: String,
        position_title: Option<String>,
    }

    let organization_units = sqlx::query_as::<_, PublicOrganizationUnitRow>(
        "SELECT ou.id, ou.code, ou.name, om.position_code, om.position_title
         FROM organization_members om
         JOIN organization_units ou ON om.organization_unit_id = ou.id
         WHERE om.user_id = $1",
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
        organization_units: organization_units
            .into_iter()
            .map(|unit| PublicStaffOrganizationUnit {
                id: unit.id,
                code: unit.code,
                name: unit.name,
                position_code: unit.position_code,
                position_title: unit.position_title,
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
            organization_unit_id: None,
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

    #[test]
    fn user_role_bulk_rows_dedupes_roles_and_marks_primary() {
        let role_a = Uuid::new_v4();
        let role_b = Uuid::new_v4();

        assert_eq!(
            user_role_bulk_rows(&[role_a, role_b, role_a], Some(role_b)),
            vec![
                UserRoleBulkRow {
                    role_id: role_a,
                    is_primary: false,
                },
                UserRoleBulkRow {
                    role_id: role_b,
                    is_primary: true,
                },
            ]
        );
    }

    #[test]
    fn organization_assignments_to_bulk_rows_keeps_latest_unit_assignment() {
        let organization_unit_id = Uuid::new_v4();

        assert_eq!(
            organization_assignments_to_bulk_rows(&[
                OrganizationAssignment {
                    organization_unit_id,
                    position_code: "member".to_string(),
                    position_title: None,
                    is_primary: Some(false),
                    responsibilities: None,
                },
                OrganizationAssignment {
                    organization_unit_id,
                    position_code: "head".to_string(),
                    position_title: Some("หัวหน้ากลุ่ม".to_string()),
                    is_primary: Some(true),
                    responsibilities: Some("ดูแลหลักสูตร".to_string()),
                },
            ]),
            vec![OrganizationMembershipBulkRow {
                organization_unit_id,
                position_code: "head".to_string(),
                position_title: Some("หัวหน้ากลุ่ม".to_string()),
                is_primary: true,
                responsibilities: Some("ดูแลหลักสูตร".to_string()),
            }]
        );
    }
}
