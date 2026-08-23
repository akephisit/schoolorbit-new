use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::facility::models::Room;

use super::models::{
    AcademicLookupQuery, AcademicYearLookupItem, GradeLevelLookupItem, HomeroomLookupItem,
    LookupItem, LookupQuery, OrganizationUnitLookupItem, RoleLookupItem, StaffLookupItem,
    StudentLookupItem,
};

#[derive(Debug, FromRow)]
struct StaffRow {
    id: Uuid,
    title: Option<String>,
    first_name: String,
    last_name: String,
}

#[derive(Debug, FromRow)]
struct RoleRow {
    id: Uuid,
    code: String,
    name: String,
    user_type: String,
}

#[derive(Debug, FromRow)]
struct OrganizationUnitRow {
    id: Uuid,
    code: String,
    name: String,
    name_en: Option<String>,
    description: Option<String>,
    category: Option<String>,
    display_order: i32,
    is_active: bool,
    parent_unit_id: Option<Uuid>,
    unit_type: Option<String>,
    subject_group_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct GradeLevelRow {
    id: Uuid,
    level_type: String,
    year: i32,
}

#[derive(Debug, FromRow)]
struct HomeroomRow {
    id: Uuid,
    name: String,
    level_type: Option<String>,
    year: Option<i32>,
    grade_level_id: Option<Uuid>,
}

#[derive(Debug, FromRow)]
struct AcademicYearRow {
    id: Uuid,
    name: String,
    year: i32,
    status: crate::modules::academic::core::models::AcademicYearStatus,
}

#[derive(Debug, FromRow)]
struct StudentWithInfoRow {
    id: Uuid,
    title: Option<String>,
    first_name: String,
    last_name: String,
    student_id: Option<String>,
    homeroom: Option<String>,
}

#[derive(Debug, FromRow)]
struct SubjectRow {
    id: Uuid,
    code: String,
    name_th: String,
    #[sqlx(default)]
    grade_level_ids: Option<Vec<Uuid>>,
}

pub async fn verify_active_user(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND status = 'active')",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    if !exists {
        return Err(AppError::AuthError("ไม่พบผู้ใช้หรือบัญชีถูกระงับ".to_string()));
    }

    Ok(())
}

pub async fn lookup_staff(
    pool: &PgPool,
    query: LookupQuery,
) -> Result<Vec<StaffLookupItem>, AppError> {
    let limit = lookup_limit(query.limit);
    let active_only = query.active_only.unwrap_or(true);

    let mut sql = String::from(
        "SELECT id, title, first_name, last_name
         FROM users
         WHERE user_type = 'staff'",
    );

    if active_only {
        sql.push_str(" AND status = 'active'");
    }

    let search_pattern = search_pattern(query.search.clone());
    if search_pattern.is_some() {
        sql.push_str(" AND (first_name ILIKE $1 OR last_name ILIKE $1 OR username ILIKE $1)");
    }

    sql.push_str(&format!(" ORDER BY first_name, last_name LIMIT {}", limit));

    let mut query_builder = sqlx::query_as::<_, StaffRow>(&sql);
    if let Some(ref pattern) = search_pattern {
        query_builder = query_builder.bind(pattern);
    }

    let rows = query_builder.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| StaffLookupItem {
            id: row.id,
            name: format!("{} {}", row.first_name, row.last_name),
            title: row.title,
        })
        .collect())
}

pub async fn lookup_roles(
    pool: &PgPool,
    query: LookupQuery,
) -> Result<Vec<RoleLookupItem>, AppError> {
    let limit = lookup_limit(query.limit);
    let active_only = query.active_only.unwrap_or(true);

    let mut sql = String::from("SELECT id, code, name, user_type FROM roles WHERE 1=1");

    if active_only {
        sql.push_str(" AND is_active = true");
    }

    let search_pattern = search_pattern(query.search.clone());
    if search_pattern.is_some() {
        sql.push_str(" AND (name ILIKE $1 OR code ILIKE $1)");
    }

    sql.push_str(&format!(" ORDER BY level DESC, name LIMIT {}", limit));

    let mut query_builder = sqlx::query_as::<_, RoleRow>(&sql);
    if let Some(ref pattern) = search_pattern {
        query_builder = query_builder.bind(pattern);
    }

    let rows = query_builder.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| RoleLookupItem {
            id: row.id,
            code: row.code,
            name: row.name,
            user_type: row.user_type,
        })
        .collect())
}

