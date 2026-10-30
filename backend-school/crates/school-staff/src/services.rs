use school_database::DbSession;
pub mod dashboard_service;
pub mod organization_delegation_service;
pub mod organization_member_service;
pub mod organization_permission_service;
pub mod organization_unit_service;
pub mod permission_service;
#[cfg(test)]
mod personnel_migration_test_support;
#[cfg(test)]
mod personnel_simplification_test_support;
pub mod role_service;
pub mod staff_career_service;
pub mod staff_education_service;
pub mod staff_info_service;
pub mod staff_license_service;
mod staff_name_service;
pub mod staff_service;
pub mod user_role_service;

/// Audit inserts reference the actor. Take that FK-compatible lock before any
/// resource lock, matching staff/member writers that serialize on the user first.
async fn lock_audit_actor(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor_user_id: uuid::Uuid,
) -> Result<(), school_errors::AppError> {
    let actor: Option<uuid::Uuid> = DbSession::new(&mut **tx)
        .scalar(
            "staff.services.lock_audit_actor",
            "SELECT id FROM users WHERE id = $1 FOR KEY SHARE",
        )
        .bind(actor_user_id)
        .fetch_optional()
        .await?;
    actor
        .map(|_| ())
        .ok_or_else(|| school_errors::AppError::NotFound("ไม่พบผู้ดำเนินการ".into()))
}

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
mod personnel_tests;

#[cfg(test)]
mod personnel_simplification_tests;

#[cfg(test)]
mod staff_career_tests;

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

pub mod job_position_service;
pub mod personnel_overview_service;
mod staff_directory_query;
pub use staff_directory_query::{
    collaboration_portraits_on_connection,
    subject_groups_for_users, subject_groups_for_users_on_connection, StaffSubjectGroupAssignment,
};

pub mod rank_milestone_service;

#[cfg(test)]
mod staff_profile_tests;

#[cfg(test)]
mod organization_membership_tests;
