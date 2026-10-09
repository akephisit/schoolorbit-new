use super::{audit, lock_term, sessions};
use crate::{models::*, rules::invalid};
use school_errors::AppError;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;
pub async fn audiences(
    pool: &PgPool,
    term: Uuid,
) -> Result<Vec<AttendanceAudienceGroup>, AppError> {
    Ok(sqlx::query_as("SELECT id,academic_term_id,name,student_ids,row_version FROM attendance_audience_groups WHERE academic_term_id=$1 ORDER BY name,id").bind(term).fetch_all(pool).await?)
}
pub async fn save_audience(
    pool: &PgPool,
    actor: Uuid,
    term: Uuid,
    id: Uuid,
    p: SaveAttendanceAudience,
) -> Result<AttendanceAudienceGroup, AppError> {
    if p.name.trim().is_empty()
        || p.name.len() > 200
        || p.student_ids.is_empty()
        || p.student_ids.len() > 2000
        || p.student_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != p.student_ids.len()
    {
        return Err(invalid("ชื่อกลุ่มหรือสมาชิกไม่ถูกต้อง"));
    }
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, term, true, true).await?;
    let valid:i64=sqlx::query_scalar("SELECT count(*) FROM student_academic_years y JOIN academic_terms t ON t.academic_year_id=y.academic_year_id WHERE t.id=$1 AND y.student_id=ANY($2) AND y.status NOT IN ('withdrawn','graduated')").bind(term).bind(&p.student_ids).fetch_one(&mut *tx).await?;
    if valid != p.student_ids.len() as i64 {
        return Err(invalid("สมาชิกไม่ได้อยู่ในปีการศึกษานี้"));
    }
    let old:Option<i64>=sqlx::query_scalar("SELECT row_version FROM attendance_audience_groups WHERE id=$1 AND academic_term_id=$2 FOR UPDATE").bind(id).bind(term).fetch_optional(&mut *tx).await?;
    if old.is_none()
        && sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM attendance_audience_groups WHERE id=$1)",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?
    {
        return Err(AppError::Forbidden("กลุ่มไม่ได้อยู่ในภาคเรียนนี้".into()));
    }
    if old.unwrap_or(0) != p.row_version {
        return Err(AppError::Conflict("กลุ่มเปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
    }
    let row=sqlx::query_as("INSERT INTO attendance_audience_groups(id,academic_term_id,name,student_ids) VALUES($1,$2,$3,$4) ON CONFLICT(id) DO UPDATE SET name=excluded.name,student_ids=excluded.student_ids,row_version=attendance_audience_groups.row_version+1 RETURNING id,academic_term_id,name,student_ids,row_version").bind(id).bind(term).bind(p.name.trim()).bind(&p.student_ids).fetch_one(&mut *tx).await?;
    audit(&mut tx, actor, id, "update", "attendance audience group").await?;
    tx.commit().await?;
    Ok(row)
}
pub async fn templates(
    pool: &PgPool,
    term: Uuid,
) -> Result<Vec<SpecialAttendanceTemplate>, AppError> {
    let rows:Vec<(Uuid,Uuid,Json<SpecialAttendanceDefinition>,i64)>=sqlx::query_as("SELECT id,academic_term_id,definition,row_version FROM attendance_special_templates WHERE academic_term_id=$1 ORDER BY id").bind(term).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, academic_term_id, d, row_version)| SpecialAttendanceTemplate {
                id,
                academic_term_id,
                definition: d.0,
                row_version,
            },
        )
        .collect())
}
pub fn validate_definition(p: &SpecialAttendanceDefinition) -> Result<(), AppError> {
    if p.title.trim().is_empty()
        || p.title.len() > 200
        || p.dates.is_empty()
        || p.dates.len() > 366
        || p.dates
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != p.dates.len()
        || p.end_time <= p.start_time
        || p.groups.is_empty()
        || p.groups.len() > 100
        || p.groups.iter().any(|g| {
            g.name.trim().is_empty()
                || g.name.len() > 200
                || g.teacher_ids.len() > 20
                || g.homeroom_ids.len() > 100
                || g.audience_group_ids.len() > 100
                || g.student_ids.len() > 2000
        })
    {
        return Err(invalid("ชื่อ วัน เวลา หรือกลุ่มของรอบพิเศษไม่ถูกต้อง"));
    }
    Ok(())
}
pub async fn create_template(
    pool: &PgPool,
    actor: Uuid,
    term: Uuid,
    p: SpecialAttendanceDefinition,
    resolved: Vec<Vec<AttendanceSeed>>,
) -> Result<SpecialAttendanceTemplate, AppError> {
    validate_definition(&p)?;
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, term, true, true).await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO attendance_special_templates(id,academic_term_id,definition,created_by) VALUES($1,$2,$3,$4)").bind(id).bind(term).bind(Json(&p)).bind(actor).execute(&mut *tx).await?;
    for (date, seeds) in p.dates.iter().zip(resolved) {
        crate::rules::distinct_groups(
            &seeds
                .iter()
                .map(|s| s.students.iter().map(|s| s.student_id).collect())
                .collect::<Vec<_>>(),
        )?;
        let round = Uuid::new_v4();
        sqlx::query("INSERT INTO attendance_special_rounds(id,template_id,date) VALUES($1,$2,$3)")
            .bind(round)
            .bind(id)
            .bind(date)
            .execute(&mut *tx)
            .await?;
        for (i, mut seed) in seeds.into_iter().enumerate() {
            seed.session.id = Uuid::new_v5(&round, i.to_string().as_bytes());
            seed.session.special_round_id = Some(round);
            seed.session.source_key = format!("{round}:{i}");
            sessions::insert_seed(&mut tx, &seed).await?;
        }
    }
    audit(&mut tx, actor, id, "create", "special attendance rounds").await?;
    tx.commit().await?;
    Ok(SpecialAttendanceTemplate {
        id,
        academic_term_id: term,
        definition: p,
        row_version: 1,
    })
}
pub async fn devices(
    pool: &PgPool,
    actor: Uuid,
    all: bool,
) -> Result<Vec<AttendanceDevice>, AppError> {
    Ok(sqlx::query_as("SELECT id,name,enabled,operator_id FROM attendance_devices WHERE $1 OR operator_id=$2 ORDER BY name,id").bind(all).bind(actor).fetch_all(pool).await?)
}
pub async fn save_device(
    pool: &PgPool,
    actor: Uuid,
    id: Uuid,
    p: SaveAttendanceDevice,
) -> Result<AttendanceDevice, AppError> {
    if p.name.trim().is_empty() || p.name.len() > 100 {
        return Err(invalid("ชื่อเครื่องไม่ถูกต้อง"));
    }
    let staff: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND user_type='staff' AND status='active')",
    )
    .bind(p.operator_id)
    .fetch_one(pool)
    .await?;
    if !staff {
        return Err(invalid("ผู้ใช้เครื่องต้องเป็นครูที่ใช้งานอยู่"));
    }
    Ok(sqlx::query_as("INSERT INTO attendance_devices(id,name,enabled,paired_by,operator_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO UPDATE SET name=excluded.name,enabled=excluded.enabled,operator_id=excluded.operator_id RETURNING id,name,enabled,operator_id").bind(id).bind(p.name.trim()).bind(p.enabled).bind(actor).bind(p.operator_id).fetch_one(pool).await?)
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime};
    #[test]
    fn empty_or_repeated_dates_are_rejected() {
        let mut p = SpecialAttendanceDefinition {
            title: "กิจกรรม".into(),
            dates: vec![],
            start_time: NaiveTime::MIN,
            end_time: NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
            counted: true,
            notify: true,
            groups: vec![],
        };
        assert!(validate_definition(&p).is_err());
        p.dates = vec![NaiveDate::MIN, NaiveDate::MIN];
        assert!(validate_definition(&p).is_err());
    }
}
