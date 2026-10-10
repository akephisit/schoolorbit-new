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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AttendanceKind;
    use chrono::{NaiveDate, NaiveTime};
    use uuid::Uuid;
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
            kind: AttendanceKind::Flag,
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
        };
        assert!(require_session(&actor, &s, true).is_ok());
        s.teacher_ids.clear();
        assert!(require_session(&actor, &s, true).is_err());
    }
}