pub async fn lookup_organization_units(
    pool: &PgPool,
    user_id: Uuid,
    query: LookupQuery,
) -> Result<Vec<OrganizationUnitLookupItem>, AppError> {
    let limit = lookup_limit(query.limit);
    let active_only = query.active_only.unwrap_or(true);
    let member_only = query.member_only.unwrap_or(false);

    let mut sql = String::from(
        "SELECT id, code, name, name_en, description, category, display_order,
                is_active, parent_unit_id, unit_type, subject_group_id
         FROM organization_units
         WHERE 1=1",
    );

    if active_only {
        sql.push_str(" AND is_active = true");
    }

    if member_only {
        sql.push_str(
            " AND EXISTS (
                SELECT 1 FROM organization_members om
                WHERE om.organization_unit_id = organization_units.id
                  AND om.user_id = $1
                  AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
            )",
        );
    }

    let search_pattern = search_pattern(query.search.clone());
    if search_pattern.is_some() {
        let param_idx = if member_only { 2 } else { 1 };
        sql.push_str(&format!(
            " AND (name ILIKE ${0} OR code ILIKE ${0})",
            param_idx
        ));
    }

    sql.push_str(&format!(" ORDER BY display_order, name LIMIT {}", limit));

    let mut query_builder = sqlx::query_as::<_, OrganizationUnitRow>(&sql);
    if member_only {
        query_builder = query_builder.bind(user_id);
    }
    if let Some(ref pattern) = search_pattern {
        query_builder = query_builder.bind(pattern);
    }

    let rows = query_builder.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(organization_unit_lookup_item)
        .collect())
}

pub async fn lookup_organization_unit_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<OrganizationUnitLookupItem, AppError> {
    sqlx::query_as::<_, OrganizationUnitRow>(
        "SELECT id, code, name, name_en, description, category, display_order,
                is_active, parent_unit_id, unit_type, subject_group_id
         FROM organization_units
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .map(organization_unit_lookup_item)
    .ok_or(AppError::NotFound("ไม่พบหน่วยงานนี้".to_string()))
}

pub async fn lookup_grade_levels(
    pool: &PgPool,
    query: AcademicLookupQuery,
) -> Result<Vec<GradeLevelLookupItem>, AppError> {
    let limit = lookup_limit(query.limit);
    let search = query.search.clone();
    let param_idx: i32 = 2;
    let mut bind_level_type: Option<String> = None;
    let mut sql = String::from(
        "SELECT gl.id, gl.level_type, gl.year
         FROM grade_levels gl
         JOIN academic_year_grade_levels aygl ON gl.id = aygl.grade_level_id
         WHERE aygl.academic_year_id = $1",
    );

    if query.active_only.unwrap_or(true) {
        sql.push_str(" AND gl.is_active = true");
    }

    if let Some(ref level_type) = query.level_type {
        sql.push_str(&format!(" AND gl.level_type = ${}", param_idx));
        bind_level_type = Some(level_type.clone());
    }

    sql.push_str(
        " ORDER BY CASE gl.level_type
            WHEN 'kindergarten' THEN 1
            WHEN 'primary' THEN 2
            WHEN 'secondary' THEN 3
            ELSE 4
         END, gl.year ASC LIMIT 500",
    );

    let mut query_builder = sqlx::query_as::<_, GradeLevelRow>(&sql).bind(query.academic_year_id);
    if let Some(ref level_type) = bind_level_type {
        query_builder = query_builder.bind(level_type);
    }

    let rows = query_builder.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let data: Vec<GradeLevelLookupItem> = rows.into_iter().map(grade_level_lookup_item).collect();

    if let Some(search) = search {
        Ok(data
            .into_iter()
            .filter(|item| item.name.contains(&search) || item.code.contains(&search))
            .take(limit as usize)
            .collect())
    } else {
        Ok(data)
    }
}

pub async fn lookup_homerooms(
    pool: &PgPool,
    query: AcademicLookupQuery,
) -> Result<Vec<HomeroomLookupItem>, AppError> {
    let limit = lookup_limit(query.limit);

    let mut sql = String::from(
        "SELECT h.id, h.name, g.level_type, g.year, h.grade_level_id
         FROM homerooms h
         LEFT JOIN grade_levels g ON h.grade_level_id = g.id
         WHERE h.academic_year_id = $1",
    );

    if query.active_only.unwrap_or(true) {
        sql.push_str(" AND h.is_active = true");
    }

    let search_pattern = search_pattern(query.search.clone());
    if search_pattern.is_some() {
        sql.push_str(" AND h.name ILIKE $2");
    }

    sql.push_str(&format!(" ORDER BY g.year, h.name LIMIT {}", limit));

    let mut query_builder = sqlx::query_as::<_, HomeroomRow>(&sql).bind(query.academic_year_id);
    if let Some(ref pattern) = search_pattern {
        query_builder = query_builder.bind(pattern);
    }

    let rows = query_builder.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| HomeroomLookupItem {
            id: row.id,
            name: row.name,
            grade_level: homeroom_grade_level_label(row.level_type.as_deref(), row.year),
            grade_level_id: row.grade_level_id,
        })
        .collect())
}

