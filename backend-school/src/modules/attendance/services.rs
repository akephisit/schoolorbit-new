use chrono::{NaiveDate, NaiveTime, Utc};
use school_academic_core::services::attendance_roster as core;
use school_academic_timetable::models::timetable_block::TimetableBlockKind;
use school_attendance::{
    models::*,
    policy,
    rules::invalid,
    services::{sessions, settings, specials},
};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
pub async fn seeds(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<AttendanceSeed>, AppError> {
    school_attendance::services::validate_date(pool, term, date).await?;
    let students = core::students(pool, term, date).await?;
    let rooms = core::rooms(pool, term).await?;
    let c = settings::get(pool, term).await?.configuration;
    let mut seeds = vec![];
    for room in rooms {
        let roster = students
            .iter()
            .filter(|s| s.homeroom_id == Some(room.id))
            .map(|s| AttendanceRosterStudent {
                student_id: s.student_id,
                student_academic_year_id: s.student_academic_year_id,
                class_number: s.class_number,
            })
            .collect::<Vec<_>>();
        for kind in [AttendanceKind::Arrival, AttendanceKind::Flag] {
            let session = seed_session(
                term,
                date,
                kind,
                room.id.to_string(),
                format!(
                    "{} {}",
                    if kind == AttendanceKind::Arrival {
                        "เข้าโรงเรียน"
                    } else {
                        "หน้าเสาธง"
                    },
                    room.name
                ),
                room.teacher_ids.clone(),
                Some(room.id),
                None,
                None,
                c.late_after,
                c.late_after,
            );
            seeds.push(AttendanceSeed {
                session,
                students: roster.clone(),
            });
        }
    }
    let blocks =
        match school_academic_timetable::services::attendance::blocks(pool, term, date).await {
            Ok(b) => b,
            Err(AppError::NotFound(_)) => vec![],
            Err(e) => return Err(e),
        };
    let groups = blocks
        .iter()
        .filter(|b| b.block_kind != TimetableBlockKind::Structural)
        .flat_map(|b| {
            b.groups
                .iter()
                .filter(|g| g.is_active)
                .map(|g| g.learning_group_id)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let memberships =
        school_academic_delivery::services::attendance_roster::students(pool, &groups, date)
            .await?;
    let student_map: HashMap<_, _> = students.iter().map(|s| (s.student_id, s)).collect();
    for block in blocks {
        if block.block_kind == TimetableBlockKind::Structural {
            continue;
        }
        for group in block.groups.into_iter().filter(|g| g.is_active) {
            let roster = memberships
                .iter()
                .filter(|m| m.learning_group_id == group.learning_group_id)
                .map(|m| AttendanceRosterStudent {
                    student_id: m.student_id,
                    student_academic_year_id: m.student_academic_year_id,
                    class_number: student_map.get(&m.student_id).and_then(|s| s.class_number),
                })
                .collect();
            let teachers = group.instructors.iter().map(|i| i.teacher_id).collect();
            let source = format!(
                "{}:{}",
                group.learning_group_id, block.bell_schedule_period_id
            );
            let title = format!(
                "{} · {} · {}",
                block.offering_name.as_deref().unwrap_or("คาบเรียน"),
                group.name,
                block.period_name
            );
            let session = seed_session(
                term,
                date,
                AttendanceKind::Lesson,
                source,
                title,
                teachers,
                None,
                Some(group.learning_group_id),
                block.learning_offering_id,
                block.start_time,
                block.end_time,
            );
            seeds.push(AttendanceSeed {
                session,
                students: roster,
            });
        }
    }
    Ok(seeds)
}
#[allow(clippy::too_many_arguments)]
fn seed_session(
    term: Uuid,
    date: NaiveDate,
    kind: AttendanceKind,
    source_key: String,
    title: String,
    teacher_ids: Vec<Uuid>,
    homeroom_id: Option<Uuid>,
    learning_group_id: Option<Uuid>,
    offering_id: Option<Uuid>,
    start_time: NaiveTime,
    end_time: NaiveTime,
) -> AttendanceSession {
    AttendanceSession {
        id: Uuid::new_v5(
            &term,
            format!("{date}:{}:{source_key}", kind.code()).as_bytes(),
        ),
        academic_term_id: term,
        date,
        kind,
        source_key,
        title,
        teacher_ids,
        homeroom_id,
        learning_group_id,
        offering_id,
        special_round_id: None,
        start_time,
        end_time,
        count_override: None,
        cancelled: false,
        cancellation_reason: None,
        saved_at: None,
        saved_by: None,
        row_version: 0,
    }
}
pub async fn workspace(
    pool: &PgPool,
    actor: &ActorContext,
    q: AttendanceQuery,
) -> Result<AttendanceWorkspace, AppError> {
    actor.require_any_permission(&[
        codes::ATTENDANCE_READ_ASSIGNED,
        codes::ATTENDANCE_READ_SCHOOL,
        codes::ATTENDANCE_UPDATE_ASSIGNED,
        codes::ATTENDANCE_UPDATE_SCHOOL,
        codes::ATTENDANCE_MANAGE_SCHOOL,
    ])?;
    let settings = settings::get(pool, q.academic_term_id).await?;
    let counted =
        settings::is_counted(pool, q.academic_term_id, q.date, &settings.configuration).await?;
    let saved = sessions::list(pool, actor, &q).await?;
    let mut rows = if settings.archived {
        vec![]
    } else {
        seeds(pool, q.academic_term_id, q.date)
            .await?
            .into_iter()
            .filter(|s| policy::require_session(actor, &s.session, false).is_ok())
            .map(|s| s.session)
            .collect::<Vec<_>>()
    };
    for s in saved {
        if let Some(row) = rows
            .iter_mut()
            .find(|row| row.kind == s.kind && row.source_key == s.source_key)
        {
            *row = s;
        } else {
            rows.push(s);
        }
    }
    rows.sort_by_key(|s| (s.start_time, s.title.clone()));
    Ok(AttendanceWorkspace {
        date: q.date,
        counted,
        sessions: rows,
        settings,
    })
}
pub async fn open(
    pool: &PgPool,
    actor: &ActorContext,
    p: OpenAttendanceSession,
) -> Result<AttendanceDetail, AppError> {
    let seed = seeds(pool, p.academic_term_id, p.date)
        .await?
        .into_iter()
        .find(|s| s.session.kind == p.kind && s.session.source_key == p.source_key)
        .ok_or_else(|| AppError::NotFound("ไม่มีรอบนี้ในตารางหรือรายชื่อประจำวัน".into()))?;
    let s = sessions::ensure(pool, actor, seed).await?;
    sessions::detail(pool, actor, s.id).await
}
pub async fn options(
    pool: &PgPool,
    actor: &ActorContext,
    term: Uuid,
) -> Result<AttendanceOptions, AppError> {
    actor.require_any_permission(&[
        codes::ATTENDANCE_MANAGE_SCHOOL,
        codes::ATTENDANCE_ENROLL_SCHOOL,
        codes::ATTENDANCE_ENROLL_ASSIGNED,
        codes::ATTENDANCE_VERIFY_ASSIGNED,
    ])?;
    let date = Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive();
    let rows = core::students(pool, term, date).await?;
    let rooms = core::rooms(pool, term).await?;
    let all = actor.has_any_permission(&[
        codes::ATTENDANCE_MANAGE_SCHOOL,
        codes::ATTENDANCE_ENROLL_SCHOOL,
    ]);
    let assigned: HashSet<_> = rooms
        .iter()
        .filter(|r| r.teacher_ids.contains(&actor.user_id))
        .map(|r| r.id)
        .collect();
    let students = rows
        .into_iter()
        .filter(|r| all || r.homeroom_id.is_some_and(|h| assigned.contains(&h)))
        .map(|r| AttendanceStudentOption {
            id: r.student_id,
            name: r.display_name,
            homeroom_id: r.homeroom_id,
            homeroom_name: r.homeroom_name,
        })
        .collect();
    let teachers = if actor.has_permission(codes::ATTENDANCE_MANAGE_SCHOOL) {
        sqlx::query_as("SELECT id,concat_ws(' ',title,first_name,last_name) name FROM users WHERE user_type='staff' AND status='active' ORDER BY first_name COLLATE schoolorbit_thai,id LIMIT 2000").fetch_all(pool).await?
    } else {
        vec![]
    };
    Ok(AttendanceOptions {
        students,
        teachers,
        audiences: if actor.has_permission(codes::ATTENDANCE_MANAGE_SCHOOL) {
            specials::audiences(pool, term).await?
        } else {
            vec![]
        },
        specials: if actor.has_permission(codes::ATTENDANCE_MANAGE_SCHOOL) {
            specials::templates(pool, term).await?
        } else {
            vec![]
        },
        devices: specials::devices(pool, actor.user_id, all).await?,
    })
}
pub async fn create_special(
    pool: &PgPool,
    actor: &ActorContext,
    term: Uuid,
    p: SpecialAttendanceDefinition,
) -> Result<SpecialAttendanceTemplate, AppError> {
    actor.require_permission(codes::ATTENDANCE_MANAGE_SCHOOL)?;
    specials::validate_definition(&p)?;
    let audiences = specials::audiences(pool, term).await?;
    let rooms = core::rooms(pool, term).await?;
    let mut resolved = vec![];
    for date in &p.dates {
        school_attendance::services::validate_date(pool, term, *date).await?;
        let students = core::students(pool, term, *date).await?;
        let mut groups = vec![];
        for (i, g) in p.groups.iter().enumerate() {
            let mut ids: HashSet<Uuid> = g.student_ids.iter().copied().collect();
            for a in &g.audience_group_ids {
                let group = audiences
                    .iter()
                    .find(|v| v.id == *a)
                    .ok_or_else(|| invalid("ไม่พบกลุ่มที่เลือก"))?;
                ids.extend(group.student_ids.iter());
            }
            for h in &g.homeroom_ids {
                if !rooms.iter().any(|r| r.id == *h) {
                    return Err(invalid("ห้องไม่อยู่ในภาคเรียน"));
                }
                ids.extend(
                    students
                        .iter()
                        .filter(|s| s.homeroom_id == Some(*h))
                        .map(|s| s.student_id),
                );
            }
            let mut teachers: HashSet<_> = g.teacher_ids.iter().copied().collect();
            if g.use_homeroom_advisors {
                teachers.extend(
                    rooms
                        .iter()
                        .filter(|r| {
                            g.homeroom_ids.contains(&r.id)
                                || students.iter().any(|s| {
                                    ids.contains(&s.student_id) && s.homeroom_id == Some(r.id)
                                })
                        })
                        .flat_map(|r| r.teacher_ids.iter().copied()),
                );
            }
            if teachers.is_empty() {
                return Err(invalid("เลือกครูรับผิดชอบอย่างน้อยหนึ่งคน"));
            }
            let teacher_ids: Vec<_> = teachers.into_iter().collect();
            let valid:i64=sqlx::query_scalar("SELECT count(*) FROM users WHERE id=ANY($1) AND user_type='staff' AND status='active'").bind(&teacher_ids).fetch_one(pool).await?;
            if valid != teacher_ids.len() as i64 {
                return Err(invalid("ครูรับผิดชอบไม่พร้อมใช้งาน"));
            }
            let roster = students
                .iter()
                .filter(|s| ids.contains(&s.student_id))
                .map(|s| AttendanceRosterStudent {
                    student_id: s.student_id,
                    student_academic_year_id: s.student_academic_year_id,
                    class_number: s.class_number,
                })
                .collect::<Vec<_>>();
            if roster.len() != ids.len() {
                return Err(invalid("นักเรียนไม่อยู่ในรายชื่อที่มีผลในวันที่จัดกิจกรรม"));
            }
            let mut session = seed_session(
                term,
                *date,
                AttendanceKind::Special,
                i.to_string(),
                format!("{} · {}", p.title, g.name),
                teacher_ids,
                None,
                None,
                None,
                p.start_time,
                p.end_time,
            );
            session.count_override = Some(p.counted);
            session.row_version = 1;
            groups.push(AttendanceSeed {
                session,
                students: roster,
            });
        }
        resolved.push(groups);
    }
    specials::create_template(pool, actor.user_id, term, p, resolved).await
}
pub async fn require_enrollment(
    pool: &PgPool,
    actor: &ActorContext,
    student: Uuid,
) -> Result<(), AppError> {
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND user_type='student' AND status='active')").bind(student).fetch_one(pool).await?;
    if !valid {
        return Err(invalid("นักเรียนไม่พร้อมลงทะเบียน"));
    }
    if actor.has_permission(codes::ATTENDANCE_ENROLL_SCHOOL) {
        return Ok(());
    }
    actor.require_permission(codes::ATTENDANCE_ENROLL_ASSIGNED)?;
    if core::advisor_for_student(
        pool,
        actor.user_id,
        student,
        Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive(),
    )
    .await?
    {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "ลงทะเบียนได้เฉพาะนักเรียนประจำชั้นที่รับผิดชอบ".into(),
        ))
    }
}
async fn own_student_read(
    pool: &PgPool,
    actor: &ActorContext,
    student: Uuid,
) -> Result<bool, AppError> {
    if !actor.has_permission(codes::ATTENDANCE_READ_OWN) {
        return Ok(false);
    }
    if actor.user_id == student {
        return Ok(true);
    }
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM student_parents p JOIN users u ON u.id=p.parent_user_id AND u.status='active' WHERE p.parent_user_id=$1 AND p.student_user_id=$2)").bind(actor.user_id).bind(student).fetch_one(pool).await?)
}
async fn student_read_scope(
    pool: &PgPool,
    actor: &ActorContext,
    student: Uuid,
) -> Result<bool, AppError> {
    if policy::read_school(actor) {
        return Ok(true);
    }
    if own_student_read(pool, actor, student).await? {
        return Ok(true);
    }
    if actor.has_any_permission(&[
        codes::ATTENDANCE_READ_ASSIGNED,
        codes::ATTENDANCE_UPDATE_ASSIGNED,
    ]) {
        let assigned:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attendance_sessions s JOIN attendance_records r ON r.session_id=s.id WHERE r.student_id=$1 AND $2=ANY(s.teacher_ids))").bind(student).bind(actor.user_id).fetch_one(pool).await?;
        if assigned||sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM attendance_term_teacher_summaries WHERE student_id=$1 AND teacher_id=$2)").bind(student).bind(actor.user_id).fetch_one(pool).await?{return Ok(false);}
    }
    Err(AppError::Forbidden("ไม่มีสิทธิ์ดูการเช็คชื่อนักเรียนคนนี้".into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn occurrence_identity_is_stable_without_timetable_version() {
        let t = Uuid::new_v4();
        let date = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
        let make = || {
            seed_session(
                t,
                date,
                AttendanceKind::Flag,
                "room".into(),
                "title".into(),
                vec![],
                None,
                None,
                None,
                NaiveTime::MIN,
                NaiveTime::MIN,
            )
        };
        assert_eq!(make().id, make().id);
    }
}
pub async fn prepare_kiosk(
    pool: &PgPool,
    actor: &ActorContext,
    p: OpenAttendanceKiosk,
) -> Result<AttendanceKioskWorkspace, AppError> {
    school_attendance::services::faces::require_device(pool, actor, p.device_id).await?;
    if p.date
        != Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive()
    {
        return Err(invalid("เครื่องสแกนเปิดได้เฉพาะวันนี้"));
    }
    let seeds = seeds(pool, p.academic_term_id, p.date)
        .await?
        .into_iter()
        .filter(|s| s.session.kind == AttendanceKind::Arrival)
        .collect::<Vec<_>>();
    let mut tx = pool.begin().await?;
    school_attendance::services::lock_term(&mut tx, p.academic_term_id, true, false).await?;
    for seed in &seeds {
        sessions::insert_seed(&mut tx, seed).await?;
    }
    tx.commit().await?;
    let sql=format!("SELECT {} FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND kind='arrival' AND NOT cancelled ORDER BY title,id",sessions::COLUMNS);
    let sessions = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(p.academic_term_id)
        .bind(p.date)
        .fetch_all(pool)
        .await?;
    let students = core::students(pool, p.academic_term_id, p.date)
        .await?
        .into_iter()
        .map(|s| AttendanceStudentOption {
            id: s.student_id,
            name: s.display_name,
            homeroom_id: s.homeroom_id,
            homeroom_name: s.homeroom_name,
        })
        .collect();
    Ok(AttendanceKioskWorkspace {
        sessions,
        faces: school_attendance::services::faces::gallery(pool, p.academic_term_id).await?,
        students,
        configuration: settings::get(pool, p.academic_term_id).await?.configuration,
    })
}
pub async fn history(
    pool: &PgPool,
    actor: &ActorContext,
    q: AttendanceHistoryQuery,
) -> Result<Vec<AttendanceHistoryItem>, AppError> {
    let full = student_read_scope(pool, actor, q.student_id).await?;
    if q.end < q.start || (q.end - q.start).num_days() > 31 {
        return Err(invalid("ดูรายละเอียดครั้งละไม่เกิน 31 วัน"));
    }
    Ok(sqlx::query_as("SELECT s.id session_id,s.date,s.kind,s.title,r.result,r.note,r.observed_at,e.evidence_file_id,s.cancelled FROM attendance_records r JOIN attendance_sessions s ON s.id=r.session_id LEFT JOIN attendance_scan_events e ON e.session_id=s.id AND e.student_id=r.student_id WHERE s.academic_term_id=$1 AND r.student_id=$2 AND s.date BETWEEN $3 AND $4 AND ($5 OR $6=ANY(s.teacher_ids)) ORDER BY s.date DESC,s.start_time,s.id LIMIT 1000").bind(q.academic_term_id).bind(q.student_id).bind(q.start).bind(q.end).bind(full).bind(actor.user_id).fetch_all(pool).await?)
}
pub async fn scan(
    _state: &crate::AppState,
    context: &crate::utils::request_context::ActorTenantContext,
    p: AttendanceScan,
) -> Result<AttendanceScanOutcome, AppError> {
    let file = p.evidence_file_id;
    school_attendance::services::faces::require_device(
        &context.tenant.pool,
        &context.actor,
        p.device_id,
    )
    .await?;
    let session = sessions::get(&context.tenant.pool, p.session_id).await?;
    let gallery = school_attendance::services::faces::cached_gallery(
        &context.tenant.pool,
        &context.tenant.subdomain,
        session.academic_term_id,
    )
    .await?;
    let outcome = school_attendance::services::faces::scan_with_gallery(
        &context.tenant.pool,
        &context.actor,
        p.clone(),
        &gallery,
    )
    .await;
    // Keep uncommitted evidence available for retryable storage/database failures.
    // Permanent rejections and redundant duplicate uploads can be discarded.
    if outcome.as_ref().is_ok_and(|r| r.duplicate)
        || matches!(
            &outcome,
            Err(AppError::ValidationError(_)
                | AppError::BadRequest(_)
                | AppError::Forbidden(_)
                | AppError::NotFound(_))
        )
    {
        let cleanup=async{
 let mut tx=context.tenant.pool.begin().await?;
 sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE").bind(file).execute(&mut *tx).await?;
 let disposable:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM files f WHERE f.id=$1 AND f.purpose_code='attendance_evidence' AND f.created_by=$2 AND NOT EXISTS(SELECT 1 FROM attendance_scan_events e WHERE e.evidence_file_id=f.id))").bind(file).bind(context.actor.user_id).fetch_one(&mut *tx).await?;
 if disposable{school_file_platform::repository::SqlFileRepository::new(context.tenant.pool.clone()).request_delete_in_transaction(&mut tx,file).await.map_err(|_|AppError::ServiceUnavailable("เตรียมล้างภาพชั่วคราวไม่ได้".into()))?;}tx.commit().await?;Ok::<_,AppError>(())}.await;
        if cleanup.is_err() {
            tracing::warn!(
                error_code = "attendance_orphan_cleanup_pending",
                "Temporary attendance evidence will expire through file retention"
            );
        }
    }
    outcome
}
pub async fn require_evidence_upload(
    pool: &PgPool,
    actor: &ActorContext,
    student: Uuid,
) -> Result<(), AppError> {
    actor.require_permission(codes::ATTENDANCE_VERIFY_ASSIGNED)?;
    let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attendance_devices d JOIN users u ON u.id=$2 AND u.user_type='student' AND u.status='active' WHERE d.enabled AND d.operator_id=$1)").bind(actor.user_id).bind(student).fetch_one(pool).await?;
    if valid {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์อัปโหลดภาพจากเครื่องสแกนนี้".into()))
    }
}
pub async fn require_evidence_file(
    pool: &PgPool,
    actor: &ActorContext,
    file: &school_file_platform::repository::PlatformFile,
    action: crate::policies::file_access_policy::FilePolicyAction,
    resource: Option<Uuid>,
) -> Result<(), AppError> {
    use crate::policies::file_access_policy::FilePolicyAction;
    let student = file.owner_user_id.ok_or_else(|| invalid("ภาพไม่มีเจ้าของ"))?;
    if resource.is_some_and(|id| id != student) {
        return Err(AppError::Forbidden("ภาพไม่ได้เป็นของนักเรียนที่เลือก".into()));
    }
    let linked: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT session_id,student_id FROM attendance_scan_events WHERE evidence_file_id=$1",
    )
    .bind(file.id)
    .fetch_optional(pool)
    .await?;
    if let Some((session, student)) = linked {
        if action != FilePolicyAction::Read {
            return Err(AppError::Forbidden(
                "ล้างภาพหลักฐานผ่านอายุการเก็บหรือการล้างภาคเรียน".into(),
            ));
        }
        let s = sessions::get(pool, session).await?;
        if policy::require_session(actor, &s, false).is_ok() {
            return Ok(());
        }
        if own_student_read(pool, actor, student).await? {
            Ok(())
        } else {
            Err(AppError::Forbidden("ไม่มีสิทธิ์ดูภาพหลักฐานนี้".into()))
        }
    } else {
        let uploaded: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM files WHERE id=$1 AND created_by=$2)")
                .bind(file.id)
                .bind(actor.user_id)
                .fetch_one(pool)
                .await?;
        if uploaded {
            require_evidence_upload(pool, actor, student).await
        } else {
            Err(AppError::Forbidden("ไม่มีสิทธิ์ดูภาพที่ยังไม่ผูกกับผลเช็คชื่อ".into()))
        }
    }
}
pub async fn evidence_delete_guard<'a>(
    pool: &'a PgPool,
    actor: &ActorContext,
    file: &school_file_platform::repository::PlatformFile,
    resource: Option<Uuid>,
) -> Result<sqlx::Transaction<'a, sqlx::Postgres>, AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE")
        .bind(file.id)
        .execute(&mut *tx)
        .await?;
    require_evidence_file(
        pool,
        actor,
        file,
        crate::policies::file_access_policy::FilePolicyAction::Delete,
        resource,
    )
    .await?;
    Ok(tx)
}

pub async fn report(
    pool: &PgPool,
    actor: &ActorContext,
    q: AttendanceReportQuery,
) -> Result<AttendanceReport, AppError> {
    let mut full = policy::read_school(actor);
    if let Some(student) = q.student_id {
        full = student_read_scope(pool, actor, student).await?;
    } else {
        actor.require_any_permission(&[
            codes::ATTENDANCE_READ_SCHOOL,
            codes::ATTENDANCE_UPDATE_SCHOOL,
            codes::ATTENDANCE_MANAGE_SCHOOL,
            codes::ATTENDANCE_READ_ASSIGNED,
            codes::ATTENDANCE_UPDATE_ASSIGNED,
        ])?;
    }
    school_attendance::services::reports::summaries(
        pool,
        q.academic_term_id,
        q.student_id,
        if full { None } else { Some(actor.user_id) },
    )
    .await
}

#[cfg(test)]
mod db_tests;
