use crate::models::AttendanceSession;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
pub fn read_school(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ATTENDANCE_READ_SCHOOL,
        codes::ATTENDANCE_UPDATE_SCHOOL,
        codes::ATTENDANCE_MANAGE_SCHOOL,
    ])
}
pub fn require_settings_read(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::ATTENDANCE_READ_ASSIGNED,
        codes::ATTENDANCE_READ_SCHOOL,
        codes::ATTENDANCE_UPDATE_ASSIGNED,
        codes::ATTENDANCE_UPDATE_SCHOOL,
        codes::ATTENDANCE_MANAGE_SCHOOL,
        codes::ATTENDANCE_VERIFY_ASSIGNED,
    ])
}
pub fn write_school(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ATTENDANCE_UPDATE_SCHOOL,
        codes::ATTENDANCE_MANAGE_SCHOOL,
    ])
}
pub fn require_session(
    actor: &ActorContext,
    s: &AttendanceSession,
    write: bool,
) -> Result<(), AppError> {
    let allowed = if write {
        write_school(actor)
            || (actor.has_permission(codes::ATTENDANCE_UPDATE_ASSIGNED)
                && s.teacher_ids.contains(&actor.user_id))
    } else {
        read_school(actor)
            || (actor.has_any_permission(&[
                codes::ATTENDANCE_READ_ASSIGNED,
                codes::ATTENDANCE_UPDATE_ASSIGNED,
            ]) && s.teacher_ids.contains(&actor.user_id))
    };
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์เช็คชื่อกลุ่มนี้".into()))
    }
}
pub struct SessionAccess {
    assigned_groups: std::collections::HashSet<uuid::Uuid>,
}
impl SessionAccess {
    pub(crate) fn assigned_group_ids(&self) -> Vec<uuid::Uuid> {
        self.assigned_groups.iter().copied().collect()
    }
    pub fn require_session(
        &self,
        actor: &ActorContext,
        session: &AttendanceSession,
        write: bool,
    ) -> Result<(), AppError> {
        if session.kind != crate::models::AttendanceKind::Lesson {
            return require_session(actor, session, write);
        }
        let assigned = session
            .learning_group_id
            .is_some_and(|group| self.assigned_groups.contains(&group));
        let allowed = if write {
            write_school(actor)
                || (actor.has_permission(codes::ATTENDANCE_UPDATE_ASSIGNED) && assigned)
        } else {
            read_school(actor)
                || (actor.has_any_permission(&[
                    codes::ATTENDANCE_READ_ASSIGNED,
                    codes::ATTENDANCE_UPDATE_ASSIGNED,
                ]) && assigned)
        };
        if allowed {
            Ok(())
        } else {
            Err(AppError::Forbidden("ไม่มีสิทธิ์เช็คชื่อกลุ่มนี้".into()))
        }
    }
}
pub async fn session_access_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    term: uuid::Uuid,
) -> Result<SessionAccess, AppError> {
    if read_school(actor) && write_school(actor) {
        return Ok(SessionAccess {
            assigned_groups: Default::default(),
        });
    }
    let year: uuid::Uuid = school_database::DbSession::new(connection)
        .scalar(
            "attendance.policy.current_context",
            "SELECT academic_year_id FROM academic_terms WHERE id=$1",
        )
        .bind(term)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    let groups = school_academic_delivery::services::current::assigned_group_ids_on_connection(
        connection,
        year,
        term,
        actor.user_id,
    )
    .await?;
    Ok(SessionAccess {
        assigned_groups: groups.learning_group_ids.into_iter().collect(),
    })
}
pub async fn require_session_on_connection(
    connection: &mut sqlx::PgConnection,
    actor: &ActorContext,
    session: &AttendanceSession,
    write: bool,
) -> Result<(), AppError> {
    if write && session.kind == crate::models::AttendanceKind::Lesson {
        if let Some(group) = session.learning_group_id {
            school_database::DbSession::new(connection)
                .query(
                    "attendance.policy.lock_current_group",
                    "SELECT id FROM learning_groups WHERE id=$1 AND academic_term_id=$2 FOR SHARE",
                )
                .bind(group)
                .bind(session.academic_term_id)
                .execute()
                .await?;
        }
    }
    session_access_on_connection(connection, actor, session.academic_term_id)
        .await?
        .require_session(actor, session, write)
}
pub fn require_reset(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_permission(codes::ATTENDANCE_MANAGE_SCHOOL)?;
    actor.require_permission(codes::ATTENDANCE_DELETE_SCHOOL)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AttendanceKind;
    use chrono::{NaiveDate, NaiveTime};
    use uuid::Uuid;
    #[test]
    fn reset_requires_both_school_management_and_deletion() {
        let mut actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::ATTENDANCE_MANAGE_SCHOOL.into()],
        };
        assert!(require_reset(&actor).is_err());
        actor.permissions = vec![codes::ATTENDANCE_DELETE_SCHOOL.into()];
        assert!(require_reset(&actor).is_err());
        actor
            .permissions
            .push(codes::ATTENDANCE_MANAGE_SCHOOL.into());
        assert!(require_reset(&actor).is_ok());
        actor.permissions = vec![codes::ATTENDANCE_UPDATE_ASSIGNED.into()];
        assert!(require_reset(&actor).is_err());
    }
    #[test]
    fn school_writer_can_read_configuration_without_separate_read_grant() {
        let mut actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::ATTENDANCE_UPDATE_SCHOOL.into()],
        };
        assert!(require_settings_read(&actor).is_ok());
        actor.permissions = vec![codes::ATTENDANCE_READ_OWN.into()];
        assert!(require_settings_read(&actor).is_err());
    }
    #[test]
    fn assignment_never_grants_unrelated_access() {
        let id = Uuid::new_v4();
        let actor = ActorContext {
            user_id: id,
            permissions: vec![codes::ATTENDANCE_UPDATE_ASSIGNED.into()],
        };
        let mut s = AttendanceSession {
            id: Uuid::new_v4(),
            academic_term_id: Uuid::new_v4(),
            date: NaiveDate::MIN,
            kind: AttendanceKind::Arrival,
            source_key: String::new(),
            title: String::new(),
            teacher_ids: vec![id],
            homeroom_id: None,
            learning_group_id: None,
            offering_id: None,
            special_round_id: None,
            start_time: NaiveTime::MIN,
            end_time: NaiveTime::MIN,
            count_override: None,
            cancelled: false,
            cancellation_reason: None,
            saved_at: None,
            saved_by: None,
            row_version: 1,
            roster: vec![],
            roster_token: String::new(),
        };
        assert!(require_session(&actor, &s, true).is_ok());
        s.teacher_ids.clear();
        assert!(require_session(&actor, &s, true).is_err());
    }
}
