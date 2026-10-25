//! Real offline CLI acceptance on canonical disposable tenant databases, never provider I/O.
use school_file_platform::{
    namespace_binding::{self, ConfiguredNamespaces},
    r2_storage_provider::R2StorageRouting,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{collections::BTreeMap, path::PathBuf, process::Output};
use tokio::process::Command;
use uuid::Uuid;

struct Tenant {
    id: Uuid,
    database: String,
    owner: String,
    runtime: String,
    runtime_url: String,
    url: String,
    pool: PgPool,
}
struct Fixture {
    admin: PgPool,
    tenants: Vec<Tenant>,
    directory: PathBuf,
}
impl Fixture {
    async fn new() -> Self {
        Self::at_version(school_migrations::latest_migration_version()).await
    }
    async fn at_version(version: i64) -> Self {
        let url = std::env::var("TEST_DATABASE_URL").expect("owned disposable database required");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        let mut tenants = Vec::new();
        for _ in 0..2 {
            let id = Uuid::new_v4();
            let database = format!("watch_gate_{}", id.simple());
            let owner = format!("so_{}_owner", id.simple());
            let runtime = format!("so_{}_runtime", id.simple());
            let owner_password = format!("synthetic-{}", Uuid::new_v4().simple());
            let runtime_password = format!("synthetic-{}", Uuid::new_v4().simple());
            // UUID-only identifiers, synthetic credentials and exact canonical Admin markers.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE ROLE {runtime} LOGIN PASSWORD '{runtime_password}'"
            )))
            .execute(&admin)
            .await
            .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "COMMENT ON ROLE {runtime} IS 'schoolorbit:{id}:runtime'"
            )))
            .execute(&admin)
            .await
            .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE ROLE {owner} LOGIN PASSWORD '{owner_password}'"
            )))
            .execute(&admin)
            .await
            .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "COMMENT ON ROLE {owner} IS 'schoolorbit:{id}:migration'"
            )))
            .execute(&admin)
            .await
            .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE DATABASE {database} OWNER {owner}"
            )))
            .execute(&admin)
            .await
            .unwrap();
            let mut tenant_url = url::Url::parse(&url).unwrap();
            tenant_url.set_path(&format!("/{database}"));
            tenant_url
                .query_pairs_mut()
                .append_pair("options", "-csearch_path=public");
            tenant_url.set_username(&owner).unwrap();
            tenant_url.set_password(Some(&owner_password)).unwrap();
            let mut runtime_url = tenant_url.clone();
            runtime_url.set_username(&runtime).unwrap();
            runtime_url.set_password(Some(&runtime_password)).unwrap();
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(tenant_url.as_str())
                .await
                .unwrap();
            sqlx::query("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\" WITH SCHEMA public")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public")
                .execute(&pool)
                .await
                .unwrap();
            if version == school_migrations::latest_migration_version() {
                school_test_db::run_test_migrations(&pool).await;
            } else {
                school_test_db::cutover_test_support::apply_migrations_through(&pool, version)
                    .await
                    .unwrap();
            }
            tenants.push(Tenant {
                id,
                database,
                owner,
                runtime,
                runtime_url: runtime_url.to_string(),
                url: tenant_url.to_string(),
                pool,
            });
        }
        let directory =
            std::env::temp_dir().join(format!("schoolorbit-watch-cli-{}", Uuid::new_v4().simple()));
        std::fs::create_dir(&directory).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        // Stop dotenv discovery before it can find any repository/host configuration.
        std::fs::write(directory.join(".env"), b"").unwrap();
        Self {
            admin,
            tenants,
            directory,
        }
    }
    fn inventory(&self) -> Value {
        json!({"tenants": self.tenants.iter().enumerate().map(|(index,tenant)|json!({"tenant_id":tenant.id,"database_url_env":format!("FILE_STORAGE_BINDING_URL_{index}"),"r2_account_id":"synthetic-account","public_bucket_name":"public-fixture","private_bucket_name":"private-fixture"})).collect::<Vec<_>>()})
    }
    fn environment(&self) -> BTreeMap<String, String> {
        self.tenants
            .iter()
            .enumerate()
            .map(|(index, tenant)| {
                (
                    format!("FILE_STORAGE_BINDING_URL_{index}"),
                    tenant.url.clone(),
                )
            })
            .collect()
    }
    async fn check(
        &self,
        inventory: &Value,
        digest: Option<String>,
        environment: &BTreeMap<String, String>,
    ) -> Output {
        let bytes = serde_json::to_vec(inventory).unwrap();
        let path = self.directory.join("inventory.json");
        std::fs::write(&path, &bytes).unwrap();
        let digest = digest.unwrap_or_else(|| hex::encode(Sha256::digest(&bytes)));
        self.command(
            &[
                "check",
                "--inventory",
                path.to_str().unwrap(),
                "--expected-inventory-sha256",
                &digest,
            ],
            environment,
        )
        .await
    }
    async fn command(&self, args: &[&str], environment: &BTreeMap<String, String>) -> Output {
        tokio::time::timeout(
            std::time::Duration::from_secs(45),
            Command::new(env!("CARGO_BIN_EXE_file_storage_namespace"))
                .args(args)
                .current_dir(&self.directory)
                .env_clear()
                .envs(environment)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .expect("offline CLI must finish its bounded fixture work")
        .unwrap()
    }
    async fn main_command(&self, args: &[&str], environment: &BTreeMap<String, String>) -> Output {
        tokio::time::timeout(
            std::time::Duration::from_secs(45),
            Command::new(env!("CARGO_BIN_EXE_backend-school"))
                .args(args)
                .current_dir(&self.directory)
                .env_clear()
                .envs(environment)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .expect("headless operation must finish without starting a server")
        .unwrap()
    }
    async fn history_snapshot(&self) -> Value {
        let mut states = Vec::new();
        for tenant in &self.tenants {
            let state:Value=sqlx::query_scalar("SELECT jsonb_build_object('history',(SELECT jsonb_agg(to_jsonb(m) ORDER BY version) FROM _sqlx_migrations m),'permissions',(SELECT coalesce(jsonb_agg(to_jsonb(p) ORDER BY id),'[]'::jsonb) FROM permissions p))").fetch_one(&tenant.pool).await.unwrap();
            states.push(state);
        }
        json!(states)
    }
    async fn snapshot(&self) -> Value {
        let mut states = Vec::new();
        for tenant in &self.tenants {
            let state:Value=sqlx::query_scalar("SELECT jsonb_build_object('watches',(SELECT coalesce(jsonb_agg(to_jsonb(w) ORDER BY w.id),'[]'::jsonb) FROM file_storage_cleanup_watches w),'files',(SELECT coalesce(jsonb_agg(to_jsonb(f) ORDER BY f.id),'[]'::jsonb) FROM files f),'versions',(SELECT coalesce(jsonb_agg(to_jsonb(v) ORDER BY v.id),'[]'::jsonb) FROM file_versions v),'operations',(SELECT coalesce(jsonb_agg(to_jsonb(o) ORDER BY o.id),'[]'::jsonb) FROM file_operations o),'permissions',(SELECT coalesce(jsonb_agg(to_jsonb(p) ORDER BY p.id),'[]'::jsonb) FROM permissions p),'migrations',(SELECT jsonb_agg(to_jsonb(m) ORDER BY m.version) FROM _sqlx_migrations m),'acl',(SELECT relacl::text FROM pg_class WHERE oid='file_storage_cleanup_watches'::regclass),'owner',(SELECT datdba::bigint FROM pg_database WHERE datname=current_database()),'role',(SELECT shobj_description(oid,'pg_authid') FROM pg_roles WHERE rolname=$1))")
                .bind(&tenant.owner).fetch_one(&tenant.pool).await.unwrap();
            states.push(state);
        }
        json!(states)
    }
    async fn finish(self) {
        for tenant in self.tenants {
            tenant.pool.close().await;
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE {} WITH (FORCE)",
                tenant.database
            )))
            .execute(&self.admin)
            .await
            .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!("DROP ROLE {}", tenant.owner)))
                .execute(&self.admin)
                .await
                .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!("DROP ROLE {}", tenant.runtime)))
                .execute(&self.admin)
                .await
                .unwrap();
        }
        self.admin.close().await;
        std::fs::remove_dir_all(self.directory).unwrap();
    }
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn rejected(output: &Output) {
    assert!(
        !output.status.success(),
        "unsafe inventory or binding was accepted"
    );
}
async fn legacy_watch(pool: &PgPool) -> Uuid {
    let file = Uuid::new_v4();
    let version = Uuid::new_v4();
    sqlx::query("INSERT INTO files(id,purpose_code,display_filename,visibility,lifecycle_status,retention_class) VALUES($1,'certificate_template_image','synthetic.png','private','processing','temporary')").bind(file).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status) VALUES($1,$2,1,'r2','private',$3,'image/png','png',4,repeat('a',64),'clean')")
        .bind(version).bind(file).bind(format!("tenants/{}/certificates/templates/{file}/v1/original.png",Uuid::new_v4())).execute(pool).await.unwrap();
    sqlx::query_scalar("SELECT id FROM file_storage_cleanup_watches WHERE target_id=$1")
        .bind(version)
        .fetch_one(pool)
        .await
        .unwrap()
}
fn namespaces() -> ConfiguredNamespaces {
    ConfiguredNamespaces::from_r2_routing(
        &R2StorageRouting::from_values("synthetic-account", "public-fixture", "private-fixture")
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn whole_inventory_gate_checks_real_identity_schema_and_every_retained_watch_read_only() {
    let fixture = Fixture::new().await;
    let inventory = fixture.inventory();
    let environment = fixture.environment();
    let before = fixture.snapshot().await;
    let output = fixture.check(&inventory, None, &environment).await;
    success(&output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("tenants=2 checked=2 failed=0"));
    rejected(
        &fixture
            .check(&inventory, Some("a".repeat(64)), &environment)
            .await,
    );
    let trusted_digest = hex::encode(Sha256::digest(serde_json::to_vec(&inventory).unwrap()));
    let mut omitted = inventory.clone();
    omitted["tenants"].as_array_mut().unwrap().pop();
    rejected(
        &fixture
            .check(&omitted, Some(trusted_digest), &environment)
            .await,
    );
    let mut missing = environment.clone();
    missing.remove("FILE_STORAGE_BINDING_URL_1");
    rejected(&fixture.check(&inventory, None, &missing).await);
    let mut nonexistent = environment.clone();
    let mut missing_url = url::Url::parse(&fixture.tenants[1].url).unwrap();
    missing_url.set_path(&format!("/not_created_{}", Uuid::new_v4().simple()));
    nonexistent.insert("FILE_STORAGE_BINDING_URL_1".into(), missing_url.to_string());
    let output = fixture.check(&inventory, None, &nonexistent).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("tenants=2 checked=1 failed=1"));
    let mut duplicate = environment.clone();
    duplicate.insert(
        "FILE_STORAGE_BINDING_URL_1".into(),
        fixture.tenants[0].url.clone(),
    );
    rejected(&fixture.check(&inventory, None, &duplicate).await);
    let mut swapped = environment.clone();
    swapped.insert(
        "FILE_STORAGE_BINDING_URL_0".into(),
        fixture.tenants[1].url.clone(),
    );
    swapped.insert(
        "FILE_STORAGE_BINDING_URL_1".into(),
        fixture.tenants[0].url.clone(),
    );
    let output = fixture.check(&inventory, None, &swapped).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("tenants=2 checked=0 failed=2"));
    assert_eq!(before, fixture.snapshot().await);

    // Missing latest schema in just the second tenant must block the complete gate.
    sqlx::query("ALTER TABLE file_storage_cleanup_watches RENAME COLUMN prefer_watch_next TO unavailable_scheduler_column").execute(&fixture.tenants[1].pool).await.unwrap();
    let output = fixture.check(&inventory, None, &environment).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("tenants=2 checked=1 failed=1"));
    sqlx::query("ALTER TABLE file_storage_cleanup_watches RENAME COLUMN unavailable_scheduler_column TO prefer_watch_next").execute(&fixture.tenants[1].pool).await.unwrap();
    // A present scheduler column is insufficient: validate the immutable complete history too.
    let history:(i64,String,chrono::DateTime<chrono::Utc>,bool,Vec<u8>,i64)=sqlx::query_as("SELECT version,description,installed_on,success,checksum,execution_time FROM _sqlx_migrations WHERE version=137").fetch_one(&fixture.tenants[1].pool).await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET success=false WHERE version=137")
        .execute(&fixture.tenants[1].pool)
        .await
        .unwrap();
    let before = fixture.snapshot().await;
    let output = fixture.check(&inventory, None, &environment).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("tenants=2 checked=1 failed=1"));
    assert_eq!(before, fixture.snapshot().await);
    sqlx::query("UPDATE _sqlx_migrations SET success=$1,checksum=decode(repeat('00',48),'hex') WHERE version=137").bind(history.3).execute(&fixture.tenants[1].pool).await.unwrap();
    let before = fixture.snapshot().await;
    let output = fixture.check(&inventory, None, &environment).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("tenants=2 checked=1 failed=1"));
    assert_eq!(before, fixture.snapshot().await);
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version=137")
        .execute(&fixture.tenants[1].pool)
        .await
        .unwrap();
    let before = fixture.snapshot().await;
    let output = fixture.check(&inventory, None, &environment).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("tenants=2 checked=1 failed=1"));
    assert_eq!(before, fixture.snapshot().await);
    sqlx::query("INSERT INTO _sqlx_migrations(version,description,installed_on,success,checksum,execution_time) VALUES($1,$2,$3,$4,$5,$6)").bind(history.0).bind(history.1).bind(history.2).bind(history.3).bind(history.4).bind(history.5).execute(&fixture.tenants[1].pool).await.unwrap();
    success(&fixture.check(&inventory, None, &environment).await);
    let watch = legacy_watch(&fixture.tenants[1].pool).await;
    let before = fixture.snapshot().await;
    let output = fixture.check(&inventory, None, &environment).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("unresolved=1"));
    assert_eq!(before, fixture.snapshot().await);
    let manifest = namespace_binding::prepare_binding_manifest(
        &fixture.tenants[1].pool,
        &[watch],
        "b".repeat(64),
        &namespaces(),
    )
    .await
    .unwrap();
    namespace_binding::bind_attested_namespaces(
        &fixture.tenants[1].pool,
        &manifest,
        &namespaces(),
        true,
    )
    .await
    .unwrap();
    success(&fixture.check(&inventory, None, &environment).await);
    let mut rotated = inventory.clone();
    rotated["tenants"][1]["private_bucket_name"] = json!("other-private-fixture");
    let before = fixture.snapshot().await;
    let output = fixture.check(&rotated, None, &environment).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("mismatched=1"));
    assert_eq!(before, fixture.snapshot().await);
    // Even a bound locator with matching namespace cannot hide an unsupported provider.
    sqlx::query("INSERT INTO file_storage_cleanup_watches(file_id,target_kind,target_id,provider_code,storage_class,object_key,content_type,namespace_fingerprint,namespace_bound_at) SELECT $1,'version',$2,'unsupported',storage_class,object_key,content_type,namespace_fingerprint,statement_timestamp() FROM file_storage_cleanup_watches WHERE id=$3")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(watch).execute(&fixture.tenants[1].pool).await.unwrap();
    let before = fixture.snapshot().await;
    let output = fixture.check(&inventory, None, &environment).await;
    rejected(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("mismatched=1"));
    assert_eq!(before, fixture.snapshot().await);
    fixture.finish().await;
}

