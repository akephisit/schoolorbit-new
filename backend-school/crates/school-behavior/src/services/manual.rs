use super::shared::{begin_manual, bump, lock, receipt, require_open_date, save_receipt};
use crate::{
    access::{roster, BehaviorAccess, BehaviorRosterRow},
    models::*,
    rules::{digest, invalid, normalize_ids, pagination, validate_reason},
};
use chrono::NaiveDate;
use school_errors::AppError;
use serde::Serialize;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;

pub fn roster_token(
    term: Uuid,
    date: NaiveDate,
    rows: &[BehaviorRosterRow],
) -> Result<String, AppError> {
    digest(&(term, date, rows))
}
pub async fn students(
    pool: &PgPool,
    access: BehaviorAccess,
    q: BehaviorStudentsQuery,
) -> Result<BehaviorStudentPage, AppError> {
    access.require_selector()?;
    let (offset, limit) = pagination(q.offset, q.limit)?;
    if q.search.as_ref().is_some_and(|s| s.chars().count() > 200) {
        return Err(invalid("คำค้นยาวเกินไป"));
    }
    let rows: Vec<_> = roster(&mut *pool.acquire().await?, q.academic_term_id, q.date)
        .await?
        .into_iter()
        .filter(|r| r.visible(access))
        .collect();
    let token = roster_token(q.academic_term_id, q.date, &rows)?;
    let options = options_from_rows(&rows);
    let search = q.search.unwrap_or_default().to_lowercase();
    let filtered: Vec<_> = rows
        .into_iter()
        .filter(|r| {
            q.grade_level_id.is_none_or(|id| id == r.grade_level_id)
                && q.homeroom_id.is_none_or(|id| Some(id) == r.homeroom_id)
                && (search.is_empty()
                    || r.name.to_lowercase().contains(&search)
                    || r.student_code
                        .as_ref()
                        .is_some_and(|s| s.to_lowercase().contains(&search)))
        })
        .collect();
    let total = filtered.len() as i64;
    let items = filtered
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .map(BehaviorRosterRow::student)
        .collect();
    Ok(BehaviorStudentPage {
        items,
        total,
        roster_token: token,
        options,
    })
}
pub fn options_from_rows(rows: &[BehaviorRosterRow]) -> BehaviorSelectorOptions {
    let mut grades = std::collections::BTreeMap::new();
    let mut rooms = std::collections::BTreeMap::new();
    for r in rows {
        grades.insert(r.grade_level_id, r.grade_level.clone());
        if let Some(id) = r.homeroom_id {
            rooms.insert(id, r.homeroom.clone());
        }
    }
    BehaviorSelectorOptions {
        grade_levels: grades
            .into_iter()
            .map(|(id, label)| BehaviorSelectorOption { id, label })
            .collect(),
        homerooms: rooms
            .into_iter()
            .map(|(id, label)| BehaviorSelectorOption { id, label })
            .collect(),
    }
}
pub async fn selector_options(
    pool: &PgPool,
    mut access: BehaviorAccess,
    q: BehaviorStudentsQuery,
    manage: bool,
) -> Result<BehaviorSelectorOptions, AppError> {
    if manage {
        access.school = true;
    }
    access.require_selector()?;
    let rows: Vec<_> = roster(&mut *pool.acquire().await?, q.academic_term_id, q.date)
        .await?
        .into_iter()
        .filter(|r| r.visible(access))
        .collect();
    Ok(options_from_rows(&rows))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchMember {
    student_id: Uuid,
    grade_level_id: Uuid,
    homeroom_id: Option<Uuid>,
}
pub async fn create_batch(
    pool: &PgPool,
    access: BehaviorAccess,
    mut p: CreateBehaviorBatch,
) -> Result<BehaviorBatchReceipt, AppError> {
    access.require_create()?;
    p.student_ids = normalize_ids(p.student_ids)?;
    if p.detail.as_ref().is_some_and(|s| s.chars().count() > 5000) {
        return Err(invalid("รายละเอียดไม่เกิน 5,000 ตัวอักษร"));
    }
    p.detail = p
        .detail
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let date = p
        .occurred_at
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    // A committed receipt remains meaningful even after its details or academic context close.
    let mut connection = pool.acquire().await?;
    if let Some(mut previous) = receipt::<_, BehaviorBatchReceipt>(
        &mut connection,
        access.actor_id,
        p.request_id,
        "batch",
        &p,
    )
    .await?
    {
        previous.replayed = true;
        previous.details_available =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM behavior_batches WHERE id=$1)")
                .bind(previous.id)
                .fetch_one(&mut *connection)
                .await?;
        return Ok(previous);
    }
    drop(connection);
    let mut tx = begin_manual(pool, p.academic_term_id, date).await?;
    if let Some(mut previous) =
        receipt::<_, BehaviorBatchReceipt>(&mut tx, access.actor_id, p.request_id, "batch", &p)
            .await?
    {
        previous.replayed = true;
        tx.commit().await?;
        return Ok(previous);
    }
    let row:Option<(String,BehaviorSide,Uuid,i32,i64)>=sqlx::query_as("SELECT i.name,i.side,v.id,v.points,i.row_version FROM behavior_items i JOIN LATERAL(SELECT id,points FROM behavior_item_versions WHERE item_id=i.id AND effective_from<=$2 ORDER BY effective_from DESC,id DESC LIMIT 1)v ON true WHERE i.id=$1 AND i.enabled").bind(p.item_id).bind(date).fetch_optional(&mut *tx).await?;
    let (name, side, version, points, item_row_version) =
        row.ok_or_else(|| AppError::Conflict("รายการนี้ไม่มีเกณฑ์ที่ใช้งานได้ในวันเกิดเหตุ".into()))?;
    if version != p.item_version_id || item_row_version != p.item_row_version {
        return Err(AppError::Conflict("คะแนนเปลี่ยนแล้ว กรุณาตรวจเกณฑ์อีกครั้ง".into()));
    }
    let cycle = require_open_date(
        &mut tx,
        side,
        date,
        BehaviorSource::Manual,
        p.academic_term_id,
    )
    .await?;
    if p.occurred_at < cycle.starts_at
        || p.occurred_at > chrono::Utc::now() + chrono::Duration::minutes(5)
    {
        return Err(AppError::Conflict("วันเวลาอยู่นอกรอบคะแนนปัจจุบัน".into()));
    }
    let rows: Vec<_> = roster(&mut tx, p.academic_term_id, date)
        .await?
        .into_iter()
        .filter(|r| r.visible(access))
        .collect();
    if roster_token(p.academic_term_id, date, &rows)? != p.roster_token {
        return Err(AppError::Conflict(
            "รายชื่อนักเรียนหรือสิทธิ์เปลี่ยนแล้ว กรุณาตรวจใหม่".into(),
        ));
    }
    let members: Vec<_> = rows
        .iter()
        .filter(|r| p.student_ids.binary_search(&r.id).is_ok())
        .collect();
    if members.len() != p.student_ids.len() || members.iter().any(|r| !r.writable(access)) {
        return Err(AppError::Forbidden(
            "มีนักเรียนที่ไม่มีสิทธิ์บันทึกหรือไม่ได้อยู่ในรายชื่อของวันเกิดเหตุ".into(),
        ));
    }
    let batch:Uuid=sqlx::query_scalar("INSERT INTO behavior_batches(academic_term_id,item_version_id,occurred_at,date,detail,created_by)VALUES($1,$2,$3,$4,$5,$6)RETURNING id").bind(p.academic_term_id).bind(version).bind(p.occurred_at).bind(date).bind(&p.detail).bind(access.actor_id).fetch_one(&mut *tx).await?;
    let members: Vec<_> = members
        .into_iter()
        .map(|r| BatchMember {
            student_id: r.id,
            grade_level_id: r.grade_level_id,
            homeroom_id: r.homeroom_id,
        })
        .collect();
    sqlx::query(r#"INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,homeroom_id,side,source,cycle_id,points)
        SELECT x."studentId",$2,$3,x."gradeLevelId",x."homeroomId",$4,'manual',$5,$6
        FROM jsonb_to_recordset($1)x("studentId" uuid,"gradeLevelId" uuid,"homeroomId" uuid)
        ON CONFLICT(student_id,date,academic_term_id,grade_level_id,homeroom_id,side,source,cycle_id)DO UPDATE SET points=behavior_daily_totals.points+EXCLUDED.points,row_version=behavior_daily_totals.row_version+1"#)
        .bind(Json(&members)).bind(date).bind(p.academic_term_id).bind(side).bind(cycle.id).bind(points).execute(&mut *tx).await?;
    sqlx::query(r#"INSERT INTO behavior_entries(total_id,batch_id,item_name,points,created_by)
        SELECT t.id,$2,$3,$4,$5 FROM behavior_daily_totals t
        JOIN jsonb_to_recordset($1)x("studentId" uuid,"gradeLevelId" uuid,"homeroomId" uuid)ON x."studentId"=t.student_id AND x."gradeLevelId"=t.grade_level_id AND x."homeroomId" IS NOT DISTINCT FROM t.homeroom_id
        WHERE t.date=$6 AND t.academic_term_id=$7 AND t.side=$8 AND t.source='manual' AND t.cycle_id=$9"#)
        .bind(Json(&members)).bind(batch).bind(name).bind(points).bind(access.actor_id).bind(date).bind(p.academic_term_id).bind(side).bind(cycle.id).execute(&mut *tx).await?;
    let result = BehaviorBatchReceipt {
        id: batch,
        affected_students: members.len() as i64,
        side,
        points_per_student: points,
        replayed: false,
        details_available: true,
    };
    save_receipt(&mut tx, access.actor_id, p.request_id, "batch", &p, &result).await?;
    bump(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn cancel_entries(
    pool: &PgPool,
    access: BehaviorAccess,
    mut p: CancelBehaviorEntries,
) -> Result<BehaviorCancellation, AppError> {
    if !access.cancel_own && !access.cancel_school {
        return Err(AppError::Forbidden("ไม่มีสิทธิ์ยกเลิกพฤติกรรม".into()));
    }
    p.entry_ids = normalize_ids(p.entry_ids)?;
    validate_reason(&p.reason)?;
    p.reason = p.reason.trim().into();
    let mut tx = pool.begin().await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut tx).await?;
    lock(&mut tx).await?;
    if let Some(previous) = receipt(&mut tx, access.actor_id, p.request_id, "cancel", &p).await? {
        tx.commit().await?;
        return Ok(previous);
    }
    let rows:Vec<(Uuid,Option<Uuid>,Option<Uuid>,bool,String,Uuid)>=sqlx::query_as("SELECT e.id,e.batch_id,e.created_by,e.cancelled_at IS NOT NULL,c.state,t.academic_term_id FROM behavior_entries e JOIN behavior_daily_totals t ON t.id=e.total_id JOIN behavior_cycles c ON c.id=t.cycle_id WHERE e.id=ANY($1) ORDER BY e.id FOR UPDATE OF e").bind(&p.entry_ids).fetch_all(&mut *tx).await?;
    if rows.len() != p.entry_ids.len() {
        return Err(AppError::Conflict("รายการถูกล้างหรือไม่มีอยู่แล้ว".into()));
    }
    if rows
        .iter()
        .any(|(_, batch, creator, _, _, _)| batch.is_none() || !access.can_cancel(*creator))
    {
        return Err(AppError::Forbidden(
            "ยกเลิกได้เฉพาะพฤติกรรมที่มีสิทธิ์ คะแนนเช็กชื่อต้องแก้ต้นทาง".into(),
        ));
    }
    if rows
        .iter()
        .any(|(_, _, _, cancelled, state, _)| !*cancelled && state != "current")
    {
        return Err(AppError::Conflict(
            "รายการนี้เก็บเป็นประวัติแล้ว ใช้การปรับยอดประวัติ".into(),
        ));
    }
    // Transition lock prevents lifecycle races without taking academic row locks
    // after the Behavior module lock (the global ordering used by source writers).
    let terms: Vec<_> = rows.iter().map(|r| r.5).collect();
    let closed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=ANY($1) AND(t.status IN('closed','cancelled')OR y.status IN('closed','archived')))").bind(terms).fetch_one(&mut *tx).await?;
    if closed {
        return Err(AppError::Conflict("ภาคเรียนปิดแล้ว ใช้การปรับยอดประวัติ".into()));
    }
    sqlx::query("UPDATE behavior_daily_totals t SET points=t.points-x.points,row_version=t.row_version+1 FROM(SELECT total_id,sum(points)points FROM behavior_entries WHERE id=ANY($1) AND cancelled_at IS NULL GROUP BY total_id)x WHERE t.id=x.total_id").bind(&p.entry_ids).execute(&mut *tx).await?;
    let cancelled=sqlx::query("UPDATE behavior_entries SET cancelled_by=$2,cancelled_at=now(),cancellation_reason=$3 WHERE id=ANY($1)AND cancelled_at IS NULL").bind(&p.entry_ids).bind(access.actor_id).bind(&p.reason).execute(&mut *tx).await?.rows_affected()as i64;
    let outcome = BehaviorCancellation {
        cancelled,
        already_cancelled: rows.len() as i64 - cancelled,
    };
    save_receipt(
        &mut tx,
        access.actor_id,
        p.request_id,
        "cancel",
        &p,
        &outcome,
    )
    .await?;
    bump(&mut tx).await?;
    tx.commit().await?;
    Ok(outcome)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roster_token_ignores_pagination_not_membership() {
        let term = Uuid::new_v4();
        let date = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
        assert_ne!(
            roster_token(term, date, &[]).unwrap(),
            roster_token(Uuid::new_v4(), date, &[]).unwrap()
        );
    }
}
