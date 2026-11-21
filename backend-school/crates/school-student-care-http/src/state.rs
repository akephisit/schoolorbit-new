use school_auth::runtime::AuthRuntime;
#[derive(Clone)]
pub struct StudentCareHttpState {
    pub auth_runtime: AuthRuntime,
}
