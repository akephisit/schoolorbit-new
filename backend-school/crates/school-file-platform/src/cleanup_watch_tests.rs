use super::{
    cleanup_watch::{perform_cleanup_watch, CleanupObservation},
    malware_scanner::{MalwareScanner, ScanOutcome},
    namespace_binding::{self, BindingError, ConfiguredNamespaces},
    platform_service::{FilePlatform, UploadCommand},
    platform_types::{DownloadGrant, FilePurpose, StorageClass},
    repository::SqlFileRepository,
    storage_namespace::StorageNamespace,
    storage_provider::{ObjectMetadata, StorageError, StorageProvider, StoredObject},
};
use async_trait::async_trait;
use bytes::Bytes;
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, Notify};
use url::Url;
use uuid::Uuid;

struct Clean;
#[async_trait]
impl MalwareScanner for Clean {
    async fn scan(&self, _: &[u8]) -> ScanOutcome {
        ScanOutcome::Clean
    }
}
struct Provider {
    objects: Arc<Mutex<HashMap<String, Bytes>>>,
    remote: Option<mpsc::Sender<(StoredObject, Bytes)>>,
}
#[async_trait]
impl StorageProvider for Provider {
    fn namespace(&self, class: StorageClass) -> Result<StorageNamespace, StorageError> {
        StorageNamespace::from_provider_locator(
            "r2",
            "https://fixture.invalid",
            match class {
                StorageClass::Public => "public",
                StorageClass::Private => "private",
            },
        )
    }
    async fn check_readiness(&self) -> Result<(), StorageError> {
        Ok(())
    }
    async fn put(&self, object: &StoredObject, body: Bytes) -> Result<(), StorageError> {
        if let Some(remote) = &self.remote {
            remote
                .send((object.clone(), body))
                .await
                .map_err(|_| StorageError::OperationFailed)?;
            std::future::pending().await
        } else {
            self.objects
                .lock()
                .unwrap()
                .insert(object.object_key.as_str().into(), body);
            Ok(())
        }
    }
    async fn head(&self, object: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError> {
        Ok(self
            .objects
            .lock()
            .unwrap()
            .get(object.object_key.as_str())
            .map(|bytes| ObjectMetadata {
                content_type: Some(object.content_type.clone()),
                content_length: Some(bytes.len() as u64),
            }))
    }
    async fn get(&self, object: &StoredObject, _: u64) -> Result<Bytes, StorageError> {
        self.objects
            .lock()
            .unwrap()
            .get(object.object_key.as_str())
            .cloned()
            .ok_or(StorageError::OperationFailed)
    }
    async fn delete(&self, object: &StoredObject) -> Result<(), StorageError> {
        self.objects
            .lock()
            .unwrap()
            .remove(object.object_key.as_str());
        Ok(())
    }
    async fn private_download_grant(
        &self,
        _: &StoredObject,
        _: &str,
        _: Duration,
    ) -> Result<DownloadGrant, StorageError> {
        panic!("unexpected grant")
    }
    fn public_location(&self, _: &StoredObject) -> Result<Url, StorageError> {
        panic!("unexpected public URL")
    }
}
async fn fixture(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, 1).await;
    school_test_db::run_test_migrations(&pool).await;
    pool
}
fn provider() -> Arc<Provider> {
    Arc::new(Provider {
        objects: Default::default(),
        remote: None,
    })
}
fn png() -> Bytes {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        2,
        image::Rgba([4, 8, 15, 255]),
    ));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes.into()
}
async fn legacy_watch(pool: &PgPool) -> Uuid {
    let file = Uuid::new_v4();
    let target = Uuid::new_v4();
    let key = super::purpose_registry::original_object_key(
        Uuid::new_v4(),
        FilePurpose::CertificateTemplateImage,
        file,
        1,
        super::platform_types::DetectedContent::Png,
    )
    .unwrap();
    sqlx::query("INSERT INTO files(id,purpose_code,display_filename,visibility,lifecycle_status,retention_class,inspection_metadata) VALUES($1,'certificate_template_image','fixture.png','private','processing','temporary','{\"kind\":\"image\",\"width_px\":2,\"height_px\":2}')").bind(file).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status) VALUES($1,$2,1,'r2','private',$3,'image/png','png',4,$4,'clean')").bind(target).bind(file).bind(key.as_str()).bind("a".repeat(64)).execute(pool).await.unwrap();
    sqlx::query_scalar("SELECT id FROM file_storage_cleanup_watches WHERE target_id=$1")
        .bind(target)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn late_remote_put_after_client_cancel_head404_and_real039_purge_is_recovered_after_restart()
{
    let pool = fixture("watch_cancel_real039").await;
    let objects = Arc::new(Mutex::new(HashMap::new()));
    let (send, mut receive) = mpsc::channel::<(StoredObject, Bytes)>(1);
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let remote_objects = objects.clone();
    let remote_started = started.clone();
    let remote_release = release.clone();
    // An independently owned remote request survives cancellation of its client future.
    let remote = tokio::spawn(async move {
        let (object, body) = receive.recv().await.unwrap();
        remote_started.notify_one();
        remote_release.notified().await;
        remote_objects
            .lock()
            .unwrap()
            .insert(object.object_key.as_str().into(), body);
    });
    let provider = Arc::new(Provider {
        objects: objects.clone(),
        remote: Some(send),
    });
    let platform = Arc::new(FilePlatform::new(provider.clone(), Arc::new(Clean)));
    let worker_pool = pool.clone();
    let worker_platform = platform.clone();
    let image = png();
    let byte_size = image.len() as i64;
    let upload = tokio::spawn(async move {
        let repository = SqlFileRepository::new(worker_pool);
        worker_platform
            .upload(
                &repository,
                UploadCommand {
                    tenant_id: Uuid::new_v4(),
                    actor_user_id: None,
                    owner_user_id: None,
                    purpose: FilePurpose::CertificateTemplateImage,
                    display_filename: "fixture.png".into(),
                    bytes: image,
                },
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), started.notified())
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(
            Duration::from_millis(300),
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&pool)
        )
        .await
        .unwrap()
        .unwrap(),
        1
    );
    upload.abort();
    assert!(upload.await.unwrap_err().is_cancelled());
    let file: Uuid =
        sqlx::query_scalar("SELECT id FROM files WHERE purpose_code='certificate_template_image'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let repository = SqlFileRepository::new(pool.clone());
    platform.request_delete(&repository, file).await.unwrap();
    let year:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,start_date,end_date,status) VALUES(2988,'Watch fixture','2045-01-01','2045-12-31','planning') RETURNING id").fetch_one(&pool).await.unwrap();
    let campaign:Uuid=sqlx::query_scalar("INSERT INTO certificate_campaigns(academic_year_id,name,event_date,status) VALUES($1,'Watch fixture','2045-01-01','purging') RETURNING id").bind(year).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO certificate_campaign_purge_jobs(campaign_id,status,template_count,candidate_count,request_count,open_request_count,issued_certificate_count,revoked_certificate_count,file_count,total_file_bytes) VALUES($1,'finalizing',0,0,0,0,0,0,1,$2)").bind(campaign).bind(byte_size).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO certificate_campaign_purge_files(campaign_id,file_id,object_count,byte_size) VALUES($1,$2,1,$3)").bind(campaign).bind(file).bind(byte_size).execute(&pool).await.unwrap();
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT finalize_certificate_campaign_purge($1)")
            .bind(campaign)
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    let (files,versions,operations,inventory,watches):(i64,i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM files),(SELECT count(*) FROM file_versions),(SELECT count(*) FROM file_operations),(SELECT count(*) FROM certificate_campaign_purge_files),(SELECT count(*) FROM file_storage_cleanup_watches WHERE revoked_at IS NOT NULL)").fetch_one(&pool).await.unwrap();
    assert_eq!(
        (files, versions, operations, inventory, watches),
        (0, 0, 0, 0, 1)
    );
    let namespaces = platform.storage_namespaces().unwrap();
    let watch = repository
        .claim_cleanup_watch("before-restart", Duration::from_secs(60), &namespaces)
        .await
        .unwrap()
        .unwrap();
    let observed = perform_cleanup_watch(&platform, &watch, Duration::from_secs(1)).await;
    assert!(matches!(observed, CleanupObservation::Absent));
    repository
        .finish_cleanup_watch(&watch, observed, Duration::from_millis(1))
        .await
        .unwrap();
    assert!(objects.lock().unwrap().is_empty());
    release.notify_one();
    remote.await.unwrap();
    assert_eq!(objects.lock().unwrap().len(), 1);
    drop(repository);
    // Reconstruct the worker from PostgreSQL only; no prior in-memory write result exists.
    let restarted = SqlFileRepository::new(pool.clone());
    sqlx::query("UPDATE file_storage_cleanup_watches SET next_probe_at=statement_timestamp()")
        .execute(&pool)
        .await
        .unwrap();
    let watch = restarted
        .claim_cleanup_watch("after-restart", Duration::from_secs(60), &namespaces)
        .await
        .unwrap()
        .unwrap();
    let observed = perform_cleanup_watch(&platform, &watch, Duration::from_secs(1)).await;
    assert!(matches!(observed, CleanupObservation::Deleted));
    restarted
        .finish_cleanup_watch(&watch, observed, Duration::from_secs(60))
        .await
        .unwrap();
    assert!(objects.lock().unwrap().is_empty());
    let scheduled:bool=sqlx::query_scalar("SELECT revoked_at IS NOT NULL AND next_probe_at>statement_timestamp() AND namespace_fingerprint IS NOT NULL FROM file_storage_cleanup_watches").fetch_one(&pool).await.unwrap();
    assert!(scheduled);
}