#[tokio::test]
async fn real_cli_plan_dry_run_and_owner_apply_preserve_revision_and_atomic_binding_policy() {
    let fixture = Fixture::new().await;
    let first = legacy_watch(&fixture.tenants[0].pool).await;
    let second = legacy_watch(&fixture.tenants[0].pool).await;
    let environment = BTreeMap::from([
        (
            "FILE_STORAGE_BINDING_DATABASE_URL".into(),
            fixture.tenants[0].url.clone(),
        ),
        ("R2_ACCOUNT_ID".into(), "synthetic-account".into()),
        ("R2_PUBLIC_BUCKET_NAME".into(), "public-fixture".into()),
        ("R2_PRIVATE_BUCKET_NAME".into(), "private-fixture".into()),
    ]);
    let path = fixture.directory.join("binding.json");
    let path = path.to_str().unwrap();
    let first_id = first.to_string();
    let second_id = second.to_string();
    let evidence = "c".repeat(64);
    let before = fixture.snapshot().await;
    success(
        &fixture
            .command(
                &[
                    "plan",
                    "--watch-id",
                    &first_id,
                    "--watch-id",
                    &second_id,
                    "--attestation-digest",
                    &evidence,
                    "--output",
                    path,
                ],
                &environment,
            )
            .await,
    );
    success(
        &fixture
            .command(&["dry-run", "--manifest", path], &environment)
            .await,
    );
    rejected(
        &fixture
            .command(&["apply", "--manifest", path], &environment)
            .await,
    );
    assert_eq!(before, fixture.snapshot().await);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    sqlx::query(
        "UPDATE file_storage_cleanup_watches SET revoked_at=statement_timestamp() WHERE id=$1",
    )
    .bind(second)
    .execute(&fixture.tenants[0].pool)
    .await
    .unwrap();
    let before = fixture.snapshot().await;
    rejected(
        &fixture
            .command(
                &["apply", "--manifest", path, "--attest-historical-namespace"],
                &environment,
            )
            .await,
    );
    assert_eq!(before, fixture.snapshot().await);
    std::fs::remove_file(path).unwrap();
    success(
        &fixture
            .command(
                &[
                    "plan",
                    "--watch-id",
                    &first_id,
                    "--watch-id",
                    &second_id,
                    "--attestation-digest",
                    &evidence,
                    "--output",
                    path,
                ],
                &environment,
            )
            .await,
    );
    success(
        &fixture
            .command(
                &["apply", "--manifest", path, "--attest-historical-namespace"],
                &environment,
            )
            .await,
    );
    assert!(
        namespace_binding::check_namespace_readiness(&fixture.tenants[0].pool, &namespaces())
            .await
            .unwrap()
            .ready()
    );
    fixture.finish().await;
}

