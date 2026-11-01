use super::{file_relationships, models::*, services};
use crate::integration_tests::policies::file_access_policy::FilePolicyAction;
use async_trait::async_trait;
use bytes::Bytes;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_file_platform::{
    malware_scanner::{MalwareScanner, ScanOutcome},
    platform_service::FilePlatform,
    platform_types::DetectedContent,
    platform_types::{DownloadGrant, FileLifecycleStatus, FilePurpose, FileVisibility},
    purpose_registry::original_object_key,
    repository::{PlatformFile, SqlFileRepository},
    storage_provider::{ObjectMetadata, StorageError, StorageProvider, StoredObject},
};
use school_permissions::registry::codes;
use school_test_db::{
    create_named_test_pool_with_max_connections, create_test_user, run_test_migrations,
};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use url::Url;
use uuid::Uuid;

struct FailingStorage;
#[async_trait]
impl StorageProvider for FailingStorage {
    fn namespace(
        &self,
        storage_class: school_file_platform::platform_types::StorageClass,
    ) -> Result<
        school_file_platform::storage_namespace::StorageNamespace,
        school_file_platform::storage_provider::StorageError,
    > {
        school_file_platform::storage_namespace::StorageNamespace::from_provider_locator(
            "fixture",
            "https://fixture.invalid",
            match storage_class {
                school_file_platform::platform_types::StorageClass::Public => "public",
                school_file_platform::platform_types::StorageClass::Private => "private",
            },
        )
    }

