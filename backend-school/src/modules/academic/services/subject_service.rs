use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::models::curriculum::{
    AddSubjectDefaultInstructorRequest, CreateSubjectRequest, Subject, SubjectDefaultInstructor,
    SubjectFilter, SubjectGroup, UpdateSubjectRequest,
};
use crate::permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_user_subject_group_id(user_id: Uuid, pool: &PgPool) -> Option<Uuid> {
    sqlx::query_scalar(
        r#"SELECT d.subject_group_id
           FROM department_members dm
           JOIN departments d ON d.id = dm.department_id
           WHERE dm.user_id = $1
             AND d.subject_group_id IS NOT NULL
             AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
           ORDER BY dm.is_primary_department DESC NULLS LAST
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub async fn ensure_subject_manage(
    actor: &ActorContext,
    pool: &PgPool,
    subject_id: Uuid,
    manage_code: &str,
    read_only: bool,
) -> Result<(), AppError> {
    let has_all = actor.has_permission(manage_code)
        || (read_only
            && actor.has_any_permission(&[
                codes::ACADEMIC_CURRICULUM_READ_ALL,
                codes::ACADEMIC_CURRICULUM_MANAGE_DEPT,
                manage_code,
            ]));
    let has_dept = actor.has_permission(codes::ACADEMIC_CURRICULUM_MANAGE_DEPT);
    if !has_all && !has_dept {
        return Err(AppError::Forbidden(format!("ไม่มีสิทธิ์ {}", manage_code)));
    }

    if !has_all && has_dept {
        let teacher_group = match get_user_subject_group_id(actor.user_id, pool).await {
            Some(group_id) => group_id,
            None => return Err(AppError::Forbidden("ไม่พบกลุ่มสาระที่สังกัด".to_string())),
        };
        let subject_group = get_subject_group_id(pool, subject_id).await.ok().flatten();
        if subject_group != Some(teacher_group) {
            return Err(AppError::Forbidden(
                "ไม่สามารถจัดการวิชาในกลุ่มสาระอื่นได้".to_string(),
            ));
        }
    }

    Ok(())
}

pub async fn list_subject_groups(pool: &PgPool) -> Result<Vec<SubjectGroup>, AppError> {
    sqlx::query_as::<_, SubjectGroup>(
        "SELECT * FROM subject_groups WHERE is_active = true ORDER BY display_order ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to fetch subject groups: {}", e);
        AppError::InternalServerError("Failed to fetch subject groups".to_string())
    })
}

pub async fn list_subjects(
    pool: &PgPool,
    filter: SubjectFilter,
    dept_group_id: Option<Uuid>,
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

    let effective_group_id: Option<Uuid> = if dept_group_id.is_some() {
        dept_group_id
    } else {
        filter.group_id
    };
    if effective_group_id.is_some() {
        idx += 1;
        query.push_str(&format!(" AND s.group_id = ${idx}"));
    }
    if filter.subject_type.is_some() {
        idx += 1;
        query.push_str(&format!(" AND s.type = ${idx}"));
    }

    let search_pattern = filter.search.as_ref().and_then(|s| {
        if s.is_empty() {
            None
        } else {
            Some(format!("%{s}%"))
        }
    });
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
    if let Some(gid) = effective_group_id {
        q = q.bind(gid);
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
        eprintln!("Failed to fetch subjects: {}", e);
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
        eprintln!("Failed to create subject: {}", e);
        AppError::InternalServerError("Failed to create subject".to_string())
    })?;

    if let Some(level_ids) = &payload.grade_level_ids {
        for lid in level_ids {
            sqlx::query(
                "INSERT INTO subject_grade_levels (subject_id, grade_level_id) VALUES ($1, $2)",
            )
            .bind(subject.id)
            .bind(lid)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                eprintln!("Failed to link grade level: {}", e);
                AppError::InternalServerError("Failed to save grade level links".to_string())
            })?;
        }
        subject.grade_level_ids = Some(level_ids.clone());
    }

    if let Some(team) = &payload.default_instructors {
        sqlx::query("DELETE FROM subject_default_instructors WHERE subject_id = $1")
            .bind(subject.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to clear team: {}", e)))?;
        for t in team {
            if t.role != "primary" && t.role != "secondary" {
                return Err(AppError::BadRequest(
                    "role must be 'primary' or 'secondary'".to_string(),
                ));
            }
            sqlx::query(
                "INSERT INTO subject_default_instructors (subject_id, instructor_id, role)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (subject_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
            )
            .bind(subject.id)
            .bind(t.instructor_id)
            .bind(&t.role)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to save team: {}", e)))?;
        }
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
        eprintln!("Failed to update subject {}: {}", id, e);
        AppError::InternalServerError("Failed to update subject".to_string())
    })?;

    if let Some(level_ids) = &payload.grade_level_ids {
        sqlx::query("DELETE FROM subject_grade_levels WHERE subject_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to clear links: {}", e)))?;
        for lid in level_ids {
            sqlx::query(
                "INSERT INTO subject_grade_levels (subject_id, grade_level_id) VALUES ($1, $2)",
            )
            .bind(id)
            .bind(lid)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::InternalServerError(format!("Failed to link grade level: {}", e))
            })?;
        }
        subject.grade_level_ids = Some(level_ids.clone());
    }

    if let Some(team) = &payload.default_instructors {
        sqlx::query("DELETE FROM subject_default_instructors WHERE subject_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to clear team: {}", e)))?;
        for t in team {
            if t.role != "primary" && t.role != "secondary" {
                return Err(AppError::BadRequest(
                    "role must be 'primary' or 'secondary'".to_string(),
                ));
            }
            sqlx::query(
                "INSERT INTO subject_default_instructors (subject_id, instructor_id, role)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (subject_id, instructor_id) DO UPDATE SET role = EXCLUDED.role",
            )
            .bind(id)
            .bind(t.instructor_id)
            .bind(&t.role)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to save team: {}", e)))?;
        }
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
            eprintln!("Failed to delete subject {}: {}", id, e);
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
    let role = body.role.unwrap_or_else(|| "secondary".to_string());
    if role != "primary" && role != "secondary" {
        return Err(AppError::BadRequest(
            "role must be 'primary' or 'secondary'".to_string(),
        ));
    }

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
    if role != "primary" && role != "secondary" {
        return Err(AppError::BadRequest(
            "role must be 'primary' or 'secondary'".to_string(),
        ));
    }

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
) -> Result<std::collections::HashMap<Uuid, Vec<SubjectDefaultInstructor>>, AppError> {
    if subject_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows: Vec<SubjectDefaultInstructor> = sqlx::query_as(
        r#"SELECT sdi.*, concat(u.first_name, ' ', u.last_name) AS instructor_name
           FROM subject_default_instructors sdi
           JOIN users u ON u.id = sdi.instructor_id
           WHERE sdi.subject_id = ANY($1)
           ORDER BY sdi.subject_id, sdi.role, sdi.created_at"#,
    )
    .bind(&subject_ids)
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