#[tokio::test]
async fn attested_binding_is_read_only_until_apply_and_refuses_changed_revision() {
    let pool = fixture("watch_attestation_revision").await;
    let id = legacy_watch(&pool).await;
    let provider = provider();
    let namespaces = ConfiguredNamespaces::from_provider(provider.as_ref()).unwrap();
    assert_eq!(
        namespace_binding::check_namespace_readiness(&pool, &namespaces)
            .await
            .unwrap()
            .unresolved,
        1
    );
    let manifest =
        namespace_binding::prepare_binding_manifest(&pool, &[id], "a".repeat(64), &namespaces)
            .await
            .unwrap();
    assert_eq!(
        namespace_binding::bind_attested_namespaces(&pool, &manifest, &namespaces, false)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        namespace_binding::check_namespace_readiness(&pool, &namespaces)
            .await
            .unwrap()
            .unresolved,
        1
    );
    sqlx::query(
        "UPDATE file_storage_cleanup_watches SET write_generation=write_generation+1 WHERE id=$1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        namespace_binding::bind_attested_namespaces(&pool, &manifest, &namespaces, true).await,
        Err(BindingError::ChangedWatch)
    ));
    let manifest =
        namespace_binding::prepare_binding_manifest(&pool, &[id], "b".repeat(64), &namespaces)
            .await
            .unwrap();
    namespace_binding::bind_attested_namespaces(&pool, &manifest, &namespaces, true)
        .await
        .unwrap();
    assert!(
        namespace_binding::check_namespace_readiness(&pool, &namespaces)
            .await
            .unwrap()
            .ready()
    );
}

