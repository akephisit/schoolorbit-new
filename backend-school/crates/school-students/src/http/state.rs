use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct StudentHttpState {
    pub auth_runtime: AuthRuntime,
}
