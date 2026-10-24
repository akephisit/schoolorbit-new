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
            // UUID-only identifiers and exact canonical Admin resource markers.
            sqlx::query(sqlx::AssertSqlSafe(format!("CREATE ROLE {owner} NOLOGIN")))
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
            school_test_db::run_test_migrations(&pool).await;
            tenants.push(Tenant {
                id,
                database,
                owner,
                url: tenant_url.to_string(),
                pool,
            });
        }
        let directory =
            std::env::temp_dir().join(format!("schoolorbit-watch-cli-{}", Uuid::new_v4().simple()));
        std::fs::create_dir(&directory).unwrap();
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
                .env_clear()
                .envs(environment)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .expect("offline CLI must finish its bounded fixture work")
        .unwrap()
    }
    async fn snapshot(&self) -> Value {
        let mut states = Vec::new();
        for tenant in &self.tenants {
            let state:Value=sqlx::query_scalar("SELECT jsonb_build_object('watches',(SELECT coalesce(jsonb_agg(to_jsonb(w) ORDER BY w.id),'[]'::jsonb) FROM file_storage_cleanup_watches w),'permissions',(SELECT coalesce(jsonb_agg(to_jsonb(p) ORDER BY p.id),'[]'::jsonb) FROM permissions p),'migrations',(SELECT jsonb_agg(to_jsonb(m) ORDER BY m.version) FROM _sqlx_migrations m),'acl',(SELECT relacl::text FROM pg_class WHERE oid='file_storage_cleanup_watches'::regclass),'owner',(SELECT datdba::bigint FROM pg_database WHERE datname=current_database()),'role',(SELECT shobj_description(oid,'pg_authid') FROM pg_roles WHERE rolname=$1))")
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
