use crate::personnel::StaffEducationInput;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

/// The parent staff operation holds the user row lock; the collection has its own revision.
pub async fn replace_educations(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    expected_revision: i64,
    items: &[StaffEducationInput],
    creating: bool,
) -> Result<(), AppError> {
    let revision: i64 =
        sqlx::query_scalar("SELECT education_revision FROM staff_info WHERE user_id=$1 FOR UPDATE")
            .bind(staff_id)
            .fetch_one(&mut **tx)
            .await?;
    if revision != expected_revision {
        return Err(AppError::Conflict(
            "รายการวุฒิเปลี่ยนแปลงแล้ว กรุณาโหลดข้อมูลล่าสุดก่อนบันทึกอีกครั้ง".into(),
        ));
    }
    let existing: Vec<(Uuid, Option<String>)> =
        sqlx::query_as("SELECT id,education_level FROM staff_educations WHERE staff_id=$1")
            .bind(staff_id)
            .fetch_all(&mut **tx)
            .await?;
    if items.len() > 100 {
        return Err(AppError::BadRequest("ระบุวุฒิได้ไม่เกิน 100 รายการ".into()));
    }
    let mut kept = HashSet::new();
    let (mut ids, mut levels, mut majors, mut universities) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for item in items {
        let id = match item.id {
            Some(id) if !creating && existing.iter().any(|row| row.0 == id) && kept.insert(id) => {
                id
            }
            Some(_) => {
                return Err(AppError::BadRequest(
                    "รายการวุฒิไม่อยู่ในบุคลากรนี้หรือมีรายการซ้ำ".into(),
                ))
            }
            None => {
                if item.education_level.is_none() {
                    return Err(AppError::BadRequest("กรุณาระบุระดับวุฒิสำหรับรายการใหม่".into()));
                }
                let id = Uuid::now_v7();
                kept.insert(id);
                id
            }
        };
        if item.education_level.is_none()
            && existing.iter().any(|row| row.0 == id && row.1.is_some())
        {
            return Err(AppError::BadRequest("กรุณาระบุระดับวุฒิ".into()));
        }
        let major = item
            .major
            .as_deref()
            .map(super::staff_info_service::normalize_education_text)
            .transpose()?
            .flatten();
        let university = item
            .university
            .as_deref()
            .map(super::staff_info_service::normalize_education_text)
            .transpose()?
            .flatten();
        if item.education_level.is_none() && major.is_none() && university.is_none() {
            return Err(AppError::BadRequest(
                "รายการวุฒิต้องมีข้อมูล กรุณาลบรายการที่ไม่ใช้".into(),
            ));
        }
        ids.push(id);
        levels.push(item.education_level.map(|level| level.as_str().to_owned()));
        majors.push(major);
        universities.push(university);
    }
    sqlx::query("INSERT INTO staff_educations(id,staff_id,education_level,major,university) SELECT id,$1,level,major,university FROM unnest($2::uuid[],$3::text[],$4::text[],$5::text[]) AS input(id,level,major,university) ON CONFLICT(id) DO UPDATE SET education_level=EXCLUDED.education_level,major=EXCLUDED.major,university=EXCLUDED.university,updated_at=now() WHERE staff_educations.staff_id=EXCLUDED.staff_id")
        .bind(staff_id).bind(ids).bind(levels).bind(majors).bind(universities).execute(&mut **tx).await?;
    let kept = kept.into_iter().collect::<Vec<_>>();
    sqlx::query("DELETE FROM staff_educations WHERE staff_id=$1 AND NOT(id=ANY($2))")
        .bind(staff_id)
        .bind(kept)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE staff_info SET education_revision=education_revision+1,updated_at=now() WHERE user_id=$1")
        .bind(staff_id).execute(&mut **tx).await?;
    Ok(())
}
