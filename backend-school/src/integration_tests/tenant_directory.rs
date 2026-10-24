//! Loopback Admin transport and fixed-field observations for actual managed-worker fixtures.
use axum::{routing::get, Json, Router};
use school_tenancy::{AdminClient, AdminClientConfig};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::sync::Notify;
use uuid::Uuid;

pub(crate) struct DirectoryServer {
    mapping: Arc<Mutex<(Uuid, String)>>,
    blocked: Arc<AtomicBool>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
    calls: Arc<AtomicUsize>,
    address: std::net::SocketAddr,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for DirectoryServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl DirectoryServer {
    pub(crate) async fn start(id: Uuid, database_url: String, blocked: bool) -> Self {
        let mapping = Arc::new(Mutex::new((id, database_url)));
        let blocked = Arc::new(AtomicBool::new(blocked));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let (reply, pause, requested, proceed, count) = (
            mapping.clone(),
            blocked.clone(),
            entered.clone(),
            release.clone(),
            calls.clone(),
        );
        let app = Router::new().route("/internal/schools/{tenant}", get(move || {
            let (reply, pause, requested, proceed, count) = (reply.clone(), pause.clone(), requested.clone(), proceed.clone(), count.clone());
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if pause.load(Ordering::SeqCst) { requested.notify_one(); proceed.notified().await; }
                let (id, url) = reply.lock().unwrap().clone();
                Json(serde_json::json!({"id": id, "name": "synthetic", "db_connection_string": url}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            mapping,
            blocked,
            entered,
            release,
            calls,
            address,
            task,
        }
    }
    pub(crate) fn client(&self, capacity: usize, ttl: Duration) -> Arc<AdminClient> {
        Arc::new(AdminClient::new(
            format!("http://{}", self.address),
            "synthetic".into(),
            AdminClientConfig::for_tests(Duration::from_secs(3), 1, Duration::from_millis(1))
                .with_directory_for_tests(capacity, ttl),
        ))
    }
    pub(crate) async fn wait_for_request(&self) {
        tokio::time::timeout(Duration::from_secs(2), self.entered.notified())
            .await
            .unwrap();
    }
    pub(crate) fn unblock(&self) {
        self.blocked.store(false, Ordering::SeqCst);
        self.release.notify_one();
    }
    pub(crate) fn set_mapping(&self, id: Uuid, url: String) {
        *self.mapping.lock().unwrap() = (id, url);
    }
    pub(crate) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

pub(crate) async fn database_url(pool: &sqlx::PgPool) -> reqwest::Url {
    let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
        .fetch_one(pool)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
    url.query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema},public"));
    url
}

pub(crate) struct PhaseSignals {
    phase: &'static str,
    pub(crate) resolved: Notify,
    pub(crate) physical_wait: Notify,
}
impl PhaseSignals {
    pub(crate) fn for_phase(phase: &'static str) -> (Arc<Self>, tracing::Dispatch) {
        let signals = Arc::new(Self {
            phase,
            resolved: Notify::new(),
            physical_wait: Notify::new(),
        });
        let dispatcher = tracing::Dispatch::new(PhaseSubscriber(signals.clone()));
        (signals, dispatcher)
    }
}
struct PhaseSubscriber(Arc<PhaseSignals>);
impl tracing::Subscriber for PhaseSubscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target().starts_with("school_notifications")
            || metadata.target().starts_with("school_tenancy")
            || metadata.target().ends_with("services::cleaner")
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        #[derive(Default)]
        struct Fields {
            operation: String,
            phase: String,
        }
        impl tracing::field::Visit for Fields {
            fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                match field.name() {
                    "operation" => self.operation = value.into(),
                    "phase" => self.phase = value.into(),
                    _ => {}
                }
            }
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        if matches!(
            fields.operation.as_str(),
            "notification_database_phase" | "file_database_phase"
        ) && fields.phase == self.0.phase
        {
            self.0.resolved.notify_one();
        }
        if fields.operation == "tenant_physical_budget" && fields.phase == "waiting" {
            self.0.physical_wait.notify_one();
        }
    }
}
