
// SSE endpoint for creating school with real-time logs
pub async fn create_school_sse(
    Json(data): Json<CreateSchool>,
) -> axum::response::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::ReceiverStream;
    use crate::utils::sse::SseLogger;

    let (tx, rx) = mpsc::channel(100);
    let logger = SseLogger::new(tx);

    // Spawn background task
    tokio::spawn(async move {
        let pool = match DB_POOL.get() {
            Some(pool) => pool.clone(),
            None => {
                let _ = logger.error_complete("Database not initialized".to_string()).await;
                return;
            }
        };

        let service = SchoolService::new(pool);

        if let Err(e) = service.create_school_stream(data, logger.clone()).await {
            let _ = logger.error_complete(e.to_string()).await;
        }
    });

    axum::response::Sse::new(ReceiverStream::new(rx))
}

// SSE endpoint for deleting school with real-time logs
pub async fn delete_school_sse(
    Path(id): Path<Uuid>,
) -> axum::response::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::ReceiverStream;
    use crate::utils::sse::SseLogger;

    let (tx, rx) = mpsc::channel(100);
    let logger = SseLogger::new(tx);

    tokio::spawn(async move {
        let pool = match DB_POOL.get() {
            Some(pool) => pool.clone(),
            None => {
                let _ = logger.error_complete("Database not initialized".to_string()).await;
                return;
            }
        };

        let service = SchoolService::new(pool);

        if let Err(e) = service.delete_school_stream(id, logger.clone()).await {
            let _ = logger.error_complete(e.to_string()).await;
        }
    });

    axum::response::Sse::new(ReceiverStream::new(rx))
}
