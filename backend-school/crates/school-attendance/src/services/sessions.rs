use super::{audit, lock_term, notifications, settings};
use crate::{
    models::*,
    policy,
    rules::{invalid, validate_inputs},
};
use chrono::Utc;
use school_authorization::ActorContext;
use school_errors::AppError;
use sha2::{Digest, Sha256};
use sqlx::{types::Json, PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;
pub const COLUMNS: &str = "id,academic_term_id,date,kind,source_key,title,teacher_ids,homeroom_id,learning_group_id,offering_id,special_round_id,start_time,end_time,count_override,cancelled,cancellation_reason,saved_at,saved_by,row_version,roster";
#[derive(serde::Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct ResultSnapshot {
    student_id: Uuid,
    result: AttendanceResult,
    row_version: i64,
}
pub async fn module_generation(pool: &PgPool) -> Result<Uuid, AppError> {
    module_generation_on_connection(&mut *pool.acquire().await?).await
}
pub async fn module_generation_on_connection(
    connection: &mut PgConnection,
) -> Result<Uuid, AppError> {
    Ok(
        sqlx::query_scalar("SELECT generation FROM attendance_module_state WHERE singleton")
            .fetch_one(&mut *connection)
            .await?,
    )
}
pub async fn with_token(
    pool: &PgPool,
    s: AttendanceSession,
) -> Result<AttendanceSession, AppError> {
    with_token_on_connection(&mut *pool.acquire().await?, s).await
}
pub async fn with_token_on_connection(
    connection: &mut PgConnection,
    s: AttendanceSession,
) -> Result<AttendanceSession, AppError> {
    let epoch = module_generation_on_connection(connection).await?;
    token_with_generation(s, epoch)
}
pub fn token_with_generation(
    mut s: AttendanceSession,
    epoch: Uuid,
) -> Result<AttendanceSession, AppError> {
    let mut roster = s.roster.clone();
    let mut teachers = s.teacher_ids.clone();
    teachers.sort();
    teachers.dedup();
    roster.sort_by_key(|r| r.student_id);
    let bytes = serde_json::to_vec(&(
        epoch,
        s.id,
        s.academic_term_id,
        s.date,
        s.kind,
        &s.source_key,
        teachers,
        s.start_time,
        s.end_time,
        roster,
    ))
    .map_err(|_| invalid("อ่านรายชื่ออ้างอิงไม่ได้"))?;
    s.roster_token = hex::encode(Sha256::digest(bytes));
    Ok(s)
}
pub async fn get_optional(pool: &PgPool, id: Uuid) -> Result<Option<AttendanceSession>, AppError> {
    get_optional_on_connection(&mut *pool.acquire().await?, id).await
}
pub async fn get_optional_on_connection(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Option<AttendanceSession>, AppError> {
    let sql = format!("SELECT {COLUMNS} FROM attendance_sessions WHERE id=$1");
    let row = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut *connection)
        .await?;
    match row {
        Some(s) => Ok(Some(with_token_on_connection(connection, s).await?)),
        None => Ok(None),
    }
}
pub async fn get(pool: &PgPool, id: Uuid) -> Result<AttendanceSession, AppError> {
    get_optional(pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบเช็คชื่อ".into()))
}
pub async fn list(
    pool: &PgPool,
    actor: &ActorContext,
    q: &AttendanceQuery,
) -> Result<Vec<AttendanceSession>, AppError> {
    let sql = format!(
        "SELECT {COLUMNS} FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND ($3 OR $4=ANY(teacher_ids)) ORDER BY start_time,title,id"
    );
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(q.academic_term_id)
        .bind(q.date)
        .bind(policy::read_school(actor))
        .bind(actor.user_id)
        .fetch_all(pool)
        .await?)
}
pub async fn ensure(
    pool: &PgPool,
    actor: &ActorContext,
    seed: AttendanceSeed,
) -> Result<AttendanceSession, AppError> {
    policy::require_session(actor, &seed.session, true)?;
    if seed.students.len() > 2000 {
        return Err(invalid("รอบเช็คชื่อเกิน 2000 คน กรุณาแบ่งกลุ่ม"));
    }
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, seed.session.academic_term_id, true, false).await?;
    insert_seed(&mut tx, &seed).await?;
    tx.commit().await?;
    let s = &seed.session;
    let sql = format!(
        "SELECT {COLUMNS} FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND kind=$3 AND source_key=$4"
    );
    let found: AttendanceSession = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(s.academic_term_id)
        .bind(s.date)
        .bind(s.kind)
        .bind(&s.source_key)
        .fetch_one(pool)
        .await?;
    policy::require_session(actor, &found, true)?;
    with_token(pool, found).await
}
pub async fn insert_seed(
    tx: &mut Transaction<'_, Postgres>,
    seed: &AttendanceSeed,
) -> Result<(), AppError> {
    let s = &seed.session;
    if seed.students.len() > 2000 {
        return Err(invalid("รายชื่อเกินขนาดที่รองรับ"));
    }
    sqlx::query("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,homeroom_id,learning_group_id,offering_id,special_round_id,start_time,end_time,count_override,roster) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) ON CONFLICT(academic_term_id,date,kind,source_key) DO NOTHING")
        .bind(s.id).bind(s.academic_term_id).bind(s.date).bind(s.kind).bind(&s.source_key).bind(&s.title).bind(&s.teacher_ids)
        .bind(s.homeroom_id).bind(s.learning_group_id).bind(s.offering_id).bind(s.special_round_id).bind(s.start_time).bind(s.end_time)
        .bind(s.count_override).bind(Json(&seed.students)).execute(&mut **tx).await?;
    Ok(())
}
pub async fn detail(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<AttendanceDetail, AppError> {
    let s = get(pool, id).await?;
    policy::require_session(actor, &s, false)?;
    detail_unchecked(pool, s).await
}
pub async fn detail_unchecked(
    pool: &PgPool,
    s: AttendanceSession,
) -> Result<AttendanceDetail, AppError> {
    let mut connection = pool.acquire().await?;
    let settings = settings::get_on_connection(&mut connection, s.academic_term_id).await?;
    detail_unchecked_on_connection(&mut connection, s, &settings).await
}
pub(crate) async fn detail_unchecked_on_connection(
    connection: &mut PgConnection,
    s: AttendanceSession,
    settings: &AttendanceSettings,
) -> Result<AttendanceDetail, AppError> {
    if settings.academic_term_id != s.academic_term_id {
        return Err(invalid("บริบทการตั้งค่าไม่ตรงกับรอบเช็คชื่อ"));
    }
    let counted = s.count_override.unwrap_or(
        settings::is_counted_on_connection(
            connection,
            s.academic_term_id,
            s.date,
            &settings.configuration,
        )
        .await?,
    ) && !s.cancelled;
    let students=sqlx::query_as(r#"WITH participants AS (
        SELECT * FROM jsonb_to_recordset($4) AS p("studentId" uuid,"studentAcademicYearId" uuid,"classNumber" integer)
    ), arrivals AS (
        SELECT a.student_id,min(a.observed_at) observed_at,
          (array_agg(a.result ORDER BY (a.result IN ('present','late')) DESC,a.result,arrival.id))[1] result FROM attendance_records a
        JOIN attendance_sessions arrival ON arrival.id=a.session_id
        WHERE arrival.academic_term_id=$2 AND arrival.date=$3 AND arrival.kind='arrival'
          AND NOT arrival.cancelled AND a.student_id IN (SELECT "studentId" FROM participants) GROUP BY a.student_id
    ) SELECT p."studentId" student_id,p."studentAcademicYearId" student_academic_year_id,
        concat_ws(' ',concat(u.title,u.first_name),u.last_name) display_name,p."classNumber" class_number,
        COALESCE(r.result,'unchecked') result,COALESCE(r.origin,'none') origin,
        r.observed_at,e.evidence_file_id,a.observed_at arrival_at,a.result arrival_result,r.processed_at,COALESCE(r.row_version,0) row_version
        FROM participants p JOIN users u ON u.id=p."studentId"
        LEFT JOIN attendance_records r ON r.session_id=$1 AND r.student_id=p."studentId"
        LEFT JOIN attendance_scan_events e ON e.session_id=$1 AND e.student_id=p."studentId"
        LEFT JOIN arrivals a ON a.student_id=p."studentId"
        ORDER BY p."classNumber" NULLS LAST,u.first_name COLLATE schoolorbit_thai,u.last_name COLLATE schoolorbit_thai,u.id"#)
        .bind(s.id).bind(s.academic_term_id).bind(s.date).bind(Json(&s.roster)).fetch_all(&mut *connection).await?;
    let term_writable:bool=sqlx::query_scalar("SELECT t.status NOT IN ('closed','cancelled') AND y.status NOT IN ('closed','archived') FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1").bind(s.academic_term_id).fetch_one(&mut *connection).await?;
    Ok(AttendanceDetail {
        writable: term_writable
            && !settings.archived
            && !s.cancelled
            && s.date
                <= Utc::now()
                    .with_timezone(&chrono_tz::Asia::Bangkok)
                    .date_naive(),
        session: s,
        students,
        counted,
    })
}
async fn lock_session(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    version: i64,
) -> Result<AttendanceSession, AppError> {
    let sql = format!("SELECT {COLUMNS} FROM attendance_sessions WHERE id=$1 FOR UPDATE");
    let s: AttendanceSession = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบเช็คชื่อ".into()))?;
    if s.row_version != version {
        return Err(AppError::Conflict(
            "มีการบันทึกหรือสแกนเพิ่มแล้ว กรุณาโหลดรายชื่อใหม่".into(),
        ));
    }
    Ok(s)
}
pub async fn save(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    p: SaveAttendanceResults,
) -> Result<AttendanceDetail, AppError> {
    save_with_seed(pool, actor, id, p, None).await
}
pub async fn save_with_seed(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    p: SaveAttendanceResults,
    seed: Option<AttendanceSeed>,
) -> Result<AttendanceDetail, AppError> {
    let epoch = module_generation(pool).await?;
    let existing = get_optional(pool, id).await?;
    let initial = match existing {
        Some(s) => s,
        None => {
            let mut s = seed
                .as_ref()
                .ok_or_else(|| AppError::NotFound("ไม่พบรอบเช็คชื่อ".into()))?
                .session
                .clone();
            s.roster = seed
                .as_ref()
                .ok_or_else(|| invalid("ไม่มีรายชื่ออ้างอิง"))?
                .students
                .clone();
            with_token(pool, s).await?
        }
    };
    policy::require_session(actor, &initial, true)?;
    if initial.id != id
        || initial.academic_term_id != p.session.academic_term_id
        || initial.date != p.session.date
        || initial.kind != p.session.kind
        || initial.source_key != p.session.source_key
        || initial.roster_token != p.roster_token
    {
        return Err(AppError::Conflict(
            "รายชื่อหรือบริบทเปลี่ยนแล้ว กรุณาเปิดรอบใหม่".into(),
        ));
    }
    if initial.date
        > Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive()
    {
        return Err(invalid("ยังบันทึกผลของวันในอนาคตไม่ได้"));
    }
    validate_inputs(
        &p.students,
        &initial
            .roster
            .iter()
            .map(|r| r.student_id)
            .collect::<Vec<_>>(),
    )?;
    if p.reason.len() > 1000 {
        return Err(invalid("เหตุผลยาวเกินกำหนด"));
    }
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, initial.academic_term_id, true, false).await?;
    if module_generation_in_tx(&mut tx).await? != epoch {
        return Err(AppError::Conflict("ข้อมูลโมดูลเปลี่ยนแล้ว กรุณาเปิดใหม่".into()));
    }
    if initial.row_version == 0 {
        if p.row_version != 0 {
            return Err(AppError::Conflict("รอบนี้ถูกเปลี่ยนแล้ว".into()));
        }
        let seed = seed
            .as_ref()
            .ok_or_else(|| invalid("ไม่มีรายชื่อสำหรับบันทึกครั้งแรก"))?;
        insert_seed(&mut tx, seed).await?;
        let actual: i64 = sqlx::query_scalar(
            "SELECT row_version FROM attendance_sessions WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if actual != 1 {
            return Err(AppError::Conflict("มีการบันทึกเพิ่มแล้ว กรุณาเปิดใหม่".into()));
        }
    }
    let s = lock_session(
        &mut tx,
        id,
        if initial.row_version == 0 {
            1
        } else {
            p.row_version
        },
    )
    .await?;
    policy::require_session(actor, &s, true)?;
    if s.cancelled {
        return Err(AppError::Conflict("รอบนี้งดเช็คชื่อ".into()));
    }
    let before:Vec<ResultSnapshot>=sqlx::query_as("SELECT student_id,result,row_version FROM attendance_records WHERE session_id=$1 ORDER BY student_id FOR UPDATE").bind(id).fetch_all(&mut *tx).await?;
    let old: std::collections::HashMap<_, _> =
        before.iter().map(|r| (r.student_id, r.result)).collect();
    let changed: Vec<_> = p
        .students
        .iter()
        .filter(|i| old.get(&i.student_id).is_none_or(|r| *r != i.result))
        .collect();
    if changed.iter().any(|i| old.contains_key(&i.student_id)) && p.reason.trim().is_empty() {
        return Err(invalid("ระบุเหตุผลเมื่อแก้ไขผลที่บันทึกแล้ว"));
    }
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ExplicitResult {
        student_id: Uuid,
        student_academic_year_id: Uuid,
        class_number: Option<i32>,
        result: AttendanceResult,
    }
    let entries: Vec<_> = changed
        .iter()
        .map(|i| {
            let r = s
                .roster
                .iter()
                .find(|r| r.student_id == i.student_id)
                .ok_or_else(|| invalid("นักเรียนไม่อยู่ในรายชื่ออ้างอิง"))?;
            Ok(ExplicitResult {
                student_id: i.student_id,
                student_academic_year_id: r.student_academic_year_id,
                class_number: r.class_number,
                result: i.result,
            })
        })
        .collect::<Result<_, AppError>>()?;
    sqlx::query(r#"INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,special_round_id,class_number,result,origin,updated_by)
        SELECT $1,"studentId","studentAcademicYearId",$4,"classNumber",result,'teacher',$3
        FROM jsonb_to_recordset($2) AS i("studentId" uuid,"studentAcademicYearId" uuid,"classNumber" integer,result text)
        ON CONFLICT(session_id,student_id) DO UPDATE SET result=excluded.result,origin='teacher',
        updated_by=$3,processed_at=NULL,row_version=attendance_records.row_version+1"#).bind(id).bind(Json(&entries)).bind(actor.user_id).bind(s.special_round_id).execute(&mut *tx).await?;
    if !changed.is_empty() {
        let after:Vec<ResultSnapshot>=sqlx::query_as("SELECT student_id,result,row_version FROM attendance_records WHERE session_id=$1 ORDER BY student_id").bind(id).fetch_all(&mut *tx).await?;
        let notices: Vec<_> = after
            .iter()
            .filter(|r| changed.iter().any(|i| i.student_id == r.student_id))
            .map(|r| {
                notifications::result_notice(
                    &s,
                    r.student_id,
                    r.result,
                    r.row_version,
                    old.contains_key(&r.student_id),
                )
            })
            .collect();
        if notifications::should_notify(&mut tx, &s).await? {
            notifications::queue_batch(&mut tx, s.academic_term_id, &notices).await?;
        }
        sqlx::query("UPDATE attendance_sessions SET saved_at=now(),saved_by=$2,row_version=row_version+1 WHERE id=$1").bind(id).bind(actor.user_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO audit_logs(user_id,action,entity_type,entity_id,old_values,new_values,description) VALUES($1,'update','attendance',$2,$3,$4,$5)").bind(actor.user_id).bind(id).bind(Json(&before)).bind(Json(&after)).bind(&p.reason).execute(&mut *tx).await?;
        super::score_changes::enqueue_day(&mut tx, s.academic_term_id, s.date).await?;
    }
    tx.commit().await?;
    detail(pool, actor, id).await
}
pub async fn module_generation_in_tx(tx: &mut Transaction<'_, Postgres>) -> Result<Uuid, AppError> {
    Ok(
        sqlx::query_scalar("SELECT generation FROM attendance_module_state WHERE singleton")
            .fetch_one(&mut **tx)
            .await?,
    )
}
pub async fn cancel(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    p: AttendanceCancellation,
) -> Result<AttendanceDetail, AppError> {
    cancel_with_seed(pool, actor, id, p, None).await
}
pub async fn cancel_with_seed(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    p: AttendanceCancellation,
    seed: Option<AttendanceSeed>,
) -> Result<AttendanceDetail, AppError> {
    let epoch = module_generation(pool).await?;
    let s = match get_optional(pool, id).await? {
        Some(s) => s,
        None => {
            let seed = seed
                .as_ref()
                .ok_or_else(|| AppError::NotFound("ไม่พบรอบ".into()))?;
            let mut s = seed.session.clone();
            s.roster = seed.students.clone();
            with_token(pool, s).await?
        }
    };
    policy::require_session(actor, &s, true)?;
    if s.roster_token != p.roster_token
        || s.academic_term_id != p.session.academic_term_id
        || s.date != p.session.date
        || s.kind != p.session.kind
        || s.source_key != p.session.source_key
    {
        return Err(AppError::Conflict("บริบทเปลี่ยนแล้ว กรุณาเปิดใหม่".into()));
    }
    if p.reason.trim().is_empty() || p.reason.len() > 1000 {
        return Err(invalid("ระบุเหตุผลงดหรือคืนรอบเช็คชื่อ"));
    }
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, s.academic_term_id, true, false).await?;
    if module_generation_in_tx(&mut tx).await? != epoch {
        return Err(AppError::Conflict("ข้อมูลโมดูลเปลี่ยนแล้ว กรุณาเปิดใหม่".into()));
    }
    if s.row_version == 0 {
        if p.row_version != 0 || id != s.id {
            return Err(AppError::Conflict("บริบทเปลี่ยนแล้ว".into()));
        }
        insert_seed(
            &mut tx,
            seed.as_ref().ok_or_else(|| invalid("ไม่มีรายชื่ออ้างอิง"))?,
        )
        .await?;
    }
    let s = lock_session(
        &mut tx,
        id,
        if s.row_version == 0 { 1 } else { p.row_version },
    )
    .await?;
    if token_with_generation(s.clone(), epoch)?.roster_token != p.roster_token {
        return Err(AppError::Conflict("รายชื่อเปลี่ยนแล้ว กรุณาเปิดใหม่".into()));
    }
    policy::require_session(actor, &s, true)?;
    sqlx::query("UPDATE attendance_sessions SET cancelled=$2,cancellation_reason=$3,row_version=row_version+1 WHERE id=$1").bind(id).bind(p.cancelled).bind(&p.reason).execute(&mut *tx).await?;
    audit(&mut tx, actor.user_id, id, "update", &p.reason).await?;
    super::score_changes::enqueue_day(&mut tx, s.academic_term_id, s.date).await?;
    sqlx::query("DELETE FROM attendance_finalized_dates WHERE generation=$1 AND academic_term_id=$2 AND date=$3")
        .bind(epoch).bind(s.academic_term_id).bind(s.date).execute(&mut *tx).await?;
    tx.commit().await?;
    detail(pool, actor, id).await
}
