use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct SupervisionHttpState {
    pub auth_runtime: AuthRuntime,
}
