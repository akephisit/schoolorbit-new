use super::{handlers, state::WorkflowHttpState, work_handlers};
use axum::Router;
pub fn workflow_routes() -> Router<WorkflowHttpState> {
    handlers::routes()
}
pub fn work_routes() -> Router<WorkflowHttpState> {
    work_handlers::routes()
}
