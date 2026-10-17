//! Transactional Student Care operations. Private prose/answers and replay outcomes
//! are encrypted; searchable metadata remains deliberately small.
use crate::{
    models::*,
    policy::{self, CareAccess},
    scoring::{self, invalid},
};
use chrono::{Datelike, NaiveDate, Utc};
use school_errors::AppError;
use school_permissions::registry::codes;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{types::Json, FromRow, PgConnection, PgPool, Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

type Tx<'a> = Transaction<'a, Postgres>;
fn internal() -> AppError {
    AppError::InternalServerError("care_private_data_unavailable".into())
}
fn encrypt<T: Serialize>(value: &T) -> Result<String, AppError> {
    let text = serde_json::to_string(value).map_err(|_| internal())?;
    school_crypto::encrypt(&text).map_err(|_| internal())
}
fn decrypt<T: DeserializeOwned>(value: &str) -> Result<T, AppError> {
    let text = school_crypto::decrypt(value).map_err(|_| internal())?;
    serde_json::from_str(&text).map_err(|_| internal())
}
fn conflict() -> AppError {
    AppError::Conflict("ข้อมูลเปลี่ยนแล้ว กรุณาโหลดข้อมูลล่าสุด".into())
}
fn missing() -> AppError {
    AppError::NotFound("ไม่พบรายการดูแลช่วยเหลือ".into())
}
fn page(q: &CareListQuery) -> Result<i64, AppError> {
    let n = q.limit.unwrap_or(50);
    if !(1..=100).contains(&n) {
        Err(invalid("จำนวนรายการต้องอยู่ระหว่าง 1 ถึง 100"))
    } else {
        Ok(n)
    }
}
fn ids(values: &[Uuid], max: usize) -> Result<Vec<Uuid>, AppError> {
    let sorted: Vec<_> = values
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if sorted.is_empty() || sorted.len() != values.len() || sorted.len() > max {
        Err(invalid("รายชื่อว่าง ซ้ำ หรือเกินกำหนด"))
    } else {
        Ok(sorted)
    }
}
fn age_at(birth: NaiveDate, date: NaiveDate) -> i32 {
    date.year()
        - birth.year()
        - i32::from((date.month(), date.day()) < (birth.month(), birth.day()))
}
fn enum_text<T: Serialize>(value: T) -> Result<String, AppError> {
    let value = serde_json::to_value(value).map_err(|_| internal())?;
    value.as_str().map(str::to_owned).ok_or_else(internal)
}
async fn receipt<T: DeserializeOwned, P: Serialize>(
    tx: &mut Tx<'_>,
    a: CareAccess,
    id: Uuid,
    operation: &str,
    payload: &P,
) -> Result<Option<T>, AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("care:{}:{}", a.actor_id, id))
        .execute(&mut **tx)
        .await?;
    let input = serde_json::to_vec(payload).map_err(|_| internal())?;
    let digest = hex::encode(Sha256::digest(&input));
    let old:Option<(String,String,String)>=sqlx::query_as("SELECT operation,digest,outcome_ciphertext FROM care_mutation_receipts WHERE actor_id=$1 AND request_id=$2").bind(a.actor_id).bind(id).fetch_optional(&mut **tx).await?;
    match old {
        Some((op, hash, result)) if op == operation && hash == digest => {
            Ok(Some(decrypt(&result)?))
        }
        Some(_) => Err(AppError::Conflict("requestId ถูกใช้กับข้อมูลอื่นแล้ว".into())),
        None => Ok(None),
    }
}
async fn finish<T: Serialize, P: Serialize>(
    mut tx: Tx<'_>,
    a: CareAccess,
    id: Uuid,
    operation: &str,
    payload: &P,
    result: &T,
    resource: Uuid,
    revision: i64,
) -> Result<(), AppError> {
    let digest = hex::encode(Sha256::digest(
        serde_json::to_vec(payload).map_err(|_| internal())?,
    ));
    sqlx::query("INSERT INTO care_mutation_receipts(actor_id,request_id,operation,digest,outcome_ciphertext) VALUES($1,$2,$3,$4,$5)").bind(a.actor_id).bind(id).bind(operation).bind(digest).bind(encrypt(result)?).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO care_audit(actor_id,resource_id,action,revision) VALUES($1,$2,$3,$4)")
        .bind(a.actor_id)
        .bind(resource)
        .bind(operation)
        .bind(revision)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
async fn audit_read(
    connection: &mut PgConnection,
    a: CareAccess,
    resource: Uuid,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO care_audit(actor_id,resource_id,action,revision) VALUES($1,$2,'private_read',0)").bind(a.actor_id).bind(resource).execute(connection).await?;
    Ok(())
}
async fn active_staff(connection: &mut PgConnection, selected: &[Uuid]) -> Result<(), AppError> {
    let found:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM users WHERE id=ANY($1) AND user_type='staff' AND status='active' ORDER BY id FOR SHARE").bind(selected).fetch_all(connection).await?;
    if found.len() != selected.len() {
        Err(invalid("ผู้รับมอบหมายต้องเป็นบุคลากรที่ใช้งานอยู่"))
    } else {
        Ok(())
    }
}
async fn sensitive_staff(pool: &PgPool, selected: &[Uuid]) -> Result<(), AppError> {
    let mut eligible = BTreeSet::new();
    let mut managers = BTreeSet::new();
    for permission in [
        codes::STUDENT_CARE_SENSITIVE_READ_ASSIGNED,
        codes::STUDENT_CARE_SENSITIVE_READ_SCHOOL,
        codes::STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED,
        codes::STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL,
    ] {
        let found =
            school_authorization::staff_users_with_all_permissions(pool, &[permission]).await?;
        if permission == codes::STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED
            || permission == codes::STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL
        {
            managers.extend(found.iter().copied());
        }
        eligible.extend(found);
    }
    if selected.iter().all(|id| eligible.contains(id))
        && selected.iter().any(|id| managers.contains(id))
    {
        Ok(())
    } else {
        Err(invalid("ผู้ดูแลทุกคนต้องมีสิทธิ์ข้อมูลลับ และอย่างน้อยหนึ่งคนต้องมีสิทธิ์จัดการ"))
    }
}
async fn notify(
    tx: &mut Tx<'_>,
    recipient: Uuid,
    key: String,
    link: String,
) -> Result<(), AppError> {
    let item = school_notifications::dispatch::QueuedNotification {
        id: Uuid::now_v7(),
        notification_id: Uuid::now_v7(),
        recipient_id: recipient,
        source: school_notifications::dispatch::NotificationSource::Direct,
        dedup_key: Some(key),
        title: "งานดูแลช่วยเหลือที่ต้องตรวจสอบ".into(),
        message: "มีรายการที่มอบหมายให้คุณ กรุณาเปิดระบบเพื่อตรวจสอบ".into(),
        type_: school_notifications::publisher::NotificationType::Warning,
        link: Some(link),
    };
    school_notifications::dispatch::store_and_enqueue_in_transaction(tx, &item).await?;
    Ok(())
}

pub async fn instruments(
    pool: &PgPool,
    a: CareAccess,
    q: CareListQuery,
) -> Result<Vec<CareInstrumentVersion>, AppError> {
    a.require_read()?;
    let limit = page(&q)?;
    Ok(sqlx::query_scalar::<_,Json<CareInstrumentVersion>>("SELECT document FROM care_instrument_versions WHERE ($1::uuid IS NULL OR id>$1) ORDER BY id LIMIT $2").bind(q.after_id).bind(limit).fetch_all(pool).await?.into_iter().map(|v|v.0).collect())
}
pub async fn instrument(
    pool: &PgPool,
    a: CareAccess,
    id: Uuid,
) -> Result<CareInstrumentVersion, AppError> {
    a.require_read()?;
    let mut c = pool.acquire().await?;
    load_instrument(&mut c, id).await
}
async fn load_instrument(
    c: &mut PgConnection,
    id: Uuid,
) -> Result<CareInstrumentVersion, AppError> {
    sqlx::query_scalar::<_, Json<CareInstrumentVersion>>(
        "SELECT document FROM care_instrument_versions WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(c)
    .await?
    .map(|v| v.0)
    .ok_or_else(missing)
}
pub async fn create_instrument(
    pool: &PgPool,
    a: CareAccess,
    p: CreateCareInstrumentVersion,
) -> Result<CareInstrumentVersion, AppError> {
    a.require_configure()?;
    scoring::validate_definition(&p.definition)?;
    let mut tx = pool.begin().await?;
    if let Some(result) = receipt(&mut tx, a, p.request_id, "instrument_create", &p).await? {
        return Ok(result);
    }
    let instrument_id = p.instrument_id.unwrap_or_else(Uuid::now_v7);
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("care-instrument:{instrument_id}"))
        .execute(&mut *tx)
        .await?;
    let latest: Option<i32> = sqlx::query_scalar(
        "SELECT max(version) FROM care_instrument_versions WHERE instrument_id=$1",
    )
    .bind(instrument_id)
    .fetch_one(&mut *tx)
    .await?;
    if latest != p.expected_version || (latest.is_none() && p.instrument_id.is_some()) {
        return Err(conflict());
    }
    let result = CareInstrumentVersion {
        id: Uuid::now_v7(),
        instrument_id,
        version: latest.unwrap_or(0) + 1,
        definition: p.definition.clone(),
        created_at: Utc::now(),
    };
    sqlx::query("INSERT INTO care_instrument_versions(id,instrument_id,version,document,created_by) VALUES($1,$2,$3,$4,$5)").bind(result.id).bind(result.instrument_id).bind(result.version).bind(Json(&result)).bind(a.actor_id).execute(&mut *tx).await?;
    finish(
        tx,
        a,
        p.request_id,
        "instrument_create",
        &p,
        &result,
        result.id,
        i64::from(result.version),
    )
    .await?;
    Ok(result)
}
pub async fn rounds(
    pool: &PgPool,
    a: CareAccess,
    q: CareListQuery,
) -> Result<Vec<CareRound>, AppError> {
    a.require_read()?;
    let limit = page(&q)?;
    Ok(sqlx::query_scalar::<_,Json<CareRound>>("SELECT r.document FROM care_rounds r WHERE ($1::uuid IS NULL OR r.id>$1) AND ($2::text IS NULL OR r.document->>'status'=$2) AND ($3 OR EXISTS(SELECT 1 FROM care_assignments ca WHERE ca.round_id=r.id AND (($4 AND ca.respondent_id=$5) OR ($6 AND $5=ANY(ca.reviewer_ids))))) ORDER BY r.id LIMIT $7").bind(q.after_id).bind(q.status).bind(a.routine_school()||a.routine_assigned()||a.configure||a.sensitive_school()).bind(a.own).bind(a.actor_id).bind(a.sensitive_assigned()).bind(limit).fetch_all(pool).await?.into_iter().map(|v|v.0).collect())
}
async fn load_round(c: &mut PgConnection, id: Uuid, lock: bool) -> Result<CareRound, AppError> {
    let sql = if lock {
        "SELECT document FROM care_rounds WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT document FROM care_rounds WHERE id=$1"
    };
    sqlx::query_scalar::<_, Json<CareRound>>(sql)
        .bind(id)
        .fetch_optional(c)
        .await?
        .map(|v| v.0)
        .ok_or_else(missing)
}
pub async fn create_round(
    pool: &PgPool,
    a: CareAccess,
    p: CreateCareRound,
) -> Result<CareRound, AppError> {
    a.require_manage()?;
    if !a.manage_school {
        return Err(policy::denied());
    }
    scoring::text(&p.name, 200, true)?;
    scoring::text(&p.purpose, 5000, true)?;
    let versions = ids(&p.instrument_version_ids, 30)?;
    if p.due_date < p.roster_date || (p.due_date - p.roster_date).num_days() > 366 {
        return Err(invalid("ช่วงรอบประเมินไม่ถูกต้อง"));
    }
    let mut tx = pool.begin().await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut tx).await?;
    if let Some(v) = receipt(&mut tx, a, p.request_id, "round_create", &p).await? {
        return Ok(v);
    }
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id=$1)")
        .bind(p.academic_term_id)
        .fetch_one(&mut *tx)
        .await?;
    if !valid {
        return Err(invalid("ไม่พบภาคเรียน"));
    }
    for id in &versions {
        load_instrument(&mut tx, *id).await?;
    }
    let result = CareRound {
        id: Uuid::now_v7(),
        name: p.name.clone(),
        purpose: p.purpose.clone(),
        instrument_version_ids: versions.clone(),
        academic_term_id: p.academic_term_id,
        roster_date: p.roster_date,
        due_date: p.due_date,
        status: CareRoundStatus::Open,
        row_version: 1,
        created_at: Utc::now(),
    };
    sqlx::query(
        "INSERT INTO care_rounds(id,academic_term_id,document,created_by) VALUES($1,$2,$3,$4)",
    )
    .bind(result.id)
    .bind(result.academic_term_id)
    .bind(Json(&result))
    .bind(a.actor_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO care_round_versions(round_id,version_id) SELECT $1,unnest($2::uuid[])",
    )
    .bind(result.id)
    .bind(versions)
    .execute(&mut *tx)
    .await?;
    finish(
        tx,
        a,
        p.request_id,
        "round_create",
        &p,
        &result,
        result.id,
        1,
    )
    .await?;
    Ok(result)
}
pub async fn update_round(
    pool: &PgPool,
    a: CareAccess,
    id: Uuid,
    p: UpdateCareRound,
) -> Result<CareRound, AppError> {
    if !a.manage_school {
        return Err(policy::denied());
    }
    let mut tx = pool.begin().await?;
    if let Some(v) = receipt(&mut tx, a, p.request_id, "round_update", &(id, &p)).await? {
        return Ok(v);
    }
    let mut r = load_round(&mut tx, id, true).await?;
    if r.row_version != p.row_version
        || r.status == CareRoundStatus::Cancelled
        || p.status == CareRoundStatus::Open
    {
        return Err(conflict());
    }
    r.status = p.status;
    r.row_version += 1;
    if p.status == CareRoundStatus::Cancelled {
        sqlx::query("UPDATE care_assignments SET document=jsonb_set(jsonb_set(document,'{status}','\"cancelled\"'),'{rowVersion}',to_jsonb((document->>'rowVersion')::bigint+1)) WHERE round_id=$1 AND document->>'status' IN('assigned','in_progress')").bind(id).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE care_rounds SET document=$2 WHERE id=$1")
        .bind(id)
        .bind(Json(&r))
        .execute(&mut *tx)
        .await?;
    finish(
        tx,
        a,
        p.request_id,
        "round_update",
        &(id, &p),
        &r,
        id,
        r.row_version,
    )
    .await?;
    Ok(r)
}
pub async fn students(
    pool: &PgPool,
    a: CareAccess,
    q: CareStudentQuery,
) -> Result<Vec<CareStudentOption>, AppError> {
    if !a.sensitive_manage_school {
        a.require_manage()?;
    }
    let limit = page(&CareListQuery {
        limit: q.limit,
        ..Default::default()
    })?;
    if let Some(s) = &q.search {
        scoring::text(s, 120, false)?;
    }
    let mut c = pool.acquire().await?;
    let rows = school_academic_core::services::dated_student_directory::students_on_connection(
        &mut c,
        q.academic_term_id,
        q.date,
    )
    .await?;
    let search = q.search.unwrap_or_default().to_lowercase();
    Ok(rows
        .into_iter()
        .filter(|s| {
            a.manage_school || a.sensitive_manage_school || s.advisor_ids.contains(&a.actor_id)
        })
        .filter(|s| q.after_id.is_none_or(|id| s.id > id))
        .filter(|s| {
            search.is_empty()
                || s.name.to_lowercase().contains(&search)
                || s.student_code
                    .as_ref()
                    .is_some_and(|c| c.to_lowercase().contains(&search))
        })
        .take(limit as usize)
        .map(|s| CareStudentOption {
            id: s.id,
            name: s.name,
            student_code: s.student_code,
            homeroom: s.homeroom,
            advisor_ids: s.advisor_ids,
            guardian_ids: s.parent_ids,
        })
        .collect())
}
pub async fn staff(pool: &PgPool, a: CareAccess) -> Result<Vec<CareStaffOption>, AppError> {
    a.require_manage()
        .or_else(|_| a.require_sensitive_manage())?;
    let rows:Vec<(Uuid,String)>=sqlx::query_as("SELECT id,display_name FROM user_display_names WHERE user_type='staff' AND status='active' ORDER BY display_name,id LIMIT 1001").fetch_all(pool).await?;
    if rows.len() > 1000 {
        return Err(invalid("รายชื่อบุคลากรเกินขนาดที่รองรับ"));
    }
    Ok(rows
        .into_iter()
        .map(|(id, name)| CareStaffOption { id, name })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredResponse {
    answers: Vec<CareAnswer>,
    manual_scores: Vec<CareManualScore>,
    manual_note: Option<String>,
    results: Vec<CareScaleResult>,
}
#[derive(FromRow)]
struct AssignmentRow {
    created_by: Uuid,
    document: Json<CareAssignment>,
    reviewer_ids: Vec<Uuid>,
    response_ciphertext: String,
    purpose_acknowledged_at: Option<chrono::DateTime<Utc>>,
}
async fn load_assignment(
    c: &mut PgConnection,
    id: Uuid,
    lock: bool,
) -> Result<AssignmentRow, AppError> {
    let query = if lock {
        "SELECT created_by,document,reviewer_ids,response_ciphertext,purpose_acknowledged_at FROM care_assignments WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT created_by,document,reviewer_ids,response_ciphertext,purpose_acknowledged_at FROM care_assignments WHERE id=$1"
    };
    sqlx::query_as(query)
        .bind(id)
        .fetch_optional(c)
        .await?
        .ok_or_else(missing)
}
async fn own_assignment(
    c: &mut PgConnection,
    a: CareAccess,
    r: &CareAssignment,
) -> Result<bool, AppError> {
    if !a.can_response(r.respondent_id) {
        return Ok(false);
    }
    if r.respondent == CareRespondent::Guardian {
        school_students::guardian_relationships::require_active_guardian_on_connection(
            c,
            r.student_id,
            a.actor_id,
        )
        .await
        .map_err(|error| match error {
            AppError::Forbidden(_) => missing(),
            other => other,
        })?;
    }
    Ok(true)
}
async fn assignment_detail(
    c: &mut PgConnection,
    a: CareAccess,
    row: AssignmentRow,
) -> Result<CareAssignmentDetail, AppError> {
    if !a.can_review(&row.reviewer_ids) && !own_assignment(c, a, &row.document).await? {
        return Err(missing());
    }
    let r = row.document.0;
    let instrument = load_instrument(c, r.instrument_version_id).await?;
    let round = load_round(c, r.round_id, false).await?;
    let private: StoredResponse = decrypt(&row.response_ciphertext)?;
    Ok(CareAssignmentDetail {
        assignment: r,
        instrument,
        answers: private.answers,
        manual_scores: private.manual_scores,
        results: private.results,
        manual_note: private.manual_note,
        purpose: round.purpose,
        purpose_acknowledged_at: row.purpose_acknowledged_at,
    })
}
pub async fn assignments(
    pool: &PgPool,
    a: CareAccess,
    q: CareListQuery,
) -> Result<Vec<CareAssignment>, AppError> {
    a.require_read()?;
    let limit = page(&q)?;
    let rows=sqlx::query_scalar::<_,Json<CareAssignment>>("SELECT ca.document FROM care_assignments ca WHERE ($1::uuid IS NULL OR ca.id>$1) AND ($2::uuid IS NULL OR ca.round_id=$2) AND ($3::uuid IS NULL OR ca.student_id=$3) AND ($4::text IS NULL OR ca.document->>'status'=$4) AND (($5 AND ca.respondent_id=$6 AND (ca.document->>'respondent'<>'guardian' OR EXISTS(SELECT 1 FROM student_parents p JOIN users u ON u.id=p.parent_user_id AND u.status='active' WHERE p.parent_user_id=$6 AND p.student_user_id=ca.student_id))) OR $7 OR ($8 AND $6=ANY(ca.reviewer_ids)) OR $10 OR ($11 AND ca.created_by=$6)) AND ($12::uuid IS NULL OR ca.respondent_id=$12) ORDER BY ca.id LIMIT $9")
 .bind(q.after_id).bind(q.round_id).bind(q.student_id).bind(q.status).bind(a.own).bind(a.actor_id).bind(a.sensitive_school()).bind(a.sensitive_assigned()).bind(limit).bind(a.routine_school()).bind(a.routine_assigned()).bind(q.respondent_id).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let mut item = r.0;
            if !a.can_review(&item.reviewer_user_ids) && !a.can_response(item.respondent_id) {
                item.urgent = false;
                item.acknowledged_at = None;
            }
            item
        })
        .collect())
}
pub async fn get_assignment(
    pool: &PgPool,
    a: CareAccess,
    id: Uuid,
) -> Result<CareAssignmentDetail, AppError> {
    let mut tx = pool.begin().await?;
    let row = load_assignment(&mut tx, id, false).await?;
    let result = assignment_detail(&mut tx, a, row).await?;
    audit_read(&mut tx, a, id).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn create_assignments(
    pool: &PgPool,
    a: CareAccess,
    p: CreateCareAssignments,
) -> Result<Vec<CareAssignment>, AppError> {
    a.require_manage()?;
    let reviewers = ids(&p.reviewer_user_ids, 30)?;
    if p.targets.is_empty() || p.targets.len() > 100 {
        return Err(invalid("ต้องเลือกผู้ตอบ 1 ถึง 100 รายการ"));
    }
    let mut unique = BTreeSet::new();
    if p.targets
        .iter()
        .any(|t| !unique.insert((t.student_id, t.respondent_id)))
    {
        return Err(invalid("รายชื่อผู้ตอบซ้ำ"));
    }
    sensitive_staff(pool, &reviewers).await?;
    let mut tx = pool.begin().await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut tx).await?;
    if let Some(v) = receipt(&mut tx, a, p.request_id, "assignments_create", &p).await? {
        return Ok(v);
    }
    let round = load_round(&mut tx, p.round_id, true).await?;
    if round.status != CareRoundStatus::Open
        || !round
            .instrument_version_ids
            .contains(&p.instrument_version_id)
    {
        return Err(conflict());
    }
    active_staff(&mut tx, &reviewers).await?;
    let instrument = load_instrument(&mut tx, p.instrument_version_id).await?;
    let selected: Vec<_> = p.targets.iter().map(|t| t.student_id).collect();
    let roster =
        school_academic_core::services::dated_student_directory::locked_students_on_connection(
            &mut tx,
            round.academic_term_id,
            round.roster_date,
            &selected,
        )
        .await?;
    let mut result = Vec::new();
    for target in &p.targets {
        let student = roster
            .iter()
            .find(|s| s.id == target.student_id)
            .ok_or_else(|| invalid("นักเรียนไม่อยู่ในรายชื่อ ณ วันที่รอบประเมิน"))?;
        if !a.manage_school && !student.advisor_ids.contains(&a.actor_id) {
            return Err(policy::denied());
        }
        let birth:Option<NaiveDate>=sqlx::query_scalar("SELECT date_of_birth FROM users WHERE id=$1 AND user_type='student' AND status='active' FOR SHARE").bind(student.id).fetch_optional(&mut *tx).await?.flatten();
        let age = age_at(
            birth.ok_or_else(|| invalid("ต้องมีวันเกิดเพื่อยืนยันช่วงอายุแบบประเมิน"))?,
            round.roster_date,
        );
        if age < instrument.definition.minimum_age || age > instrument.definition.maximum_age {
            return Err(invalid("อายุนักเรียนไม่ตรงรุ่นแบบประเมิน"));
        }
        match instrument.definition.respondent {
            CareRespondent::SelfReport => {
                if target.respondent_id != student.id {
                    return Err(invalid("แบบผู้เรียนต้องตอบโดยผู้เรียนที่ได้รับมอบหมาย"));
                }
            }
            CareRespondent::Teacher => {
                if !student.advisor_ids.contains(&target.respondent_id) {
                    return Err(invalid("ผู้ตอบครูต้องเป็นที่ปรึกษาตามรายชื่อ ณ วันที่เลือก"));
                }
                active_staff(&mut tx, &[target.respondent_id]).await?;
            }
            CareRespondent::Guardian => {
                school_students::guardian_relationships::require_active_guardian_on_connection(
                    &mut tx,
                    student.id,
                    target.respondent_id,
                )
                .await?
            }
        }
        let r = CareAssignment {
            id: Uuid::now_v7(),
            round_id: round.id,
            student_id: student.id,
            student_name: student.name.clone(),
            respondent_id: target.respondent_id,
            respondent: instrument.definition.respondent,
            instrument_version_id: instrument.id,
            instrument_name: instrument.definition.name.clone(),
            reviewer_user_ids: reviewers.clone(),
            status: CareAssignmentStatus::Assigned,
            row_version: 1,
            submitted_at: None,
            acknowledged_at: None,
            urgent: false,
        };
        sqlx::query("INSERT INTO care_assignments(id,round_id,student_id,respondent_id,instrument_version_id,reviewer_ids,document,response_ciphertext,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(r.id).bind(r.round_id).bind(r.student_id).bind(r.respondent_id).bind(r.instrument_version_id).bind(&reviewers).bind(Json(&r)).bind(encrypt(&StoredResponse::default())?).bind(a.actor_id).execute(&mut *tx).await?;
        result.push(r);
    }
    finish(
        tx,
        a,
        p.request_id,
        "assignments_create",
        &p,
        &result,
        p.round_id,
        round.row_version,
    )
    .await?;
    Ok(result)
}
pub async fn save_response(
    pool: &PgPool,
    a: CareAccess,
    id: Uuid,
    p: SaveCareResponse,
    submit: bool,
) -> Result<CareAssignmentDetail, AppError> {
    let operation = if submit {
        "response_submit"
    } else {
        "response_draft"
    };
    let mut tx = pool.begin().await?;
    // Lock the round before assignment everywhere to serialize close/cancel with submission.
    let round_id: Uuid = sqlx::query_scalar("SELECT round_id FROM care_assignments WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(missing)?;
    let round = load_round(&mut tx, round_id, true).await?;
    let mut row = load_assignment(&mut tx, id, true).await?;
    let version = load_instrument(&mut tx, row.document.instrument_version_id).await?;
    if version.definition.mode == CareInstrumentMode::ManualResult {
        if !a.can_manage_case(&row.reviewer_ids) {
            return Err(policy::denied());
        }
    } else if !own_assignment(&mut tx, a, &row.document).await? {
        return Err(policy::denied());
    }
    if let Some(v) = receipt(&mut tx, a, p.request_id, operation, &(id, &p)).await? {
        return Ok(v);
    }
    if round.status != CareRoundStatus::Open
        || row.document.row_version != p.row_version
        || !matches!(
            row.document.status,
            CareAssignmentStatus::Assigned | CareAssignmentStatus::InProgress
        )
    {
        return Err(conflict());
    }
    scoring::text(p.manual_note.as_deref().unwrap_or(""), 5000, false)?;
    if version.definition.mode == CareInstrumentMode::Digital {
        scoring::validate_answers(&version.definition, &p.answers, submit)?;
        if !p.manual_scores.is_empty() || p.manual_note.is_some() {
            return Err(invalid("แบบดิจิทัลไม่รับคะแนนหรือหมายเหตุผลด้วยตนเอง"));
        }
    } else {
        if submit {
            scoring::text(p.manual_note.as_deref().unwrap_or(""), 5000, true)?;
        }
        if !p.answers.is_empty() || p.manual_scores.len() > 30 {
            return Err(invalid("คะแนนแบบประเมินไม่ถูกต้อง"));
        }
        let mut keys = BTreeSet::new();
        for score in &p.manual_scores {
            if !keys.insert(&score.scale_key)
                || !version.definition.scales.iter().any(|s| {
                    s.key == score.scale_key
                        && s.source_scale_keys.is_empty()
                        && s.bands
                            .iter()
                            .any(|b| score.score >= b.minimum && score.score <= b.maximum)
                })
            {
                return Err(invalid("คะแนนนอกช่วงหรือมาตรวัดซ้ำ"));
            }
        }
    }
    if submit && !p.acknowledge_purpose && row.purpose_acknowledged_at.is_none() {
        return Err(invalid("กรุณารับทราบวัตถุประสงค์ก่อนส่งแบบประเมิน"));
    }
    let (results, urgent) = if submit {
        scoring::score(&version.definition, &p.answers, &p.manual_scores)?
    } else {
        (Vec::new(), false)
    };
    if p.acknowledge_purpose && row.purpose_acknowledged_at.is_none() {
        row.purpose_acknowledged_at = Some(Utc::now());
    }
    row.response_ciphertext = encrypt(&StoredResponse {
        answers: p.answers.clone(),
        manual_scores: p.manual_scores.clone(),
        manual_note: p.manual_note.clone(),
        results,
    })?;
    row.document.row_version += 1;
    row.document.status = if submit {
        CareAssignmentStatus::Submitted
    } else {
        CareAssignmentStatus::InProgress
    };
    row.document.urgent = urgent;
    if submit {
        row.document.submitted_at = Some(Utc::now());
    }
    sqlx::query("UPDATE care_assignments SET document=$2,response_ciphertext=$3,purpose_acknowledged_at=$4 WHERE id=$1").bind(id).bind(&row.document).bind(&row.response_ciphertext).bind(row.purpose_acknowledged_at).execute(&mut *tx).await?;
    if submit && urgent {
        for reviewer in &row.reviewer_ids {
            notify(
                &mut tx,
                *reviewer,
                format!("care:urgent:{id}:{reviewer}"),
                format!("/student-care/assignments/{id}"),
            )
            .await?;
        }
    }
    let revision = row.document.row_version;
    let result = assignment_detail(&mut tx, a, row).await?;
    finish(
        tx,
        a,
        p.request_id,
        operation,
        &(id, &p),
        &result,
        id,
        revision,
    )
    .await?;
    Ok(result)
}
pub async fn assignment_command(
    pool: &PgPool,
    a: CareAccess,
    id: Uuid,
    p: CareRevisionCommand,
    ack: bool,
) -> Result<CareAssignment, AppError> {
    let operation = if ack {
        "assignment_acknowledge"
    } else {
        "assignment_cancel"
    };
    scoring::text(&p.reason, 1000, true)?;
    let mut tx = pool.begin().await?;
    let round_id: Uuid = sqlx::query_scalar("SELECT round_id FROM care_assignments WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(missing)?;
    load_round(&mut tx, round_id, true).await?;
    let row = load_assignment(&mut tx, id, true).await?;
    if ack {
        if !a.can_manage_case(&row.reviewer_ids) {
            return Err(policy::denied());
        }
    } else if !a.manage_school && !(a.manage_assigned && row.created_by == a.actor_id) {
        return Err(policy::denied());
    }
    if let Some(v) = receipt(&mut tx, a, p.request_id, operation, &(id, &p)).await? {
        return Ok(v);
    }
    let mut r = row.document.0;
    if r.row_version != p.row_version {
        return Err(conflict());
    }
    if ack {
        if r.status != CareAssignmentStatus::Submitted || r.acknowledged_at.is_some() {
            return Err(conflict());
        }
        r.acknowledged_at = Some(Utc::now());
    } else {
        if !matches!(
            r.status,
            CareAssignmentStatus::Assigned | CareAssignmentStatus::InProgress
        ) {
            return Err(conflict());
        }
        r.status = CareAssignmentStatus::Cancelled;
    }
    r.row_version += 1;
    sqlx::query("UPDATE care_assignments SET document=$2 WHERE id=$1")
        .bind(id)
        .bind(Json(&r))
        .execute(&mut *tx)
        .await?;
    finish(
        tx,
        a,
        p.request_id,
        operation,
        &(id, &p),
        &r,
        id,
        r.row_version,
    )
    .await?;
    Ok(r)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredVisit {
    visited_date: Option<NaiveDate>,
    contact_person: String,
    factual_observations: String,
    strengths: String,
    needs: String,
    agreed_actions: String,
    next_follow_up: Option<NaiveDate>,
}
#[derive(FromRow)]
struct VisitRow {
    document: Json<CareVisit>,
    visitor_ids: Vec<Uuid>,
    record_ciphertext: String,
}
async fn load_visit(c: &mut PgConnection, id: Uuid, lock: bool) -> Result<VisitRow, AppError> {
    let sql = if lock {
        "SELECT document,visitor_ids,record_ciphertext FROM care_visits WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT document,visitor_ids,record_ciphertext FROM care_visits WHERE id=$1"
    };
    sqlx::query_as(sql)
        .bind(id)
        .fetch_optional(c)
        .await?
        .ok_or_else(missing)
}
fn visit_read(a: CareAccess, visitors: &[Uuid]) -> bool {
    a.routine_school() || (a.routine_assigned() && visitors.contains(&a.actor_id))
}
fn visit_record(row: VisitRow) -> Result<CareVisitRecord, AppError> {
    let s: StoredVisit = decrypt(&row.record_ciphertext)?;
    Ok(CareVisitRecord {
        visit: row.document.0,
        visited_date: s.visited_date,
        contact_person: s.contact_person,
        factual_observations: s.factual_observations,
        strengths: s.strengths,
        needs: s.needs,
        agreed_actions: s.agreed_actions,
        next_follow_up: s.next_follow_up,
    })
}
pub async fn visits(
    pool: &PgPool,
    a: CareAccess,
    q: CareListQuery,
) -> Result<Vec<CareVisit>, AppError> {
    a.require_read()?;
    let limit = page(&q)?;
    Ok(sqlx::query_scalar::<_,Json<CareVisit>>("SELECT document FROM care_visits WHERE ($1::uuid IS NULL OR id>$1) AND ($2::uuid IS NULL OR student_id=$2) AND ($3::text IS NULL OR document->>'status'=$3) AND ($4 OR ($5 AND $6=ANY(visitor_ids))) ORDER BY id LIMIT $7").bind(q.after_id).bind(q.student_id).bind(q.status).bind(a.routine_school()).bind(a.routine_assigned()).bind(a.actor_id).bind(limit).fetch_all(pool).await?.into_iter().map(|v|v.0).collect())
}
pub async fn get_visit(
    pool: &PgPool,
    a: CareAccess,
    id: Uuid,
) -> Result<CareVisitRecord, AppError> {
    let mut tx = pool.begin().await?;
    let row = load_visit(&mut tx, id, false).await?;
    if !visit_read(a, &row.visitor_ids) {
        return Err(missing());
    }
    let result = visit_record(row)?;
    audit_read(&mut tx, a, id).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn save_visit(
    pool: &PgPool,
    a: CareAccess,
    id: Option<Uuid>,
    p: SaveCareVisit,
) -> Result<CareVisitRecord, AppError> {
    a.require_manage()?;
    let visitors = ids(&p.visitor_ids, 30)?;
    for value in [
        &p.contact_person,
        &p.factual_observations,
        &p.strengths,
        &p.needs,
        &p.agreed_actions,
    ] {
        scoring::text(value, 5000, false)?;
    }
    if p.status == CareVisitStatus::Completed
        && (p.visited_date.is_none()
            || p.contact_person.trim().is_empty()
            || p.factual_observations.trim().is_empty()
            || p.agreed_actions.trim().is_empty())
    {
        return Err(invalid(
            "ต้องระบุวันที่ ผู้ติดต่อ ข้อเท็จจริง และสิ่งที่ตกลงก่อนบันทึกการเยี่ยมเสร็จสิ้น",
        ));
    }
    if p.next_follow_up
        .is_some_and(|v| v < p.visited_date.unwrap_or(p.planned_date))
    {
        return Err(invalid("วันติดตามต้องไม่ก่อนวันที่เยี่ยม"));
    }
    let operation = if id.is_some() {
        "visit_update"
    } else {
        "visit_create"
    };
    let mut tx = pool.begin().await?;
    school_academic_core::services::lifecycle_guard::lock_transition_shared(&mut tx).await?;
    if let Some(v) =
        receipt::<CareVisitRecord, _>(&mut tx, a, p.request_id, operation, &(id, &p)).await?
    {
        let current = load_visit(&mut tx, v.visit.id, false).await?;
        if !visit_read(a, &current.visitor_ids) {
            return Err(policy::denied());
        }
        return Ok(v);
    }
    let prior = if let Some(id) = id {
        Some(load_visit(&mut tx, id, true).await?)
    } else {
        None
    };
    let (id, revision, name, completed_at) = if let Some(old) = prior {
        let r = old.document.0;
        if !a.manage_school && !(a.manage_assigned && old.visitor_ids.contains(&a.actor_id)) {
            return Err(policy::denied());
        }
        if p.row_version != Some(r.row_version)
            || p.student_id != r.student_id
            || p.academic_term_id != r.academic_term_id
            || r.status == CareVisitStatus::Cancelled
        {
            return Err(conflict());
        }
        if !a.manage_school && visitors != old.visitor_ids {
            return Err(policy::denied());
        }
        (
            r.id,
            r.row_version + 1,
            r.student_name,
            if p.status == CareVisitStatus::Completed {
                r.completed_at.or(Some(Utc::now()))
            } else {
                None
            },
        )
    } else {
        if p.row_version.is_some() || p.status == CareVisitStatus::Cancelled {
            return Err(invalid("สถานะเริ่มต้นไม่ถูกต้อง"));
        }
        let roster =
            school_academic_core::services::dated_student_directory::locked_students_on_connection(
                &mut tx,
                p.academic_term_id,
                p.planned_date,
                &[p.student_id],
            )
            .await?;
        let student = roster
            .iter()
            .find(|s| s.id == p.student_id)
            .ok_or_else(|| invalid("นักเรียนไม่อยู่ในรายชื่อวันที่เยี่ยม"))?;
        if !a.manage_school
            && (!student.advisor_ids.contains(&a.actor_id) || !visitors.contains(&a.actor_id))
        {
            return Err(policy::denied());
        }
        (
            Uuid::now_v7(),
            1,
            student.name.clone(),
            if p.status == CareVisitStatus::Completed {
                Some(Utc::now())
            } else {
                None
            },
        )
    };
    active_staff(&mut tx, &visitors).await?;
    let visit = CareVisit {
        id,
        student_id: p.student_id,
        student_name: name,
        academic_term_id: p.academic_term_id,
        planned_date: p.planned_date,
        visitor_ids: visitors.clone(),
        status: p.status,
        row_version: revision,
        completed_at,
    };
    let private = StoredVisit {
        visited_date: p.visited_date,
        contact_person: p.contact_person.clone(),
        factual_observations: p.factual_observations.clone(),
        strengths: p.strengths.clone(),
        needs: p.needs.clone(),
        agreed_actions: p.agreed_actions.clone(),
        next_follow_up: p.next_follow_up,
    };
    let cipher = encrypt(&private)?;
    if revision == 1 {
        sqlx::query("INSERT INTO care_visits(id,student_id,visitor_ids,document,record_ciphertext) VALUES($1,$2,$3,$4,$5)").bind(id).bind(p.student_id).bind(&visitors).bind(Json(&visit)).bind(&cipher).execute(&mut *tx).await?;
    } else {
        sqlx::query(
            "UPDATE care_visits SET visitor_ids=$2,document=$3,record_ciphertext=$4 WHERE id=$1",
        )
        .bind(id)
        .bind(&visitors)
        .bind(Json(&visit))
        .bind(&cipher)
        .execute(&mut *tx)
        .await?;
    }
    let result = visit_record(VisitRow {
        document: Json(visit),
        visitor_ids: visitors,
        record_ciphertext: cipher,
    })?;
    finish(
        tx,
        a,
        p.request_id,
        operation,
        &(p.row_version.map(|_| id), &p),
        &result,
        id,
        revision,
    )
    .await?;
    Ok(result)
}

#[derive(FromRow)]
struct CaseRow {
    student_id: Uuid,
    assigned_user_ids: Vec<Uuid>,
    record_ciphertext: String,
}
async fn load_case(c: &mut PgConnection, id: Uuid, lock: bool) -> Result<CaseRow, AppError> {
    let sql = if lock {
        "SELECT student_id,assigned_user_ids,record_ciphertext FROM care_cases WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT student_id,assigned_user_ids,record_ciphertext FROM care_cases WHERE id=$1"
    };
    sqlx::query_as(sql)
        .bind(id)
        .fetch_optional(c)
        .await?
        .ok_or_else(missing)
}
pub async fn cases(
    pool: &PgPool,
    a: CareAccess,
    q: CareListQuery,
) -> Result<Vec<CareCase>, AppError> {
    let limit = page(&q)?;
    if !a.sensitive_school() && !a.sensitive_assigned() {
        return Err(policy::denied());
    }
    let rows:Vec<String>=sqlx::query_scalar("SELECT record_ciphertext FROM care_cases WHERE ($1::uuid IS NULL OR id>$1) AND ($2::uuid IS NULL OR student_id=$2) AND ($3::text IS NULL OR status=$3) AND ($4 OR ($5 AND $6=ANY(assigned_user_ids))) AND ($8::date IS NULL OR (status<>'closed' AND next_follow_up<=$8)) ORDER BY id LIMIT $7").bind(q.after_id).bind(q.student_id).bind(q.status).bind(a.sensitive_school()).bind(a.sensitive_assigned()).bind(a.actor_id).bind(limit).bind(q.follow_up_before).fetch_all(pool).await?;
    rows.into_iter().map(|r| decrypt::<CareCase>(&r)).collect()
}
pub async fn get_case(pool: &PgPool, a: CareAccess, id: Uuid) -> Result<CareCaseDetail, AppError> {
    let mut tx = pool.begin().await?;
    let row = load_case(&mut tx, id, false).await?;
    if !a.can_review(&row.assigned_user_ids) {
        return Err(missing());
    }
    let result = case_detail(&mut tx, &row).await?;
    audit_read(&mut tx, a, id).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn save_case(
    pool: &PgPool,
    a: CareAccess,
    id: Option<Uuid>,
    p: SaveCareCase,
) -> Result<CareCaseDetail, AppError> {
    a.require_sensitive_manage()?;
    let assigned = ids(&p.assigned_user_ids, 30)?;
    scoring::text(&p.title, 200, true)?;
    scoring::text(&p.note, 5000, true)?;
    sensitive_staff(pool, &assigned).await?;
    let operation = if id.is_some() {
        "case_update"
    } else {
        "case_create"
    };
    let mut tx = pool.begin().await?;
    if let Some(v) =
        receipt::<CareCaseDetail, _>(&mut tx, a, p.request_id, operation, &(id, &p)).await?
    {
        let current = load_case(&mut tx, v.case.id, false).await?;
        if !a.can_manage_case(&current.assigned_user_ids) {
            return Err(policy::denied());
        }
        return Ok(v);
    }
    let prior = if let Some(id) = id {
        Some(load_case(&mut tx, id, true).await?)
    } else {
        None
    };
    let mut newly_assigned = assigned.clone();
    let mut result = if let Some(old) = prior {
        if !a.can_manage_case(&old.assigned_user_ids) {
            return Err(policy::denied());
        }
        let mut detail = case_detail(&mut tx, &old).await?;
        if p.row_version != Some(detail.case.row_version)
            || p.student_id != old.student_id
            || p.source_assignment_id != detail.case.source_assignment_id
            || p.source_visit_id != detail.case.source_visit_id
            || detail.case.status == CareCaseStatus::Closed
        {
            return Err(conflict());
        }
        if !a.sensitive_manage_school && assigned != old.assigned_user_ids {
            return Err(policy::denied());
        }
        newly_assigned.retain(|id| !old.assigned_user_ids.contains(id));
        detail.case.row_version += 1;
        detail.case.title = p.title.clone();
        detail.case.assigned_user_ids = assigned.clone();
        detail.case.next_follow_up = p.next_follow_up;
        detail
    } else {
        if p.row_version.is_some() {
            return Err(invalid("ไม่พบรุ่นข้อมูลเดิม"));
        }
        let mut authorized_source = false;
        if let Some(source) = p.source_assignment_id {
            let row = load_assignment(&mut tx, source, false).await?;
            if row.document.student_id != p.student_id || !a.can_manage_case(&row.reviewer_ids) {
                return Err(policy::denied());
            }
            authorized_source = true;
        }
        if let Some(source) = p.source_visit_id {
            let row = load_visit(&mut tx, source, false).await?;
            if row.document.student_id != p.student_id || !visit_read(a, &row.visitor_ids) {
                return Err(policy::denied());
            }
            authorized_source = true;
        }
        if !a.sensitive_manage_school && (!authorized_source || !assigned.contains(&a.actor_id)) {
            return Err(policy::denied());
        }
        let name:String=sqlx::query_scalar("SELECT concat_ws(' ',title,first_name,last_name) FROM users WHERE id=$1 AND user_type='student' AND status='active' FOR SHARE").bind(p.student_id).fetch_optional(&mut *tx).await?.ok_or_else(missing)?;
        CareCaseDetail {
            case: CareCase {
                id: Uuid::now_v7(),
                source_assignment_id: p.source_assignment_id,
                source_visit_id: p.source_visit_id,
                student_id: p.student_id,
                student_name: name,
                title: p.title.clone(),
                status: CareCaseStatus::Open,
                assigned_user_ids: assigned.clone(),
                next_follow_up: p.next_follow_up,
                row_version: 1,
                created_at: Utc::now(),
            },
            events: Vec::new(),
        }
    };
    active_staff(&mut tx, &assigned).await?;
    let new_event = CareCaseEvent {
        id: Uuid::now_v7(),
        kind: CareEventKind::Note,
        note: p.note.clone(),
        created_by: a.actor_id,
        created_at: Utc::now(),
    };
    let r = &result.case;
    let cipher = encrypt(&result.case)?;
    if r.row_version == 1 {
        sqlx::query("INSERT INTO care_cases(id,student_id,assigned_user_ids,status,row_version,source_assignment_id,source_visit_id,record_ciphertext,next_follow_up) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(r.id).bind(r.student_id).bind(&assigned).bind(enum_text(r.status)?).bind(r.row_version).bind(r.source_assignment_id).bind(r.source_visit_id).bind(cipher).bind(r.next_follow_up).execute(&mut *tx).await?;
    } else {
        sqlx::query("UPDATE care_cases SET assigned_user_ids=$2,row_version=$3,record_ciphertext=$4,next_follow_up=$5 WHERE id=$1").bind(r.id).bind(&assigned).bind(r.row_version).bind(cipher).bind(r.next_follow_up).execute(&mut *tx).await?;
    }
    store_case_event(&mut tx, result.case.id, &new_event).await?;
    result.events = read_case_events(&mut tx, result.case.id, None, 100).await?;
    for recipient in newly_assigned {
        notify(
            &mut tx,
            recipient,
            format!("care:case:{}:{}:{recipient}", r.id, r.row_version),
            format!("/staff/student-care/cases/{}", r.id),
        )
        .await?;
    }
    finish(
        tx,
        a,
        p.request_id,
        operation,
        &(id, &p),
        &result,
        r.id,
        r.row_version,
    )
    .await?;
    Ok(result)
}
pub async fn add_case_event(
    pool: &PgPool,
    a: CareAccess,
    id: Uuid,
    p: AddCareCaseEvent,
) -> Result<CareCaseDetail, AppError> {
    a.require_sensitive_manage()?;
    scoring::text(&p.note, 5000, true)?;
    let mut tx = pool.begin().await?;
    let old = load_case(&mut tx, id, true).await?;
    if !a.can_manage_case(&old.assigned_user_ids) {
        return Err(policy::denied());
    }
    if let Some(v) = receipt(&mut tx, a, p.request_id, "case_event", &(id, &p)).await? {
        return Ok(v);
    }
    let mut result = case_detail(&mut tx, &old).await?;
    if result.case.row_version != p.row_version {
        return Err(conflict());
    }
    let previous = result.case.status;
    let transition_valid = match p.kind {
        CareEventKind::Closure => {
            previous != CareCaseStatus::Closed && p.status == CareCaseStatus::Closed
        }
        CareEventKind::Reopened => {
            previous == CareCaseStatus::Closed && p.status != CareCaseStatus::Closed
        }
        _ => previous != CareCaseStatus::Closed && p.status != CareCaseStatus::Closed,
    };
    if !transition_valid {
        return Err(invalid("การเปลี่ยนสถานะเคสต้องระบุเหตุการณ์ปิดหรือเปิดใหม่ให้ตรงกัน"));
    }
    result.case.row_version += 1;
    result.case.status = p.status;
    result.case.next_follow_up = p.next_follow_up;
    let new_event = CareCaseEvent {
        id: Uuid::now_v7(),
        kind: p.kind,
        note: p.note.clone(),
        created_by: a.actor_id,
        created_at: Utc::now(),
    };
    store_case_event(&mut tx, id, &new_event).await?;
    result.events = read_case_events(&mut tx, id, None, 100).await?;
    sqlx::query("UPDATE care_cases SET status=$2,row_version=$3,record_ciphertext=$4,next_follow_up=$5 WHERE id=$1")
        .bind(id)
        .bind(enum_text(p.status)?)
        .bind(result.case.row_version)
        .bind(encrypt(&result.case)?)
        .bind(result.case.next_follow_up)
        .execute(&mut *tx)
        .await?;
    finish(
        tx,
        a,
        p.request_id,
        "case_event",
        &(id, &p),
        &result,
        id,
        result.case.row_version,
    )
    .await?;
    Ok(result)
}
pub async fn overview(pool: &PgPool, a: CareAccess) -> Result<CareOverview, AppError> {
    a.require_read()?;
    // Each aggregate applies the same explicit-resource authority as its list.
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE ca.document->>'status' IN('assigned','in_progress')),count(*) FILTER(WHERE ca.document->>'status'='submitted'),count(*) FILTER(WHERE ca.document->>'urgent'='true' AND ca.document->>'acknowledgedAt' IS NULL AND ($3 OR ($4 AND $1=ANY(ca.reviewer_ids)))) FROM care_assignments ca WHERE ($2 AND ca.respondent_id=$1 AND (ca.document->>'respondent'<>'guardian' OR EXISTS(SELECT 1 FROM student_parents p JOIN users u ON u.id=p.parent_user_id AND u.status='active' WHERE p.parent_user_id=$1 AND p.student_user_id=ca.student_id))) OR $3 OR ($4 AND $1=ANY(ca.reviewer_ids)) OR $5 OR ($6 AND ca.created_by=$1)")
 .bind(a.actor_id).bind(a.own).bind(a.sensitive_school()).bind(a.sensitive_assigned()).bind(a.routine_school()).bind(a.routine_assigned()).fetch_one(pool).await?;
    let visits:i64=sqlx::query_scalar("SELECT count(*) FROM care_visits WHERE document->>'status'='planned' AND ($1 OR ($2 AND $3=ANY(visitor_ids)))").bind(a.routine_school()).bind(a.routine_assigned()).bind(a.actor_id).fetch_one(pool).await?;
    let cases:i64=sqlx::query_scalar("SELECT count(*) FROM care_cases WHERE status<>'closed' AND ($1 OR ($2 AND $3=ANY(assigned_user_ids)))").bind(a.sensitive_school()).bind(a.sensitive_assigned()).bind(a.actor_id).fetch_one(pool).await?;
    let today = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let due:i64=sqlx::query_scalar("SELECT count(*) FROM care_cases WHERE status<>'closed' AND next_follow_up<=$4 AND ($1 OR ($2 AND $3=ANY(assigned_user_ids)))").bind(a.sensitive_school()).bind(a.sensitive_assigned()).bind(a.actor_id).bind(today).fetch_one(pool).await?;
    Ok(CareOverview {
        follow_up_due_count: due,
        assigned_count: counts.0,
        submitted_count: counts.1,
        urgent_unacknowledged_count: counts.2,
        pending_visit_count: visits,
        open_case_count: cases,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn birthdays_and_page_bounds_are_explicit() {
        assert_eq!(
            age_at(
                NaiveDate::from_ymd_opt(2010, 10, 18).unwrap(),
                NaiveDate::from_ymd_opt(2026, 10, 17).unwrap()
            ),
            15
        );
        assert!(page(&CareListQuery {
            limit: Some(101),
            ..Default::default()
        })
        .is_err());
        assert!(ids(&[Uuid::nil(), Uuid::nil()], 30).is_err());
    }
}
#[cfg(test)]
mod db_tests;

pub async fn get_round(pool: &PgPool, a: CareAccess, id: Uuid) -> Result<CareRound, AppError> {
    a.require_read()?;
    let visible:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM care_rounds r WHERE r.id=$1 AND ($2 OR EXISTS(SELECT 1 FROM care_assignments ca WHERE ca.round_id=r.id AND (($3 AND ca.respondent_id=$4 AND (ca.document->>'respondent'<>'guardian' OR EXISTS(SELECT 1 FROM student_parents p WHERE p.student_user_id=ca.student_id AND p.parent_user_id=$4))) OR ($5 AND $4=ANY(ca.reviewer_ids))))))").bind(id).bind(a.routine_school()||a.routine_assigned()||a.configure||a.sensitive_school()).bind(a.own).bind(a.actor_id).bind(a.sensitive_assigned()).fetch_one(pool).await?;
    if !visible {
        return Err(missing());
    }
    let mut c = pool.acquire().await?;
    load_round(&mut c, id, false).await
}

async fn read_case_events(
    c: &mut PgConnection,
    id: Uuid,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<CareCaseEvent>, AppError> {
    let rows:Vec<String>=sqlx::query_scalar("SELECT event_ciphertext FROM care_case_events WHERE case_id=$1 AND ($2::uuid IS NULL OR id<$2) ORDER BY id DESC LIMIT $3").bind(id).bind(after).bind(limit).fetch_all(c).await?;
    rows.into_iter().map(|s| decrypt(&s)).collect()
}
async fn store_case_event(
    c: &mut PgConnection,
    id: Uuid,
    event: &CareCaseEvent,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO care_case_events(id,case_id,event_ciphertext) VALUES($1,$2,$3)")
        .bind(event.id)
        .bind(id)
        .bind(encrypt(event)?)
        .execute(c)
        .await?;
    Ok(())
}
async fn case_detail(c: &mut PgConnection, row: &CaseRow) -> Result<CareCaseDetail, AppError> {
    let case: CareCase = decrypt(&row.record_ciphertext)?;
    let events = read_case_events(c, case.id, None, 100).await?;
    Ok(CareCaseDetail { case, events })
}
pub async fn list_case_events(
    pool: &PgPool,
    a: CareAccess,
    id: Uuid,
    q: CareListQuery,
) -> Result<Vec<CareCaseEvent>, AppError> {
    let limit = page(&q)?;
    let mut tx = pool.begin().await?;
    let row = load_case(&mut tx, id, false).await?;
    if !a.can_review(&row.assigned_user_ids) {
        return Err(missing());
    }
    let result = read_case_events(&mut tx, id, q.after_id, limit).await?;
    audit_read(&mut tx, a, id).await?;
    tx.commit().await?;
    Ok(result)
}
