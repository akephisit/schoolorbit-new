pub mod models;
pub mod handlers;

use axum::routing::{get, post, put, delete};
use axum::Router;
use crate::AppState;

pub fn admission_routes() -> Router<AppState> {
    Router::new()
        // Admission Periods (รอบรับสมัคร)
        .route("/periods", get(handlers::list_periods).post(handlers::create_period))
        .route("/periods/{id}", get(handlers::get_period).put(handlers::update_period).delete(handlers::delete_period))
        .route("/periods/{id}/stats", get(handlers::get_period_stats))
        
        // Applications (ใบสมัคร)
        .route("/applications", get(handlers::list_applications).post(handlers::create_application))
        .route("/applications/{id}", get(handlers::get_application).put(handlers::update_application_status))
        .route("/applications/{id}/logs", get(handlers::get_application_logs))
        
        // Interviews (สัมภาษณ์)
        .route("/interviews", post(handlers::create_interview))
        .route("/interviews/{id}", put(handlers::update_interview))
        
        // Selections (รายชื่อผู้ผ่านคัดเลือก)
        .route("/periods/{id}/selections", get(handlers::list_selections).post(handlers::create_selections))
        .route("/selections/{id}/confirm", post(handlers::confirm_selection))
        
        // Generate student accounts
        .route("/periods/{id}/generate-students", post(handlers::generate_students))
}