struct OperationalAdmin {
    address: String,
    owners: std::sync::Arc<std::sync::Mutex<BTreeMap<String, String>>>,
    rows: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    statuses: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
    listings: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    drift_after: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for OperationalAdmin {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl OperationalAdmin {
    async fn start(fixture: &Fixture) -> Self {
        use axum::{
            extract::{Path, Query},
            http::HeaderMap,
            routing::{get, put},
            Json, Router,
        };
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        };
        let rows:Vec<Value>=fixture.tenants.iter().enumerate().map(|(index,tenant)|json!({"id":tenant.id,"subdomain":format!("school-{index}"),"status":if index==0 {"active"}else{"inactive"},"db_connection_string":tenant.runtime_url})).collect();
        let owners = Arc::new(Mutex::new(
            fixture
                .tenants
                .iter()
                .enumerate()
                .map(|(index, tenant)| (format!("school-{index}"), tenant.url.clone()))
                .collect::<BTreeMap<_, _>>(),
        ));
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let listings = Arc::new(AtomicUsize::new(0));
        let drift_after = Arc::new(AtomicUsize::new(usize::MAX));
        let rows = Arc::new(Mutex::new(rows));
        let response_rows = rows.clone();
        let (count, drift) = (listings.clone(), drift_after.clone());
        let app = Router::new().route(
            "/internal/schools",
            get(
                move |headers: HeaderMap, query: Query<BTreeMap<String, String>>| {
                    let (mut rows, count, drift) = (
                        response_rows.lock().unwrap().clone(),
                        count.clone(),
                        drift.clone(),
                    );
                    async move {
                        assert_eq!(headers.get("X-Internal-Caller").unwrap(), "backend-school");
                        assert_eq!(
                            headers.get("X-Internal-Secret").unwrap(),
                            "synthetic-internal-secret"
                        );
                        assert!(
                            query.is_empty(),
                            "the operational snapshot must be complete and unfiltered"
                        );
                        let current = count.fetch_add(1, Ordering::SeqCst) + 1;
                        if current > drift.load(Ordering::SeqCst) {
                            rows[0]["status"] = json!("suspended");
                        }
                        Json(json!({"total":rows.len(),"schools":rows}))
                    }
                },
            ),
        );
        let lookup = owners.clone();
        let app = app.route(
            "/internal/schools/{school}/migration-connection",
            get(
                move |Path(school): Path<String>,
                      headers: HeaderMap,
                      query: Query<BTreeMap<String, String>>| {
                    let lookup = lookup.clone();
                    async move {
                        assert_eq!(headers.get("X-Internal-Caller").unwrap(), "backend-school");
                        assert_eq!(
                            query.get("include_inactive").map(String::as_str),
                            Some("true")
                        );
                        let url = lookup.lock().unwrap().get(&school).unwrap().clone();
                        Json(json!({"db_migration_connection_string":url}))
                    }
                },
            ),
        );
        let recorded = statuses.clone();
        let app = app.route(
            "/internal/schools/{school}/migration-status",
            put(
                move |Path(school): Path<String>, Json(mut payload): Json<Value>| {
                    let recorded = recorded.clone();
                    async move {
                        payload["school"] = json!(school);
                        recorded.lock().unwrap().push(payload);
                        Json(json!({"ok":true}))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            address,
            owners,
            rows,
            statuses,
            listings,
            drift_after,
            task,
        }
    }
    fn environment(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("BACKEND_ADMIN_URL".into(), self.address.clone()),
            (
                "INTERNAL_API_SECRET".into(),
                "synthetic-internal-secret".into(),
            ),
            ("R2_ACCOUNT_ID".into(), "synthetic-account".into()),
            ("R2_PUBLIC_BUCKET_NAME".into(), "public-fixture".into()),
            ("R2_PRIVATE_BUCKET_NAME".into(), "private-fixture".into()),
        ])
    }
}
fn json_success(output: &Output) -> Value {
    success(output);
    serde_json::from_slice(&output.stdout)
        .expect("headless stdout must contain exactly its safe JSON object")
}

#[tokio::test]
async fn headless_context_works_at_135_then_migrates_complete_inactive_inventory_without_server() {
    let fixture = Fixture::at_version(135).await;
    let admin = OperationalAdmin::start(&fixture).await;
    let environment = admin.environment();
    let before = fixture.history_snapshot().await;
    let context = json_success(
        &fixture
            .main_command(&["storage-namespace", "context-admin"], &environment)
            .await,
    );
    assert_eq!(context["tenants"], 2);
    assert_eq!(context["inventory_sha256"].as_str().unwrap().len(), 64);
    rejected(
        &fixture
            .main_command(&["storage-namespace", "check-admin"], &environment)
            .await,
    );
    assert_eq!(
        before,
        fixture.history_snapshot().await,
        "context/check must not migrate a predecessor"
    );
    for args in [
        vec!["unsupported-mode"],
        vec!["migrate-tenants", "extra"],
        vec!["storage-namespace", "attest-admin", "--automatic"],
    ] {
        let output = fixture.main_command(&args, &environment).await;
        assert_eq!(output.status.code(), Some(2));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let output = Command::new(env!("CARGO_BIN_EXE_backend-school"))
            .arg(std::ffi::OsString::from_vec(vec![0xff]))
            .current_dir(&fixture.directory)
            .env_clear()
            .envs(&environment)
            .kill_on_drop(true)
            .output()
            .await
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            String::from_utf8(output.stderr).unwrap().trim(),
            "unsupported_operational_arguments"
        );
    }
    assert_eq!(before, fixture.history_snapshot().await);
    let migrated = json_success(
        &fixture
            .main_command(&["migrate-tenants"], &environment)
            .await,
    );
    assert_eq!(
        (
            migrated["tenants"].as_u64(),
            migrated["migrated"].as_u64(),
            migrated["failed"].as_u64()
        ),
        (Some(2), Some(2), Some(0))
    );
    for tenant in &fixture.tenants {
        let mut connection = tenant.pool.acquire().await.unwrap();
        assert_eq!(
            school_migrations::verify_tenant_schema(&mut *connection)
                .await
                .unwrap(),
            school_migrations::latest_migration_version()
        );
    }
    let runtime_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&fixture.tenants[0].runtime_url)
        .await
        .unwrap();
    let role: String = sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(&runtime_pool)
        .await
        .unwrap();
    assert_eq!(role, fixture.tenants[0].runtime);
    let authority:bool=sqlx::query_scalar("SELECT pg_has_role(current_user,datdba,'USAGE') FROM pg_database WHERE datname=current_database()").fetch_one(&runtime_pool).await.unwrap();
    assert!(
        !authority,
        "readiness must be proved with the actual non-owner runtime identity"
    );
    runtime_pool.close().await;
    let checked = json_success(
        &fixture
            .main_command(&["storage-namespace", "check-admin"], &environment)
            .await,
    );
    assert_eq!(
        (
            checked["tenants"].as_u64(),
            checked["checked"].as_u64(),
            checked["failed"].as_u64()
        ),
        (Some(2), Some(2), Some(0))
    );
    let statuses = admin.statuses.lock().unwrap().clone();
    assert_eq!(statuses.len(), 2);
    assert!(statuses.iter().all(|status| status["migration_version"]
        == school_migrations::latest_migration_version()
        && status["migration_status"] == "migrated"));
    fixture.finish().await;
}

