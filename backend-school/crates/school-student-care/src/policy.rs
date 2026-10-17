use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use uuid::Uuid;
#[derive(Clone, Copy)]
pub struct CareAccess {
    pub actor_id: Uuid,
    pub own: bool,
    pub read_assigned: bool,
    pub read_school: bool,
    pub manage_assigned: bool,
    pub manage_school: bool,
    pub sensitive_read_assigned: bool,
    pub sensitive_read_school: bool,
    pub sensitive_manage_assigned: bool,
    pub sensitive_manage_school: bool,
    pub configure: bool,
}
impl CareAccess {
    pub fn new(a: &ActorContext) -> Self {
        Self {
            actor_id: a.user_id,
            own: a.has_permission(codes::STUDENT_CARE_READ_OWN),
            read_assigned: a.has_permission(codes::STUDENT_CARE_READ_ASSIGNED),
            read_school: a.has_permission(codes::STUDENT_CARE_READ_SCHOOL),
            manage_assigned: a.has_permission(codes::STUDENT_CARE_MANAGE_ASSIGNED),
            manage_school: a.has_permission(codes::STUDENT_CARE_MANAGE_SCHOOL),
            sensitive_read_assigned: a.has_permission(codes::STUDENT_CARE_SENSITIVE_READ_ASSIGNED),
            sensitive_read_school: a.has_permission(codes::STUDENT_CARE_SENSITIVE_READ_SCHOOL),
            sensitive_manage_assigned: a
                .has_permission(codes::STUDENT_CARE_SENSITIVE_MANAGE_ASSIGNED),
            sensitive_manage_school: a.has_permission(codes::STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL),
            configure: a.has_permission(codes::STUDENT_CARE_CONFIG_MANAGE_SCHOOL),
        }
    }
    pub fn routine_school(self) -> bool {
        self.read_school || self.manage_school
    }
    pub fn routine_assigned(self) -> bool {
        self.read_assigned || self.manage_assigned
    }
    pub fn sensitive_school(self) -> bool {
        self.sensitive_read_school || self.sensitive_manage_school
    }
    pub fn sensitive_assigned(self) -> bool {
        self.sensitive_read_assigned || self.sensitive_manage_assigned
    }
    pub fn require_read(self) -> Result<(), AppError> {
        if self.own
            || self.routine_school()
            || self.routine_assigned()
            || self.configure
            || self.sensitive_school()
            || self.sensitive_assigned()
        {
            Ok(())
        } else {
            Err(denied())
        }
    }
    pub fn require_manage(self) -> Result<(), AppError> {
        if self.manage_school || self.manage_assigned {
            Ok(())
        } else {
            Err(denied())
        }
    }
    pub fn require_sensitive_manage(self) -> Result<(), AppError> {
        if self.sensitive_manage_school || self.sensitive_manage_assigned {
            Ok(())
        } else {
            Err(denied())
        }
    }
    pub fn require_configure(self) -> Result<(), AppError> {
        if self.configure {
            Ok(())
        } else {
            Err(denied())
        }
    }
    pub fn can_response(self, respondent: Uuid) -> bool {
        self.own && respondent == self.actor_id
    }
    pub fn can_review(self, reviewers: &[Uuid]) -> bool {
        self.sensitive_school() || (self.sensitive_assigned() && reviewers.contains(&self.actor_id))
    }
    pub fn can_manage_case(self, assigned: &[Uuid]) -> bool {
        self.sensitive_manage_school
            || (self.sensitive_manage_assigned && assigned.contains(&self.actor_id))
    }
}
pub fn denied() -> AppError {
    AppError::Forbidden("ไม่มีสิทธิ์เข้าถึงงานดูแลช่วยเหลือนี้".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routine_room_scope_never_grants_private_results() {
        let a = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::STUDENT_CARE_READ_ASSIGNED.into()],
        };
        let p = CareAccess::new(&a);
        assert!(!p.can_review(&[a.user_id]));
    }
    #[test]
    fn private_access_is_explicit_and_scopes_union() {
        let a = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![
                codes::STUDENT_CARE_READ_OWN.into(),
                codes::STUDENT_CARE_SENSITIVE_READ_ASSIGNED.into(),
            ],
        };
        let p = CareAccess::new(&a);
        assert!(p.can_response(a.user_id));
        assert!(p.can_review(&[a.user_id]));
        assert!(!p.can_review(&[Uuid::new_v4()]));
        assert!(!p.can_manage_case(&[a.user_id]));
    }
}
