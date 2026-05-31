use crate::models::CreateSchool;
use crate::services::SchoolService;
use crate::AppState;
use axum::{
    extract::{Path, State},
    response::Json,
};
use uuid::Uuid;

// SSE endpoint for creating school with real-time logs
pub async fn create_school_sse(
    State(state): State<AppState>,
    Json(data): Json<CreateSchool>,
) -> axum::response::Sse<
    impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use crate::utils::sse::SseLogger;
    use axum::response::sse::KeepAlive;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::ReceiverStream;

    let (tx, rx) = mpsc::channel(100);
    let logger = SseLogger::new(tx);
    let pool = state.pool.clone();

    // Spawn background task
    tokio::spawn(async move {
        let service = SchoolService::new(pool);

        if let Err(e) = service.create_school_stream(data, logger.clone()).await {
            let _ = logger.error_complete(e.to_string()).await;
        }
    });

    axum::response::Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(5)))
}

// SSE endpoint for deleting school with real-time logs
pub async fn delete_school_sse(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> axum::response::Sse<
    impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use crate::utils::sse::SseLogger;
    use axum::response::sse::KeepAlive;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::ReceiverStream;

    let (tx, rx) = mpsc::channel(100);
    let logger = SseLogger::new(tx);
    let pool = state.pool.clone();

    tokio::spawn(async move {
        let service = SchoolService::new(pool);

        if let Err(e) = service.delete_school_stream(id, logger.clone()).await {
            let _ = logger.error_complete(e.to_string()).await;
        }
    });

    axum::response::Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(5)))
}
