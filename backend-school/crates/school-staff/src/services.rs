pub mod dashboard_service;
pub mod organization_delegation_service;
pub mod organization_member_service;
pub mod organization_permission_service;
pub mod organization_unit_service;
pub mod permission_service;
pub mod role_service;
pub mod staff_service;
pub mod user_role_service;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTransitionOutcome {
    Changed { is_active: bool },
    Unchanged,
}

impl StatusTransitionOutcome {
    pub fn changed(self) -> bool {
        matches!(self, Self::Changed { .. })
    }
}

#[cfg(test)]
mod status_tests;

#[cfg(test)]
mod tests {
    use super::StatusTransitionOutcome;

    #[test]
    fn status_transition_outcome_reports_only_real_changes() {
        assert!(StatusTransitionOutcome::Changed { is_active: false }.changed());
        assert!(StatusTransitionOutcome::Changed { is_active: true }.changed());
        assert!(!StatusTransitionOutcome::Unchanged.changed());
    }
}
