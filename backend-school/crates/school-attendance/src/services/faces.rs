use super::{audit, lock_term, notifications, sessions, settings};
use crate::{
    models::*,
    policy,
    rules::{face_distance, invalid, validate_descriptor, FACE_MODEL},
};
use chrono::Utc;
use school_authorization::ActorContext;
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
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO attendance_face_enrollments(student_id,model,encrypted_descriptors,consent_at,enrolled_by) VALUES($1,$2,$3,now(),$4) ON CONFLICT(student_id) DO UPDATE SET model=excluded.model,encrypted_descriptors=excluded.encrypted_descriptors,consent_at=now(),enrolled_by=excluded.enrolled_by,enrolled_at=now(),row_version=attendance_face_enrollments.row_version+1").bind(student).bind(&p.model).bind(encrypted).bind(actor).execute(&mut *tx).await?;
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
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM attendance_face_enrollments WHERE student_id=$1")
        .bind(student)
        .execute(&mut *tx)
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
pub async fn require_device(pool: &PgPool, actor: &ActorContext, id: Uuid) -> Result<(), AppError> {
    actor.require_permission(codes::ATTENDANCE_VERIFY_ASSIGNED)?;
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attendance_devices WHERE id=$1 AND enabled AND operator_id=$2)").bind(id).bind(actor.user_id).fetch_one(pool).await?;
    if valid {
        Ok(())
    } else {
        Err(AppError::Forbidden("เครื่องถูกปิดหรือไม่ได้มอบหมายให้บัญชีนี้".into()))
    }
}
pub async fn gallery(pool: &PgPool, term: Uuid) -> Result<Vec<AttendanceFace>, AppError> {
    let rows:Vec<(Uuid,String,String,i64)>=sqlx::query_as("SELECT f.student_id,f.model,f.encrypted_descriptors,f.row_version FROM attendance_face_enrollments f JOIN student_academic_years y ON y.student_id=f.student_id JOIN academic_terms t ON t.academic_year_id=y.academic_year_id JOIN users u ON u.id=f.student_id AND u.status='active' WHERE t.id=$1 AND y.status='active' AND f.model=$2 ORDER BY f.student_id LIMIT 10001").bind(term).bind(FACE_MODEL).fetch_all(pool).await?;
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
pub async fn scan(
    pool: &PgPool,
    actor: &ActorContext,
    p: AttendanceScan,
) -> Result<AttendanceScanOutcome, AppError> {
    let session = sessions::get(pool, p.session_id).await?;
    let faces = gallery(pool, session.academic_term_id).await?;
    scan_with_gallery(pool, actor, p, &faces).await
}
pub async fn scan_with_gallery(
    pool: &PgPool,
    actor: &ActorContext,
    p: AttendanceScan,
    faces: &[AttendanceFace],
) -> Result<AttendanceScanOutcome, AppError> {
    require_device(pool, actor, p.device_id).await?;
    let s = sessions::get(pool, p.session_id).await?;
    if s.kind != AttendanceKind::Arrival {
        policy::require_session(actor, &s, true)?;
    }
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
    let revision = faces
        .iter()
        .find(|f| f.student_id == p.student_id)
        .map(|f| f.row_version)
        .ok_or_else(|| invalid("ไม่พบใบหน้า"))?;
    let mut tx = pool.begin().await?;
    // Serialize acceptance with enrollment replacement/withdrawal, including cached galleries.
    let enrollment_current: Option<Uuid> = sqlx::query_scalar("SELECT student_id FROM attendance_face_enrollments WHERE student_id=$1 AND model=$2 AND row_version=$3 FOR SHARE")
        .bind(p.student_id).bind(FACE_MODEL).bind(revision).fetch_optional(&mut *tx).await?;
    if enrollment_current.is_none() {
        return Err(AppError::Conflict(
            "ใบหน้าลงทะเบียนเปลี่ยนแล้ว กรุณาหยุดและเปิดสแกนใหม่".into(),
        ));
    }
    lock_term(&mut tx, s.academic_term_id, true, false).await?;
    let cancelled: bool =
        sqlx::query_scalar("SELECT cancelled FROM attendance_sessions WHERE id=$1 FOR UPDATE")
            .bind(s.id)
            .fetch_one(&mut *tx)
            .await?;
    if cancelled {
        return Err(invalid("รอบนี้งดเช็คชื่อ"));
    }
    let existing:Option<(Uuid,Uuid,Uuid)>=sqlx::query_as("SELECT id,session_id,student_id FROM attendance_scan_events WHERE id=$1 OR (session_id=$2 AND student_id=$3)").bind(p.event_id).bind(s.id).bind(p.student_id).fetch_optional(&mut *tx).await?;
    let current:Option<(AttendanceResult,String,i64)>=sqlx::query_as("SELECT result,origin,row_version FROM attendance_records WHERE session_id=$1 AND student_id=$2 FOR UPDATE").bind(s.id).bind(p.student_id).fetch_optional(&mut *tx).await?;
    let (current, origin, revision) = current.ok_or_else(|| invalid("นักเรียนไม่อยู่ในรอบนี้"))?;
    if let Some((event, session, student)) = existing {
        if session != s.id || student != p.student_id {
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
    let file_ok:Option<Uuid>=sqlx::query_scalar("SELECT id FROM files WHERE id=$1 AND purpose_code='attendance_evidence' AND visibility='private' AND lifecycle_status='ready' AND owner_user_id=$2 AND created_by=$3 AND created_at>now()-interval '10 minutes' FOR UPDATE").bind(p.evidence_file_id).bind(p.student_id).bind(actor.user_id).fetch_optional(&mut *tx).await?;
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
    sqlx::query("INSERT INTO attendance_scan_events(id,session_id,student_id,device_id,evidence_file_id,captured_at) VALUES($1,$2,$3,$4,$5,$6)").bind(p.event_id).bind(s.id).bind(p.student_id).bind(p.device_id).bind(p.evidence_file_id).bind(p.captured_at).execute(&mut *tx).await?;
    sqlx::query("UPDATE files SET expires_at=now()+($2::integer*interval '1 day') WHERE id=$1")
        .bind(p.evidence_file_id)
        .bind(configuration.evidence_days as i32)
        .execute(&mut *tx)
        .await?;
    if !teacher_result {
        sqlx::query("UPDATE attendance_records SET result=$3,origin='scan',observed_at=$4,updated_by=$5,row_version=row_version+1 WHERE session_id=$1 AND student_id=$2").bind(s.id).bind(p.student_id).bind(result).bind(p.captured_at).bind(actor.user_id).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE attendance_sessions SET row_version=row_version+1 WHERE id=$1")
        .bind(s.id)
        .execute(&mut *tx)
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
pub async fn expire_evidence(pool: &PgPool) -> Result<i64, AppError> {
    let mut tx = pool.begin().await?;
    let files:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM files WHERE purpose_code='attendance_evidence' AND expires_at<=now() AND lifecycle_status='ready' ORDER BY expires_at,id LIMIT 100 FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
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