#[tokio::test]
async fn headless_wrong_owner_mapping_and_inventory_drift_never_migrate_wrong_tenant() {
    let fixture = Fixture::at_version(135).await;
    let admin = OperationalAdmin::start(&fixture).await;
    let environment = admin.environment();
    admin
        .owners
        .lock()
        .unwrap()
        .insert("school-0".into(), fixture.tenants[1].url.clone());
    admin
        .owners
        .lock()
        .unwrap()
        .insert("school-1".into(), fixture.tenants[0].url.clone());
    let before = fixture.history_snapshot().await;
    let output = fixture
        .main_command(&["migrate-tenants"], &environment)
        .await;
    rejected(&output);
    let status: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["failed"], 2);
    assert_eq!(status["migrated"], 0);
    assert_eq!(before, fixture.history_snapshot().await);
    assert!(
        admin.statuses.lock().unwrap().is_empty(),
        "never stamp version zero without verified migration history"
    );
    let next = admin.listings.load(std::sync::atomic::Ordering::SeqCst) + 1;
    admin
        .drift_after
        .store(next, std::sync::atomic::Ordering::SeqCst);
    rejected(
        &fixture
            .main_command(&["storage-namespace", "context-admin"], &environment)
            .await,
    );
    assert_eq!(before, fixture.history_snapshot().await);
    fixture.finish().await;
}

