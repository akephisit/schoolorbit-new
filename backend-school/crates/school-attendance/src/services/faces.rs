use super::{audit, lock_term, notifications, sessions, settings};
use crate::{
    models::*,
    policy,
    rules::{FACE_MODEL, face_distance, invalid, validate_descriptor},
};
use chrono::Utc;
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_file_platform::repository::SqlFileRepository;
use school_permissions::registry::codes;
use sqlx::PgPool;
use uuid::Uuid;
pub async fn enroll(
    pool: &PgPool,
    actor: Uuid,
    student: Uuid,
    p: EnrollAttendanceFace,
) -> Result<(), AppError> {
    if !p.consent_confirmed || p.model != FACE_MODEL || !(2..=5).contains(&p.descriptors.len()) {
        return Err(invalid("ยืนยันการลงทะเบียนและเก็บภาพใบหน้า 2–5 ตัวอย่าง"));
    }
    for d in &p.descriptors {
        validate_descriptor(d)?;
    }
    let data = serde_json::to_string(&p.descriptors).map_err(|_| invalid("ข้อมูลใบหน้าไม่ถูกต้อง"))?;
    let encrypted = school_crypto::encrypt(&data)
        .map_err(|_| AppError::ServiceUnavailable("ไม่สามารถเข้ารหัสข้อมูลใบหน้า".into()))?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.faces.transaction").await?;
    super::lock_module(&mut tx).await?;
    DbSession::new(&mut *tx).query("attendance.services.faces.enroll", "INSERT INTO attendance_face_enrollments(student_id,model,encrypted_descriptors,consent_at,enrolled_by) VALUES($1,$2,$3,now(),$4) ON CONFLICT(student_id) DO UPDATE SET model=excluded.model,encrypted_descriptors=excluded.encrypted_descriptors,consent_at=now(),enrolled_by=excluded.enrolled_by,enrolled_at=now(),row_version=attendance_face_enrollments.row_version+1").bind(student).bind(&p.model).bind(encrypted).bind(actor).execute().await?;
    audit(
        &mut tx,
        actor,
        student,
        "update",
        "face enrollment; consent confirmed",
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
pub async fn remove(pool: &PgPool, actor: Uuid, student: Uuid) -> Result<(), AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.faces.transaction").await?;
    super::lock_module(&mut tx).await?;
    DbSession::new(&mut *tx)
        .query(
            "attendance.services.faces.remove",
            "DELETE FROM attendance_face_enrollments WHERE student_id=$1",
        )
        .bind(student)
        .execute()
        .await?;
    audit(
        &mut tx,
        actor,
        student,
        "delete",
        "face enrollment withdrawn",
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
pub(crate) fn require_device_permission(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_permission(codes::ATTENDANCE_VERIFY_ASSIGNED)
}
pub async fn require_device(pool: &PgPool, actor: &ActorContext, id: Uuid) -> Result<(), AppError> {
    require_device_permission(actor)?;
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.faces.require_device")
            .await?;
    require_device_on_connection(&mut connection, actor, id).await
}
pub(crate) async fn require_device_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    id: Uuid,
) -> Result<(), AppError> {
    require_device_permission(actor)?;
    let valid:bool=DbSession::new(&mut *connection).scalar("attendance.services.faces.require_device", "SELECT EXISTS(SELECT 1 FROM attendance_devices WHERE id=$1 AND enabled AND operator_id=$2)").bind(id).bind(actor.user_id).fetch_one().await?;
    if valid {
        Ok(())
    } else {
        Err(AppError::Forbidden("เครื่องถูกปิดหรือไม่ได้มอบหมายให้บัญชีนี้".into()))
    }
}
pub async fn enrollments(
    pool: &PgPool,
    authorized_students: &[Uuid],
) -> Result<Vec<AttendanceFaceEnrollment>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.faces.enrollments")
            .await?;
    enrollments_on_connection(&mut connection, authorized_students).await
}
pub(crate) async fn enrollments_on_connection(
    connection: &mut sqlx::PgConnection,
    authorized_students: &[Uuid],
) -> Result<Vec<AttendanceFaceEnrollment>, AppError> {
    Ok(DbSession::new(&mut *connection)
        .query_as(
            "attendance.services.faces.enrollments",
            "SELECT f.student_id,f.enrolled_at FROM attendance_face_enrollments f
         JOIN users u ON u.id=f.student_id AND u.status='active' AND u.user_type='student'
         WHERE f.student_id=ANY($1) ORDER BY f.enrolled_at DESC,f.student_id LIMIT 10000",
        )
        .bind(authorized_students)
        .fetch_all()
        .await?)
}
pub async fn gallery(pool: &PgPool, term: Uuid) -> Result<Vec<AttendanceFace>, AppError> {
    let rows = {
        let mut connection =
            school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.faces.gallery")
                .await?;
        gallery_rows_on_connection(&mut connection, term).await?
    };
    decode_gallery_rows(rows)
}
pub(crate) async fn gallery_rows_on_connection(
    connection: &mut sqlx::PgConnection,
    term: Uuid,
) -> Result<Vec<(Uuid, String, String, i64)>, AppError> {
    let rows:Vec<(Uuid,String,String,i64)>=DbSession::new(&mut *connection).query_as("attendance.services.faces.gallery", "SELECT f.student_id,f.model,f.encrypted_descriptors,f.row_version FROM attendance_face_enrollments f JOIN student_academic_years y ON y.student_id=f.student_id JOIN academic_terms t ON t.academic_year_id=y.academic_year_id JOIN users u ON u.id=f.student_id AND u.status='active' WHERE t.id=$1 AND y.status='active' AND f.model=$2 ORDER BY f.student_id LIMIT 10001").bind(term).bind(FACE_MODEL).fetch_all().await?;
    Ok(rows)
}
pub(crate) fn decode_gallery_rows(
    rows: Vec<(Uuid, String, String, i64)>,
) -> Result<Vec<AttendanceFace>, AppError> {
    if rows.len() > 10000 {
        return Err(invalid("โรงเรียนมีข้อมูลใบหน้าเกินขนาดที่รองรับ"));
    }
    rows.into_iter()
        .map(|(student_id, model, data, row_version)| {
            let plain = school_crypto::decrypt(&data)
                .map_err(|_| AppError::ServiceUnavailable("อ่านข้อมูลใบหน้าไม่ได้".into()))?;
            let descriptors =
                serde_json::from_str(&plain).map_err(|_| invalid("ข้อมูลใบหน้าเสียหาย"))?;
            Ok(AttendanceFace {
                student_id,
                model,
                descriptors,
                row_version,
            })
        })
        .collect()
}
pub fn match_face(
    gallery: &[AttendanceFace],
    descriptor: &FaceDescriptor,
    c: &AttendanceConfiguration,
) -> Result<Uuid, AppError> {
    validate_descriptor(descriptor)?;
    let mut scores = gallery
        .iter()
        .map(|g| {
            (
                g.student_id,
                g.descriptors
                    .iter()
                    .map(|d| face_distance(d, descriptor))
                    .fold(f32::INFINITY, f32::min),
            )
        })
        .collect::<Vec<_>>();
    scores.sort_by(|a, b| a.1.total_cmp(&b.1));
    let best = scores
        .first()
        .ok_or_else(|| invalid("ยังไม่มีการลงทะเบียนใบหน้า"))?;
    if best.1 > c.face_distance
        || scores
            .get(1)
            .is_some_and(|next| next.1 - best.1 < c.face_margin)
    {
        return Err(invalid("ใบหน้าไม่ชัดเจนหรือใกล้เคียงหลายคน กรุณาลองใหม่หรือให้ครูเช็ค"));
    }
    Ok(best.0)
}
// Decrypted templates remain only in bounded process memory for 30 seconds.
// Enrollment endpoints invalidate the tenant cache before returning success.
#[derive(Clone)]
struct GalleryCache {
    loaded: std::time::Instant,
    faces: Vec<AttendanceFace>,
    values: usize,
}
static GALLERY: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<(String, Uuid), GalleryCache>>,
> = std::sync::OnceLock::new();
fn cache() -> &'static std::sync::Mutex<std::collections::HashMap<(String, Uuid), GalleryCache>> {
    GALLERY.get_or_init(Default::default)
}
pub fn invalidate_gallery(tenant: &str) -> Result<(), AppError> {
    cache()
        .lock()
        .map_err(|_| AppError::ServiceUnavailable("ข้อมูลจับคู่ยังไม่พร้อม".into()))?
        .retain(|(key, _), _| key != tenant);
    Ok(())
}
pub async fn cached_gallery(
    pool: &PgPool,
    tenant: &str,
    term: Uuid,
) -> Result<Vec<AttendanceFace>, AppError> {
    let key = (tenant.to_string(), term);
    {
        let map = cache()
            .lock()
            .map_err(|_| AppError::ServiceUnavailable("ข้อมูลจับคู่ยังไม่พร้อม".into()))?;
        if let Some(entry) = map.get(&key).filter(|e| e.loaded.elapsed().as_secs() < 30) {
            return Ok(entry.faces.clone());
        }
    }
    let faces = gallery(pool, term).await?;
    let values = faces
        .iter()
        .flat_map(|f| &f.descriptors)
        .map(|d| d.values.len())
        .sum();
    let mut map = cache()
        .lock()
        .map_err(|_| AppError::ServiceUnavailable("ข้อมูลจับคู่ยังไม่พร้อม".into()))?;
    map.retain(|_, e| e.loaded.elapsed().as_secs() < 30);
    // Limit descriptor payloads to 16 MiB across all schools; oversized galleries are read directly.
    if values <= 4_194_304 {
        while map.values().map(|e| e.values).sum::<usize>() + values > 4_194_304 {
            let oldest = map
                .iter()
                .min_by_key(|(_, e)| e.loaded)
                .map(|(k, _)| k.clone());
            if let Some(key) = oldest {
                map.remove(&key);
            } else {
                break;
            }
        }
        map.insert(
            key,
            GalleryCache {
                loaded: std::time::Instant::now(),
                faces: faces.clone(),
                values,
            },
        );
    }
    Ok(faces)
}
async fn scan_context(
    pool: &PgPool,
    actor: &ActorContext,
    p: &AttendanceScan,
    seed: Option<AttendanceSeed>,
) -> Result<(AttendanceSession, Option<AttendanceScanOutcome>), AppError> {
    require_device_permission(actor)?;
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.faces.scan_context")
            .await?;
    require_device_on_connection(&mut connection, actor, p.device_id).await?;
    let session = match sessions::get_optional_on_connection(&mut connection, p.session_id).await? {
        Some(s) => s,
        None => {
            let mut seed = seed.ok_or_else(|| AppError::NotFound("ไม่มีรอบสำหรับสแกน".into()))?;
            seed.session.roster = seed.students;
            sessions::with_token_on_connection(&mut connection, seed.session).await?
        }
    };
    if session.id != p.session_id
        || session.roster_token != p.roster_token
        || session.academic_term_id != p.session.academic_term_id
        || session.date != p.session.date
        || session.kind != p.session.kind
        || session.source_key != p.session.source_key
    {
        return Err(AppError::Conflict(
            "บริบทหรือรายชื่อสแกนเปลี่ยนแล้ว กรุณาเปิดใหม่".into(),
        ));
    }
    if session.kind != AttendanceKind::Arrival {
        policy::require_session(actor, &session, true)?;
    }
    // Reconcile committed events before fresh-capture or enrollment validation.
    let accepted: Option<(Uuid, Uuid, Uuid, Uuid, chrono::DateTime<Utc>, AttendanceResult, String)> =
        DbSession::new(&mut *connection).query_as("attendance.services.faces.scan_context", "SELECT e.session_id,e.student_id,e.device_id,e.evidence_file_id,e.captured_at,r.result,r.origin FROM attendance_scan_events e JOIN attendance_records r ON r.session_id=e.session_id AND r.student_id=e.student_id WHERE e.id=$1")
        .bind(p.event_id).fetch_optional().await?;
    let outcome =
        if let Some((session_id, student_id, device_id, file_id, captured_at, result, origin)) =
            accepted
        {
            if (session_id, student_id, device_id, file_id)
                != (p.session_id, p.student_id, p.device_id, p.evidence_file_id)
                || captured_at.timestamp_micros() != p.captured_at.timestamp_micros()
            {
                return Err(AppError::Conflict("รหัสสแกนถูกใช้กับรายการอื่น".into()));
            }
            Some(AttendanceScanOutcome {
                event_id: p.event_id,
                student_id,
                result,
                duplicate: true,
                teacher_conflict: origin == "teacher",
            })
        } else {
            None
        };
    Ok((session, outcome))
}
pub async fn scan(
    pool: &PgPool,
    actor: &ActorContext,
    p: AttendanceScan,
) -> Result<AttendanceScanOutcome, AppError> {
    let (session, accepted) = scan_context(pool, actor, &p, None).await?;
    if let Some(outcome) = accepted {
        return Ok(outcome);
    }
    let faces = gallery(pool, session.academic_term_id).await?;
    accept_scan(pool, actor, p, &faces, session).await
}
pub async fn scan_with_cached_gallery(
    pool: &PgPool,
    tenant: &str,
    actor: &ActorContext,
    p: AttendanceScan,
) -> Result<AttendanceScanOutcome, AppError> {
    scan_with_seed_and_cached_gallery(pool, tenant, actor, p, None).await
}
pub async fn scan_with_seed_and_cached_gallery(
    pool: &PgPool,
    tenant: &str,
    actor: &ActorContext,
    p: AttendanceScan,
    seed: Option<AttendanceSeed>,
) -> Result<AttendanceScanOutcome, AppError> {
    let (session, accepted) = scan_context(pool, actor, &p, seed).await?;
    if let Some(outcome) = accepted {
        return Ok(outcome);
    }
    let faces = cached_gallery(pool, tenant, session.academic_term_id).await?;
    accept_scan(pool, actor, p, &faces, session).await
}
pub async fn scan_with_gallery(
    pool: &PgPool,
    actor: &ActorContext,
    p: AttendanceScan,
    faces: &[AttendanceFace],
) -> Result<AttendanceScanOutcome, AppError> {
    let (session, accepted) = scan_context(pool, actor, &p, None).await?;
    if let Some(outcome) = accepted {
        return Ok(outcome);
    }
    accept_scan(pool, actor, p, faces, session).await
}
async fn accept_scan(
    pool: &PgPool,
    actor: &ActorContext,
    p: AttendanceScan,
    faces: &[AttendanceFace],
    s: AttendanceSession,
) -> Result<AttendanceScanOutcome, AppError> {
    if s.date
        != Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive()
        || p.captured_at > Utc::now() + chrono::Duration::seconds(30)
        || p.captured_at < Utc::now() - chrono::Duration::minutes(5)
    {
        return Err(invalid("รอบหรือเวลาสแกนไม่ใช่เวลาปัจจุบัน"));
    }
    let configuration = settings::get(pool, s.academic_term_id).await?.configuration;
    if match_face(faces, &p.descriptor, &configuration)? != p.student_id {
        return Err(invalid("ผลจับคู่ไม่ตรงกับนักเรียนที่ส่งมา"));
    }
    let enrolled = faces
        .iter()
        .find(|f| f.student_id == p.student_id)
        .ok_or_else(|| invalid("ไม่พบใบหน้า"))?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.faces.transaction").await?;
    super::lock_module(&mut tx).await?;
    lock_term(&mut tx, s.academic_term_id, true, false).await?;
    let device: Option<Uuid> = DbSession::new(&mut *tx).scalar("attendance.services.faces.accept_scan",
        "SELECT id FROM attendance_devices WHERE id=$1 AND enabled AND operator_id=$2 FOR SHARE",
    )
    .bind(p.device_id)
    .bind(actor.user_id)
    .fetch_optional()
    .await?;
    if device.is_none() {
        return Err(AppError::Forbidden("เครื่องถูกปิดหรือเปลี่ยนผู้รับผิดชอบแล้ว".into()));
    }
    // Serialize acceptance with enrollment replacement/withdrawal, including cached galleries.
    let enrollment_current: Option<String> = DbSession::new(&mut *tx).scalar("attendance.services.faces.accept_scan", "SELECT encrypted_descriptors FROM attendance_face_enrollments WHERE student_id=$1 AND model=$2 AND row_version=$3 FOR SHARE")
        .bind(p.student_id).bind(FACE_MODEL).bind(enrolled.row_version).fetch_optional().await?;
    let current_descriptors = enrollment_current
        .map(|encrypted| {
            let plain = school_crypto::decrypt(&encrypted)
                .map_err(|_| AppError::ServiceUnavailable("อ่านข้อมูลใบหน้าไม่ได้".into()))?;
            serde_json::from_str::<Vec<FaceDescriptor>>(&plain)
                .map_err(|_| invalid("ข้อมูลใบหน้าเสียหาย"))
        })
        .transpose()?;
    // A withdrawn/recreated enrollment can reuse a row version; compare the authoritative template too.
    if current_descriptors.as_ref() != Some(&enrolled.descriptors) {
        return Err(AppError::Conflict(
            "ใบหน้าลงทะเบียนเปลี่ยนแล้ว กรุณาหยุดและเปิดสแกนใหม่".into(),
        ));
    }
    let epoch = sessions::module_generation_in_tx(&mut tx).await?;
    if sessions::token_with_generation(s.clone(), epoch)?.roster_token != p.roster_token {
        return Err(AppError::Conflict("ข้อมูลโมดูลเปลี่ยนแล้ว กรุณาเปิดสแกนใหม่".into()));
    }
    if s.row_version == 0 {
        sessions::insert_seed(
            &mut tx,
            &AttendanceSeed {
                session: s.clone(),
                students: s.roster.clone(),
            },
        )
        .await?;
    }
    let sql = format!(
        "SELECT {} FROM attendance_sessions WHERE id=$1 FOR UPDATE",
        sessions::COLUMNS
    );
    let s: AttendanceSession = DbSession::new(&mut *tx)
        .query_as(
            "attendance.services.faces.accept_scan",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(s.id)
        .fetch_one()
        .await?;
    if s.cancelled {
        return Err(invalid("รอบนี้งดเช็คชื่อ"));
    }
    if sessions::token_with_generation(s.clone(), epoch)?.roster_token != p.roster_token {
        return Err(AppError::Conflict("รายชื่อเปลี่ยนแล้ว กรุณาเปิดสแกนใหม่".into()));
    }
    let participant = s
        .roster
        .iter()
        .find(|r| r.student_id == p.student_id)
        .ok_or_else(|| invalid("นักเรียนไม่อยู่ในรอบนี้"))?;
    let existing:Option<(Uuid,Uuid,Uuid,Uuid,Uuid,chrono::DateTime<Utc>)>=DbSession::new(&mut *tx).query_as("attendance.services.faces.accept_scan", "SELECT id,session_id,student_id,device_id,evidence_file_id,captured_at FROM attendance_scan_events WHERE id=$1 OR (session_id=$2 AND student_id=$3)").bind(p.event_id).bind(s.id).bind(p.student_id).fetch_optional().await?;
    let current:Option<(AttendanceResult,String,i64)>=DbSession::new(&mut *tx).query_as("attendance.services.faces.accept_scan", "SELECT result,origin,row_version FROM attendance_records WHERE session_id=$1 AND student_id=$2 FOR UPDATE").bind(s.id).bind(p.student_id).fetch_optional().await?;
    let (current, origin, revision) =
        current.unwrap_or((AttendanceResult::Unchecked, "none".into(), 0));
    if let Some((event, session, student, device, file, captured)) = existing {
        if session != s.id
            || student != p.student_id
            || (event == p.event_id
                && ((device, file) != (p.device_id, p.evidence_file_id)
                    || captured.timestamp_micros() != p.captured_at.timestamp_micros()))
        {
            return Err(AppError::Conflict("รหัสสแกนถูกใช้กับรายการอื่น".into()));
        }
        return Ok(AttendanceScanOutcome {
            event_id: event,
            student_id: p.student_id,
            result: current,
            duplicate: true,
            teacher_conflict: origin == "teacher",
        });
    }
    let file_ok:Option<Uuid>=DbSession::new(&mut *tx).scalar("attendance.services.faces.accept_scan", "SELECT id FROM files WHERE id=$1 AND purpose_code='attendance_evidence' AND visibility='private' AND lifecycle_status='ready' AND owner_user_id=$2 AND created_by=$3 AND created_at>now()-interval '10 minutes' FOR UPDATE").bind(p.evidence_file_id).bind(p.student_id).bind(actor.user_id).fetch_optional().await?;
    if file_ok.is_none() {
        return Err(invalid("ภาพหลักฐานไม่พร้อมหรือไม่ได้อัปโหลดสำหรับนักเรียนคนนี้"));
    }
    let local = p
        .captured_at
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .time();
    let cutoff = if s.kind == AttendanceKind::Arrival {
        configuration.late_after
    } else {
        s.start_time
    };
    let detected = if local > cutoff {
        AttendanceResult::Late
    } else {
        AttendanceResult::Present
    };
    let conflict = origin == "teacher" && current != detected;
    let teacher_result = origin == "teacher";
    let result = if teacher_result { current } else { detected };
    DbSession::new(&mut *tx)
        .query(
            "attendance.services.faces.accept_scan",
            "UPDATE files SET expires_at=now()+($2::integer*interval '1 day') WHERE id=$1",
        )
        .bind(p.evidence_file_id)
        .bind(configuration.evidence_days as i32)
        .execute()
        .await?;
    if !teacher_result {
        DbSession::new(&mut *tx).query("attendance.services.faces.accept_scan", "INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,special_round_id,class_number,result,origin,observed_at,updated_by) VALUES($1,$2,$6,$7,$8,$3,'scan',$4,$5) ON CONFLICT(session_id,student_id) DO UPDATE SET result=excluded.result,origin='scan',observed_at=excluded.observed_at,updated_by=excluded.updated_by,row_version=attendance_records.row_version+1")
            .bind(s.id).bind(p.student_id).bind(result).bind(p.captured_at).bind(actor.user_id)
            .bind(participant.student_academic_year_id).bind(s.special_round_id).bind(participant.class_number).execute().await?;
    }
    DbSession::new(&mut *tx).query("attendance.services.faces.accept_scan", "INSERT INTO attendance_scan_events(id,session_id,student_id,device_id,evidence_file_id,captured_at) VALUES($1,$2,$3,$4,$5,$6)").bind(p.event_id).bind(s.id).bind(p.student_id).bind(p.device_id).bind(p.evidence_file_id).bind(p.captured_at).execute().await?;
    DbSession::new(&mut *tx)
        .query(
            "attendance.services.faces.accept_scan",
            "UPDATE attendance_sessions SET row_version=row_version+1 WHERE id=$1",
        )
        .bind(s.id)
        .execute()
        .await?;
    let mut notice_session = s.clone();
    notice_session.start_time = local;
    if !teacher_result {
        notifications::queue_result(
            &mut tx,
            &notice_session,
            p.student_id,
            result,
            revision + 1,
            current != AttendanceResult::Unchecked && current != result,
        )
        .await?;
        super::score_changes::enqueue_day(&mut tx, s.academic_term_id, s.date).await?;
    }
    audit(
        &mut tx,
        actor.user_id,
        p.event_id,
        "create",
        "webcam attendance evidence accepted",
    )
    .await?;
    tx.commit().await?;
    Ok(AttendanceScanOutcome {
        event_id: p.event_id,
        student_id: p.student_id,
        result,
        duplicate: false,
        teacher_conflict: conflict,
    })
}
pub const EVIDENCE_EXPIRY_BATCH: i64 = 100;
pub async fn expire_evidence(pool: &PgPool) -> Result<i64, AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.faces.transaction").await?;
    super::lock_module(&mut tx).await?;
    let files:Vec<Uuid>=DbSession::new(&mut *tx).scalar("attendance.services.faces.expire_evidence", "SELECT id FROM files WHERE purpose_code='attendance_evidence' AND expires_at<=now() AND lifecycle_status='ready' ORDER BY expires_at,id LIMIT $1 FOR UPDATE SKIP LOCKED").bind(EVIDENCE_EXPIRY_BATCH).fetch_all().await?;
    let repository = SqlFileRepository::new(pool.clone());
    for id in &files {
        repository
            .request_delete_in_transaction(&mut tx, *id)
            .await
            .map_err(|_| AppError::ServiceUnavailable("เตรียมล้างภาพไม่ได้".into()))?;
    }
    tx.commit().await?;
    Ok(files.len() as i64)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_faces_fail_closed() {
        let id = Uuid::new_v4();
        let d = FaceDescriptor {
            values: vec![0.0; 128],
        };
        let f = AttendanceFace {
            student_id: id,
            model: crate::rules::FACE_MODEL.into(),
            descriptors: vec![d.clone()],
            row_version: 1,
        };
        let mut g = f.clone();
        g.student_id = Uuid::new_v4();
        assert_eq!(
            match_face(
                std::slice::from_ref(&f),
                &d,
                &AttendanceConfiguration::default()
            )
            .unwrap(),
            id
        );
        assert!(match_face(&[f, g], &d, &AttendanceConfiguration::default()).is_err());
    }
}
