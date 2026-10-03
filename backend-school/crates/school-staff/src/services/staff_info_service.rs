use crate::personnel::{CreateStaffInfoRequest, StaffInfoResponse, UpdateStaffInfoRequest};
use school_errors::AppError;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn read_staff_info(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<StaffInfoResponse>, AppError> {
    let row: Option<Json<StaffInfoResponse>> = sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
          'job_position', CASE WHEN position.id IS NOT NULL THEN jsonb_build_object('id', position.id, 'code', position.code, 'name', position.name, 'isActive', position.is_active, 'isSelectable', position.is_selectable) END,
          'academic_rank', info.academic_rank, 'education_level', info.education_level,
          'major', info.major, 'university', info.university
        ) FROM staff_info info
        LEFT JOIN staff_job_positions position ON position.id = info.job_position_id
        WHERE info.user_id = $1"#,
    ).bind(user_id).fetch_optional(pool).await?;
    Ok(row.map(|Json(info)| info))
}

pub fn normalize_education_text(value: &str) -> Result<Option<String>, AppError> {
    if value.chars().any(char::is_control) {
        return Err(AppError::BadRequest("สาขาและสถาบันต้องไม่มีอักขระควบคุม".into()));
    }
    let value = value.trim();
    if value.chars().count() > 200 {
        return Err(AppError::BadRequest(
            "สาขาและสถาบันต้องยาวไม่เกิน 200 ตัวอักษร".into(),
        ));
    }
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

async fn validate_position(
    tx: &mut Transaction<'_, Postgres>,
    selected: Option<Uuid>,
    existing: Option<Uuid>,
) -> Result<(), AppError> {
    if let Some(id) = selected {
        let row: Option<(bool, bool)> = sqlx::query_as(
            "SELECT is_active,is_selectable FROM staff_job_positions WHERE id=$1 FOR SHARE",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
        if !row.is_some_and(|(active, selectable)| (active && selectable) || existing == Some(id)) {
            return Err(AppError::BadRequest(
                "ตำแหน่งที่เลือกไม่พร้อมใช้งาน กรุณาเลือกใหม่".into(),
            ));
        }
    }
    Ok(())
}

pub async fn create_staff_info(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    input: &CreateStaffInfoRequest,
) -> Result<(), AppError> {
    let major = input
        .major
        .as_deref()
        .map(normalize_education_text)
        .transpose()?
        .flatten();
    let university = input
        .university
        .as_deref()
        .map(normalize_education_text)
        .transpose()?
        .flatten();
    validate_position(tx, input.job_position_id, None).await?;
    sqlx::query("INSERT INTO staff_info (user_id, job_position_id, academic_rank, education_level, major, university, teaching_license_number, teaching_license_expiry) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(user_id).bind(input.job_position_id).bind(input.academic_rank.map(|v| v.as_str()))
        .bind(input.education_level.map(|v| v.as_str())).bind(major).bind(university)
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
    let major = patch
        .major
        .as_ref()
        .and_then(|v| v.as_deref())
        .map(normalize_education_text)
        .transpose()?
        .flatten();
    let university = patch
        .university
        .as_ref()
        .and_then(|v| v.as_deref())
        .map(normalize_education_text)
        .transpose()?
        .flatten();
    let prior: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT job_position_id FROM staff_info WHERE user_id=$1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut **tx)
            .await?;
    validate_position(
        tx,
        patch.job_position_id.flatten(),
        prior.and_then(|(id,)| id),
    )
    .await?;
    sqlx::query("INSERT INTO staff_info (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"UPDATE staff_info SET
      job_position_id = CASE WHEN $2 THEN $3 ELSE job_position_id END,
      academic_rank = CASE WHEN $4 THEN $5 ELSE academic_rank END,
      education_level = CASE WHEN $6 THEN $7 ELSE education_level END,
      major = CASE WHEN $8 THEN $9 ELSE major END,
      university = CASE WHEN $10 THEN $11 ELSE university END,
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
    .bind(patch.major.is_some())
    .bind(major)
    .bind(patch.university.is_some())
    .bind(university)
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

    #[test]
    fn personnel_education_text_preserves_spelling_and_validates_scalars() {
        assert_eq!(
            normalize_education_text("  คณิตศาสตร์  ประยุกต์  ").unwrap(),
            Some("คณิตศาสตร์  ประยุกต์".into())
        );
        assert_eq!(normalize_education_text("   ").unwrap(), None);
        for valid in ["ก".repeat(200), "😀".repeat(200)] {
            assert_eq!(normalize_education_text(&valid).unwrap(), Some(valid));
        }
        for invalid in [
            "ก".repeat(201),
            "😀".repeat(201),
            "คณิตศาสตร์\n".into(),
            "\tA".into(),
            "A\u{0085}B".into(),
        ] {
            assert!(normalize_education_text(&invalid).is_err());
        }
    }

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
