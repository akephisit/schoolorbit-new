use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupplyAction {
    Read,
    Manage,
    Propose,
    Review,
    BundleWrite,
}

pub fn require_action(
    actor: &ActorContext,
    action: SupplyAction,
    owner: Option<Uuid>,
    shared: bool,
) -> Result<(), AppError> {
    match action {
        SupplyAction::Read => actor.require_permission(codes::SUPPLY_CATALOG_READ_SCHOOL),
        SupplyAction::Manage => actor.require_permission(codes::SUPPLY_CATALOG_MANAGE_SCHOOL),
        SupplyAction::Propose => {
            actor.require_permission(codes::SUPPLY_CATALOG_REQUEST_OWN)?;
            require_owner(actor, owner)
        }
        SupplyAction::Review => actor.require_permission(codes::SUPPLY_CATALOG_APPROVE_SCHOOL),
        SupplyAction::BundleWrite => {
            actor.require_permission(codes::SUPPLY_CATALOG_READ_SCHOOL)?;
            if shared {
                actor.require_permission(codes::SUPPLY_CATALOG_MANAGE_SCHOOL)
            } else {
                require_owner(actor, owner)
            }
        }
    }
}
fn require_owner(actor: &ActorContext, owner: Option<Uuid>) -> Result<(), AppError> {
    if owner.is_none_or(|id| id == actor.user_id) {
        Ok(())
    } else {
        Err(AppError::Forbidden("ไม่มีสิทธิ์แก้ไขรายการของผู้อื่น".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actor_authority_is_action_and_owner_specific() {
        let owner = Uuid::now_v7();
        let other = Uuid::now_v7();
        let reader = ActorContext {
            user_id: owner,
            permissions: vec![codes::SUPPLY_CATALOG_READ_SCHOOL.into()],
        };
        assert!(require_action(&reader, SupplyAction::Read, None, false).is_ok());
        assert!(require_action(&reader, SupplyAction::BundleWrite, Some(owner), false).is_ok());
        assert!(require_action(&reader, SupplyAction::BundleWrite, Some(other), false).is_err());
        assert!(require_action(&reader, SupplyAction::BundleWrite, Some(owner), true).is_err());
        assert!(require_action(&reader, SupplyAction::Manage, None, false).is_err());
        assert!(require_action(&reader, SupplyAction::Propose, None, false).is_err());
        assert!(require_action(&reader, SupplyAction::Review, None, false).is_err());
        let proposer = ActorContext {
            user_id: owner,
            permissions: vec![codes::SUPPLY_CATALOG_REQUEST_OWN.into()],
        };
        assert!(require_action(&proposer, SupplyAction::Propose, Some(owner), false).is_ok());
        assert!(require_action(&proposer, SupplyAction::Propose, Some(other), false).is_err());
        assert!(require_action(&proposer, SupplyAction::Review, None, false).is_err());
    }
}