#[tokio::test]
async fn stolen_cleanup_claim_has_zero_bookkeeping_effects_and_repeated_absence_stays_due() {
    let pool = fixture("watch_claim_steal").await;
    let id = legacy_watch(&pool).await;
    let provider = provider();
    let namespaces = ConfiguredNamespaces::from_provider(provider.as_ref()).unwrap();
    let manifest =
        namespace_binding::prepare_binding_manifest(&pool, &[id], "c".repeat(64), &namespaces)
            .await
            .unwrap();
    namespace_binding::bind_attested_namespaces(&pool, &manifest, &namespaces, true)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE file_storage_cleanup_watches SET revoked_at=statement_timestamp() WHERE id=$1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    let repository = SqlFileRepository::new(pool.clone());
    let allowed = vec![provider.namespace(StorageClass::Private).unwrap()];
    let old = repository
        .claim_cleanup_watch("same-owner", Duration::from_secs(60), &allowed)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE file_storage_cleanup_watches SET leased_at=statement_timestamp()-interval '120 seconds',lease_expires_at=statement_timestamp()-interval '60 seconds',next_probe_at=statement_timestamp() WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    let new = repository
        .claim_cleanup_watch("same-owner", Duration::from_secs(60), &allowed)
        .await
        .unwrap()
        .unwrap();
    assert!(new.claim.generation > old.claim.generation);
    let before: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(w) FROM file_storage_cleanup_watches w WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(matches!(
        repository
            .finish_cleanup_watch(&old, CleanupObservation::Deleted, Duration::from_secs(1))
            .await,
        Err(super::repository::RepositoryError::LeaseLost)
    ));
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(w) FROM file_storage_cleanup_watches w WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    repository
        .finish_cleanup_watch(&new, CleanupObservation::Absent, Duration::from_secs(1))
        .await
        .unwrap();
    let retained:bool=sqlx::query_scalar("SELECT revoked_at IS NOT NULL AND last_absent_at IS NOT NULL AND next_probe_at>statement_timestamp() FROM file_storage_cleanup_watches WHERE id=$1").bind(id).fetch_one(&pool).await.unwrap();
    assert!(retained);
}