    async fn check_readiness(&self) -> Result<(), StorageError> {
        Ok(())
    }
    async fn put(&self, _: &StoredObject, _: Bytes) -> Result<(), StorageError> {
        panic!("no upload in domain test")
    }
    async fn get(&self, _: &StoredObject, _: u64) -> Result<Bytes, StorageError> {
        panic!("no object read in domain test")
    }
    async fn head(&self, _: &StoredObject) -> Result<Option<ObjectMetadata>, StorageError> {
        panic!("no provider inspection in domain test")
    }
    async fn delete(&self, _: &StoredObject) -> Result<(), StorageError> {
        Err(StorageError::OperationFailed)
    }
    async fn private_download_grant(
        &self,
        _: &StoredObject,
        _: &str,
        _: Duration,
    ) -> Result<DownloadGrant, StorageError> {
        panic!("no download in domain test")
    }
    fn public_location(&self, _: &StoredObject) -> Result<Url, StorageError> {
        panic!("private files only")
    }
}
struct CleanScanner;
#[async_trait]
impl MalwareScanner for CleanScanner {
    async fn scan(&self, _: &[u8]) -> ScanOutcome {
        ScanOutcome::Clean
    }
}
fn platform() -> FilePlatform {
    FilePlatform::new(Arc::new(FailingStorage), Arc::new(CleanScanner))
}
fn actor(id: Uuid, permissions: &[&str]) -> ActorContext {
    ActorContext {
        user_id: id,
        permissions: permissions.iter().map(|s| s.to_string()).collect(),
    }
}
async fn fixture(name: &str) -> (PgPool, ActorContext, ActorContext, Uuid) {
    let pool = create_named_test_pool_with_max_connections(name, 6).await;
    run_test_migrations(&pool).await;
    let user = create_test_user(
        &pool,
        &format!("{name}-manager@example.test"),
        "fixture-password",
    )
    .await
    .unwrap();
    let other = create_test_user(
        &pool,
        &format!("{name}-other@example.test"),
        "fixture-password",
    )
    .await
    .unwrap();
    let manager = actor(
        user,
        &[
            codes::SUPPLY_CATALOG_READ_SCHOOL,
            codes::SUPPLY_CATALOG_MANAGE_SCHOOL,
            codes::SUPPLY_CATALOG_REQUEST_OWN,
            codes::SUPPLY_CATALOG_APPROVE_SCHOOL,
        ],
    );
    let proposer = actor(
        other,
        &[
            codes::SUPPLY_CATALOG_READ_SCHOOL,
            codes::SUPPLY_CATALOG_REQUEST_OWN,
        ],
    );
    let category = sqlx::query_scalar("SELECT id FROM supply_categories ORDER BY name LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    (pool, manager, proposer, category)
}
fn product(category: Uuid, name: &str, image: Option<Uuid>) -> SupplyItemWrite {
    SupplyItemWrite {
        name: name.into(),
        description: "specification".into(),
        category_id: category,
        unit: "ชิ้น".into(),
        image_file_id: image,
        price: "12.34".into(),
        shop: "Fixture shop".into(),
    }
}
fn proposal(category: Uuid, image: Option<Uuid>, source: Option<Uuid>) -> SupplyProposalWrite {
    SupplyProposalWrite {
        name: "Proposal product".into(),
        description: "Proposal description".into(),
        category_id: category,
        unit: "ชิ้น".into(),
        image_file_id: image,
        price: "34.56".into(),
        shop: "Proposal shop".into(),
        source_file_id: source,
        source_url: None,
    }
}
fn approval(row: &SupplyProposal) -> SupplyProposalApprove {
    SupplyProposalApprove {
        revision: row.revision,
        existing_item_id: None,
        existing_item_revision: None,
        name: row.name.clone(),
        description: row.description.clone(),
        category_id: row.category_id,
        unit: row.unit.clone(),
        image_file_id: row.image_file_id,
        price: row.price.clone(),
        shop: row.shop.clone(),
    }
}
async fn image(pool: &PgPool, owner: Uuid, purpose: FilePurpose) -> PlatformFile {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO files(id,display_filename,owner_user_id,purpose_code,visibility,lifecycle_status,retention_class,expires_at,inspection_metadata,created_by)VALUES($1,'fixture.png',$2,$3,'private','processing','temporary',now()+interval '1 day','{\"kind\":\"image\",\"width_px\":20,\"height_px\":20}'::jsonb,$2)").bind(id).bind(owner).bind(purpose.code()).execute(pool).await.unwrap();
    let tenant = Uuid::now_v7();
    let object = original_object_key(tenant, purpose, id, 1, DetectedContent::Png).unwrap();
    let version=sqlx::query_scalar::<_,Uuid>("INSERT INTO file_versions(file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status,scanned_at,created_by)VALUES($1,1,'r2','private','stored',$2,'image/png','png',100,repeat('a',64),'clean',now(),$3)RETURNING id").bind(id).bind(object.as_str()).bind(owner).fetch_one(pool).await.unwrap();
    sqlx::query("UPDATE files SET current_version_id=$2,lifecycle_status='ready' WHERE id=$1")
        .bind(id)
        .bind(version)
        .execute(pool)
        .await
        .unwrap();
    PlatformFile {
        id,
        owner_user_id: Some(owner),
        purpose,
        visibility: FileVisibility::Private,
        lifecycle_status: FileLifecycleStatus::Ready,
        current_version: Some(1),
        display_filename: "fixture.png".into(),
        detected_mime_type: "image/png".into(),
        byte_size: 100,
    }
}
async fn lifecycle(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT lifecycle_status FROM files WHERE id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn seeded_category_names_reject_duplicates_in_both_thai_unicode_forms() {
    let (pool, manager, _, _) = fixture("supply-category-seed-duplicates").await;
    let categories = services::list_categories(&pool, &manager).await.unwrap();
    for category in &categories {
        for name in [
            category.name.clone(),
            school_administration::supply_catalog::normalize(&category.name),
        ] {
            let result = services::write_category(
                &pool,
                &manager,
                None,
                SupplyCategoryWrite {
                    name,
                    revision: None,
                },
            )
            .await;
            assert!(
                matches!(result, Err(AppError::Conflict(_))),
                "{}",
                category.name
            );
        }
    }
    assert_eq!(
        services::list_categories(&pool, &manager)
            .await
            .unwrap()
            .len(),
        categories.len()
    );
    pool.close().await;
}

#[tokio::test]
async fn category_key_repair_preserves_canonical_keys_and_refuses_collisions() {
    const REPAIR: &str = include_str!("../../../migrations/112_supply_category_normalization.sql");
    let (pool, manager, _, _) = fixture("supply-category-key-repair").await;
    let custom = services::write_category(
        &pool,
        &manager,
        None,
        SupplyCategoryWrite {
            name: "custom  category".into(),
            revision: None,
        },
    )
    .await
    .unwrap();
    sqlx::raw_sql(REPAIR).execute(&pool).await.unwrap();
    let key: String =
        sqlx::query_scalar("SELECT normalized_name FROM supply_categories WHERE id=$1")
            .bind(custom.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(key, "custom category");

    let office: Uuid =
        sqlx::query_scalar("SELECT id FROM supply_categories WHERE name='วัสดุสำนักงาน'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let item = services::create_item(&pool, &manager, product(office, "Office paper", None))
        .await
        .unwrap();
    sqlx::query("UPDATE supply_categories SET normalized_name=name WHERE id=$1")
        .bind(office)
        .execute(&pool)
        .await
        .unwrap();
    services::write_category(
        &pool,
        &manager,
        None,
        SupplyCategoryWrite {
            name: "วัสดุสำนักงาน".into(),
            revision: None,
        },
    )
    .await
    .unwrap();
    let before: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id,name,normalized_name FROM supply_categories ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let error = sqlx::raw_sql(REPAIR).execute(&pool).await.unwrap_err();
    assert!(error
        .as_database_error()
        .is_some_and(|error| error.is_unique_violation()));
    let after: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id,name,normalized_name FROM supply_categories ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(after, before);
    assert_eq!(
        services::get_item(&pool, &manager, item.id)
            .await
            .unwrap()
            .item
            .category_id,
        office
    );
    pool.close().await;
}

#[tokio::test]
async fn description_search_normalizes_thai_and_fullwidth_text() {
    let (pool, manager, _, category) = fixture("supply-description-search").await;
    let mut payload = product(category, "Paper", None);
    payload.description = "กระดาษสำหรับสำนักงาน Ａ４".into();
    let item = services::create_item(&pool, &manager, payload)
        .await
        .unwrap();
    for search in ["สำนักงาน", "สํานักงาน", "a4", "Ａ４"] {
        let result = services::list_items(
            &pool,
            &manager,
            SupplyItemQuery {
                search: Some(search.into()),
                category_id: None,
                favorites_only: None,
                page: None,
                page_size: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(result.total, 1, "{search}");
        assert_eq!(result.items[0].id, item.id, "{search}");
    }
    pool.close().await;
}

#[tokio::test]
async fn normalized_duplicates_and_concurrent_creation_are_conflicts() {
    let (pool, manager, _, category) = fixture("supply_duplicates").await;
    let (a, b) = tokio::join!(
        services::create_item(&pool, &manager, product(category, "  PAPER A4  ", None)),
        services::create_item(&pool, &manager, product(category, "paper a4", None))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(AppError::Conflict(_))
    ));
    let duplicates = services::duplicates(
        &pool,
        &manager,
        SupplyDuplicateQuery {
            name: "paper a5".into(),
            unit: "ชิ้น".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(duplicates.len(), 1);
}
#[tokio::test]
async fn versions_serialize_append_and_edit_with_stale_refusal() {
    let (pool, manager, _, category) = fixture("supply_versions").await;
    let item = services::create_item(&pool, &manager, product(category, "Price item", None))
        .await
        .unwrap();
    let platform = platform();
    let request = || SupplyVersionAppend {
        price: "100.01".into(),
        shop: "Second shop".into(),
        revision: item.revision,
        image_file_id: None,
    };
    let (a, b) = tokio::join!(
        services::append_version(&pool, &platform, &manager, item.id, request()),
        services::append_version(&pool, &platform, &manager, item.id, request())
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let detail = services::get_item(&pool, &manager, item.id).await.unwrap();
    assert_eq!(detail.versions.len(), 2);
    assert_eq!(detail.item.latest_version.version, 2);
    let first = &detail.versions[1];
    let edited = services::write_version(
        &pool,
        &manager,
        item.id,
        Some(first.id),
        SupplyVersionWrite {
            price: "9.99".into(),
            shop: "Correction".into(),
            revision: first.revision,
        },
    )
    .await
    .unwrap();
    assert_eq!(edited.item.latest_version.price, "100.01");
    assert_eq!(edited.versions[1].price, "9.99");
    assert!(matches!(
        services::write_version(
            &pool,
            &manager,
            item.id,
            Some(first.id),
            SupplyVersionWrite {
                price: "8".into(),
                shop: "Stale".into(),
                revision: first.revision
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
}
#[tokio::test]
async fn price_snapshot_preserves_selected_version_authority_and_transaction_locks() {
    let (pool, manager, reader, category) = fixture("supply_price_snapshot").await;
    let item = services::create_item(
        &pool,
        &manager,
        product(category, "  Snapshot item  ", None),
    )
    .await
    .unwrap();
    let selected = item.latest_version.clone();
    let other = services::create_item(&pool, &manager, product(category, "Other item", None))
        .await
        .unwrap();
    let appended = services::write_version(
        &pool,
        &manager,
        item.id,
        None,
        SupplyVersionWrite {
            price: "99.90".into(),
            shop: "Latest shop".into(),
            revision: item.revision,
        },
    )
    .await
    .unwrap();
    assert_ne!(appended.item.latest_version.id, selected.id);

    let mut denied_tx = pool.begin().await.unwrap();
    let no_read = actor(manager.user_id, &[codes::SUPPLY_CATALOG_MANAGE_SCHOOL]);
    assert!(matches!(
        services::price_snapshot_in_transaction(&mut denied_tx, &no_read, item.id, selected.id)
            .await,
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        services::price_snapshot_in_transaction(
            &mut denied_tx,
            &reader,
            item.id,
            other.latest_version.id,
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    denied_tx.rollback().await.unwrap();

    let mut snapshot_tx = pool.begin().await.unwrap();
    let retained =
        services::price_snapshot_in_transaction(&mut snapshot_tx, &reader, item.id, selected.id)
            .await
            .unwrap();
    assert_eq!(retained.item_id, item.id);
    assert_eq!(retained.version_id, selected.id);
    assert_eq!(retained.revision, selected.revision);
    assert_eq!(retained.name, "Snapshot item");
    assert_eq!(retained.unit, "ชิ้น");
    assert_eq!(retained.price_satang, 1234);

    // Consumers must retain one coherent item/selected-price snapshot until their
    // own transaction commits, even though catalog metadata and prices are editable.
    let locks = [
        (
            "SELECT id FROM supply_items WHERE id=$1 FOR UPDATE NOWAIT",
            item.id,
        ),
        (
            "SELECT id FROM supply_item_versions WHERE id=$1 FOR UPDATE NOWAIT",
            selected.id,
        ),
    ];
    for (query, id) in locks {
        let error = sqlx::query(query)
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("55P03")
        );
    }
    snapshot_tx.commit().await.unwrap();
    for (query, id) in locks {
        sqlx::query(query).bind(id).fetch_one(&pool).await.unwrap();
    }

    services::update_item(
        &pool,
        &platform(),
        &manager,
        item.id,
        SupplyItemUpdate {
            name: "Updated snapshot item".into(),
            description: item.description.clone(),
            category_id: category,
            unit: "กล่อง".into(),
            image_file_id: None,
            revision: appended.item.revision,
        },
    )
    .await
    .unwrap();
    services::write_version(
        &pool,
        &manager,
        item.id,
        Some(selected.id),
        SupplyVersionWrite {
            price: "78.90".into(),
            shop: "Corrected shop".into(),
            revision: selected.revision,
        },
    )
    .await
    .unwrap();
    let mut fresh_tx = pool.begin().await.unwrap();
    let fresh =
        services::price_snapshot_in_transaction(&mut fresh_tx, &reader, item.id, selected.id)
            .await
            .unwrap();
    fresh_tx.commit().await.unwrap();
    assert_eq!((fresh.item_id, fresh.version_id), (item.id, selected.id));
    assert_eq!(fresh.revision, selected.revision + 1);
    assert_eq!(fresh.name, "Updated snapshot item");
    assert_eq!(fresh.unit, "กล่อง");
    assert_eq!(fresh.price_satang, 7890);
    assert_eq!(retained.revision, selected.revision);
    assert_eq!(retained.name, "Snapshot item");
    assert_eq!(retained.unit, "ชิ้น");
    assert_eq!(retained.price_satang, 1234);
    pool.close().await;
}
#[tokio::test]
async fn bundles_resolve_latest_prices_and_keep_ownership() {
    let (pool, manager, reader, category) = fixture("supply_bundles").await;
    let item = services::create_item(&pool, &manager, product(category, "Bundle item", None))
        .await
        .unwrap();
    let bundle = services::write_bundle(
        &pool,
        &reader,
        None,
        SupplyBundleWrite {
            name: "Own bundle".into(),
            shared: false,
            entries: vec![SupplyBundleEntryWrite {
                item_id: item.id,
                quantity: 3,
            }],
            revision: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(bundle.total_price, "37.02");
    assert!(services::write_bundle(
        &pool,
        &manager,
        Some(bundle.id),
        SupplyBundleWrite {
            name: "Other edit".into(),
            shared: false,
            entries: vec![SupplyBundleEntryWrite {
                item_id: item.id,
                quantity: 1
            }],
            revision: Some(bundle.revision)
        }
    )
    .await
    .is_err());
    assert!(services::write_bundle(
        &pool,
        &reader,
        None,
        SupplyBundleWrite {
            name: "No shared authority".into(),
            shared: true,
            entries: vec![SupplyBundleEntryWrite {
                item_id: item.id,
                quantity: 1
            }],
            revision: None
        }
    )
    .await
    .is_err());
    services::append_version(
        &pool,
        &platform(),
        &manager,
        item.id,
        SupplyVersionAppend {
            price: "20.00".into(),
            shop: "Latest shop".into(),
            revision: item.revision,
            image_file_id: None,
        },
    )
    .await
    .unwrap();
    let page = services::list_bundles(
        &pool,
        &reader,
        SupplyPageQuery {
            page: None,
            page_size: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(page.items[0].total_price, "60.00");
    assert_eq!(
        page.items[0].entries[0].item.latest_version.shop,
        "Latest shop"
    );
    assert!(services::delete_bundle(
        &pool,
        &reader,
        bundle.id,
        SupplyRevision {
            revision: bundle.revision + 1
        }
    )
    .await
    .is_err());
}
#[tokio::test]
async fn proposal_approval_is_atomic_private_and_retires_source_on_provider_failure() {
    let (pool, manager, proposer, category) = fixture("supply_approval").await;
    let photo = image(&pool, proposer.user_id, FilePurpose::SupplyCatalogImage).await;
    let source = image(&pool, proposer.user_id, FilePurpose::SupplyProposalSource).await;
    let row = services::create_proposal(
        &pool,
        &proposer,
        proposal(category, Some(photo.id), Some(source.id)),
    )
    .await
    .unwrap();
    let outsider = actor(manager.user_id, &[codes::SUPPLY_CATALOG_REQUEST_OWN]);
    assert!(services::get_proposal(&pool, &outsider, row.id)
        .await
        .is_err());
    assert!(file_relationships::require_file_access(
        &pool,
        &outsider,
        &source,
        FilePolicyAction::Read,
        Some(row.id)
    )
    .await
    .is_err());
    let retention: (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT retention_class,expires_at FROM files WHERE id=$1")
            .bind(photo.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retention, ("standard".into(), None));
    let mut invalid = approval(&row);
    invalid.category_id = Uuid::now_v7();
    assert!(
        services::approve_proposal(&pool, &platform(), &manager, row.id, invalid)
            .await
            .is_err()
    );
    assert_eq!(
        services::get_proposal(&pool, &proposer, row.id)
            .await
            .unwrap()
            .status,
        "pending"
    );
    assert_eq!(lifecycle(&pool, source.id).await, "ready");
    let platform = platform();
    let (a, b) = tokio::join!(
        services::approve_proposal(&pool, &platform, &manager, row.id, approval(&row)),
        services::approve_proposal(&pool, &platform, &manager, row.id, approval(&row))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let detail = a.ok().or(b.ok()).unwrap();
    assert_eq!(detail.item.image_file_id, Some(photo.id));
    assert_eq!(detail.versions_total, 1);
    let decided = services::get_proposal(&pool, &proposer, row.id)
        .await
        .unwrap();
    assert_eq!(decided.status, "approved");
    assert!(
        decided.source_url.is_none()
            && decided.source_file_id.is_none()
            && decided.image_file_id.is_none()
    );
    assert_eq!(lifecycle(&pool, source.id).await, "delete_requested");
    assert_eq!(lifecycle(&pool, photo.id).await, "ready");
    assert!(file_relationships::require_file_access(
        &pool,
        &proposer,
        &photo,
        FilePolicyAction::Read,
        Some(row.id)
    )
    .await
    .is_err());
    file_relationships::require_file_access(
        &pool,
        &manager,
        &photo,
        FilePolicyAction::Read,
        Some(detail.item.id),
    )
    .await
    .unwrap();
}
#[tokio::test]
async fn rejection_clears_private_sources_and_requests_durable_deletion() {
    let (pool, manager, proposer, category) = fixture("supply_rejection").await;
    let photo = image(&pool, proposer.user_id, FilePurpose::SupplyCatalogImage).await;
    let source = image(&pool, proposer.user_id, FilePurpose::SupplyProposalSource).await;
    let row = services::create_proposal(
        &pool,
        &proposer,
        proposal(category, Some(photo.id), Some(source.id)),
    )
    .await
    .unwrap();
    let decided = services::reject_proposal(
        &pool,
        &platform(),
        &manager,
        row.id,
        SupplyProposalReject {
            revision: row.revision,
            reason: "Not standardized".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(decided.status, "rejected");
    assert!(
        decided.image_file_id.is_none()
            && decided.source_file_id.is_none()
            && decided.source_url.is_none()
    );
    assert_eq!(lifecycle(&pool, photo.id).await, "delete_requested");
    assert_eq!(lifecycle(&pool, source.id).await, "delete_requested");
    assert!(services::reject_proposal(
        &pool,
        &platform(),
        &manager,
        row.id,
        SupplyProposalReject {
            revision: row.revision,
            reason: "Second decision".into()
        }
    )
    .await
    .is_err());
}
#[tokio::test]
async fn existing_approval_edits_metadata_appends_and_replaces_shared_photo() {
    let (pool, manager, proposer, category) = fixture("supply_existing_approval").await;
    let old = image(&pool, manager.user_id, FilePurpose::SupplyCatalogImage).await;
    let item = services::create_item(&pool, &manager, product(category, "Old name", Some(old.id)))
        .await
        .unwrap();
    let photo = image(&pool, proposer.user_id, FilePurpose::SupplyCatalogImage).await;
    let row = services::create_proposal(&pool, &proposer, proposal(category, Some(photo.id), None))
        .await
        .unwrap();
    let mut request = approval(&row);
    request.existing_item_id = Some(item.id);
    request.existing_item_revision = Some(item.revision);
    request.name = "Reviewed name".into();
    request.description = "Reviewed details".into();
    request.unit = "กล่อง".into();
    request.price = "99.90".into();
    request.shop = "Reviewed shop".into();
    let detail = services::approve_proposal(&pool, &platform(), &manager, row.id, request)
        .await
        .unwrap();
    assert_eq!(detail.item.id, item.id);
    assert_eq!(detail.item.name, "Reviewed name");
    assert_eq!(detail.item.unit, "กล่อง");
    assert_eq!(detail.item.latest_version.price, "99.90");
    assert_eq!(detail.item.image_file_id, Some(photo.id));
    assert_eq!(detail.versions_total, 2);
    assert_eq!(lifecycle(&pool, old.id).await, "delete_requested");
}
#[tokio::test]
async fn attachments_require_creator_clean_ready_upload_and_global_exclusivity() {
    let (pool, manager, other, category) = fixture("supply_image_attachment").await;
    let foreign = image(&pool, other.user_id, FilePurpose::SupplyCatalogImage).await;
    assert!(services::create_item(
        &pool,
        &manager,
        product(category, "Foreign photo", Some(foreign.id))
    )
    .await
    .is_err());
    let file = image(&pool, manager.user_id, FilePurpose::SupplyCatalogImage).await;
    sqlx::query("UPDATE file_versions SET scan_status='skipped' WHERE file_id=$1")
        .bind(file.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(services::create_item(
        &pool,
        &manager,
        product(category, "Unchecked photo", Some(file.id))
    )
    .await
    .is_err());
    sqlx::query("UPDATE file_versions SET scan_status='clean' WHERE file_id=$1")
        .bind(file.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE files SET expires_at=now()-interval '1 second' WHERE id=$1")
        .bind(file.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(services::create_item(
        &pool,
        &manager,
        product(category, "Expired photo", Some(file.id))
    )
    .await
    .is_err());
    sqlx::query("UPDATE files SET expires_at=now()+interval '1 day' WHERE id=$1")
        .bind(file.id)
        .execute(&pool)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        services::create_item(
            &pool,
            &manager,
            product(category, "Attached one", Some(file.id))
        ),
        services::create_proposal(&pool, &manager, proposal(category, Some(file.id), None))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(
        file_relationships::authorize_delete_guard(&pool, &manager, &file, None)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn generic_delete_guard_holds_file_lock_and_blocks_racing_attach() {
    let (pool, manager, _, category) = fixture("supply_file_delete_race").await;
    let file = image(&pool, manager.user_id, FilePurpose::SupplyCatalogImage).await;
    let mut tx = file_relationships::authorize_delete_guard(&pool, &manager, &file, None)
        .await
        .unwrap();
    let error = sqlx::query("SELECT id FROM files WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(file.id)
        .fetch_one(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("55P03")
    );
    let pool_clone = pool.clone();
    let actor_clone = manager.clone();
    let pending = tokio::spawn(async move {
        services::create_item(
            &pool_clone,
            &actor_clone,
            product(category, "Race attachment", Some(file.id)),
        )
        .await
    });
    let repository = SqlFileRepository::new(pool.clone());
    repository
        .request_delete_in_transaction(&mut tx, file.id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(pending.await.unwrap().is_err());
    assert_eq!(lifecycle(&pool, file.id).await, "delete_requested");
}

#[tokio::test]
async fn history_paging_keeps_old_versions_editable_without_changing_latest_price() {
    let (pool, manager, _, category) = fixture("supply_history_paging").await;
    let item = services::create_item(&pool, &manager, product(category, "History item", None))
        .await
        .unwrap();
    sqlx::query("INSERT INTO supply_item_versions(id,item_id,version,price_satang,shop,created_by) SELECT uuidv7(),$1,history.version,history.version*100,'History fixture',$2 FROM generate_series(2,102) AS history(version)").bind(item.id).bind(manager.user_id).execute(&pool).await.unwrap();
    let detail = services::get_item(&pool, &manager, item.id).await.unwrap();
    assert_eq!(detail.versions.len(), 100);
    assert_eq!(detail.versions_total, 102);
    let older = services::list_versions(
        &pool,
        &manager,
        item.id,
        SupplyPageQuery {
            page: Some(2),
            page_size: Some(100),
        },
    )
    .await
    .unwrap();
    assert_eq!(older.total, 102);
    assert_eq!(
        older.items.iter().map(|v| v.version).collect::<Vec<_>>(),
        vec![2, 1]
    );
    let first = older.items.iter().find(|v| v.version == 1).unwrap();
    let updated = services::write_version(
        &pool,
        &manager,
        item.id,
        Some(first.id),
        SupplyVersionWrite {
            price: "99.99".into(),
            shop: "Edited old shop".into(),
            revision: first.revision,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.item.latest_version.price, "102.00");
    let older = services::list_versions(
        &pool,
        &manager,
        item.id,
        SupplyPageQuery {
            page: Some(2),
            page_size: Some(100),
        },
    )
    .await
    .unwrap();
    assert_eq!(older.items.last().unwrap().price, "99.99");
}

#[tokio::test]
async fn actual_app_catalog_routes_share_the_session_boundary_and_exact_prefix() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use school_auth::{
        config::SessionConfig, runtime::AuthRuntime, session_crypto::SessionHmacKey,
    };
    use school_authorization::PermissionCache;
    use school_tenancy::{AdminClient, AdminClientConfig, PoolManager};
    use tower::ServiceExt;
    let (pool, _, _, _) = fixture("supply_route_session_boundary").await;
    let pool_manager = Arc::new(PoolManager::new());
    pool_manager
        .insert_test_pool("test-pool://supply-route", pool)
        .await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let tenant_id = Uuid::now_v7();
    let server = tokio::spawn(async move {
        axum::serve(listener, axum::Router::new().route("/internal/schools/{subdomain}", axum::routing::get(move || async move {
            axum::Json(serde_json::json!({"id":tenant_id,"name":"Fixture","db_connection_string":"test-pool://supply-route"}))
        }))).await.unwrap();
    });
    let admin_client = Arc::new(AdminClient::new(
        format!("http://{address}"),
        "fixture-internal-secret".into(),
        AdminClientConfig::for_tests(Duration::from_secs(1), 1, Duration::from_millis(1)),
    ));
    let permission_cache = Arc::new(PermissionCache::new());
    let permission_events = tokio::sync::broadcast::channel(8).0;
    let state = crate::AppState {
        admin_client: admin_client.clone(),
        pool_manager: pool_manager.clone(),

        auth_runtime: AuthRuntime {
            permission_cache,
            admin_client,
            pool_manager,

            identity_cache: Arc::new(school_auth::session_cache::SessionCache::new()),
            config: Arc::new({
                let mut c = SessionConfig::for_tests(SessionHmacKey::for_tests([32; 32]));
                c.base_domain = "schoolorbit.app".into();
                c
            }),
            session_events: tokio::sync::broadcast::channel(8).0,
            permission_events: permission_events.clone(),
        },
        websocket_manager: Arc::new(school_academic_http::websockets::WebSocketManager::new()),
        notification_channel: tokio::sync::broadcast::channel(8).0,

        work_event_channel: tokio::sync::broadcast::channel(8).0,
        file_platform: Arc::new(platform()),
        file_transfer: Arc::new(
            school_staff::file_transfer::runtime::FileTransferRuntime::default(),
        ),
        certificate_verification_limiter: Arc::new(
            school_certificates::verification_limiter::CertificateVerificationLimiter::new(),
        ),
    };
    let app = crate::app::build_app(state);
    let id = "11111111-1111-7111-8111-111111111111";
    for (method, tail) in [
        ("GET", "categories".to_owned()),
        ("POST", "categories".to_owned()),
        ("PUT", format!("categories/{id}")),
        ("GET", "items".to_owned()),
        ("POST", "items".to_owned()),
        ("GET", format!("items/{id}")),
        ("PUT", format!("items/{id}")),
        ("GET", format!("items/{id}/versions")),
        ("POST", format!("items/{id}/versions")),
        ("PUT", format!("items/{id}/versions/{id}")),
        ("PUT", format!("items/{id}/favorite")),
        ("GET", "duplicates".to_owned()),
        ("GET", "bundles".to_owned()),
        ("POST", "bundles".to_owned()),
        ("PUT", format!("bundles/{id}")),
        ("DELETE", format!("bundles/{id}")),
        ("GET", "proposals".to_owned()),
        ("POST", "proposals".to_owned()),
        ("GET", format!("proposals/{id}")),
        ("POST", format!("proposals/{id}/approve")),
        ("POST", format!("proposals/{id}/reject")),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/api/supply-catalog/{tail}"))
                    .header("Origin", "https://fixture.schoolorbit.app")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {tail} must be a mounted protected route"
        );
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/categories")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
