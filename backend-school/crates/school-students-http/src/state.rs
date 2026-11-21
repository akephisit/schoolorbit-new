use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct StudentHttpState {
    pub auth_runtime: AuthRuntime,
    pub placement_rosters: &'static dyn school_academic_core::ports::PlacementRosterPort,
}