#[tokio::test]
async fn stale_expiry_candidate_promoted_under_file_lock_is_not_revoked() {
    let pool = school_test_db::create_named_test_pool_with_max_connections(
        "watch_expiry_promotion_race",
        3,
    )
    .await;
    school_test_db::run_test_migrations(&pool).await;
    let id = legacy_watch(&pool).await;
    let file: Uuid =
        sqlx::query_scalar("SELECT file_id FROM file_storage_cleanup_watches WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "UPDATE files SET expires_at=statement_timestamp()-interval '1 minute' WHERE id=$1",
    )
    .bind(file)
    .execute(&pool)
    .await
    .unwrap();
    let discovered:Uuid=sqlx::query_scalar("SELECT id FROM files WHERE retention_class='temporary' AND expires_at<=statement_timestamp()").fetch_one(&pool).await.unwrap();
    assert_eq!(discovered, file);
    let mut promotion = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE")
        .bind(file)
        .execute(&mut *promotion)
        .await
        .unwrap();
    let promotion_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *promotion)
        .await
        .unwrap();
    sqlx::query("UPDATE files SET retention_class='standard',expires_at=NULL WHERE id=$1")
        .bind(file)
        .execute(&mut *promotion)
        .await
        .unwrap();
    let worker_pool = pool.clone();
    let worker = tokio::spawn(async move {
        SqlFileRepository::new(worker_pool)
            .request_expired_delete(discovered)
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            sqlx::query("SELECT pg_stat_clear_snapshot()")
                .execute(&pool)
                .await
                .unwrap();
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(promotion_pid)
            .fetch_one(&pool)
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    promotion.commit().await.unwrap();
    assert!(!tokio::time::timeout(Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap());
    let state:(String,String,bool,i64)=sqlx::query_as("SELECT f.lifecycle_status,f.retention_class,w.revoked_at IS NULL,(SELECT count(*) FROM file_operations WHERE file_id=f.id AND operation_type='delete_object') FROM files f JOIN file_storage_cleanup_watches w ON w.file_id=f.id WHERE f.id=$1")
        .bind(file).fetch_one(&pool).await.unwrap();
    assert_eq!(state, ("processing".into(), "standard".into(), true, 0));
}

#[tokio::test]
async fn runtime_role_cannot_bind_legacy_namespace_even_with_attestation_guc() {
    let pool = fixture("watch_runtime_binding_refused").await;
    let id = legacy_watch(&pool).await;
    let role = format!("watch_runtime_{}", Uuid::new_v4().simple());
    // Role and grants are rolled back, including on assertion failure.
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE ROLE {role} NOLOGIN")))
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "GRANT SELECT, UPDATE ON file_storage_cleanup_watches TO {role}"
    )))
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("SET LOCAL ROLE {role}")))
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("SELECT set_config('schoolorbit.storage_binding_attestation',repeat('a',64),true)")
        .execute(&mut *transaction)
        .await
        .unwrap();
    let error = sqlx::query("UPDATE file_storage_cleanup_watches SET namespace_fingerprint=repeat('b',64),namespace_bound_at=statement_timestamp(),binding_evidence_digest=repeat('a',64) WHERE id=$1")
        .bind(id).execute(&mut *transaction).await.unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("42501")
    );
    transaction.rollback().await.unwrap();
    // The same legacy row can be bound by the actual owner through the guarded API.
    let namespaces = ConfiguredNamespaces::from_provider(provider().as_ref()).unwrap();
    let manifest =
        namespace_binding::prepare_binding_manifest(&pool, &[id], "c".repeat(64), &namespaces)
            .await
            .unwrap();
    namespace_binding::bind_attested_namespaces(&pool, &manifest, &namespaces, true)
        .await
        .unwrap();
    assert!(
        namespace_binding::check_namespace_readiness(&pool, &namespaces)
            .await
            .unwrap()
            .ready()
    );
}

