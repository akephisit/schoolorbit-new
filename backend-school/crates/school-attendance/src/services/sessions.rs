use super::{audit, lock_term, notifications, settings};
use crate::{
    models::*,
    policy,
    rules::{invalid, validate_inputs},
};
use chrono::Utc;
use school_authorization::ActorContext;
use school_database::DbSession;
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
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.module_generation")
            .await?;
    module_generation_on_connection(&mut connection).await
}
pub(crate) async fn module_generation_on_connection(
    connection: &mut PgConnection,
) -> Result<Uuid, AppError> {
    Ok(DbSession::new(&mut *connection)
        .scalar(
            "attendance.services.sessions.module_generation_on_connection",
            "SELECT generation FROM attendance_module_state WHERE singleton",
        )
        .fetch_one()
        .await?)
}
pub async fn with_token(
    pool: &PgPool,
    s: AttendanceSession,
) -> Result<AttendanceSession, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.sessions.with_token")
            .await?;
    with_token_on_connection(&mut connection, s).await
}
pub(crate) async fn with_token_on_connection(
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
    get_optional_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.sessions.read")
            .await?,
        id,
    )
    .await
}
pub(crate) async fn get_optional_on_connection(
    connection: &mut PgConnection,
    id: Uuid,
) -> Result<Option<AttendanceSession>, AppError> {
    let sql = format!("SELECT {COLUMNS} FROM attendance_sessions WHERE id=$1");
    let row = DbSession::new(&mut *connection)
        .query_as(
            "attendance.services.sessions.get_optional_on_connection",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
        .await?;
    match row {
        Some(s) => {
            let generation = DbSession::new(&mut *connection)
                .scalar(
                    "attendance.services.sessions.get_optional_on_connection",
                    "SELECT generation FROM attendance_module_state WHERE singleton",
                )
                .fetch_one()
                .await?;
            Ok(Some(token_with_generation(s, generation)?))
        }
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
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.sessions.list")
            .await?;
    list_on_connection(&mut connection, actor, q).await
}
pub(crate) async fn list_on_connection(
    connection: &mut PgConnection,
    actor: &ActorContext,
    q: &AttendanceQuery,
) -> Result<Vec<AttendanceSession>, AppError> {
    let sql = format!(
        "SELECT {COLUMNS} FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND ($3 OR $4=ANY(teacher_ids)) ORDER BY start_time,title,id"
    );
    Ok(DbSession::new(&mut *connection)
        .query_as(
            "attendance.services.sessions.list",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(q.academic_term_id)
        .bind(q.date)
        .bind(policy::read_school(actor))
        .bind(actor.user_id)
        .fetch_all()
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
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.sessions.transaction").await?;
    lock_term(&mut tx, seed.session.academic_term_id, true, false).await?;
    insert_seed(&mut tx, &seed).await?;
    tx.commit().await?;
    let s = &seed.session;
    let sql = format!(
        "SELECT {COLUMNS} FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND kind=$3 AND source_key=$4"
    );
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.sessions.ensure")
            .await?;
    let found: AttendanceSession = DbSession::new(&mut *connection)
        .query_as(
            "attendance.services.sessions.ensure",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(s.academic_term_id)
        .bind(s.date)
        .bind(s.kind)
        .bind(&s.source_key)
        .fetch_one()
        .await?;
    policy::require_session(actor, &found, true)?;
    let generation = module_generation_on_connection(&mut connection).await?;
    token_with_generation(found, generation)
}
pub async fn insert_seed(
    tx: &mut Transaction<'_, Postgres>,
    seed: &AttendanceSeed,
) -> Result<(), AppError> {
    let s = &seed.session;
    if seed.students.len() > 2000 {
        return Err(invalid("รายชื่อเกินขนาดที่รองรับ"));
    }
    if s.kind == AttendanceKind::Lesson {
        let persisted: bool=DbSession::new(&mut **tx).scalar("attendance.services.sessions.insert_seed.accepted_source",
            "SELECT EXISTS(SELECT 1 FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND kind=$3 AND source_key=$4)")
            .bind(s.academic_term_id).bind(s.date).bind(s.kind).bind(&s.source_key).fetch_one().await?;
        if !persisted {
            let current = crate::application::resolve_seed_on_connection(
                &mut **tx,
                &OpenAttendanceSession {
                    academic_term_id: s.academic_term_id,
                    date: s.date,
                    kind: s.kind,
                    source_key: s.source_key.clone(),
                },
            )
            .await
            .map_err(|error| match error {
                AppError::NotFound(_) => {
                    AppError::Conflict("ตารางหรือแหล่งรายชื่อเปลี่ยนไป กรุณาโหลดรอบเช็คชื่อใหม่".into())
                }
                error => error,
            })?;
            let generation = module_generation_on_connection(&mut **tx).await?;
            if !same_seed_source(seed, &current, generation)? {
                return Err(AppError::Conflict(
                    "ตารางหรือแหล่งรายชื่อเปลี่ยนไป กรุณาโหลดรอบเช็คชื่อใหม่".into(),
                ));
            }
        }
    }
    insert_seed_row(tx, seed).await
}

pub(super) fn seed_source_fingerprint(
    seed: &AttendanceSeed,
    generation: Uuid,
) -> Result<String, AppError> {
    let mut source = seed.session.clone();
    source.roster = seed.students.clone();
    let source = token_with_generation(source, generation)?;
    let bytes = serde_json::to_vec(&(
        source.learning_group_id,
        source.offering_id,
        source.homeroom_id,
        source.special_round_id,
        source.count_override,
        &source.title,
        source.roster_token,
    ))
    .map_err(|_| invalid("สร้างข้อมูลยืนยันแหล่งรายชื่อไม่ได้"))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
pub(super) fn same_seed_source(
    left: &AttendanceSeed,
    right: &AttendanceSeed,
    generation: Uuid,
) -> Result<bool, AppError> {
    Ok(seed_source_fingerprint(left, generation)? == seed_source_fingerprint(right, generation)?)
}

pub(super) async fn insert_finalization_seed(
    tx: &mut Transaction<'_, Postgres>,
    seed: &AttendanceSeed,
    batch: &super::finalization::ValidatedFinalizationSeeds,
) -> Result<(), AppError> {
    if !batch.accepts(seed)? {
        return Err(AppError::Conflict(
            "แหล่งรายชื่อปิดยอดเปลี่ยนไป กรุณาประมวลผลใหม่".into(),
        ));
    }
    insert_seed_row(tx, seed).await
}

async fn insert_seed_row(
    tx: &mut Transaction<'_, Postgres>,
    seed: &AttendanceSeed,
) -> Result<(), AppError> {
    let s = &seed.session;
    DbSession::new(&mut **tx).query("attendance.services.sessions.insert_seed", "INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,homeroom_id,learning_group_id,offering_id,special_round_id,start_time,end_time,count_override,roster) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) ON CONFLICT(academic_term_id,date,kind,source_key) DO NOTHING")
        .bind(s.id).bind(s.academic_term_id).bind(s.date).bind(s.kind).bind(&s.source_key).bind(&s.title).bind(&s.teacher_ids)
        .bind(s.homeroom_id).bind(s.learning_group_id).bind(s.offering_id).bind(s.special_round_id).bind(s.start_time).bind(s.end_time)
        .bind(s.count_override).bind(Json(&seed.students)).execute().await?;
    Ok(())
}
pub async fn detail(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<AttendanceDetail, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.sessions.detail")
            .await?;
    let s = get_optional_on_connection(&mut connection, id)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบเช็คชื่อ".into()))?;
    policy::require_session(actor, &s, false)?;
    let context =
        settings::AttendanceReadContext::load(&mut connection, s.academic_term_id).await?;
    detail_with_context(&mut connection, s, &context).await
}
pub async fn detail_unchecked(
    pool: &PgPool,
    s: AttendanceSession,
) -> Result<AttendanceDetail, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.sessions.read")
            .await?;
    let context =
        settings::AttendanceReadContext::load(&mut connection, s.academic_term_id).await?;
    detail_with_context(&mut connection, s, &context).await
}
pub(crate) async fn detail_with_context(
    connection: &mut PgConnection,
    s: AttendanceSession,
    context: &settings::AttendanceReadContext,
) -> Result<AttendanceDetail, AppError> {
    let settings = &context.settings;
    let counted = match s.count_override {
        Some(value) => value,
        None => {
            settings::is_counted_on_connection(
                connection,
                s.academic_term_id,
                s.date,
                &settings.configuration,
            )
            .await?
        }
    } && !s.cancelled;
    let students=DbSession::new(&mut *connection).query_as("attendance.services.sessions.detail_with_context", r#"WITH participants AS (
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
        .bind(s.id).bind(s.academic_term_id).bind(s.date).bind(Json(&s.roster)).fetch_all().await?;
    let term_writable = context
        .term_writable
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
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
    let s: AttendanceSession = DbSession::new(&mut **tx)
        .query_as(
            "attendance.services.sessions.lock_session",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(id)
        .fetch_optional()
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
    let mut preflight =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.sessions.preflight")
            .await?;
    let epoch = module_generation_on_connection(&mut preflight).await?;
    let existing = get_optional_on_connection(&mut preflight, id).await?;
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
            with_token_on_connection(&mut preflight, s).await?
        }
    };
    drop(preflight);
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
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.sessions.transaction").await?;
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
        let actual: i64 = DbSession::new(&mut *tx)
            .scalar(
                "attendance.services.sessions.save_with_seed",
                "SELECT row_version FROM attendance_sessions WHERE id=$1 FOR UPDATE",
            )
            .bind(id)
            .fetch_one()
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
    let before:Vec<ResultSnapshot>=DbSession::new(&mut *tx).query_as("attendance.services.sessions.save_with_seed", "SELECT student_id,result,row_version FROM attendance_records WHERE session_id=$1 ORDER BY student_id FOR UPDATE").bind(id).fetch_all().await?;
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
    DbSession::new(&mut *tx).query("attendance.services.sessions.save_with_seed", r#"INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,special_round_id,class_number,result,origin,updated_by)
        SELECT $1,"studentId","studentAcademicYearId",$4,"classNumber",result,'teacher',$3
        FROM jsonb_to_recordset($2) AS i("studentId" uuid,"studentAcademicYearId" uuid,"classNumber" integer,result text)
        ON CONFLICT(session_id,student_id) DO UPDATE SET result=excluded.result,origin='teacher',
        updated_by=$3,processed_at=NULL,row_version=attendance_records.row_version+1"#).bind(id).bind(Json(&entries)).bind(actor.user_id).bind(s.special_round_id).execute().await?;
    if !changed.is_empty() {
        let after:Vec<ResultSnapshot>=DbSession::new(&mut *tx).query_as("attendance.services.sessions.save_with_seed", "SELECT student_id,result,row_version FROM attendance_records WHERE session_id=$1 ORDER BY student_id").bind(id).fetch_all().await?;
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
        DbSession::new(&mut *tx).query("attendance.services.sessions.save_with_seed", "UPDATE attendance_sessions SET saved_at=now(),saved_by=$2,row_version=row_version+1 WHERE id=$1").bind(id).bind(actor.user_id).execute().await?;
        DbSession::new(&mut *tx).query("attendance.services.sessions.save_with_seed", "INSERT INTO audit_logs(user_id,action,entity_type,entity_id,old_values,new_values,description) VALUES($1,'update','attendance',$2,$3,$4,$5)").bind(actor.user_id).bind(id).bind(Json(&before)).bind(Json(&after)).bind(&p.reason).execute().await?;
        super::score_changes::enqueue_day(&mut tx, s.academic_term_id, s.date).await?;
    }
    tx.commit().await?;
    detail(pool, actor, id).await
}
pub async fn module_generation_in_tx(tx: &mut Transaction<'_, Postgres>) -> Result<Uuid, AppError> {
    Ok(DbSession::new(&mut **tx)
        .scalar(
            "attendance.services.sessions.module_generation_in_tx",
            "SELECT generation FROM attendance_module_state WHERE singleton",
        )
        .fetch_one()
        .await?)
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
    let mut preflight =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.sessions.preflight")
            .await?;
    let epoch = module_generation_on_connection(&mut preflight).await?;
    let s = match get_optional_on_connection(&mut preflight, id).await? {
        Some(s) => s,
        None => {
            let seed = seed
                .as_ref()
                .ok_or_else(|| AppError::NotFound("ไม่พบรอบ".into()))?;
            let mut s = seed.session.clone();
            s.roster = seed.students.clone();
            with_token_on_connection(&mut preflight, s).await?
        }
    };
    drop(preflight);
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
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.sessions.transaction").await?;
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
    DbSession::new(&mut *tx).query("attendance.services.sessions.cancel_with_seed", "UPDATE attendance_sessions SET cancelled=$2,cancellation_reason=$3,row_version=row_version+1 WHERE id=$1").bind(id).bind(p.cancelled).bind(&p.reason).execute().await?;
    audit(&mut tx, actor.user_id, id, "update", &p.reason).await?;
    super::score_changes::enqueue_day(&mut tx, s.academic_term_id, s.date).await?;
    DbSession::new(&mut *tx).query("attendance.services.sessions.cancel_with_seed", "DELETE FROM attendance_finalized_dates WHERE generation=$1 AND academic_term_id=$2 AND date=$3")
        .bind(epoch).bind(s.academic_term_id).bind(s.date).execute().await?;
    tx.commit().await?;
    detail(pool, actor, id).await
}
