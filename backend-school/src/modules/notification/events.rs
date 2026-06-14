use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PermissionChangeEvent {
    pub target_user_id: Option<Uuid>,
}

impl PermissionChangeEvent {
    pub fn for_user(user_id: Uuid) -> Self {
        Self {
            target_user_id: Some(user_id),
        }
    }

    pub fn for_all_users() -> Self {
        Self {
            target_user_id: None,
        }
    }

    pub fn applies_to(&self, user_id: Uuid) -> bool {
        self.target_user_id
            .map(|target_user_id| target_user_id == user_id)
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WorkChangeKind {
    WorkItemsChanged,
    WorkflowWindowChanged,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WorkChangeEvent {
    pub kind: WorkChangeKind,
}

impl WorkChangeEvent {
    pub fn work_items_changed() -> Self {
        Self {
            kind: WorkChangeKind::WorkItemsChanged,
        }
    }

    pub fn workflow_window_changed() -> Self {
        Self {
            kind: WorkChangeKind::WorkflowWindowChanged,
        }
    }

    pub fn event_name(self) -> &'static str {
        match self.kind {
            WorkChangeKind::WorkItemsChanged => "work_items_changed",
            WorkChangeKind::WorkflowWindowChanged => "workflow_window_changed",
        }
    }
}
