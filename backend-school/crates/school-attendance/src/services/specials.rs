use super::{audit, lock_term};
use crate::{models::*, rules::invalid};
use school_errors::AppError;
use sqlx::{types::Json, PgConnection, PgPool};
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
    if let Some(scoring) = &p.scoring {
        validate_scoring(scoring)?;
    }
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
pub fn validate_scoring(scoring: &[AttendanceScoreRule]) -> Result<(), AppError> {
    let mut results = std::collections::HashSet::new();
    if scoring.len() != 7
        || scoring.iter().any(|rule| {
            rule.result == AttendanceResult::Unchecked
                || !results.insert(rule.result)
                || match rule.action {
                    ScoreAction::None => rule.points != 0,
                    ScoreAction::Merit | ScoreAction::Deduction => {
                        !(1..=10_000).contains(&rule.points)
                    }
                }
        })
    {
        return Err(invalid(
            "กำหนดคะแนนรอบพิเศษให้ครบทั้ง 7 สถานะโดยไม่ซ้ำ ไม่คิดคะแนนใช้ 0 และคะแนนต้องอยู่ระหว่าง 1 ถึง 10000",
        ));
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
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO attendance_special_templates(id,academic_term_id,definition,created_by) VALUES($1,$2,$3,$4)").bind(id).bind(term).bind(Json(&p)).bind(actor).execute(&mut *tx).await?;
    if resolved.len() != p.dates.len() {
        return Err(invalid("รอบพิเศษไม่ตรงกับวันที่กำหนด"));
    }
    for (date, seeds) in p.dates.iter().zip(resolved) {
        if seeds.is_empty() || seeds.iter().any(|s| s.students.len() > 2000) {
            return Err(invalid("แบ่งกลุ่มรอบพิเศษให้ไม่เกิน 2000 คนต่อกลุ่ม"));
        }
        crate::rules::distinct_groups(
            &seeds
                .iter()
                .map(|s| s.students.iter().map(|s| s.student_id).collect())
                .collect::<Vec<_>>(),
        )?;
        let round = Uuid::now_v7();
        let participants: Vec<_> = seeds
            .into_iter()
            .enumerate()
            .map(|(i, seed)| SpecialRoundParticipants {
                session_id: Uuid::new_v5(&round, i.to_string().as_bytes()),
                source_key: format!("{round}:{i}"),
                title: seed.session.title,
                teacher_ids: seed.session.teacher_ids,
                students: seed.students,
                start_time: seed.session.start_time,
                end_time: seed.session.end_time,
                count_override: seed.session.count_override,
            })
            .collect();
        sqlx::query("INSERT INTO attendance_special_rounds(id,template_id,date,participants) VALUES($1,$2,$3,$4)")
            .bind(round).bind(id).bind(date).bind(Json(&participants)).execute(&mut *tx).await?;
    }
    audit(&mut tx, actor, id, "create", "special attendance rounds").await?;
    for date in &p.dates {
        super::score_changes::enqueue_day(&mut tx, term, *date).await?;
        sqlx::query("DELETE FROM attendance_finalized_dates WHERE generation=(SELECT generation FROM attendance_module_state WHERE singleton) AND academic_term_id=$1 AND date=$2")
            .bind(term).bind(date).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(SpecialAttendanceTemplate {
        id,
        academic_term_id: term,
        definition: p,
        row_version: 1,
    })
}
pub async fn seeds(
    pool: &PgPool,
    term: Uuid,
    date: chrono::NaiveDate,
) -> Result<Vec<AttendanceSeed>, AppError> {
    seeds_on_connection(&mut *pool.acquire().await?, term, date).await
}
pub async fn seeds_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: chrono::NaiveDate,
) -> Result<Vec<AttendanceSeed>, AppError> {
    let rows:Vec<(Uuid,Json<Vec<SpecialRoundParticipants>>)>=sqlx::query_as("SELECT r.id,r.participants FROM attendance_special_rounds r JOIN attendance_special_templates t ON t.id=r.template_id WHERE t.academic_term_id=$1 AND r.date=$2 ORDER BY r.id").bind(term).bind(date).fetch_all(&mut *connection).await?;
    Ok(rows
        .into_iter()
        .flat_map(|(round, Json(groups))| {
            groups.into_iter().map(move |g| AttendanceSeed {
                session: AttendanceSession {
                    id: g.session_id,
                    academic_term_id: term,
                    date,
                    kind: AttendanceKind::Special,
                    source_key: g.source_key,
                    title: g.title,
                    teacher_ids: g.teacher_ids,
                    homeroom_id: None,
                    learning_group_id: None,
                    offering_id: None,
                    special_round_id: Some(round),
                    start_time: g.start_time,
                    end_time: g.end_time,
                    count_override: g.count_override,
                    cancelled: false,
                    cancellation_reason: None,
                    saved_at: None,
                    saved_by: None,
                    row_version: 0,
                    roster: g.students.clone(),
                    roster_token: String::new(),
                },
                students: g.students,
            })
        })
        .collect())
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
    let mut tx = pool.begin().await?;
    super::lock_module(&mut tx).await?;
    let device=sqlx::query_as("INSERT INTO attendance_devices(id,name,enabled,paired_by,operator_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO UPDATE SET name=excluded.name,enabled=excluded.enabled,operator_id=excluded.operator_id RETURNING id,name,enabled,operator_id").bind(id).bind(p.name.trim()).bind(p.enabled).bind(actor).bind(p.operator_id).fetch_one(&mut *tx).await?;
    audit(&mut tx, actor, id, "update", "attendance device").await?;
    tx.commit().await?;
    Ok(device)
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
            scoring: None,
        };
        assert!(validate_definition(&p).is_err());
        p.dates = vec![NaiveDate::MIN, NaiveDate::MIN];
        assert!(validate_definition(&p).is_err());
    }
    #[test]
    fn special_scoring_requires_complete_distinct_writable_results_and_valid_amounts() {
        let mut rules = [
            AttendanceResult::Present,
            AttendanceResult::Absent,
            AttendanceResult::Late,
            AttendanceResult::Skipped,
            AttendanceResult::SickLeave,
            AttendanceResult::PersonalLeave,
            AttendanceResult::Activity,
        ]
        .into_iter()
        .map(|result| AttendanceScoreRule {
            result,
            action: ScoreAction::None,
            points: 0,
        })
        .collect::<Vec<_>>();
        assert!(validate_scoring(&rules).is_ok());
        rules[0].action = ScoreAction::Merit;
        assert!(validate_scoring(&rules).is_err());
        rules[0].points = 5;
        assert!(validate_scoring(&rules).is_ok());
        rules[1].result = rules[0].result;
        assert!(validate_scoring(&rules).is_err());
        rules[1].result = AttendanceResult::Unchecked;
        assert!(validate_scoring(&rules).is_err());
        rules[1].result = AttendanceResult::Absent;
        rules[0].points = 10_001;
        assert!(validate_scoring(&rules).is_err());
        rules.pop();
        assert!(validate_scoring(&rules).is_err());
    }
}
