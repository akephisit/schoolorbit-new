use crate::error::AppError;
use crate::modules::academic::models::curriculum::{
    AddSubjectDefaultInstructorRequest, CreateSubjectRequest, DefaultInstructorInput, Subject,
    SubjectDefaultInstructor, SubjectFilter, SubjectGroup, UpdateSubjectRequest,
};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubjectGroupAccess {
    All,
    Groups(Vec<Uuid>),
}

fn ordered_unique_subject_grade_level_ids(level_ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = HashSet::with_capacity(level_ids.len());
    level_ids
        .iter()
        .copied()
        .filter(|level_id| seen.insert(*level_id))
        .collect()
}

fn unique_subject_default_instructors(
    team: &[DefaultInstructorInput],
) -> Result<Vec<DefaultInstructorInput>, AppError> {
    let mut rows: Vec<DefaultInstructorInput> = Vec::with_capacity(team.len());
    let mut index_by_instructor = HashMap::with_capacity(team.len());

    for instructor in team {
        let role = instructor.role.clone();
        validate_subject_instructor_role(&role)?;
        let row = DefaultInstructorInput {
            instructor_id: instructor.instructor_id,
            role,
        };
        if let Some(index) = index_by_instructor.get(&instructor.instructor_id).copied() {
            rows[index] = row;
        } else {
            index_by_instructor.insert(instructor.instructor_id, rows.len());
            rows.push(row);
        }
    }

    Ok(rows)
}

async fn bulk_insert_subject_grade_levels(
    tx: &mut Transaction<'_, Postgres>,
    subject_id: Uuid,
    level_ids: &[Uuid],
) -> Result<(), AppError> {
    let level_ids = ordered_unique_subject_grade_level_ids(level_ids);
    if level_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO subject_grade_levels (subject_id, grade_level_id)
         SELECT $1, grade_level_id
         FROM UNNEST($2::uuid[]) AS rows(grade_level_id)
         ON CONFLICT DO NOTHING",
    )
    .bind(subject_id)
    .bind(&level_ids)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        AppError::InternalServerError(format!("Failed to save grade level links: {}", e))
    })?;

    Ok(())
}

async fn bulk_upsert_subject_default_instructors(
    tx: &mut Transaction<'_, Postgres>,
    subject_id: Uuid,
    team: &[DefaultInstructorInput],
) -> Result<(), AppError> {
    let team = unique_subject_default_instructors(team)?;
    if team.is_empty() {
        return Ok(());
    }

    let instructor_ids: Vec<Uuid> = team.iter().map(|item| item.instructor_id).collect();
    let roles: Vec<String> = team.iter().map(|item| item.role.clone()).collect();

    sqlx::query(
        "INSERT INTO subject_default_instructors (subject_id, instructor_id, role)
         SELECT $1, instructor_id, role
         FROM UNNEST($2::uuid[], $3::text[]) AS rows(instructor_id, role)
         ON CONFLICT (subject_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(subject_id)
    .bind(&instructor_ids)
    .bind(&roles)
    .execute(&mut **tx)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to save team: {}", e)))?;

    Ok(())
}

pub async fn list_subject_groups(pool: &PgPool) -> Result<Vec<SubjectGroup>, AppError> {
    sqlx::query_as::<_, SubjectGroup>(
        "SELECT * FROM subject_groups WHERE is_active = true ORDER BY display_order ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch subject groups: {}", e);
        AppError::InternalServerError("Failed to fetch subject groups".to_string())
    })
}