#[tokio::test]
async fn explicit_admin_attestation_pins_context_and_refuses_live_claims_before_binding() {
    let fixture = Fixture::new().await;
    let watches = [
        legacy_watch(&fixture.tenants[0].pool).await,
        legacy_watch(&fixture.tenants[1].pool).await,
    ];
    let admin = OperationalAdmin::start(&fixture).await;
    let mut environment = admin.environment();
    let runtime_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&fixture.tenants[0].runtime_url)
        .await
        .unwrap();
    let manifest = namespace_binding::prepare_binding_manifest(
        &fixture.tenants[0].pool,
        &[watches[0]],
        "d".repeat(64),
        &namespaces(),
    )
    .await
    .unwrap();
    assert!(matches!(
        namespace_binding::bind_attested_namespaces(&runtime_pool, &manifest, &namespaces(), true)
            .await,
        Err(namespace_binding::BindingError::OwnerRequired)
    ));
    runtime_pool.close().await;
    let context = json_success(
        &fixture
            .main_command(&["storage-namespace", "context-admin"], &environment)
            .await,
    );
    let evidence = json!({"schema_version":1,"evidence_digest":"d".repeat(64),"expected_inventory_sha256":context["inventory_sha256"],"expected_public_namespace_sha256":context["public_namespace_sha256"],"expected_private_namespace_sha256":context["private_namespace_sha256"],"scope":"all_unbound"});
    environment.insert(
        "FILE_STORAGE_NAMESPACE_MANIFEST_DIRECTORY".into(),
        fixture.directory.to_str().unwrap().into(),
    );
    let before = fixture.snapshot().await;
    rejected(
        &fixture
            .main_command(&["storage-namespace", "attest-admin"], &environment)
            .await,
    );
    let mut wrong = evidence.clone();
    wrong["expected_inventory_sha256"] = json!("a".repeat(64));
    environment.insert(
        "FILE_STORAGE_NAMESPACE_ATTESTATION_JSON".into(),
        wrong.to_string(),
    );
    rejected(
        &fixture
            .main_command(&["storage-namespace", "attest-admin"], &environment)
            .await,
    );
    for field in [
        "expected_public_namespace_sha256",
        "expected_private_namespace_sha256",
    ] {
        let mut wrong = evidence.clone();
        wrong[field] = json!("a".repeat(64));
        environment.insert(
            "FILE_STORAGE_NAMESPACE_ATTESTATION_JSON".into(),
            wrong.to_string(),
        );
        rejected(
            &fixture
                .main_command(&["storage-namespace", "attest-admin"], &environment)
                .await,
        );
        assert_eq!(before, fixture.snapshot().await);
    }
    let mut unknown = evidence.clone();
    unknown["automatic"] = json!(true);
    environment.insert(
        "FILE_STORAGE_NAMESPACE_ATTESTATION_JSON".into(),
        unknown.to_string(),
    );
    rejected(
        &fixture
            .main_command(&["storage-namespace", "attest-admin"], &environment)
            .await,
    );
    assert_eq!(before, fixture.snapshot().await);
    // Normal startup must fail at the mandatory barrier, before provider/session/runtime setup.
    let refused = fixture.main_command(&[], &environment).await;
    rejected(&refused);
    assert!(String::from_utf8(refused.stderr)
        .unwrap()
        .contains("tenant_storage_readiness"));
    assert_eq!(before, fixture.snapshot().await);
    environment.insert(
        "FILE_STORAGE_NAMESPACE_ATTESTATION_JSON".into(),
        evidence.to_string(),
    );
    sqlx::query("INSERT INTO file_operations(file_id,file_version_id,operation_type,status,attempt_count,lease_owner,leased_at,lease_expires_at,next_retry_at) SELECT file_id,target_id,'reconcile','leased',1,'drained-old-writer',statement_timestamp(),statement_timestamp()+interval '1 minute',statement_timestamp() FROM file_storage_cleanup_watches WHERE id=$1")
        .bind(watches[0]).execute(&fixture.tenants[0].pool).await.unwrap();
    let before = fixture.snapshot().await;
    let refused = fixture
        .main_command(&["storage-namespace", "attest-admin"], &environment)
        .await;
    rejected(&refused);
    assert!(String::from_utf8(refused.stderr)
        .unwrap()
        .contains("storage_binding_active_claim"));
    assert_eq!(before, fixture.snapshot().await);
    sqlx::query("UPDATE file_operations SET leased_at=statement_timestamp()-interval '2 minutes',lease_expires_at=statement_timestamp()-interval '1 minute' WHERE lease_owner='drained-old-writer'").execute(&fixture.tenants[0].pool).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fixture.directory, std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let before = fixture.snapshot().await;
        rejected(
            &fixture
                .main_command(&["storage-namespace", "attest-admin"], &environment)
                .await,
        );
        assert_eq!(before, fixture.snapshot().await);
        std::fs::set_permissions(&fixture.directory, std::fs::Permissions::from_mode(0o700))
            .unwrap();
    }
    let applied = json_success(
        &fixture
            .main_command(&["storage-namespace", "attest-admin"], &environment)
            .await,
    );
    assert_eq!(applied["applied"], 2);
    assert_eq!(applied["ready"], true);
    let manifests = std::fs::read_dir(&fixture.directory)
        .unwrap()
        .map(|path| path.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("namespace-attestation-")
        })
        .collect::<Vec<_>>();
    assert_eq!(manifests.len(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&manifests[0])
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let plan: Value = serde_json::from_slice(&std::fs::read(&manifests[0]).unwrap()).unwrap();
    assert_eq!(plan["inventory_sha256"], context["inventory_sha256"]);
    assert_eq!(plan["tenants"].as_array().unwrap().len(), 2);
    let selected = plan["tenants"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|tenant| tenant["manifests"].as_array().unwrap())
        .flat_map(|manifest| manifest["watches"].as_array().unwrap())
        .map(|watch| watch["watch_id"].as_str().unwrap().to_string())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(selected, watches.iter().map(Uuid::to_string).collect());
    let before = fixture.snapshot().await;
    let checked = json_success(
        &fixture
            .main_command(&["storage-namespace", "check-admin"], &environment)
            .await,
    );
    assert_eq!(checked["unresolved"], 0);
    let replay = json_success(
        &fixture
            .main_command(&["storage-namespace", "attest-admin"], &environment)
            .await,
    );
    assert_eq!(replay["applied"], 0);
    assert_eq!(
        before,
        fixture.snapshot().await,
        "explicit retry must not replace accepted bindings"
    );
    fixture.finish().await;
}

