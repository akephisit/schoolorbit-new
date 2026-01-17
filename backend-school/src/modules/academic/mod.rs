pub mod models;
pub mod handlers;

use axum::routing::{get, post, put};
use axum::Router;
use crate::AppState;

pub fn academic_routes() -> Router<AppState> {
    Router::new()
        // Structure (Years, Levels, Semesters)
        .route("/structure", get(handlers::list_academic_structure))
        
        // Academic Years
        .route("/years", post(handlers::create_academic_year))
        .route("/years/{id}/active", put(handlers::toggle_active_year))

        // Classrooms
        .route("/classrooms", get(handlers::list_classrooms).post(handlers::create_classroom))
}