pub async fn list_subjects(
    pool: &PgPool,
    filter: SubjectFilter,
    access: SubjectGroupAccess,
) -> Result<Vec<Subject>, AppError> {
    let mut query = String::from(
        r#"SELECT s.*, sg.name_th as group_name_th,
                  (SELECT COALESCE(array_agg(sgl.grade_level_id), '{}')
                   FROM subject_grade_levels sgl WHERE sgl.subject_id = s.id) as grade_level_ids,
                  concat(u.first_name, ' ', u.last_name) as default_instructor_name
           FROM subjects s
           LEFT JOIN subject_groups sg ON s.group_id = sg.id
           LEFT JOIN users u ON s.default_instructor_id = u.id
           WHERE 1=1"#,
    );

    let mut idx = 0u32;

    if let Some(active) = filter.active_only {
        if active {
            query.push_str(" AND s.is_active = true");
        }
    }

    let effective_group_ids = effective_subject_group_filter(filter.group_id, &access)?;
    if effective_group_ids.is_some() {
        idx += 1;
        query.push_str(&format!(" AND s.group_id = ANY(${idx})"));
    }
    if filter.subject_type.is_some() {
        idx += 1;
        query.push_str(&format!(" AND s.type = ${idx}"));
    }

    let search_pattern = subject_search_pattern(filter.search.clone());
    if search_pattern.is_some() {
        idx += 1;
        query.push_str(&format!(
            " AND (s.code ILIKE ${idx} OR s.name_th ILIKE ${idx} OR s.name_en ILIKE ${idx})"
        ));
    }

    let active_in_year_id: Option<Uuid> = filter.active_in_year_id;
    let latest_only = filter.latest_only.unwrap_or(true);

    if active_in_year_id.is_some() {
        idx += 1;
        query.push_str(&format!(
            r#" AND s.id IN (
                SELECT DISTINCT ON (sub.code) sub.id
                FROM subjects sub
                JOIN academic_years ay  ON ay.id  = sub.start_academic_year_id
                JOIN academic_years ayt ON ayt.id = ${idx}
                WHERE ay.year <= ayt.year
                ORDER BY sub.code, ay.year DESC
            )"#
        ));
    } else if latest_only {
        query.push_str(
            r#" AND s.id IN (
                SELECT DISTINCT ON (sub.code) sub.id
                FROM subjects sub
                JOIN academic_years ay ON ay.id = sub.start_academic_year_id
                ORDER BY sub.code, ay.year DESC
            )"#,
        );
    }

    if filter.term.is_some() {
        idx += 1;
        query.push_str(&format!(" AND (s.term = ${idx} OR s.term IS NULL)"));
    }

    query.push_str(" ORDER BY s.code ASC");

    let mut q = sqlx::query_as::<_, Subject>(&query);
    if let Some(group_ids) = effective_group_ids {
        q = q.bind(group_ids);
    }
    if let Some(ref stype) = filter.subject_type {
        q = q.bind(stype);
    }
    if let Some(ref pattern) = search_pattern {
        q = q.bind(pattern);
    }
    if let Some(year_id) = active_in_year_id {
        q = q.bind(year_id);
    }
    if let Some(ref term) = filter.term {
        q = q.bind(term);
    }

    q.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Failed to fetch subjects: {}", e);
        AppError::InternalServerError("Failed to fetch subjects".to_string())
    })
}

