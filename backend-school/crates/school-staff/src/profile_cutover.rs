use school_database::DbSession;
use school_errors::AppError;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCutoverStatus {
    pub status: String,
    pub migration_version: i64,
    pub passed: bool,
    pub checks: Vec<ProfileCutoverCheck>,
}
#[derive(Serialize)]
pub struct ProfileCutoverCheck {
    pub code: &'static str,
    pub passed: bool,
}
impl ProfileCutoverStatus {
    pub fn unavailable(version: i32) -> Self {
        Self {
            status: if version < 114 {
                "cutoverPending"
            } else {
                "failed"
            }
            .into(),
            migration_version: 114,
            passed: false,
            checks: Vec::new(),
        }
    }
}
pub async fn read_profile_cutover(pool: &PgPool) -> Result<ProfileCutoverStatus, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.profile_cutover.read_profile_cutover",
    )
    .await?;
    read_profile_cutover_on_connection(&mut connection).await
}

pub async fn read_profile_cutover_on_connection(
    connection: &mut sqlx::PgConnection,
) -> Result<ProfileCutoverStatus, AppError> {
    let (completed,old_columns,invalid_owners,views): (bool,i64,i64,bool)=DbSession::new(connection).query_as("staff.profile_cutover.read_profile_cutover_on_connection",
        "SELECT coalesce((SELECT completed AND completed_at IS NOT NULL AND users_fingerprint ~ '^[a-f0-9]{32}$' AND staff_fingerprint ~ '^[a-f0-9]{32}$' FROM staff_profile_cutover_audit WHERE migration_version=114),false),
        (SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='staff_info' AND column_name IN ('education_level','major','university')),
        (SELECT count(*) FROM staff_educations e JOIN users u ON u.id=e.staff_id WHERE u.user_type<>'staff'),
        to_regclass(current_schema()||'.staff_highest_education') IS NOT NULL AND to_regclass(current_schema()||'.user_display_names') IS NOT NULL")
        .fetch_one().await?;
    let checks = vec![
        ProfileCutoverCheck {
            code: "STAFF_PROFILE_VALUES_PRESERVED",
            passed: completed,
        },
        ProfileCutoverCheck {
            code: "STAFF_PROFILE_SINGLE_EDUCATION_REMOVED",
            passed: old_columns == 0,
        },
        ProfileCutoverCheck {
            code: "STAFF_PROFILE_EDUCATION_OWNERS_VALID",
            passed: invalid_owners == 0,
        },
        ProfileCutoverCheck {
            code: "STAFF_PROFILE_READ_PROJECTIONS_READY",
            passed: views,
        },
    ];
    let passed = checks.iter().all(|check| check.passed);
    Ok(ProfileCutoverStatus {
        status: if passed { "cutoverCompleted" } else { "failed" }.into(),
        migration_version: 114,
        passed,
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staff_profile_audit_unavailable_fails_closed() {
        assert_eq!(
            ProfileCutoverStatus::unavailable(113).status,
            "cutoverPending"
        );
        let status = ProfileCutoverStatus::unavailable(114);
        assert!(!status.passed);
        assert_eq!(status.status, "failed");
    }
}
