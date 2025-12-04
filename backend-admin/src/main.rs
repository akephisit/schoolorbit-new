use ohkami::{Ohkami, Route};
use ohkami::claw::{Path, status};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct ApiResponse {
    success: bool,
    data: Option<String>,
    message: String,
}

async fn health_check() -> status::OK<String> {
    let response = HealthResponse {
        status: "ok".to_string(),
        message: "SchoolOrbit Backend is running".to_string(),
    };
    status::OK(serde_json::to_string(&response).unwrap())
}

async fn hello(Path(name): Path<&str>) -> String {
    format!("Hello, {}! Welcome to SchoolOrbit API", name)
}

async fn get_api_info() -> status::OK<String> {
    let response = ApiResponse {
        success: true,
        data: Some("SchoolOrbit API v0.1.0".to_string()),
        message: "API is running successfully".to_string(),
    };
    status::OK(serde_json::to_string(&response).unwrap())
}

#[tokio::main]
async fn main() {
    println!("🚀 Starting SchoolOrbit Backend Server...");

    Ohkami::new((
        "/".GET(get_api_info),
        "/health".GET(health_check),
        "/api/hello/:name".GET(hello),
    ))
    .howl("localhost:8080")
    .await
}
