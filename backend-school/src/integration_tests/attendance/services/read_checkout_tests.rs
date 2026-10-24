//! Bounded real-provider diagnostics: no HTTP, workload generator, or private SQL output.
use super::*;
use sqlx::postgres::{PgPoolOptions, PgSslMode};
use std::{
    io,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Notify, Semaphore},
    task::{JoinHandle, JoinSet},
};

struct Counts {
    query: AtomicU64,
    execute: AtomicU64,
    sync: AtomicU64,
    standalone_sync: AtomicU64,
    ready: AtomicU64,
    checkouts: AtomicU64,
    hold_ns: AtomicU64,
    held_since: Mutex<Option<Instant>>,
    pause: AtomicBool,
    paused: Notify,
    resume: Semaphore,
}

impl Default for Counts {
    fn default() -> Self {
        Self {
            query: AtomicU64::new(0),
            execute: AtomicU64::new(0),
            sync: AtomicU64::new(0),
            standalone_sync: AtomicU64::new(0),
            ready: AtomicU64::new(0),
            checkouts: AtomicU64::new(0),
            hold_ns: AtomicU64::new(0),
            held_since: Mutex::new(None),
            pause: AtomicBool::new(false),
            paused: Notify::new(),
            resume: Semaphore::new(0),
        }
    }
}

#[derive(Debug)]
struct Sample {
    query: u64,
    execute: u64,
    sync: u64,
    standalone_sync: u64,
    ready: u64,
    checkouts: u64,
    hold_ms: f64,
    wall_ms: f64,
}

impl Counts {
    fn reset(&self) {
        for count in [
            &self.query,
            &self.execute,
            &self.sync,
            &self.standalone_sync,
            &self.ready,
            &self.checkouts,
            &self.hold_ns,
        ] {
            count.store(0, Ordering::SeqCst);
        }
        assert!(self.held_since.lock().unwrap().is_none());
    }

    fn sample(&self, wall: Duration) -> Sample {
        Sample {
            query: self.query.load(Ordering::SeqCst),
            execute: self.execute.load(Ordering::SeqCst),
            sync: self.sync.load(Ordering::SeqCst),
            standalone_sync: self.standalone_sync.load(Ordering::SeqCst),
            ready: self.ready.load(Ordering::SeqCst),
            checkouts: self.checkouts.load(Ordering::SeqCst),
            hold_ms: self.hold_ns.load(Ordering::SeqCst) as f64 / 1_000_000.0,
            wall_ms: wall.as_secs_f64() * 1000.0,
        }
    }
}

struct ProtocolProxy {
    port: u16,
    counts: Arc<Counts>,
    task: JoinHandle<()>,
}

impl Drop for ProtocolProxy {
    fn drop(&mut self) {
        // Dropping JoinSet in the accept task aborts every owned forwarding task.
        self.task.abort();
    }
}