pub async fn lookup_academic_years(
    pool: &PgPool,
    query: LookupQuery,
) -> Result<Vec<AcademicYearLookupItem>, AppError> {
    let limit = lookup_limit(query.limit);
    let active_only = query.active_only.unwrap_or(true);

    let mut sql = String::from("SELECT id, name, year, status FROM academic_years WHERE 1=1");

    if active_only {
        sql.push_str(" AND status <> 'archived'");
    }

    let search_pattern = search_pattern(query.search.clone());
    if search_pattern.is_some() {
        sql.push_str(" AND name ILIKE $1");
    }

    sql.push_str(&format!(
        " ORDER BY (status = 'active') DESC, year DESC LIMIT {}",
        limit
    ));

    let mut query_builder = sqlx::query_as::<_, AcademicYearRow>(&sql);
    if let Some(ref pattern) = search_pattern {
        query_builder = query_builder.bind(pattern);
    }

    let rows = query_builder.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| AcademicYearLookupItem {
            id: row.id,
            name: row.name,
            year: row.year,
            status: row.status,
        })
        .collect())
}

pub async fn lookup_students(
    pool: &PgPool,
    query: AcademicLookupQuery,
) -> Result<Vec<StudentLookupItem>, AppError> {
    let limit = lookup_limit(query.limit);
    let active_only = query.active_only.unwrap_or(true);

    let mut sql = String::from(
        "SELECT u.id, u.title, u.first_name, u.last_name,
                si.student_id,
                h.name as homeroom
         FROM users u
         JOIN student_academic_years sy
           ON sy.student_id = u.id
          AND sy.academic_year_id = $1
         LEFT JOIN student_info si ON u.id = si.user_id
         LEFT JOIN LATERAL (
             SELECT placement.homeroom_id
             FROM homeroom_placements placement
             WHERE placement.student_academic_year_id = sy.id
               AND placement.academic_year_id = sy.academic_year_id
               AND placement.status IN ('current', 'planned')
             ORDER BY (placement.status = 'current') DESC, placement.start_date DESC
             LIMIT 1
         ) placement ON true
         LEFT JOIN homerooms h ON h.id = placement.homeroom_id
         WHERE u.user_type = 'student'",
    );

    if active_only {
        sql.push_str(" AND u.status = 'active'");
    }

    let search_pattern = search_pattern(query.search.clone());
    if search_pattern.is_some() {
        sql.push_str(
            " AND (u.first_name ILIKE $2 OR u.last_name ILIKE $2 OR u.username ILIKE $2 OR si.student_id ILIKE $2)",
        );
    }

    sql.push_str(&format!(
        " ORDER BY u.first_name, u.last_name LIMIT {}",
        limit
    ));

    let mut query_builder =
        sqlx::query_as::<_, StudentWithInfoRow>(&sql).bind(query.academic_year_id);
    if let Some(ref pattern) = search_pattern {
        query_builder = query_builder.bind(pattern);
    }

    let rows = query_builder.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| StudentLookupItem {
            id: row.id,
            name: format!("{} {}", row.first_name, row.last_name),
            title: row.title,
            student_id: row.student_id,
            homeroom: row.homeroom,
        })
        .collect())
}

