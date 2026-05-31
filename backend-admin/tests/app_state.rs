use backend_admin::{build_app, AppState};
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
async fn app_can_be_built_from_provided_state() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://schoolorbit:schoolorbit@localhost/schoolorbit_test")
        .expect("test pool should be constructible without connecting");

    let state = AppState::new(pool);
    let _app = build_app(state);
}
