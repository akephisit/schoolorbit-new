//! Dated duties are a narrow overlay over the current weekly timetable. They never
//! mutate Delivery assignments or saved Attendance occurrences.
use chrono::{NaiveDate, NaiveTime};
use school_database::DbSession;
use school_errors::AppError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DatedTeachingLesson {
    pub lesson_key: String,
    pub date: NaiveDate,
    pub block_id: Uuid,
    pub allocation_id: Uuid,
    pub teacher_id: Uuid,
    pub bell_schedule_period_id: Uuid,
    pub period_name: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub learning_group_id: Option<Uuid>,
    pub learning_group_name: Option<String>,
    pub offering_id: Option<Uuid>,
    pub offering_name: Option<String>,
    pub grade_level_ids: Vec<Uuid>,
    pub homeroom_ids: Vec<Uuid>,
    pub homeroom_names: Vec<String>,
    pub room_id: Option<Uuid>,
    pub room_name: Option<String>,
    pub source_revision: i64,
    pub allocation_revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DatedTeachingSnapshot {
    pub source_digest: String,
    pub timetable_state: String,
    pub lessons: Vec<DatedTeachingLesson>,
}
#[derive(Clone, Debug)]
pub struct DatedDutyInput {
    pub id: Uuid,
    pub teacher_id: Uuid,
    pub original_teacher_id: Uuid,
    pub date: NaiveDate,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub lesson: Option<DatedTeachingLesson>,
    pub kind: String,
    pub pair_id: Option<Uuid>,
    pub pair_leg: Option<i16>,
    pub taught_offering_id: Option<Uuid>,
    pub taught_learning_group_id: Option<Uuid>,
    pub taught_activity: Option<String>,
}
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ApprovedDatedDuty {
    pub id: Uuid,
    pub teacher_id: Uuid,
    pub original_teacher_id: Uuid,
    pub source_snapshot: sqlx::types::Json<DatedTeachingLesson>,
    pub taught_offering_id: Option<Uuid>,
    pub taught_learning_group_id: Option<Uuid>,
    pub taught_activity: Option<String>,
}
fn invalid(message: &str) -> AppError { AppError::ValidationError(message.into()) }
pub fn intervals_overlap(a: NaiveTime, b: NaiveTime, c: NaiveTime, d: NaiveTime) -> bool { a < d && c < b }
pub fn lesson_key(date: NaiveDate, allocation: Uuid, teacher: Uuid) -> String { format!("{date}:{allocation}:{teacher}") }
pub async fn lock_context(tx: &mut Transaction<'_, Postgres>, year: Uuid, term: Uuid) -> Result<(),AppError> {
    school_academic_core::services::lifecycle_guard::require_term_write_exclusive(tx,year,term).await
}
pub async fn lock_teacher_dates(connection: &mut PgConnection, keys: &[(Uuid,NaiveDate)]) -> Result<(),AppError> {
    for (teacher,date) in keys.iter().copied().collect::<BTreeSet<_>>() {
        DbSession::new(connection).query("academic_timetable.dated.lock_teacher_dates", "SELECT pg_advisory_xact_lock(hashtextextended($1,0))").bind(format!("teaching-duty:{teacher}:{date}")).execute().await?;
    }
    Ok(())
}
pub async fn snapshot_on_connection(connection: &mut PgConnection, year: Uuid, term: Uuid, teacher: Uuid, from: NaiveDate, to: NaiveDate, start: NaiveTime, end: NaiveTime) -> Result<DatedTeachingSnapshot,AppError> {
    if to < from || (to-from).num_days()>366 || start>=end { return Err(invalid("ช่วงวันและเวลาสำหรับจัดสอนไม่ถูกต้อง")); }
    let current = super::timetable_service::get_for_term_on_connection(connection,term).await?;
    let Some(current) = current else { return Ok(DatedTeachingSnapshot {source_digest:String::new(), timetable_state:"missing".into(), lessons:vec![]}); };
    if current.academic_year_id!=year { return Err(invalid("ภาคเรียนไม่อยู่ในปีการศึกษาที่เลือก")); }
    let blocks = super::attendance::current_pattern_on_connection(connection,term).await?;
    let source=school_academic_delivery::services::current::complete_workspace_on_connection(connection,year,term).await?;
    let mut lessons = vec![];
    for n in 0..=(to-from).num_days() {
        let date=from+chrono::Duration::days(n);
        let day=super::daily_teaching::day_code_from_date(date);
        for block in blocks.iter().filter(|b| b.day_of_week==day && intervals_overlap(start,end,b.start_time,b.end_time)) {
            for group in block.groups.iter().filter(|g| g.is_active && g.instructors.iter().any(|t| t.teacher_id==teacher)) {
                lessons.push(DatedTeachingLesson { lesson_key:lesson_key(date,group.id,teacher),date,block_id:block.id,allocation_id:group.id,teacher_id:teacher,bell_schedule_period_id:block.bell_schedule_period_id,period_name:block.period_name.clone(),start_time:block.start_time,end_time:block.end_time,learning_group_id:Some(group.learning_group_id),learning_group_name:Some(group.name.clone()),offering_id:block.learning_offering_id,offering_name:block.offering_name.clone(),grade_level_ids:source.offerings.iter().find(|o|Some(o.id)==block.learning_offering_id).map(|o|o.targets.iter().filter(|t|t.homeroom_id.is_none() || t.homeroom_id.is_some_and(|id|group.homeroom_ids.contains(&id))).map(|t|t.grade_level_id).collect()).unwrap_or_default(),homeroom_ids:group.homeroom_ids.clone(),homeroom_names:group.homeroom_names.clone(),room_id:group.room_id,room_name:group.room_name.clone(),source_revision:block.row_version,allocation_revision:group.row_version });
            }
            if !block.groups.iter().any(|g| g.is_active && g.instructors.iter().any(|t| t.teacher_id==teacher)) {
                for assigned in block.teachers.iter().filter(|t| t.is_active && t.teacher_id==teacher) {
                    lessons.push(DatedTeachingLesson { lesson_key:lesson_key(date,assigned.id,teacher),date,block_id:block.id,allocation_id:assigned.id,teacher_id:teacher,bell_schedule_period_id:block.bell_schedule_period_id,period_name:block.period_name.clone(),start_time:block.start_time,end_time:block.end_time,learning_group_id:None,learning_group_name:None,offering_id:block.learning_offering_id,offering_name:block.offering_name.clone().or(block.title.clone()),grade_level_ids:vec![],homeroom_ids:block.homerooms.iter().filter(|r|r.is_active).map(|r|r.homeroom_id).collect(),homeroom_names:block.homerooms.iter().filter(|r|r.is_active).map(|r|r.name.clone()).collect(),room_id:None,room_name:None,source_revision:block.row_version,allocation_revision:assigned.row_version });
                }
            }
        }
    }
    lessons.sort_by_key(|l|(l.date,l.start_time,l.allocation_id,l.teacher_id));
    let bytes=serde_json::to_vec(&(year,term,teacher,from,to,start,end,&lessons)).map_err(|_|invalid("ข้อมูลตารางเรียนไม่ถูกต้อง"))?;
    let source_digest=Sha256::digest(bytes).iter().map(|v|format!("{v:02x}")).collect();
    Ok(DatedTeachingSnapshot {source_digest,timetable_state:"ready".into(),lessons})
}
/// Occupancy is global across date-free terms and compares local intervals, not period IDs.
pub async fn teacher_free_on_connection(connection:&mut PgConnection, teacher:Uuid, date:NaiveDate, start:NaiveTime,end:NaiveTime,request:Uuid,ignored:&[String]) -> Result<bool,AppError> {
    let active:bool=DbSession::new(connection).scalar("academic_timetable.dated.active_teacher","SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND user_type='staff' AND status='active')").bind(teacher).fetch_one().await?;
    if !active {return Ok(false)}
    let reserved:bool=DbSession::new(connection).scalar("academic_timetable.dated.reserved_overlap","SELECT EXISTS(SELECT 1 FROM academic_teaching_coverage_duties WHERE teacher_id=$1 AND occurrence_date=$2 AND state<>'released' AND request_id<>$5 AND start_time<$4 AND $3<end_time)").bind(teacher).bind(date).bind(start).bind(end).bind(request).fetch_one().await?;
    if reserved {return Ok(false)}
    let day=super::daily_teaching::day_code_from_date(date);
    let allocations:Vec<(Uuid,Uuid)>=DbSession::new(connection).query_as("academic_timetable.dated.weekly_occupancy",r#"SELECT b.id,g.id FROM academic_timetable_blocks b JOIN academic_timetables t ON t.id=b.timetable_id JOIN bell_schedule_periods p ON p.id=b.bell_schedule_period_id JOIN academic_timetable_block_groups g ON g.block_id=b.id AND g.is_active JOIN academic_timetable_block_group_instructors i ON i.block_group_id=g.id AND i.instructor_id=$1 WHERE b.is_active AND b.day_of_week=$2 AND p.start_time<$4 AND $3<p.end_time
UNION SELECT b.id,bt.id FROM academic_timetable_blocks b JOIN academic_timetables t ON t.id=b.timetable_id JOIN bell_schedule_periods p ON p.id=b.bell_schedule_period_id JOIN academic_timetable_block_teachers bt ON bt.block_id=b.id AND bt.is_active AND bt.teacher_id=$1 WHERE b.is_active AND b.day_of_week=$2 AND p.start_time<$4 AND $3<p.end_time AND NOT EXISTS(SELECT 1 FROM academic_timetable_block_groups g JOIN academic_timetable_block_group_instructors i ON i.block_group_id=g.id WHERE g.block_id=b.id AND g.is_active AND i.instructor_id=$1)"#).bind(teacher).bind(day).bind(start).bind(end).fetch_all().await?;
    for (_,allocation) in allocations {
        if ignored.contains(&lesson_key(date,allocation,teacher)) {continue}
        let replaced:bool=DbSession::new(connection).scalar("academic_timetable.dated.replaced_occupancy","SELECT EXISTS(SELECT 1 FROM academic_teaching_coverage_duties WHERE occurrence_date=$1 AND source_allocation_id=$2 AND original_teacher_id=$3 AND state='approved' AND kind<>'absence')").bind(date).bind(allocation).bind(teacher).fetch_one().await?;
        if !replaced {return Ok(false)}
    }
    Ok(true)
}
pub async fn reserve_on_connection(connection:&mut PgConnection, request:Uuid,revision:i64,year:Uuid,term:Uuid,duties:&[DatedDutyInput]) -> Result<(),AppError> {
    let keys=duties.iter().map(|d|(d.teacher_id,d.date)).collect::<Vec<_>>();
    lock_teacher_dates(connection,&keys).await?;
    for (n,a) in duties.iter().enumerate() {
        if a.start_time>=a.end_time {return Err(invalid("เวลาสอนไม่ถูกต้อง"))}
        if duties[..n].iter().any(|b|a.teacher_id==b.teacher_id && a.date==b.date && intervals_overlap(a.start_time,a.end_time,b.start_time,b.end_time)) {return Err(invalid("ผู้สอนมีหน้าที่ซ้อนกันในแผน"))}
        if a.kind!="absence" {
            let ignored=duties.iter().filter_map(|d|d.lesson.as_ref()).map(|l|l.lesson_key.clone()).collect::<Vec<_>>();
            if !teacher_free_on_connection(connection,a.teacher_id,a.date,a.start_time,a.end_time,request,&ignored).await? {return Err(invalid("ผู้สอนที่เลือกไม่ว่างแล้ว กรุณาเลือกใหม่"))}
        } else {
            let overlap:bool=DbSession::new(connection).scalar("academic_timetable.dated.absence_conflict","SELECT EXISTS(SELECT 1 FROM academic_teaching_coverage_duties WHERE teacher_id=$1 AND occurrence_date=$2 AND state<>'released' AND request_id<>$3 AND start_time<$5 AND $4<end_time)").bind(a.teacher_id).bind(a.date).bind(request).bind(a.start_time).bind(a.end_time).fetch_one().await?;
            if overlap {return Err(invalid("ช่วงลาเวลานี้ซ้อนหน้าที่สอนที่รับไว้แล้ว"))}
        }
    }
    release_on_connection(connection,request,false).await?;
    for d in duties {
        DbSession::new(connection).query("academic_timetable.dated.reserve", "INSERT INTO academic_teaching_coverage_duties(id,request_id,plan_revision,academic_year_id,academic_term_id,occurrence_date,start_time,end_time,teacher_id,original_teacher_id,source_block_id,source_allocation_id,learning_group_id,room_id,kind,pair_id,pair_leg,state,source_snapshot,taught_offering_id,taught_learning_group_id,taught_activity) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,'reserved',$18,$19,$20,$21)")
        .bind(d.id).bind(request).bind(revision).bind(year).bind(term).bind(d.date).bind(d.start_time).bind(d.end_time).bind(d.teacher_id).bind(d.original_teacher_id).bind(d.lesson.as_ref().map(|l|l.block_id)).bind(d.lesson.as_ref().map(|l|l.allocation_id)).bind(d.lesson.as_ref().and_then(|l|l.learning_group_id)).bind(d.lesson.as_ref().and_then(|l|l.room_id)).bind(&d.kind).bind(d.pair_id).bind(d.pair_leg).bind(d.lesson.as_ref().map(sqlx::types::Json)).bind(d.taught_offering_id).bind(d.taught_learning_group_id).bind(&d.taught_activity).execute().await?;
    }
    Ok(())
}
pub async fn release_on_connection(connection:&mut PgConnection,request:Uuid,future_only:bool)->Result<(),AppError>{
    DbSession::new(connection).query("academic_timetable.dated.release","UPDATE academic_teaching_coverage_duties SET state='released' WHERE request_id=$1 AND state<>'released' AND (NOT $2 OR occurrence_date>=(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date)").bind(request).bind(future_only).execute().await?;Ok(())
}
pub async fn approve_on_connection(connection:&mut PgConnection,request:Uuid)->Result<(),AppError>{
    DbSession::new(connection).query("academic_timetable.dated.approve","UPDATE academic_teaching_coverage_duties SET state='approved' WHERE request_id=$1 AND state='reserved'").bind(request).execute().await?;Ok(())
}
pub async fn approved_on_connection(connection:&mut PgConnection,term:Uuid,date:NaiveDate)->Result<Vec<ApprovedDatedDuty>,AppError>{
    Ok(DbSession::new(connection).query_as("academic_timetable.dated.approved","SELECT id,teacher_id,original_teacher_id,source_snapshot,taught_offering_id,taught_learning_group_id,taught_activity FROM academic_teaching_coverage_duties WHERE academic_term_id=$1 AND occurrence_date=$2 AND state='approved' AND kind<>'absence' ORDER BY start_time,id").bind(term).bind(date).fetch_all().await?)
}
#[cfg(test)]
mod tests {
 use super::*;
 use chrono::Datelike;
 #[test] fn actual_intervals_exclude_touching_boundaries() {let t=|h,m|NaiveTime::from_hms_opt(h,m,0).unwrap();assert!(intervals_overlap(t(9,0),t(10,0),t(9,59),t(11,0)));assert!(!intervals_overlap(t(9,0),t(10,0),t(10,0),t(11,0)));}
 #[test] fn identity_includes_absent_instructor_and_civil_date(){let a=Uuid::new_v4();let b=Uuid::new_v4();let c=Uuid::new_v4();let d=NaiveDate::from_ymd_opt(2026,11,17).unwrap();assert_ne!(lesson_key(d,a,b),lesson_key(d,a,c));assert_ne!(lesson_key(d,a,b),lesson_key(d.succ_opt().unwrap(),a,b));assert_eq!(d.weekday().number_from_monday(),2);}
}