pub async fn lookup_rooms(pool: &PgPool) -> Result<Vec<Room>, AppError> {
    sqlx::query_as::<_, Room>(
        r#"
        SELECT r.id, r.building_id, r.name_th, r.name_en, r.code,
               r.room_type, r.capacity, r.floor, r.status, r.description,
               r.created_at, r.updated_at, b.name_th as building_name
        FROM rooms r
        LEFT JOIN buildings b ON r.building_id = b.id
        WHERE r.status = 'ACTIVE'
        ORDER BY b.code NULLS LAST, r.floor NULLS FIRST, r.code ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Lookup Rooms Error: {}", e);
        AppError::InternalServerError("Failed to fetch rooms".to_string())
    })
}

pub async fn lookup_subjects(
    pool: &PgPool,
    query: AcademicLookupQuery,
) -> Result<Vec<LookupItem>, AppError> {
    let limit = lookup_limit(query.limit);
    let active_only = query.active_only.unwrap_or(true);

    let mut sql = String::from(
        "SELECT subject.id, subject.code, version.name_th,
                (SELECT array_agg(mapping.grade_level_id)
                 FROM subject_version_grade_levels mapping
                 WHERE mapping.subject_id = version.id) AS grade_level_ids
         FROM subjects subject
         JOIN academic_years year ON year.id = $1
         JOIN LATERAL (
             SELECT candidate.*
             FROM subject_versions candidate
             WHERE candidate.subject_id = subject.id
               AND candidate.effective_from <= year.end_date
               AND (candidate.effective_until IS NULL OR candidate.effective_until > year.start_date)",
    );

    if active_only {
        sql.push_str(" AND candidate.status = 'published'");
    }
    sql.push_str(
        " ORDER BY candidate.effective_from DESC, candidate.version_no DESC
          LIMIT 1
         ) version ON true
         WHERE 1=1",
    );

    let mut param_idx: i32 = 2;
    let mut bind_subject_type: Option<String> = None;
    let mut bind_search: Option<String> = None;

    if let Some(ref subject_type) = query.subject_type {
        sql.push_str(&format!(" AND type = ${}", param_idx));
        bind_subject_type = Some(subject_type.clone());
        param_idx += 1;
    }

    if let Some(ref pattern) = search_pattern(query.search.clone()) {
        sql.push_str(&format!(
            " AND (name_th ILIKE ${0} OR code ILIKE ${0})",
            param_idx
        ));
        bind_search = Some(pattern.clone());
    }

    sql.push_str(&format!(" ORDER BY code, name_th LIMIT {}", limit));

    let mut query_builder = sqlx::query_as::<_, SubjectRow>(&sql).bind(query.academic_year_id);
    if let Some(ref subject_type) = bind_subject_type {
        query_builder = query_builder.bind(subject_type);
    }
    if let Some(ref pattern) = bind_search {
        query_builder = query_builder.bind(pattern);
    }

    let rows = query_builder.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| LookupItem {
            id: row.id,
            name: row.name_th,
            code: Some(row.code),
            grade_level_ids: row.grade_level_ids,
        })
        .collect())
}

fn organization_unit_lookup_item(row: OrganizationUnitRow) -> OrganizationUnitLookupItem {
    OrganizationUnitLookupItem {
        id: row.id,
        code: row.code,
        name: row.name,
        name_en: row.name_en,
        description: row.description,
        category: row.category,
        display_order: row.display_order,
        is_active: row.is_active,
        parent_unit_id: row.parent_unit_id,
        unit_type: row.unit_type,
        subject_group_id: row.subject_group_id,
    }
}

fn lookup_limit(limit: Option<i32>) -> i32 {
    limit.unwrap_or(100).min(500)
}

fn search_pattern(search: Option<String>) -> Option<String> {
    search
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"))
}