async fn frame<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> io::Result<(u8, Vec<u8>)> {
    let tag = reader.read_u8().await?;
    let length = reader.read_u32().await?;
    if !(4..=16 * 1024 * 1024).contains(&length) {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    let mut bytes = Vec::with_capacity(length as usize + 1);
    bytes.push(tag);
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.resize(length as usize + 1, 0);
    reader.read_exact(&mut bytes[5..]).await?;
    Ok((tag, bytes))
}

async fn forward(
    mut client: TcpStream,
    upstream: std::net::SocketAddr,
    counts: Arc<Counts>,
    delay: Duration,
) -> io::Result<()> {
    client.set_nodelay(true)?;
    let mut server = TcpStream::connect(upstream).await?;
    server.set_nodelay(true)?;
    // Tests explicitly disable TLS. Startup and CancelRequest are untagged packets.
    let length = client.read_u32().await?;
    if !(8..=65536).contains(&length) {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    let mut startup = vec![0; length as usize];
    startup[..4].copy_from_slice(&length.to_be_bytes());
    client.read_exact(&mut startup[4..]).await?;
    let code = u32::from_be_bytes(startup[4..8].try_into().unwrap());
    if code != 196608 && code != 80877102 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    server.write_all(&startup).await?;
    if code == 80877102 {
        server.shutdown().await?;
        return Ok(());
    }
    let (mut cr, mut cw) = client.into_split();
    let (mut sr, mut sw) = server.into_split();
    let frontend_counts = counts.clone();
    let frontend = async move {
        let mut executed = false;
        loop {
            let (tag, bytes) = frame(&mut cr).await?;
            match tag {
                b'Q' => {
                    frontend_counts.query.fetch_add(1, Ordering::SeqCst);
                    executed = false;
                }
                b'E' => {
                    frontend_counts.execute.fetch_add(1, Ordering::SeqCst);
                    executed = true;
                }
                b'S' => {
                    frontend_counts.sync.fetch_add(1, Ordering::SeqCst);
                    if !executed {
                        frontend_counts
                            .standalone_sync
                            .fetch_add(1, Ordering::SeqCst);
                    }
                    executed = false;
                }
                _ => {}
            }
            sw.write_all(&bytes).await?;
            if tag == b'X' {
                return Ok::<(), io::Error>(());
            }
        }
    };
    let backend = async move {
        loop {
            let (tag, bytes) = frame(&mut sr).await?;
            if tag == b'Z' {
                counts.ready.fetch_add(1, Ordering::SeqCst);
                if counts.query.load(Ordering::SeqCst) + counts.execute.load(Ordering::SeqCst) > 0
                    && counts.pause.swap(false, Ordering::SeqCst)
                {
                    counts.paused.notify_one();
                    counts.resume.acquire().await.unwrap().forget();
                }
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
            cw.write_all(&bytes).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), io::Error>(())
    };
    // Either EOF/cancellation drops the opposite forwarding future immediately.
    tokio::select! { result = frontend => result, result = backend => result }
}

impl ProtocolProxy {
    async fn start(options: &sqlx::postgres::PgConnectOptions, delay: Duration) -> Self {
        let upstream = format!("{}:{}", options.get_host(), options.get_port())
            .parse::<std::net::SocketAddr>()
            .expect("owned fixture must use a numeric local TCP address");
        assert!(upstream.ip().is_loopback());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let counts = Arc::new(Counts::default());
        let captured = counts.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (client, _) = accepted.unwrap();
                        connections.spawn(forward(client, upstream, captured.clone(), delay));
                    }
                    Some(result) = connections.join_next(), if !connections.is_empty() => {
                        if let Ok(Err(error)) = result {
                            assert!(matches!(error.kind(), io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe));
                        }
                    }
                }
            }
        });
        Self { port, counts, task }
    }

    async fn pool(&self, options: sqlx::postgres::PgConnectOptions) -> PgPool {
        let checked = self.counts.clone();
        let released = self.counts.clone();
        PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .test_before_acquire(true)
            .before_acquire(move |_, _| {
                checked.checkouts.fetch_add(1, Ordering::SeqCst);
                assert!(checked
                    .held_since
                    .lock()
                    .unwrap()
                    .replace(Instant::now())
                    .is_none());
                Box::pin(async { Ok(true) })
            })
            .after_release(move |_, _| {
                if let Some(started) = released.held_since.lock().unwrap().take() {
                    released
                        .hold_ns
                        .fetch_add(started.elapsed().as_nanos() as u64, Ordering::SeqCst);
                }
                Box::pin(async { Ok(true) })
            })
            .connect_with(
                options
                    .host("127.0.0.1")
                    .port(self.port)
                    .ssl_mode(PgSslMode::Disable),
            )
            .await
            .unwrap()
    }
}

async fn drained(pool: &PgPool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while pool.num_idle() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("single owned connection must return after rollback/health check");
}

#[tokio::test]
async fn attendance_read_checkout_protocol_baseline_and_waiter() {
    tokio::time::timeout(Duration::from_secs(60), baseline())
        .await
        .expect("bounded local protocol fixture must terminate");
}

