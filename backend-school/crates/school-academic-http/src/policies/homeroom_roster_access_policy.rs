use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;

#[derive(Clone, Copy)]
pub enum RosterAction {
    Read,
    Manage,
}

pub fn require_roster_access(actor: &ActorContext, action: RosterAction) -> Result<(), AppError> {
    match action {
        RosterAction::Read => actor.require_any_permission(&[
            codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL,
            codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL,
        ]),
        RosterAction::Manage => {
            actor.require_permission(codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn roster_read_and_write_require_student_year_capabilities() {
        for (permission, read, manage) in [
            (codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL, true, false),
            (codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL, true, true),
            (codes::HOMEROOM_READ_SCHOOL, false, false),
            (codes::HOMEROOM_MANAGE_SCHOOL, false, false),
            ("", false, false),
        ] {
            let actor = ActorContext {
                user_id: Uuid::new_v4(),
                permissions: vec![permission.into()],
            };
            assert_eq!(
                require_roster_access(&actor, RosterAction::Read).is_ok(),
                read
            );
            assert_eq!(
                require_roster_access(&actor, RosterAction::Manage).is_ok(),
                manage
            );
        }
    }
}
