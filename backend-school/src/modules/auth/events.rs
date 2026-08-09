use uuid::Uuid;

use super::session_repository::SessionRevocationTarget;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRevocationEvent {
    pub tenant: String,
    pub user_id: Uuid,
    pub target: SessionRevocationTarget,
}

impl SessionRevocationEvent {
    pub fn session(tenant: &str, user_id: Uuid, session_id: Uuid) -> Self {
        Self {
            tenant: tenant.to_string(),
            user_id,
            target: SessionRevocationTarget::Session(session_id),
        }
    }

    pub fn user(tenant: &str, user_id: Uuid, except_session_id: Option<Uuid>) -> Self {
        Self {
            tenant: tenant.to_string(),
            user_id,
            target: SessionRevocationTarget::User { except_session_id },
        }
    }

    pub fn applies_to(&self, tenant: &str, user_id: Uuid, session_id: Uuid) -> bool {
        if self.tenant != tenant || self.user_id != user_id {
            return false;
        }

        match self.target {
            SessionRevocationTarget::Session(target_session_id) => target_session_id == session_id,
            SessionRevocationTarget::User { except_session_id } => {
                except_session_id != Some(session_id)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revocation_targets_match_only_the_intended_tenant_user_and_session() {
        let user_id = Uuid::new_v4();
        let current = Uuid::new_v4();
        let other = Uuid::new_v4();

        assert!(SessionRevocationEvent::session("demo", user_id, current)
            .applies_to("demo", user_id, current));
        assert!(!SessionRevocationEvent::session("demo", user_id, current)
            .applies_to("demo", user_id, other));
        assert!(!SessionRevocationEvent::session("demo", user_id, current)
            .applies_to("other", user_id, current));
        assert!(
            SessionRevocationEvent::user("demo", user_id, None).applies_to("demo", user_id, other)
        );
        assert!(
            !SessionRevocationEvent::user("demo", user_id, Some(current))
                .applies_to("demo", user_id, current)
        );
    }
}