async fn baseline() {
    let f = db_tests::fixture("attendance_read_checkout_protocol").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    sqlx::query("UPDATE attendance_module_state SET coverage_start=$1")
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    let room: Uuid = sqlx::query_scalar("SELECT homeroom_id FROM homeroom_placements WHERE student_academic_year_id=$1 AND status='current'")
        .bind(f.students[0].student_academic_year_id).fetch_one(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO homeroom_advisors(homeroom_id,user_id,role) VALUES($1,$2,'primary') ON CONFLICT(homeroom_id,user_id) DO NOTHING")
        .bind(room).bind(db_tests::TEACHER).execute(&f.pool).await.unwrap();
    let mut actor = ActorContext {
        user_id: db_tests::TEACHER,
        permissions: vec![
            codes::ATTENDANCE_READ_SCHOOL.into(),
            codes::ATTENDANCE_UPDATE_ASSIGNED.into(),
        ],
    };
    let context = OpenAttendanceSession {
        academic_term_id: db_tests::TERM,
        date,
        kind: AttendanceKind::Arrival,
        source_key: room.to_string(),
    };
    let query = AttendanceHistoryQuery {
        academic_term_id: db_tests::TERM,
        student_id: db_tests::STUDENT,
        start: date,
        end: date,
    };
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let options = (*f.pool.connect_options())
        .clone()
        .options([("search_path", format!("{schema},public"))]);
    let proxy = ProtocolProxy::start(&options, Duration::from_millis(1)).await;
    let pool = proxy.pool(options).await;
    // Warm the exact providers/statements; setup and authentication are outside measurement.
    let expected_history = history(&pool, &actor, query.clone()).await.unwrap();
    let expected_preview = open(&pool, &actor, context.clone()).await.unwrap();
    drained(&pool).await;
    proxy.counts.reset();
    let started = Instant::now();
    let actual_history = history(&pool, &actor, query.clone()).await.unwrap();
    drained(&pool).await;
    let history_sample = proxy.counts.sample(started.elapsed());
    assert_eq!(
        serde_json::to_value(&actual_history).unwrap(),
        serde_json::to_value(&expected_history).unwrap()
    );
    assert_eq!(history_sample.checkouts, 1);
    assert!(
        history_sample.query + history_sample.execute <= 27,
        "history must not reload the same snapshot settings"
    );
    assert!(history_sample.ready <= 29);
    assert!(history_sample.query + history_sample.execute > 0);
    assert!(history_sample.standalone_sync >= 2);
    assert!(history_sample.ready >= history_sample.query + history_sample.execute);
    assert!(history_sample.sync >= history_sample.standalone_sync);
    assert!(history_sample.hold_ms > 0.0 && history_sample.wall_ms >= history_sample.hold_ms);
    eprintln!("attendance_read_checkout history {history_sample:?}");
    proxy.counts.reset();
    let started = Instant::now();
    let actual_preview = open(&pool, &actor, context.clone()).await.unwrap();
    drained(&pool).await;
    let preview_sample = proxy.counts.sample(started.elapsed());
    assert_eq!(
        serde_json::to_value(&actual_preview).unwrap(),
        serde_json::to_value(&expected_preview).unwrap()
    );
    assert_eq!(
        preview_sample.checkouts, 1,
        "preview must retain its one snapshot connection"
    );
    assert_eq!(preview_sample.standalone_sync, 2);
    assert!(
        preview_sample.ready <= 18,
        "preview must avoid repeated checkout/return health checks"
    );
    assert!(preview_sample.query + preview_sample.execute > 0);
    assert!(preview_sample.ready >= preview_sample.query + preview_sample.execute);
    eprintln!("attendance_read_checkout preview {preview_sample:?}");

    // One occupied slot and exactly one waiting checkout; no arrival/workload loop.
    proxy.counts.reset();
    proxy.counts.pause.store(true, Ordering::SeqCst);
    let reader_pool = pool.clone();
    let reader_query = query.clone();
    let reader = tokio::spawn(async move { history(&reader_pool, &actor, reader_query).await });
    proxy.counts.paused.notified().await;
    let waiter_pool = pool.clone();
    let waiter = tokio::spawn(async move {
        let started = Instant::now();
        let mut connection = waiter_pool.acquire().await.unwrap();
        let waited_ms = started.elapsed().as_secs_f64() * 1000.0;
        let (no_write, transaction_reset): (bool, bool) = sqlx::query_as(
            "SELECT NOT EXISTS(SELECT 1 FROM attendance_sessions), current_setting('transaction_read_only')='off'",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        (waited_ms, no_write, transaction_reset)
    });
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert!(
        !waiter.is_finished(),
        "occupied pool slot cannot be borrowed twice"
    );
    proxy.counts.resume.add_permits(1);
    let returned_history = reader.await.unwrap().unwrap();
    let (waited_ms, no_write, transaction_reset) = waiter.await.unwrap();
    assert!(
        transaction_reset,
        "read-only report transaction must roll back before reuse"
    );
    assert!(no_write, "preview and report must not materialize rounds");
    assert_eq!(
        serde_json::to_value(returned_history).unwrap(),
        serde_json::to_value(expected_history).unwrap()
    );
    eprintln!("attendance_read_checkout single_waiter waited_ms={waited_ms:.3}");
    drained(&pool).await;
    // Removing permissions still denies the real owner call; no fixture-only bypass.
    actor = ActorContext {
        user_id: db_tests::TEACHER,
        permissions: vec![],
    };
    assert!(matches!(
        history(&pool, &actor, query).await,
        Err(AppError::Forbidden(_))
    ));
    drained(&pool).await;
    pool.close().await;
    f.pool.close().await;
}