fn grade_level_lookup_item(row: GradeLevelRow) -> GradeLevelLookupItem {
    let (name, code, short_name) = match row.level_type.as_str() {
        "kindergarten" => (
            format!("อนุบาลปีที่ {}", row.year),
            format!("K{}", row.year),
            format!("อ.{}", row.year),
        ),
        "primary" => (
            format!("ประถมศึกษาปีที่ {}", row.year),
            format!("P{}", row.year),
            format!("ป.{}", row.year),
        ),
        "secondary" => (
            format!("มัธยมศึกษาปีที่ {}", row.year),
            format!("M{}", row.year),
            format!("ม.{}", row.year),
        ),
        _ => (
            format!("Other {}", row.year),
            format!("O{}", row.year),
            format!("?{}", row.year),
        ),
    };
    let order_base = match row.level_type.as_str() {
        "kindergarten" => 1,
        "primary" => 2,
        "secondary" => 3,
        _ => 4,
    };

    GradeLevelLookupItem {
        id: row.id,
        code,
        name,
        short_name: Some(short_name),
        level_type: row.level_type,
        level_order: order_base * 100 + row.year,
    }
}

fn homeroom_grade_level_label(level_type: Option<&str>, year: Option<i32>) -> Option<String> {
    match (level_type, year) {
        (Some("kindergarten"), Some(year)) => Some(format!("อ.{}", year)),
        (Some("primary"), Some(year)) => Some(format!("ป.{}", year)),
        (Some("secondary"), Some(year)) => Some(format!("ม.{}", year)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::academic::cutover_test_support::{
            apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
        },
        test_helpers::create_named_test_pool,
    };

    fn academic_query(academic_year_id: Uuid) -> AcademicLookupQuery {
        AcademicLookupQuery {
            academic_year_id,
            active_only: Some(true),
            search: None,
            limit: Some(100),
            level_type: None,
            subject_type: None,
        }
    }

    #[test]
    fn lookup_limit_defaults_to_one_hundred_and_caps_at_five_hundred() {
        assert_eq!(lookup_limit(None), 100);
        assert_eq!(lookup_limit(Some(20)), 20);
        assert_eq!(lookup_limit(Some(900)), 500);
    }

    #[test]
    fn search_pattern_wraps_non_empty_input_for_ilike() {
        assert_eq!(
            search_pattern(Some("math".to_string())),
            Some("%math%".to_string())
        );
        assert_eq!(search_pattern(Some("".to_string())), None);
        assert_eq!(search_pattern(None), None);
    }

    #[test]
    fn grade_level_lookup_item_formats_secondary_level_consistently() {
        let item = grade_level_lookup_item(GradeLevelRow {
            id: Uuid::new_v4(),
            level_type: "secondary".to_string(),
            year: 4,
        });

        assert_eq!(item.code, "M4");
        assert_eq!(item.name, "มัธยมศึกษาปีที่ 4");
        assert_eq!(item.short_name.as_deref(), Some("ม.4"));
        assert_eq!(item.level_order, 304);
    }

    #[tokio::test]
    async fn academic_lookups_read_only_the_caller_selected_canonical_year() {
        let pool = create_named_test_pool("canonical_academic_lookups").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 44).await.unwrap();
        let academic_year_id: Uuid =
            sqlx::query_scalar("SELECT id FROM academic_years WHERE status = 'active'")
                .fetch_one(&pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO academic_year_grade_levels (academic_year_id, grade_level_id)
             SELECT DISTINCT academic_year_id, grade_level_id
             FROM homerooms
             WHERE academic_year_id = $1
             ON CONFLICT DO NOTHING",
        )
        .bind(academic_year_id)
        .execute(&pool)
        .await
        .unwrap();

        let grade_levels = lookup_grade_levels(&pool, academic_query(academic_year_id))
            .await
            .unwrap();
        let homerooms = lookup_homerooms(&pool, academic_query(academic_year_id))
            .await
            .unwrap();
        let students = lookup_students(&pool, academic_query(academic_year_id))
            .await
            .unwrap();
        let subjects = lookup_subjects(&pool, academic_query(academic_year_id))
            .await
            .unwrap();

        assert!(!grade_levels.is_empty());
        assert!(!homerooms.is_empty());
        assert!(students.iter().any(|student| student.homeroom.is_some()));
        assert!(!subjects.is_empty());
    }

    #[test]
    fn classroom_grade_level_label_returns_none_for_missing_level_data() {
        assert_eq!(
            homeroom_grade_level_label(Some("primary"), Some(6)),
            Some("ป.6".to_string())
        );
        assert_eq!(homeroom_grade_level_label(None, Some(6)), None);
        assert_eq!(homeroom_grade_level_label(Some("primary"), None), None);
    }
}