#[tokio::test]
async fn claimed_watch_advances_due_order_even_when_worker_never_finishes() {
    let pool = fixture("watch_lost_finish_fairness").await;
    let first = legacy_watch(&pool).await;
    let second = legacy_watch(&pool).await;
    let provider = provider();
    let namespaces = ConfiguredNamespaces::from_provider(provider.as_ref()).unwrap();
    let manifest = namespace_binding::prepare_binding_manifest(
        &pool,
        &[first, second],
        "d".repeat(64),
        &namespaces,
    )
    .await
    .unwrap();
    namespace_binding::bind_attested_namespaces(&pool, &manifest, &namespaces, true)
        .await
        .unwrap();
    sqlx::query("UPDATE file_storage_cleanup_watches SET revoked_at=statement_timestamp(),next_probe_at=statement_timestamp()-CASE WHEN id=$1 THEN interval '2 minutes' ELSE interval '1 minute' END")
        .bind(first).execute(&pool).await.unwrap();
    let allowed = vec![provider.namespace(StorageClass::Private).unwrap()];
    let repository = SqlFileRepository::new(pool.clone());
    assert!(!repository
        .cleanup_starting_preference(&allowed)
        .await
        .unwrap());
    // Discard all local sweep state: the next reconstructed sweep starts with watches.
    drop(repository);
    let repository = SqlFileRepository::new(pool.clone());
    assert!(repository
        .cleanup_starting_preference(&allowed)
        .await
        .unwrap());
    let claim = repository
        .claim_cleanup_watch("lost-finish", Duration::from_secs(60), &allowed)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.id, first);
    sqlx::query("UPDATE file_storage_cleanup_watches SET leased_at=statement_timestamp()-interval '120 seconds',lease_expires_at=statement_timestamp()-interval '60 seconds' WHERE id=$1")
        .bind(first).execute(&pool).await.unwrap();
    // A crashed oldest claim no longer monopolizes the ordering after its lease expires.
    drop(repository);
    let restarted = SqlFileRepository::new(pool.clone());
    let next = restarted
        .claim_cleanup_watch("healthy-next", Duration::from_secs(60), &allowed)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.id, second);
}
