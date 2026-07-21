use super::models::Notification;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TenantNotificationEvent {
    pub tenant: String,
    pub user_id: Uuid,
    pub notification: Notification,
}

impl TenantNotificationEvent {
    pub fn new(tenant: &str, user_id: Uuid, notification: Notification) -> Self {
        Self {
            tenant: tenant.to_string(),
            user_id,
            notification,
        }
    }

    pub fn applies_to(&self, tenant: &str, user_id: Uuid) -> bool {
        self.tenant == tenant && self.user_id == user_id
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WorkChangeKind {
    WorkItemsChanged,
    WorkflowWindowChanged,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkChangeEvent {
    pub tenant: String,
    pub kind: WorkChangeKind,
}

impl WorkChangeEvent {
    pub fn work_items_changed(tenant: &str) -> Self {
        Self {
            tenant: tenant.to_string(),
            kind: WorkChangeKind::WorkItemsChanged,
        }
    }

    pub fn workflow_window_changed(tenant: &str) -> Self {
        Self {
            tenant: tenant.to_string(),
            kind: WorkChangeKind::WorkflowWindowChanged,
        }
    }

    pub fn applies_to(&self, tenant: &str) -> bool {
        self.tenant == tenant
    }

    pub fn event_name(self) -> &'static str {
        match self.kind {
            WorkChangeKind::WorkItemsChanged => "work_items_changed",
            WorkChangeKind::WorkflowWindowChanged => "workflow_window_changed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_notification() -> Notification {
        Notification {
            id: Uuid::nil(),
            title: "Test".into(),
            message: "Message".into(),
            type_: "info".into(),
            link: None,
            read_at: None,
            created_at: Utc::now(),
        }
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

    #[test]
    fn work_events_match_only_their_tenant() {
        let event = WorkChangeEvent::work_items_changed("tenant-a");
        assert!(event.applies_to("tenant-a"));
        assert!(!event.applies_to("tenant-b"));
    }

    #[test]
    fn notification_events_match_tenant_and_user() {
        let user = Uuid::new_v4();
        let other_user = Uuid::new_v4();
        let notification = test_notification();
        let event = TenantNotificationEvent::new("tenant-a", user, notification);
        assert!(event.applies_to("tenant-a", user));
        assert!(!event.applies_to("tenant-b", user));
        assert!(!event.applies_to("tenant-a", other_user));
    }
}
