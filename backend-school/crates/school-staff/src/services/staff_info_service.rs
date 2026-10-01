use crate::personnel::{
    CreateStaffInfoRequest, StaffInfoResponse, StaffReferenceKind, UpdateStaffInfoRequest,
};
use school_errors::AppError;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn read_staff_info(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<StaffInfoResponse>, AppError> {
    let row: Option<Json<StaffInfoResponse>> = sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
          'job_position', CASE WHEN position.id IS NOT NULL THEN jsonb_build_object('id', position.id, 'code', position.code, 'name', position.name, 'isActive', position.is_active) END,
          'academic_rank', info.academic_rank, 'education_level', info.education_level,
          'major', CASE WHEN major.id IS NOT NULL THEN jsonb_build_object('id', major.id, 'code', major.code, 'name', major.name, 'isActive', major.is_active) END,
          'university', CASE WHEN university.id IS NOT NULL THEN jsonb_build_object('id', university.id, 'code', university.code, 'name', university.name, 'isActive', university.is_active) END
        ) FROM staff_info info
        LEFT JOIN staff_reference_items position ON position.id = info.job_position_id
        LEFT JOIN staff_reference_items major ON major.id = info.major_id
        LEFT JOIN staff_reference_items university ON university.id = info.university_id
        WHERE info.user_id = $1"#,
    ).bind(user_id).fetch_optional(pool).await?;
    Ok(row.map(|Json(info)| info))
}

async fn validate_references(
    tx: &mut Transaction<'_, Postgres>,
    selections: &[(Uuid, StaffReferenceKind, Option<Uuid>)],
) -> Result<(), AppError> {
    if selections.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = selections.iter().map(|(id, _, _)| *id).collect();
    let rows: Vec<(Uuid, String, bool)> = sqlx::query_as(
        "SELECT id, kind, is_active FROM staff_reference_items WHERE id = ANY($1) ORDER BY id FOR SHARE",
    ).bind(ids).fetch_all(&mut **tx).await?;
    for (id, kind, existing_id) in selections {
        let valid = rows.iter().any(|(row_id, row_kind, active)| {
            row_id == id && row_kind == kind.as_str() && (*active || *existing_id == Some(*id))
        });
        if !valid {
            return Err(AppError::BadRequest(format!(
                "{}ที่เลือกไม่พร้อมใช้งาน กรุณาเลือกใหม่",
                kind.label()
            )));
        }
    }
    Ok(())
}

pub async fn create_staff_info(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    input: &CreateStaffInfoRequest,
) -> Result<(), AppError> {
    let selections: Vec<_> = [
        (input.job_position_id, StaffReferenceKind::JobPosition),
        (input.major_id, StaffReferenceKind::Major),
        (input.university_id, StaffReferenceKind::University),
    ]
    .into_iter()
    .filter_map(|(id, kind)| id.map(|id| (id, kind, None)))
    .collect();
    validate_references(tx, &selections).await?;
    sqlx::query("INSERT INTO staff_info (user_id, job_position_id, academic_rank, education_level, major_id, university_id, teaching_license_number, teaching_license_expiry) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(user_id).bind(input.job_position_id).bind(input.academic_rank.map(|v| v.as_str()))
        .bind(input.education_level.map(|v| v.as_str())).bind(input.major_id).bind(input.university_id)
        .bind(&input.teaching_license_number).bind(input.teaching_license_expiry).execute(&mut **tx).await?;
    Ok(())
}

/// The parent staff update transaction already holds the staff-user row lock.
pub async fn patch_staff_info(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    patch: &UpdateStaffInfoRequest,
) -> Result<(), AppError> {
    if patch.is_empty() {
        return Ok(());
    }
    let prior: Option<(Option<Uuid>, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT job_position_id, major_id, university_id FROM staff_info WHERE user_id=$1 FOR UPDATE",
    ).bind(user_id).fetch_optional(&mut **tx).await?;
    let (position, major, university) = prior.unwrap_or((None, None, None));
    let selections: Vec<_> = [
        (
            patch.job_position_id.flatten(),
            StaffReferenceKind::JobPosition,
            position,
        ),
        (patch.major_id.flatten(), StaffReferenceKind::Major, major),
        (
            patch.university_id.flatten(),
            StaffReferenceKind::University,
            university,
        ),
    ]
    .into_iter()
    .filter_map(|(id, kind, prior)| id.map(|id| (id, kind, prior)))
    .collect();
    validate_references(tx, &selections).await?;
    sqlx::query("INSERT INTO staff_info (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"UPDATE staff_info SET
      job_position_id = CASE WHEN $2 THEN $3 ELSE job_position_id END,
      academic_rank = CASE WHEN $4 THEN $5 ELSE academic_rank END,
      education_level = CASE WHEN $6 THEN $7 ELSE education_level END,
      major_id = CASE WHEN $8 THEN $9 ELSE major_id END,
      university_id = CASE WHEN $10 THEN $11 ELSE university_id END,
      teaching_license_number = CASE WHEN $12 THEN $13 ELSE teaching_license_number END,
      teaching_license_expiry = CASE WHEN $14 THEN $15 ELSE teaching_license_expiry END,
      updated_at=NOW() WHERE user_id=$1"#,
    )
    .bind(user_id)
    .bind(patch.job_position_id.is_some())
    .bind(patch.job_position_id.flatten())
    .bind(patch.academic_rank.is_some())
    .bind(patch.academic_rank.flatten().map(|v| v.as_str()))
    .bind(patch.education_level.is_some())
    .bind(patch.education_level.flatten().map(|v| v.as_str()))
    .bind(patch.major_id.is_some())
    .bind(patch.major_id.flatten())
    .bind(patch.university_id.is_some())
    .bind(patch.university_id.flatten())
    .bind(patch.teaching_license_number.is_some())
    .bind(
        patch
            .teaching_license_number
            .as_ref()
            .and_then(|v| v.as_deref()),
    )
    .bind(patch.teaching_license_expiry.is_some())
    .bind(patch.teaching_license_expiry.flatten())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn personnel_empty_patch_preserves_absent_info_row() {
        let pool = school_test_db::create_named_test_pool("personnel_empty_patch").await;
        school_test_db::run_test_migrations(&pool).await;
        let user = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, username, password_hash, first_name, last_name, user_type, status) VALUES ($1, $2, 'synthetic-hash', 'Fixture', 'Person', 'staff', 'active')")
            .bind(user).bind(user.to_string()).execute(&pool).await.unwrap();
        let patch = UpdateStaffInfoRequest::default();
        let mut tx = pool.begin().await.unwrap();
        patch_staff_info(&mut tx, user, &patch).await.unwrap();
        tx.commit().await.unwrap();
        assert!(read_staff_info(&pool, user).await.unwrap().is_none());
    }
}