#[tokio::test]
async fn main_operational_empty_inventory_is_valid_and_routing_requires_no_admin_or_provider() {
    let directory = std::env::temp_dir().join(format!(
        "schoolorbit-empty-rollout-{}",
        Uuid::new_v4().simple()
    ));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join(".env"), b"").unwrap();
    let mut environment = BTreeMap::from([
        ("R2_ACCOUNT_ID", "synthetic-account".to_string()),
        ("R2_PUBLIC_BUCKET_NAME", "public-fixture".to_string()),
        ("R2_PRIVATE_BUCKET_NAME", "private-fixture".to_string()),
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_backend-school"))
        .args(["storage-namespace", "routing-fingerprints"])
        .current_dir(&directory)
        .env_clear()
        .envs(&environment)
        .output()
        .await
        .unwrap();
    let fingerprints = json_success(&output);
    assert_eq!(
        fingerprints["public_namespace_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    let app = axum::Router::new().route(
        "/internal/schools",
        axum::routing::get(|| async { axum::Json(json!({"total":0,"schools":[]})) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    environment.insert(
        "BACKEND_ADMIN_URL",
        format!("http://{}", listener.local_addr().unwrap()),
    );
    environment.insert("INTERNAL_API_SECRET", "synthetic-internal-secret".into());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    for args in [
        vec!["storage-namespace", "context-admin"],
        vec!["storage-namespace", "check-admin"],
        vec!["migrate-tenants"],
    ] {
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            Command::new(env!("CARGO_BIN_EXE_backend-school"))
                .args(args)
                .current_dir(&directory)
                .env_clear()
                .envs(&environment)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(json_success(&output)["tenants"], 0);
    }
    server.abort();
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn committed_migration_version_survives_later_runtime_verification_failure() {
    let fixture = Fixture::at_version(135).await;
    let admin = OperationalAdmin::start(&fixture).await;
    let environment = admin.environment();
    let mut bad_route = url::Url::parse(&fixture.tenants[0].runtime_url).unwrap();
    bad_route.set_query(None);
    bad_route
        .query_pairs_mut()
        .append_pair("options", "-csearch_path=pg_catalog");
    admin.rows.lock().unwrap()[0]["db_connection_string"] = json!(bad_route.as_str());
    let output = fixture
        .main_command(&["migrate-tenants"], &environment)
        .await;
    rejected(&output);
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["migrated"], 1);
    assert_eq!(result["failed"], 1);
    for tenant in &fixture.tenants {
        let mut connection = tenant.pool.acquire().await.unwrap();
        assert_eq!(
            school_migrations::verify_tenant_schema(&mut *connection)
                .await
                .unwrap(),
            school_migrations::latest_migration_version()
        );
    }
    let statuses = admin.statuses.lock().unwrap().clone();
    assert_eq!(statuses.len(), 2);
    assert!(
        statuses
            .iter()
            .all(|status| status["migration_version"]
                == school_migrations::latest_migration_version())
    );
    assert!(statuses
        .iter()
        .any(|status| status["school"] == "school-0" && status["migration_status"] == "failed"));
    fixture.finish().await;
}

#[tokio::test]
async fn interrupted_binding_keeps_durable_exact_manifest_and_requires_explicit_rollforward() {
    let fixture = Fixture::new().await;
    let watches = [
        legacy_watch(&fixture.tenants[0].pool).await,
        legacy_watch(&fixture.tenants[1].pool).await,
    ];
    let admin = OperationalAdmin::start(&fixture).await;
    let mut environment = admin.environment();
    let context = json_success(
        &fixture
            .main_command(&["storage-namespace", "context-admin"], &environment)
            .await,
    );
    environment.insert("FILE_STORAGE_NAMESPACE_ATTESTATION_JSON".into(),json!({"schema_version":1,"evidence_digest":"e".repeat(64),"expected_inventory_sha256":context["inventory_sha256"],"expected_public_namespace_sha256":context["public_namespace_sha256"],"expected_private_namespace_sha256":context["private_namespace_sha256"],"scope":"all_unbound"}).to_string());
    environment.insert(
        "FILE_STORAGE_NAMESPACE_MANIFEST_DIRECTORY".into(),
        fixture.directory.to_str().unwrap().into(),
    );
    let mut blocker = fixture.tenants[0].pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM files WHERE id=(SELECT file_id FROM file_storage_cleanup_watches WHERE id=$1) FOR UPDATE").bind(watches[0]).execute(&mut *blocker).await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let before: Value =
        sqlx::query_scalar("SELECT to_jsonb(w) FROM file_storage_cleanup_watches w WHERE id=$1")
            .bind(watches[0])
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_backend-school"))
        .args(["storage-namespace", "attest-admin"])
        .current_dir(&fixture.directory)
        .env_clear()
        .envs(&environment)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            sqlx::query("SELECT pg_stat_clear_snapshot()")
                .execute(&mut *blocker)
                .await
                .unwrap();
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(blocker_pid)
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
            if waiting {
                break;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "apply must still be waiting on the authoritative file lock"
            );
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let manifests = std::fs::read_dir(&fixture.directory)
        .unwrap()
        .map(|path| path.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("namespace-attestation-")
        })
        .collect::<Vec<_>>();
    assert_eq!(manifests.len(), 1);
    let plan: Value = serde_json::from_slice(&std::fs::read(&manifests[0]).unwrap()).unwrap();
    assert_eq!(plan["inventory_sha256"], context["inventory_sha256"]);
    child.kill().await.unwrap();
    assert!(!child.wait().await.unwrap().success());
    let after: Value =
        sqlx::query_scalar("SELECT to_jsonb(w) FROM file_storage_cleanup_watches w WHERE id=$1")
            .bind(watches[0])
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
    assert_eq!(before, after);
    blocker.rollback().await.unwrap();
    // Another tenant may already have committed. Preserve it exactly on explicit retry.
    let accepted:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(w) FROM file_storage_cleanup_watches w WHERE namespace_fingerprint IS NOT NULL ORDER BY id").fetch_all(&fixture.tenants[1].pool).await.unwrap();
    let retry = json_success(
        &fixture
            .main_command(&["storage-namespace", "attest-admin"], &environment)
            .await,
    );
    assert_eq!(retry["ready"], true);
    let current:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(w) FROM file_storage_cleanup_watches w WHERE namespace_fingerprint IS NOT NULL ORDER BY id").fetch_all(&fixture.tenants[1].pool).await.unwrap();
    for row in accepted {
        assert!(
            current.contains(&row),
            "manual rollforward cannot rewrite an already accepted binding"
        );
    }
    assert_eq!(
        json_success(
            &fixture
                .main_command(&["storage-namespace", "check-admin"], &environment)
                .await
        )["unresolved"],
        0
    );
    fixture.finish().await;
}

#[tokio::test]
async fn explicit_attestation_covers_purged_locators_beyond_one_manifest_chunk() {
    let fixture = Fixture::new().await;
    // Independent historical locators deliberately have no surviving business/operation rows.
    let ids:Vec<Uuid>=sqlx::query_scalar("INSERT INTO file_storage_cleanup_watches(file_id,target_kind,target_id,provider_code,storage_class,object_key,content_type,revoked_at) SELECT uuidv7(),'version',uuidv7(),'r2','private','tenants/'||$1||'/certificate/template-image/purged-'||n::text||'.png','image/png',statement_timestamp() FROM generate_series(1,1001) AS n RETURNING id")
        .bind(fixture.tenants[0].id.to_string()).fetch_all(&fixture.tenants[0].pool).await.unwrap();
    assert_eq!(ids.len(), 1001);
    let metadata:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM files)+(SELECT count(*) FROM file_versions)+(SELECT count(*) FROM file_operations)").fetch_one(&fixture.tenants[0].pool).await.unwrap();
    assert_eq!(metadata, 0);
    let admin = OperationalAdmin::start(&fixture).await;
    let mut environment = admin.environment();
    let context = json_success(
        &fixture
            .main_command(&["storage-namespace", "context-admin"], &environment)
            .await,
    );
    environment.insert("FILE_STORAGE_NAMESPACE_ATTESTATION_JSON".into(),json!({"schema_version":1,"evidence_digest":"f".repeat(64),"expected_inventory_sha256":context["inventory_sha256"],"expected_public_namespace_sha256":context["public_namespace_sha256"],"expected_private_namespace_sha256":context["private_namespace_sha256"],"scope":"all_unbound"}).to_string());
    environment.insert(
        "FILE_STORAGE_NAMESPACE_MANIFEST_DIRECTORY".into(),
        fixture.directory.to_str().unwrap().into(),
    );
    let result = json_success(
        &fixture
            .main_command(&["storage-namespace", "attest-admin"], &environment)
            .await,
    );
    assert_eq!(result["applied"], 1001);
    assert_eq!(result["ready"], true);
    let state:(i64,i64,i64)=sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE namespace_fingerprint IS NOT NULL AND binding_evidence_digest=repeat('f',64) AND revoked_at IS NOT NULL),count(*) FILTER(WHERE namespace_fingerprint IS NULL) FROM file_storage_cleanup_watches").fetch_one(&fixture.tenants[0].pool).await.unwrap();
    assert_eq!(state, (1001, 1001, 0));
    let paths = std::fs::read_dir(&fixture.directory)
        .unwrap()
        .map(|path| path.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("namespace-attestation-")
        })
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    let plan: Value = serde_json::from_slice(&std::fs::read(&paths[0]).unwrap()).unwrap();
    let chunks = plan["tenants"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|tenant| tenant["manifests"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert!(
        chunks.len() > 1,
        "the public binding API's1000-row limit must not truncate the operational inventory"
    );
    assert!(chunks
        .iter()
        .all(|chunk| chunk["watches"].as_array().unwrap().len() <= 1000));
    let selected = chunks
        .iter()
        .flat_map(|chunk| chunk["watches"].as_array().unwrap())
        .map(|watch| watch["watch_id"].as_str().unwrap().to_string())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(selected, ids.iter().map(Uuid::to_string).collect());
    let checked = json_success(
        &fixture
            .main_command(&["storage-namespace", "check-admin"], &environment)
            .await,
    );
    assert_eq!(checked["watches"], 1001);
    assert_eq!(checked["unresolved"], 0);
    fixture.finish().await;
}
