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

#[derive(Debug, Clone)]
pub struct PermissionChangeEvent {
    pub tenant: String,
    pub target_user_id: Option<Uuid>,
}

impl PermissionChangeEvent {
    pub fn for_user(tenant: &str, user_id: Uuid) -> Self {
        Self {
            tenant: tenant.to_string(),
            target_user_id: Some(user_id),
        }
    }

    pub fn for_all_users(tenant: &str) -> Self {
        Self {
            tenant: tenant.to_string(),
            target_user_id: None,
        }
    }

    pub fn applies_to(&self, tenant: &str, user_id: Uuid) -> bool {
        self.tenant == tenant
            && self
                .target_user_id
                .map(|target_user_id| target_user_id == user_id)
                .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revocation_event_targets_current_selected_and_user_sessions() {
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

    #[test]
    fn permission_events_match_tenant_and_optional_user() {
        let user = Uuid::new_v4();
        assert!(PermissionChangeEvent::for_user("tenant-a", user).applies_to("tenant-a", user));
        assert!(!PermissionChangeEvent::for_user("tenant-a", user).applies_to("tenant-b", user));
        assert!(
            PermissionChangeEvent::for_all_users("tenant-a").applies_to("tenant-a", Uuid::new_v4())
        );
        assert!(!PermissionChangeEvent::for_all_users("tenant-a").applies_to("tenant-b", user));
    }
}
