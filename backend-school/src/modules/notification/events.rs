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
