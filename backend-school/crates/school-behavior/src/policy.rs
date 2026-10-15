use crate::access::BehaviorAccess;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
pub fn access(actor: &ActorContext) -> BehaviorAccess {
    BehaviorAccess::for_actor(actor)
}
pub fn require_catalog(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::BEHAVIOR_READ_OWN,
        codes::BEHAVIOR_READ_ASSIGNED,
        codes::BEHAVIOR_READ_SCHOOL,
        codes::BEHAVIOR_CREATE_ASSIGNED,
        codes::BEHAVIOR_CREATE_SCHOOL,
        codes::BEHAVIOR_MANAGE_SCHOOL,
    ])
}
pub fn require_manage(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_permission(codes::BEHAVIOR_MANAGE_SCHOOL)
}
pub fn require_cleanup(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_permission(codes::BEHAVIOR_CLEANUP_EXECUTE_SCHOOL)
}
pub fn require_history(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_permission(codes::BEHAVIOR_CORRECT_SCHOOL)
}
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    #[test]
    fn cancelling_own_never_grants_cleanup() {
        let actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::BEHAVIOR_DELETE_OWN.into()],
        };
        assert!(access(&actor).cancel_own);
        assert!(require_cleanup(&actor).is_err());
        assert!(require_history(&actor).is_err());
    }
}