pub async fn create_subject(
    pool: &PgPool,
    payload: CreateSubjectRequest,
) -> Result<Subject, AppError> {
    let exists: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM subjects WHERE code = $1 AND start_academic_year_id = $2)",
    )
    .bind(&payload.code)
    .bind(payload.start_academic_year_id)
    .fetch_one(pool)
    .await
    .unwrap_or(Some(false));

    if exists.unwrap_or(false) {
        let year_name: Option<String> =
            sqlx::query_scalar("SELECT name FROM academic_years WHERE id = $1")
                .bind(payload.start_academic_year_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);
        return Err(AppError::BadRequest(format!(
            "รหัสวิชา {} {} มีอยู่ในระบบแล้ว",
            payload.code,
            year_name.unwrap_or_else(|| "ในปีการศึกษานี้".to_string())
        )));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut subject = sqlx::query_as::<_, Subject>(
        r#"INSERT INTO subjects (code, name_th, name_en, credit, hours_per_semester, type, group_id, description,
                                 start_academic_year_id, term, default_instructor_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
           RETURNING *"#
    )
    .bind(&payload.code).bind(&payload.name_th).bind(&payload.name_en)
    .bind(payload.credit.unwrap_or(0.0)).bind(payload.hours_per_semester)
    .bind(&payload.subject_type).bind(payload.group_id).bind(&payload.description)
    .bind(payload.start_academic_year_id).bind(&payload.term).bind(payload.default_instructor_id)
    .fetch_one(&mut *tx).await
    .map_err(|e| {
        tracing::error!("Failed to create subject: {}", e);
        AppError::InternalServerError("Failed to create subject".to_string())
    })?;

    if let Some(level_ids) = &payload.grade_level_ids {
        bulk_insert_subject_grade_levels(&mut tx, subject.id, level_ids).await?;
        subject.grade_level_ids = Some(level_ids.clone());
    }

    if let Some(team) = &payload.default_instructors {
        sqlx::query("DELETE FROM subject_default_instructors WHERE subject_id = $1")
            .bind(subject.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to clear team: {}", e)))?;
        bulk_upsert_subject_default_instructors(&mut tx, subject.id, team).await?;
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;
    Ok(subject)
}

pub async fn update_subject(
    pool: &PgPool,
    id: Uuid,
    payload: UpdateSubjectRequest,
) -> Result<Subject, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    let mut subject = sqlx::query_as::<_, Subject>(
        r#"UPDATE subjects SET
            code = COALESCE($1, code), name_th = COALESCE($2, name_th), name_en = COALESCE($3, name_en),
            credit = COALESCE($4, credit), hours_per_semester = COALESCE($5, hours_per_semester),
            type = COALESCE($6, type), group_id = COALESCE($7, group_id),
            description = COALESCE($8, description), is_active = COALESCE($9, is_active),
            start_academic_year_id = COALESCE($10, start_academic_year_id),
            term = $12, default_instructor_id = $13, updated_at = NOW()
           WHERE id = $11 RETURNING *"#
    )
    .bind(&payload.code).bind(&payload.name_th).bind(&payload.name_en)
    .bind(payload.credit).bind(payload.hours_per_semester)
    .bind(&payload.subject_type).bind(payload.group_id).bind(&payload.description)
    .bind(payload.is_active).bind(payload.start_academic_year_id).bind(id)
    .bind(&payload.term).bind(payload.default_instructor_id)
    .fetch_one(&mut *tx).await
    .map_err(|e| {
        tracing::error!("Failed to update subject {}: {}", id, e);
        AppError::InternalServerError("Failed to update subject".to_string())
    })?;

    if let Some(level_ids) = &payload.grade_level_ids {
        sqlx::query("DELETE FROM subject_grade_levels WHERE subject_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to clear links: {}", e)))?;
        bulk_insert_subject_grade_levels(&mut tx, id, level_ids).await?;
        subject.grade_level_ids = Some(level_ids.clone());
    }

    if let Some(team) = &payload.default_instructors {
        sqlx::query("DELETE FROM subject_default_instructors WHERE subject_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to clear team: {}", e)))?;
        bulk_upsert_subject_default_instructors(&mut tx, id, team).await?;
    }

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;
    Ok(subject)
}

pub async fn delete_subject(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM subjects WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete subject {}: {}", id, e);
            AppError::BadRequest("ไม่สามารถลบรายวิชาได้ (อาจมีการใช้งานอยู่)".to_string())
        })?;
    Ok(())
}

pub async fn get_subject_group_id(pool: &PgPool, id: Uuid) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar("SELECT group_id FROM subjects WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to fetch subject".to_string()))
        .map(|opt| opt.flatten())
}

