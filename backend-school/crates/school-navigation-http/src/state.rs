use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct NavigationHttpState {
    pub auth_runtime: AuthRuntime,
}
