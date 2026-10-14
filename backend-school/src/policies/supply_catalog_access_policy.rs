use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupplyAction { Read, Manage, Propose, Review, BundleWrite }

pub fn require_action(actor: &ActorContext, action: SupplyAction, owner: Option<Uuid>, shared: bool) -> Result<(), AppError> {
    match action {
        SupplyAction::Read => actor.require_permission(codes::SUPPLY_CATALOG_READ_SCHOOL),
        SupplyAction::Manage => actor.require_permission(codes::SUPPLY_CATALOG_MANAGE_SCHOOL),
        SupplyAction::Propose => { actor.require_permission(codes::SUPPLY_CATALOG_REQUEST_OWN)?; require_owner(actor, owner) }
        SupplyAction::Review => actor.require_permission(codes::SUPPLY_CATALOG_APPROVE_SCHOOL),
        SupplyAction::BundleWrite => { actor.require_permission(codes::SUPPLY_CATALOG_READ_SCHOOL)?; if shared { actor.require_permission(codes::SUPPLY_CATALOG_MANAGE_SCHOOL) } else { require_owner(actor, owner) } }
    }
}
fn require_owner(actor: &ActorContext, owner: Option<Uuid>) -> Result<(), AppError> {
    if owner.is_none_or(|id| id == actor.user_id) { Ok(()) } else { Err(AppError::Forbidden("ไม่มีสิทธิ์แก้ไขรายการของผู้อื่น".into())) }
}