pub async fn list_subject_default_instructors(
    pool: &PgPool,
    subject_id: Uuid,
) -> Result<Vec<SubjectDefaultInstructor>, AppError> {
    sqlx::query_as::<_, SubjectDefaultInstructor>(
        r#"SELECT sdi.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM subject_default_instructors sdi
           JOIN users u ON u.id = sdi.instructor_id
           WHERE sdi.subject_id = $1
           ORDER BY sdi.role, sdi.created_at"#,
    )
    .bind(subject_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn add_subject_default_instructor(
    pool: &PgPool,
    subject_id: Uuid,
    body: AddSubjectDefaultInstructorRequest,
) -> Result<(), AppError> {
    let role = subject_instructor_role_or_default(body.role);
    validate_subject_instructor_role(&role)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    if role == "primary" {
        sqlx::query(
            "UPDATE subject_default_instructors SET role = 'secondary'
             WHERE subject_id = $1 AND instructor_id <> $2 AND role = 'primary'",
        )
        .bind(subject_id)
        .bind(body.instructor_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    sqlx::query(
        "INSERT INTO subject_default_instructors (subject_id, instructor_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (subject_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(subject_id)
    .bind(body.instructor_id)
    .bind(&role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;
    Ok(())
}

pub async fn remove_subject_default_instructor(
    pool: &PgPool,
    subject_id: Uuid,
    instructor_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "DELETE FROM subject_default_instructors WHERE subject_id = $1 AND instructor_id = $2",
    )
    .bind(subject_id)
    .bind(instructor_id)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    Ok(())
}

pub async fn update_subject_default_instructor_role(
    pool: &PgPool,
    subject_id: Uuid,
    instructor_id: Uuid,
    role: &str,
) -> Result<(), AppError> {
    validate_subject_instructor_role(role)?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::InternalServerError("Transaction failed".to_string()))?;

    if role == "primary" {
        sqlx::query(
            "UPDATE subject_default_instructors SET role = 'secondary'
             WHERE subject_id = $1 AND instructor_id <> $2 AND role = 'primary'",
        )
        .bind(subject_id)
        .bind(instructor_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;
    }

    sqlx::query(
        "UPDATE subject_default_instructors SET role = $3
         WHERE subject_id = $1 AND instructor_id = $2",
    )
    .bind(subject_id)
    .bind(instructor_id)
    .bind(role)
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|_| AppError::InternalServerError("Commit failed".to_string()))?;
    Ok(())
}

pub async fn batch_list_subject_default_instructors(
    pool: &PgPool,
    subject_ids: Vec<Uuid>,
    access: &SubjectGroupAccess,
) -> Result<std::collections::HashMap<Uuid, Vec<SubjectDefaultInstructor>>, AppError> {
    if subject_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    if matches!(access, SubjectGroupAccess::Groups(group_ids) if group_ids.is_empty()) {
        return Ok(std::collections::HashMap::new());
    }

    let mut query = String::from(
        r#"SELECT sdi.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM subject_default_instructors sdi
           JOIN subjects s ON s.id = sdi.subject_id
           JOIN users u ON u.id = sdi.instructor_id
           WHERE sdi.subject_id = ANY($1)"#,
    );

    if matches!(access, SubjectGroupAccess::Groups(_)) {
        query.push_str(" AND s.group_id = ANY($2)");
    }

    query.push_str(" ORDER BY sdi.subject_id, sdi.role, sdi.created_at");

    let mut rows_query = sqlx::query_as::<_, SubjectDefaultInstructor>(&query).bind(&subject_ids);
    if let SubjectGroupAccess::Groups(group_ids) = access {
        rows_query = rows_query.bind(group_ids);
    }

    let rows: Vec<SubjectDefaultInstructor> = rows_query
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::InternalServerError(e.to_string()))?;

    let mut grouped: std::collections::HashMap<Uuid, Vec<SubjectDefaultInstructor>> =
        std::collections::HashMap::new();
    for row in rows {
        grouped.entry(row.subject_id).or_default().push(row);
    }
    Ok(grouped)
}

fn effective_subject_group_filter(
    requested_group_id: Option<Uuid>,
    access: &SubjectGroupAccess,
) -> Result<Option<Vec<Uuid>>, AppError> {
    Ok(match access {
        SubjectGroupAccess::All => requested_group_id.map(|group_id| vec![group_id]),
        SubjectGroupAccess::Groups(group_ids) => match requested_group_id {
            Some(group_id) if group_ids.contains(&group_id) => Some(vec![group_id]),
            Some(_) => Some(Vec::new()),
            None => Some(group_ids.clone()),
        },
    })
}

pub fn subject_group_access_allows(
    access: &SubjectGroupAccess,
    target_group_id: Option<Uuid>,
) -> bool {
    match access {
        SubjectGroupAccess::All => true,
        SubjectGroupAccess::Groups(group_ids) => target_group_id
            .map(|group_id| group_ids.contains(&group_id))
            .unwrap_or(false),
    }
}

fn subject_search_pattern(search: Option<String>) -> Option<String> {
    search
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"))
}

fn subject_instructor_role_or_default(role: Option<String>) -> String {
    role.unwrap_or_else(|| "secondary".to_string())
}

fn validate_subject_instructor_role(role: &str) -> Result<(), AppError> {
    if role == "primary" || role == "secondary" {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "role must be 'primary' or 'secondary'".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_subject_group_filter_limits_requested_group_to_scope() {
        let allowed_group_a = Uuid::new_v4();
        let allowed_group_b = Uuid::new_v4();
        let requested_group_id = Uuid::new_v4();

        assert_eq!(
            effective_subject_group_filter(
                Some(allowed_group_a),
                &SubjectGroupAccess::Groups(vec![allowed_group_a, allowed_group_b])
            )
            .expect("filter should be valid"),
            Some(vec![allowed_group_a])
        );
        assert_eq!(
            effective_subject_group_filter(
                None,
                &SubjectGroupAccess::Groups(vec![allowed_group_a, allowed_group_b])
            )
            .expect("filter should include allowed groups"),
            Some(vec![allowed_group_a, allowed_group_b])
        );
        assert_eq!(
            effective_subject_group_filter(
                Some(requested_group_id),
                &SubjectGroupAccess::Groups(vec![allowed_group_a, allowed_group_b])
            )
            .expect("out-of-scope filter should produce empty result"),
            Some(Vec::new())
        );
    }

    #[test]
    fn effective_subject_group_filter_preserves_empty_limited_scope() {
        let requested_group_id = Uuid::new_v4();

        assert_eq!(
            effective_subject_group_filter(None, &SubjectGroupAccess::Groups(Vec::new()))
                .expect("empty limited scope should stay empty"),
            Some(Vec::new())
        );
        assert_eq!(
            effective_subject_group_filter(
                Some(requested_group_id),
                &SubjectGroupAccess::Groups(Vec::new())
            )
            .expect("requested group outside empty scope should stay empty"),
            Some(Vec::new())
        );
    }

    #[test]
    fn subject_group_access_requires_target_group_for_limited_access() {
        let allowed_group_id = Uuid::new_v4();
        let other_group_id = Uuid::new_v4();

        assert!(subject_group_access_allows(
            &SubjectGroupAccess::All,
            Some(other_group_id)
        ));
        assert!(subject_group_access_allows(
            &SubjectGroupAccess::Groups(vec![allowed_group_id]),
            Some(allowed_group_id)
        ));
        assert!(!subject_group_access_allows(
            &SubjectGroupAccess::Groups(vec![allowed_group_id]),
            Some(other_group_id)
        ));
        assert!(!subject_group_access_allows(
            &SubjectGroupAccess::Groups(vec![allowed_group_id]),
            None
        ));
    }

    #[test]
    fn effective_subject_group_filter_keeps_all_scope_unrestricted() {
        let requested_group_id = Uuid::new_v4();

        assert_eq!(
            effective_subject_group_filter(None, &SubjectGroupAccess::All)
                .expect("all scope without a requested group is unrestricted"),
            None
        );
        assert_eq!(
            effective_subject_group_filter(Some(requested_group_id), &SubjectGroupAccess::All)
                .expect("all scope should allow requested group filter"),
            Some(vec![requested_group_id])
        );
    }

    #[test]
    fn subject_search_pattern_ignores_empty_values() {
        assert_eq!(subject_search_pattern(None), None);
        assert_eq!(subject_search_pattern(Some("".to_string())), None);
        assert_eq!(
            subject_search_pattern(Some("math".to_string())),
            Some("%math%".to_string())
        );
        assert_eq!(
            subject_search_pattern(Some(" คณิต ".to_string())),
            Some("% คณิต %".to_string())
        );
    }

    #[test]
    fn subject_instructor_role_defaults_to_secondary() {
        assert_eq!(subject_instructor_role_or_default(None), "secondary");
        assert_eq!(
            subject_instructor_role_or_default(Some("primary".to_string())),
            "primary"
        );
    }

    #[test]
    fn ordered_unique_subject_grade_level_ids_preserves_first_seen_order() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        assert_eq!(
            ordered_unique_subject_grade_level_ids(&[first, second, first]),
            vec![first, second]
        );
    }

    #[test]
    fn unique_subject_default_instructors_keeps_latest_role_per_instructor() {
        let instructor_id = Uuid::new_v4();

        let rows = unique_subject_default_instructors(&[
            DefaultInstructorInput {
                instructor_id,
                role: "secondary".to_string(),
            },
            DefaultInstructorInput {
                instructor_id,
                role: "primary".to_string(),
            },
        ])
        .expect("roles should be valid");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].instructor_id, instructor_id);
        assert_eq!(rows[0].role, "primary");
    }

    #[test]
    fn validate_subject_instructor_role_accepts_primary_and_secondary_only() {
        assert!(validate_subject_instructor_role("primary").is_ok());
        assert!(validate_subject_instructor_role("secondary").is_ok());
        assert!(matches!(
            validate_subject_instructor_role("assistant"),
            Err(AppError::BadRequest(message)) if message == "role must be 'primary' or 'secondary'"
        ));
    }
}
