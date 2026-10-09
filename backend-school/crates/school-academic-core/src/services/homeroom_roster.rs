use super::{lifecycle_guard, parse_row_version, student_years, years_terms::append_audit};
use crate::models::*;
use school_errors::AppError;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
const MAX_ROSTER: usize = 500;

async fn read_students(
    tx: &mut Transaction<'_, Postgres>,
    room: Uuid,
    method: &str,
) -> Result<Vec<HomeroomRosterStudent>, AppError> {
    let rows = sqlx::query_as::<_, HomeroomRosterStudent>(r#"
        SELECT p.id AS placement_id,p.student_academic_year_id,y.student_id,i.student_id AS student_code,
               u.title,u.first_name,u.last_name,u.gender,p.class_number,p.status,p.start_date,p.row_version,
               y.row_version AS student_year_row_version
        FROM homeroom_placements p JOIN student_academic_years y ON y.id=p.student_academic_year_id
        JOIN users u ON u.id=y.student_id LEFT JOIN student_info i ON i.user_id=u.id
        WHERE p.homeroom_id=$1 AND p.status IN ('planned','current')
        ORDER BY CASE WHEN $2='gender_name' THEN CASE u.gender WHEN 'male' THEN 0 WHEN 'female' THEN 1 ELSE 2 END END,
          CASE WHEN $2 IN ('name','gender_name') THEN u.first_name END COLLATE schoolorbit_thai,
          CASE WHEN $2 IN ('name','gender_name') THEN u.last_name END COLLATE schoolorbit_thai,
          CASE WHEN $2='roster' THEN p.class_number END NULLS LAST,
          i.student_id NULLS LAST,u.first_name COLLATE schoolorbit_thai,u.last_name COLLATE schoolorbit_thai,p.id
        LIMIT 501"#).bind(room).bind(method).fetch_all(&mut **tx).await?;
    if rows.len() > MAX_ROSTER {
        return Err(AppError::ValidationError(
            "ห้องมีรายการจัดห้องเกิน 500 รายการ".into(),
        ));
    }
    Ok(rows)
}
fn roster_revision(
    room: &Homeroom,
    students: &[HomeroomRosterStudent],
) -> Result<String, AppError> {
    let mut stable = students.to_vec();
    stable.sort_by_key(|s| s.placement_id);
    let bytes = serde_json::to_vec(&(room.id, room.row_version, stable))
        .map_err(|_| AppError::InternalServerError("สร้างเวอร์ชันรายชื่อไม่สำเร็จ".into()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<HomeroomRoster, AppError> {
    let room=sqlx::query_as::<_,Homeroom>(r#"SELECT id,code,name,academic_year_id,grade_level_id,room_number,
        study_program_id,capacity,is_active,row_version,migration_provenance <> '{}'::jsonb AS migrated,created_at,updated_at
        FROM homerooms WHERE id=$1"#).bind(id).fetch_optional(&mut **tx).await?
        .ok_or_else(|| AppError::NotFound("ไม่พบห้องประจำชั้น".into()))?;
    let year_status = sqlx::query_scalar("SELECT status FROM academic_years WHERE id=$1")
        .bind(room.academic_year_id)
        .fetch_one(&mut **tx)
        .await?;
    let students = read_students(tx, id, "roster").await?;
    let revision = roster_revision(&room, &students)?;
    Ok(HomeroomRoster {
        homeroom: room,
        year_status,
        students,
        revision,
    })
}
async fn read_transaction(pool: &PgPool) -> Result<Transaction<'_, Postgres>, AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
pub async fn get_roster(pool: &PgPool, id: Uuid) -> Result<HomeroomRoster, AppError> {
    let mut tx = read_transaction(pool).await?;
    let result = snapshot(&mut tx, id).await?;
    tx.commit().await?;
    Ok(result)
}
async fn write_roster<'a>(
    pool: &'a PgPool,
    id: Uuid,
    revision: &str,
) -> Result<(Transaction<'a, Postgres>, HomeroomRoster), AppError> {
    let room = student_years::get_homeroom(pool, id).await?;
    let mut tx = pool.begin().await?;
    lifecycle_guard::require_year_write_exclusive(&mut tx, room.academic_year_id).await?;
    let roster = snapshot(&mut tx, id).await?;
    if roster.revision != revision {
        return Err(AppError::Conflict(
            "รายชื่อหรือเลขที่เปลี่ยนแล้ว กรุณาโหลดรายชื่อใหม่ก่อนบันทึก".into(),
        ));
    }
    if roster.homeroom.is_active != Some(true) {
        return Err(AppError::Conflict("ห้องนี้ปิดใช้งานแล้ว".into()));
    }
    Ok((tx, roster))
}
fn numbering_inputs(
    students: &[HomeroomRosterStudent],
    start: i32,
) -> Result<Vec<HomeroomNumberInput>, AppError> {
    if start <= 0 || start.checked_add(students.len() as i32).is_none() {
        return Err(AppError::ValidationError(
            "เลขที่เริ่มต้นต้องเป็นจำนวนเต็มบวกที่ไม่เกินขอบเขต".into(),
        ));
    }
    Ok(students
        .iter()
        .enumerate()
        .map(|(offset, s)| HomeroomNumberInput {
            placement_id: s.placement_id,
            class_number: start + offset as i32,
        })
        .collect())
}
pub async fn preview_numbers(
    pool: &PgPool,
    id: Uuid,
    query: HomeroomNumberingQuery,
) -> Result<HomeroomNumberingPreview, AppError> {
    let mut tx = read_transaction(pool).await?;
    let roster = snapshot(&mut tx, id).await?;
    let method = match query.method {
        HomeroomNumberingMethod::Name => "name",
        HomeroomNumberingMethod::StudentCode => "student_code",
        HomeroomNumberingMethod::GenderName => "gender_name",
    };
    let ordered = read_students(&mut tx, id, method).await?;
    let numbers = numbering_inputs(&ordered, query.start_number)?;
    tx.commit().await?;
    Ok(HomeroomNumberingPreview { roster, numbers })
}
fn validate_numbers(
    students: &[HomeroomRosterStudent],
    inputs: &[HomeroomNumberInput],
) -> Result<(), AppError> {
    if inputs.is_empty() || inputs.len() > MAX_ROSTER {
        return Err(AppError::ValidationError(
            "เลือกเลขที่ที่จะบันทึก 1–500 รายการ".into(),
        ));
    }
    let mut changes = BTreeMap::new();
    for n in inputs {
        if n.class_number <= 0 || changes.insert(n.placement_id, n.class_number).is_some() {
            return Err(AppError::ValidationError(
                "เลขที่ต้องเป็นจำนวนเต็มบวก และรายการนักเรียนต้องไม่ซ้ำ".into(),
            ));
        }
    }
    let mut used = BTreeSet::new();
    for s in students {
        if let Some(n) = changes.remove(&s.placement_id).or(s.class_number) {
            if n <= 0 || !used.insert(n) {
                return Err(AppError::Conflict(
                    "เลขที่ซ้ำในห้อง กรุณาจัดเลขที่ทั้งห้องใหม่หรือเลือกเลขที่ว่าง".into(),
                ));
            }
        }
    }
    if !changes.is_empty() {
        return Err(AppError::ValidationError("นักเรียนที่เลือกไม่ได้อยู่ในห้องนี้".into()));
    }
    Ok(())
}
pub async fn update_numbers(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    request: UpdateHomeroomNumbersRequest,
) -> Result<HomeroomRoster, AppError> {
    let (mut tx, roster) = write_roster(pool, id, &request.revision).await?;
    validate_numbers(&roster.students, &request.numbers)?;
    let ids: Vec<_> = request.numbers.iter().map(|n| n.placement_id).collect();
    let numbers: Vec<_> = request.numbers.iter().map(|n| n.class_number).collect();
    sqlx::query(r#"UPDATE homeroom_placements p SET class_number=n.number,row_version=p.row_version+1,updated_at=now()
        FROM unnest($1::uuid[],$2::int[]) AS n(id,number)
        WHERE p.id=n.id AND p.homeroom_id=$3 AND p.class_number IS DISTINCT FROM n.number"#)
        .bind(&ids).bind(&numbers).bind(id).execute(&mut *tx).await?;
    append_audit(
        &mut tx,
        "homeroom.numbers_updated",
        "homeroom",
        id,
        Some(roster.homeroom.academic_year_id),
        None,
        actor,
        serde_json::json!({"numbers":request.numbers}),
    )
    .await?;
    let updated = snapshot(&mut tx, id).await?;
    tx.commit().await?;
    Ok(updated)
}
pub async fn list_candidates(
    pool: &PgPool,
    id: Uuid,
    query: HomeroomCandidateQuery,
) -> Result<Vec<HomeroomRosterCandidate>, AppError> {
    let room = student_years::get_homeroom(pool, id).await?;
    let search = query.search.unwrap_or_default();
    if search.chars().count() > 100 {
        return Err(AppError::ValidationError("คำค้นหายาวเกิน 100 ตัวอักษร".into()));
    }
    Ok(sqlx::query_as(r#"SELECT y.id AS student_academic_year_id,y.student_id,i.student_id AS student_code,
        concat_ws(' ',u.title,u.first_name,u.last_name) AS name,y.row_version
        FROM student_academic_years y JOIN users u ON u.id=y.student_id LEFT JOIN student_info i ON i.user_id=u.id
        WHERE y.academic_year_id=$1 AND y.grade_level_id=$2 AND y.study_program_id=$3
          AND y.status IN ('planned','active') AND u.status='active'
          AND NOT EXISTS(SELECT 1 FROM homeroom_placements p WHERE p.student_academic_year_id=y.id AND p.status IN ('planned','current'))
          AND ($4='' OR strpos(lower(concat_ws(' ',u.first_name,u.last_name,i.student_id)),lower($4))>0)
        ORDER BY u.first_name COLLATE schoolorbit_thai,u.last_name COLLATE schoolorbit_thai,y.id LIMIT 100"#)
        .bind(room.academic_year_id).bind(room.grade_level_id).bind(room.study_program_id).bind(search.trim()).fetch_all(pool).await?)
}
fn validate_selections(r: &MutateHomeroomRosterRequest) -> Result<(), AppError> {
    if r.selections.is_empty() || r.selections.len() > MAX_ROSTER {
        return Err(AppError::ValidationError("เลือกนักเรียน 1–500 คน".into()));
    }
    let mut ids = BTreeSet::new();
    for s in &r.selections {
        parse_row_version(s.student_year_row_version)?;
        if !ids.insert(s.student_academic_year_id) {
            return Err(AppError::ValidationError("รายชื่อนักเรียนซ้ำ".into()));
        }
        if r.action == HomeroomRosterAction::Add {
            if s.placement_id.is_some() || s.placement_row_version.is_some() {
                return Err(AppError::ValidationError(
                    "เพิ่มได้เฉพาะนักเรียนที่ยังไม่มีห้อง".into(),
                ));
            }
        } else if s.placement_id.is_none() || s.placement_row_version.is_none() {
            return Err(AppError::ValidationError("ไม่พบข้อมูลห้องเดิมของนักเรียน".into()));
        }
    }
    if r.action == HomeroomRosterAction::Transfer {
        if r.target_homeroom_id.is_none() {
            return Err(AppError::ValidationError("กรุณาเลือกห้องปลายทาง".into()));
        }
    } else if r.target_homeroom_id.is_some() {
        return Err(AppError::ValidationError(
            "ห้องปลายทางใช้ได้เฉพาะการย้ายห้อง".into(),
        ));
    }
    if r.reason.chars().count() > 500
        || (r.action != HomeroomRosterAction::Add && r.reason.trim().is_empty())
        || student_years::contains_thirteen_digit_run(&r.reason)
    {
        return Err(AppError::ValidationError(
            "กรอกเหตุผลไม่เกิน 500 ตัวอักษร โดยไม่ใส่เลขประจำตัวประชาชน".into(),
        ));
    }
    Ok(())
}
#[derive(sqlx::FromRow)]
struct SelectedStudentYear {
    id: Uuid,
    row_version: i64,
    status: StudentAcademicYearStatus,
    grade_level_id: Uuid,
    study_program_id: Uuid,
    active: bool,
}

pub async fn mutate_roster(
    pool: &PgPool,
    rosters: &dyn crate::ports::PlacementRosterPort,
    actor: Uuid,
    id: Uuid,
    r: MutateHomeroomRosterRequest,
) -> Result<HomeroomRoster, AppError> {
    validate_selections(&r)?;
    let (mut tx, roster) = write_roster(pool, id, &r.revision).await?;
    let room = &roster.homeroom;
    let ids: Vec<_> = r
        .selections
        .iter()
        .map(|s| s.student_academic_year_id)
        .collect();
    let years=sqlx::query_as::<_,SelectedStudentYear>(r#"SELECT y.id,y.row_version,y.status,y.grade_level_id,y.study_program_id,u.status='active' AS active
        FROM student_academic_years y JOIN users u ON u.id=y.student_id
        WHERE y.academic_year_id=$1 AND y.id=ANY($2) ORDER BY y.id FOR UPDATE OF y"#)
        .bind(room.academic_year_id).bind(&ids).fetch_all(&mut *tx).await?;
    if years.len() != ids.len() {
        return Err(AppError::ValidationError("นักเรียนไม่อยู่ในปีการศึกษานี้".into()));
    }
    let valid: bool = sqlx::query_scalar(
        "SELECT $2 BETWEEN start_date AND end_date FROM academic_years WHERE id=$1",
    )
    .bind(room.academic_year_id)
    .bind(r.effective_date)
    .fetch_one(&mut *tx)
    .await?;
    if !valid {
        return Err(AppError::ValidationError("วันที่อยู่นอกปีการศึกษา".into()));
    }
    for s in &r.selections {
        let year = years
            .iter()
            .find(|y| y.id == s.student_academic_year_id)
            .ok_or_else(|| AppError::ValidationError("ไม่พบข้อมูลนักเรียนประจำปี".into()))?;
        if year.row_version != s.student_year_row_version {
            return Err(AppError::Conflict("ข้อมูลนักเรียนเปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
        }
        if !year.active
            || !matches!(
                year.status,
                StudentAcademicYearStatus::Planned | StudentAcademicYearStatus::Active
            )
        {
            return Err(AppError::Conflict("นักเรียนไม่ได้อยู่ในสถานะที่จัดห้องได้".into()));
        }
        if year.grade_level_id != room.grade_level_id
            || year.study_program_id != room.study_program_id
        {
            return Err(AppError::ValidationError(
                "ระดับชั้นและแผนการเรียนต้องตรงกับห้อง".into(),
            ));
        }
        if r.action != HomeroomRosterAction::Add {
            let old = roster
                .students
                .iter()
                .find(|p| {
                    Some(p.placement_id) == s.placement_id
                        && p.student_academic_year_id == s.student_academic_year_id
                })
                .ok_or_else(|| AppError::Conflict("นักเรียนไม่ได้อยู่ในห้องเดิมแล้ว".into()))?;
            if Some(old.row_version) != s.placement_row_version {
                return Err(AppError::Conflict("ข้อมูลห้องเดิมเปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
            }
            if old.status == HomeroomPlacementStatus::Current && r.effective_date <= old.start_date
            {
                return Err(AppError::ValidationError(
                    "วันที่ย้ายหรือนำออกต้องอยู่หลังวันที่เริ่มห้องเดิม".into(),
                ));
            }
        }
    }
    let destination = match r.action {
        HomeroomRosterAction::Transfer => {
            let target_id = r
                .target_homeroom_id
                .ok_or_else(|| AppError::ValidationError("ไม่พบห้องปลายทาง".into()))?;
            if target_id == id {
                return Err(AppError::ValidationError(
                    "เลือกห้องปลายทางที่ต่างจากห้องเดิม".into(),
                ));
            }
            let target = snapshot(&mut tx, target_id).await?;
            if target.homeroom.academic_year_id != room.academic_year_id
                || target.homeroom.grade_level_id != room.grade_level_id
                || target.homeroom.study_program_id != room.study_program_id
                || target.homeroom.is_active != Some(true)
            {
                return Err(AppError::ValidationError(
                    "ห้องปลายทางต้องเปิดใช้งานและอยู่ในปี ระดับชั้น และแผนการเรียนเดียวกัน".into(),
                ));
            }
            Some(target)
        }
        HomeroomRosterAction::Add => Some(roster.clone()),
        HomeroomRosterAction::Remove => None,
    };
    let next = if let Some(target) = &destination {
        let members = &target.students;
        if members.len() + ids.len() > target.homeroom.capacity as usize
            || members.len() + ids.len() > MAX_ROSTER
        {
            return Err(AppError::Conflict("นักเรียนเกินความจุห้องปลายทาง".into()));
        }
        let maximum = members
            .iter()
            .filter_map(|s| s.class_number)
            .max()
            .unwrap_or(0)
            .max(0);
        maximum
            .checked_add(ids.len() as i32 + 1)
            .ok_or_else(|| AppError::ValidationError("เลขที่ห้องปลายทางเกินขอบเขต".into()))?;
        maximum + 1
    } else {
        1
    };
    if r.action == HomeroomRosterAction::Add {
        let occupied: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM homeroom_placements WHERE student_academic_year_id=ANY($1) AND status IN ('planned','current'))")
            .bind(&ids).fetch_one(&mut *tx).await?;
        let overlaps: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM homeroom_placements WHERE student_academic_year_id=ANY($1) AND status='ended' AND (end_date IS NULL OR end_date >= $2))")
            .bind(&ids).bind(r.effective_date).fetch_one(&mut *tx).await?;
        if occupied || overlaps {
            return Err(AppError::Conflict(
                "นักเรียนมีห้องอยู่แล้วหรือวันที่ทับซ้อนประวัติเดิม".into(),
            ));
        }
    }
    if r.action != HomeroomRosterAction::Add {
        let old: Vec<_> = r.selections.iter().filter_map(|s| s.placement_id).collect();
        sqlx::query(r#"UPDATE homeroom_placements SET status=CASE WHEN status='planned' THEN 'cancelled' ELSE 'ended' END,
            end_date=CASE WHEN status='planned' THEN NULL ELSE $2::date-1 END,row_version=row_version+1,updated_at=now() WHERE id=ANY($1)"#)
            .bind(&old).bind(r.effective_date).execute(&mut *tx).await?;
    }
    if let Some(target) = destination {
        let target = target.homeroom;
        let placements: Vec<_> = ids.iter().map(|_| Uuid::new_v4()).collect();
        let numbers: Vec<_> = (0..ids.len()).map(|n| next + n as i32).collect();
        let statuses: Vec<_> = r
            .selections
            .iter()
            .map(|s| {
                if years.iter().any(|y| {
                    y.id == s.student_academic_year_id
                        && y.status == StudentAcademicYearStatus::Planned
                }) {
                    "planned".to_string()
                } else {
                    "current".to_string()
                }
            })
            .collect();
        sqlx::query(r#"INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,start_date,status,enrollment_type,class_number)
            SELECT n.id,n.student_year_id,$5,$6,$7,n.status,$8,n.number FROM unnest($1::uuid[],$2::uuid[],$3::int[],$4::text[]) AS n(id,student_year_id,number,status)"#)
            .bind(&placements).bind(&ids).bind(&numbers).bind(&statuses).bind(room.academic_year_id).bind(target.id).bind(r.effective_date)
            .bind(if r.action==HomeroomRosterAction::Add { "regular" } else { "room_transfer" }).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE student_academic_years SET row_version=row_version+1,updated_at=now() WHERE id=ANY($1)").bind(&ids).execute(&mut *tx).await?;
    append_audit(&mut tx,"homeroom.roster_changed","homeroom",id,Some(room.academic_year_id),None,actor,
        serde_json::json!({"action":r.action,"studentYearIds":ids,"targetHomeroomId":r.target_homeroom_id,"effectiveDate":r.effective_date,"reason":r.reason.trim()})).await?;
    rosters.reconcile(&mut tx, actor, &ids).await?;
    let updated = snapshot(&mut tx, id).await?;
    tx.commit().await?;
    Ok(updated)
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    fn student(id: Uuid, n: Option<i32>) -> HomeroomRosterStudent {
        HomeroomRosterStudent {
            placement_id: id,
            student_academic_year_id: Uuid::new_v4(),
            student_id: Uuid::new_v4(),
            student_code: None,
            title: None,
            first_name: "ทดสอบ".into(),
            last_name: "นักเรียน".into(),
            gender: None,
            class_number: n,
            status: HomeroomPlacementStatus::Current,
            start_date: NaiveDate::from_ymd_opt(2026, 5, 1).unwrap(),
            row_version: 1,
            student_year_row_version: 1,
        }
    }
    #[test]
    fn numbering_validates_the_final_set_not_transient_swaps() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let students = vec![student(a, Some(1)), student(b, Some(2))];
        let swap = vec![
            HomeroomNumberInput {
                placement_id: a,
                class_number: 2,
            },
            HomeroomNumberInput {
                placement_id: b,
                class_number: 1,
            },
        ];
        assert!(validate_numbers(&students, &swap).is_ok());
        assert!(validate_numbers(&students, &swap[..1]).is_err());
        assert!(validate_numbers(
            &students,
            &[HomeroomNumberInput {
                placement_id: Uuid::new_v4(),
                class_number: 3
            }]
        )
        .is_err());
        assert!(validate_numbers(
            &students,
            &[HomeroomNumberInput {
                placement_id: a,
                class_number: 0
            }]
        )
        .is_err());
        assert!(validate_numbers(&students, &[swap[0].clone(), swap[0].clone()]).is_err());
        assert!(numbering_inputs(&students, i32::MAX).is_err());
        assert_eq!(numbering_inputs(&students, 5).unwrap()[1].class_number, 6);
    }
}

pub async fn transfer_targets(pool: &PgPool, id: Uuid) -> Result<Vec<Homeroom>, AppError> {
    let room = student_years::get_homeroom(pool, id).await?;
    Ok(student_years::list_homerooms(pool, room.academic_year_id)
        .await?
        .into_iter()
        .filter(|target| {
            target.id != id
                && target.grade_level_id == room.grade_level_id
                && target.study_program_id == room.study_program_id
                && target.is_active == Some(true)
        })
        .collect())
}
