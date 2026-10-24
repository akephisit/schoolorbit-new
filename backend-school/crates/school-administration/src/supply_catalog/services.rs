use super::{file_relationships as files, models::*, validation::*};
use crate::supply_catalog::access_policy::require_action;
use crate::supply_catalog::access_policy::SupplyAction;
use chrono::{DateTime, Utc};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_file_platform::{platform_service::FilePlatform, platform_types::FilePurpose};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const ITEM_SELECT:&str="SELECT i.id,i.name,i.description,i.category_id,c.name AS category_name,i.unit,i.image_file_id,i.revision,v.id AS version_id,v.version,v.price_satang,v.shop,v.revision AS version_revision,v.created_at,v.updated_at,EXISTS(SELECT 1 FROM supply_favorites f WHERE f.item_id=i.id AND f.user_id=$1) AS favorite FROM supply_items i JOIN supply_categories c ON c.id=i.category_id JOIN LATERAL(SELECT * FROM supply_item_versions WHERE item_id=i.id ORDER BY version DESC LIMIT 1)v ON true";
#[derive(sqlx::FromRow)]
struct ItemRow {
    id: Uuid,
    name: String,
    description: String,
    category_id: Uuid,
    category_name: String,
    unit: String,
    image_file_id: Option<Uuid>,
    revision: i64,
    version_id: Uuid,
    version: i32,
    price_satang: i64,
    shop: String,
    version_revision: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    favorite: bool,
}
impl ItemRow {
    fn dto(self) -> SupplyItem {
        SupplyItem {
            id: self.id,
            name: self.name,
            description: self.description,
            category_id: self.category_id,
            category_name: self.category_name,
            unit: self.unit,
            image_file_id: self.image_file_id,
            revision: self.revision,
            favorite: self.favorite,
            latest_version: SupplyVersion {
                id: self.version_id,
                version: self.version,
                price: format_price(self.price_satang),
                shop: self.shop,
                revision: self.version_revision,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
        }
    }
}
fn missing() -> AppError {
    AppError::NotFound("ไม่พบรายการวัสดุ".into())
}
pub async fn list_categories(
    pool: &PgPool,
    actor: &ActorContext,
) -> Result<Vec<SupplyCategory>, AppError> {
    require_action(actor, SupplyAction::Read, None, false)?;
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.list_categories")
            .await?;
    Ok(school_database::DbSession::new(&mut *connection)
        .query_as(
            "administration.supply_catalog.services.list_categories",
            "SELECT id,name,revision FROM supply_categories ORDER BY normalized_name,id",
        )
        .fetch_all()
        .await?)
}
pub async fn write_category(
    pool: &PgPool,
    actor: &ActorContext,
    id: Option<Uuid>,
    payload: SupplyCategoryWrite,
) -> Result<SupplyCategory, AppError> {
    require_action(actor, SupplyAction::Manage, None, false)?;
    let name = text(&payload.name, 120, true)?;
    let normalized = normalize(&name);
    if let Some(id) = id {
        let expected = payload
            .revision
            .ok_or_else(|| AppError::BadRequest("ต้องระบุ revision".into()))?;
        let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "administration.update_category",
        )
        .await?;
        return school_database::DbSession::new(&mut *connection).query_as("administration.supply_catalog.services.write_category", "UPDATE supply_categories SET name=$2,normalized_name=$3,revision=revision+1 WHERE id=$1 AND revision=$4 RETURNING id,name,revision").bind(id).bind(name).bind(normalized).bind(expected).fetch_optional().await.map_err(db)?.ok_or_else(||AppError::Conflict("หมวดหมู่เปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
    }
    if payload.revision.is_some() {
        return Err(AppError::BadRequest("รายการใหม่ไม่ต้องระบุ revision".into()));
    }
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.insert_category")
            .await?;
    Ok(school_database::DbSession::new(&mut *connection).query_as("administration.supply_catalog.services.write_category", "INSERT INTO supply_categories(id,name,normalized_name)VALUES($1,$2,$3)RETURNING id,name,revision").bind(Uuid::now_v7()).bind(name).bind(normalized).fetch_one().await.map_err(db)?)
}
pub async fn list_items(
    pool: &PgPool,
    actor: &ActorContext,
    query: SupplyItemQuery,
) -> Result<SupplyItemPage, AppError> {
    require_action(actor, SupplyAction::Read, None, false)?;
    let (page, page_size, offset) = paging(query.page, query.page_size)?;
    let search = query
        .search
        .map(|s| text(&s, 200, false))
        .transpose()?
        .map(|s| normalize(&s));
    let favorite = query.favorites_only.unwrap_or(false);
    let filter=" WHERE ($2::text IS NULL OR strpos(i.normalized_name,$2)>0 OR strpos(lower(normalize(i.description,NFKC)),$2)>0) AND ($3::uuid IS NULL OR i.category_id=$3) AND (NOT $4 OR EXISTS(SELECT 1 FROM supply_favorites f WHERE f.item_id=i.id AND f.user_id=$1))";
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.list_items")
            .await?;
    let total: i64 = school_database::DbSession::new(&mut *connection)
        .scalar(
            "administration.supply_catalog.services.list_items",
            sqlx::AssertSqlSafe(format!("SELECT count(*) FROM supply_items i{filter}")),
        )
        .bind(actor.user_id)
        .bind(&search)
        .bind(query.category_id)
        .bind(favorite)
        .fetch_one()
        .await?;
    let items = school_database::DbSession::new(&mut *connection)
        .query_as::<ItemRow>(
            "administration.supply_catalog.services.list_items",
            sqlx::AssertSqlSafe(format!(
                "{ITEM_SELECT}{filter} ORDER BY i.normalized_name,i.id LIMIT $5 OFFSET $6"
            )),
        )
        .bind(actor.user_id)
        .bind(search)
        .bind(query.category_id)
        .bind(favorite)
        .bind(page_size)
        .bind(offset)
        .fetch_all()
        .await?
        .into_iter()
        .map(ItemRow::dto)
        .collect();
    Ok(SupplyItemPage {
        items,
        total,
        page,
        page_size,
    })
}
pub async fn get_item(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<SupplyItemDetail, AppError> {
    require_action(actor, SupplyAction::Read, None, false)?;
    read_item(pool, actor.user_id, id).await
}
pub async fn list_versions(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    query: SupplyPageQuery,
) -> Result<SupplyVersionPage, AppError> {
    require_action(actor, SupplyAction::Read, None, false)?;
    let (page, page_size, offset) = paging(query.page, query.page_size)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    school_database::DbSession::new(&mut *tx)
        .query(
            "administration.supply_catalog.services.list_versions",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let exists: bool = school_database::DbSession::new(&mut *tx)
        .scalar(
            "administration.supply_catalog.services.list_versions",
            "SELECT EXISTS(SELECT 1 FROM supply_items WHERE id=$1)",
        )
        .bind(id)
        .fetch_one()
        .await?;
    if !exists {
        return Err(missing());
    }
    let total = school_database::DbSession::new(&mut *tx)
        .scalar(
            "administration.supply_catalog.services.list_versions",
            "SELECT count(*) FROM supply_item_versions WHERE item_id=$1",
        )
        .bind(id)
        .fetch_one()
        .await?;
    let items = school_database::DbSession::new(&mut *tx).query_as("administration.supply_catalog.services.list_versions", "SELECT id,version,(price_satang/100)::text||'.'||lpad((price_satang%100)::text,2,'0') AS price,shop,revision,created_at,updated_at FROM supply_item_versions WHERE item_id=$1 ORDER BY version DESC LIMIT $2 OFFSET $3").bind(id).bind(page_size).bind(offset).fetch_all().await?;
    tx.commit().await?;
    Ok(SupplyVersionPage {
        items,
        total,
        page,
        page_size,
    })
}

async fn read_item(pool: &PgPool, user: Uuid, id: Uuid) -> Result<SupplyItemDetail, AppError> {
    // One snapshot covers metadata and history so concurrent price edits cannot split the response.
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    school_database::DbSession::new(&mut *tx)
        .query(
            "administration.supply_catalog.services.read_item",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let item = school_database::DbSession::new(&mut *tx)
        .query_as::<ItemRow>(
            "administration.supply_catalog.services.read_item",
            sqlx::AssertSqlSafe(format!("{ITEM_SELECT} WHERE i.id=$2")),
        )
        .bind(user)
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(missing)?
        .dto();
    let versions=school_database::DbSession::new(&mut *tx).query_as("administration.supply_catalog.services.read_item", "SELECT id,version,(price_satang/100)::text||'.'||lpad((price_satang%100)::text,2,'0') AS price,shop,revision,created_at,updated_at FROM supply_item_versions WHERE item_id=$1 ORDER BY version DESC LIMIT 100").bind(id).fetch_all().await?;
    let versions_total = school_database::DbSession::new(&mut *tx)
        .scalar(
            "administration.supply_catalog.services.read_item",
            "SELECT count(*) FROM supply_item_versions WHERE item_id=$1",
        )
        .bind(id)
        .fetch_one()
        .await?;
    tx.commit().await?;
    Ok(SupplyItemDetail {
        item,
        versions,
        versions_total,
    })
}
#[derive(Clone, sqlx::FromRow)]
struct ItemScope {
    revision: i64,
    image_file_id: Option<Uuid>,
}
async fn item_scope(pool: &PgPool, id: Uuid) -> Result<ItemScope, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.item_scope")
            .await?;
    item_scope_on_connection(&mut connection, id).await
}

async fn item_scope_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<ItemScope, AppError> {
    school_database::DbSession::new(&mut *connection)
        .query_as(
            "administration.supply_catalog.services.item_scope_on_connection",
            "SELECT revision,image_file_id FROM supply_items WHERE id=$1",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(missing)
}
async fn lock_item(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<ItemScope, AppError> {
    school_database::DbSession::new(&mut **tx)
        .query_as(
            "administration.supply_catalog.services.lock_item",
            "SELECT revision,image_file_id FROM supply_items WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(missing)
}
struct Product {
    name: String,
    description: String,
    category_id: Uuid,
    unit: String,
    image: Option<Uuid>,
    price: i64,
    shop: String,
}
fn product(payload: SupplyItemWrite) -> Result<Product, AppError> {
    Ok(Product {
        name: text(&payload.name, 200, true)?,
        description: text(&payload.description, 5000, false)?,
        category_id: payload.category_id,
        unit: text(&payload.unit, 80, true)?,
        image: payload.image_file_id,
        price: parse_price(&payload.price)?,
        shop: text(&payload.shop, 300, true)?,
    })
}
async fn insert_product(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    p: &Product,
) -> Result<Uuid, AppError> {
    let id = Uuid::now_v7();
    school_database::DbSession::new(&mut **tx).query("administration.supply_catalog.services.insert_product", "INSERT INTO supply_items(id,name,normalized_name,description,category_id,unit,normalized_unit,image_file_id,created_by)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(id).bind(&p.name).bind(normalize(&p.name)).bind(&p.description).bind(p.category_id).bind(&p.unit).bind(normalize(&p.unit)).bind(p.image).bind(actor.user_id).execute().await.map_err(db)?;
    insert_version(tx, actor, id, p.price, &p.shop).await?;
    Ok(id)
}
async fn insert_version(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    price: i64,
    shop: &str,
) -> Result<(), AppError> {
    school_database::DbSession::new(&mut **tx).query("administration.supply_catalog.services.insert_version", "INSERT INTO supply_item_versions(id,item_id,version,price_satang,shop,created_by) SELECT $1,$2,COALESCE(max(version),0)+1,$3,$4,$5 FROM supply_item_versions WHERE item_id=$2").bind(Uuid::now_v7()).bind(id).bind(price).bind(shop).bind(actor.user_id).execute().await.map_err(db)?;
    Ok(())
}
async fn update_metadata(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    p: &Product,
) -> Result<(), AppError> {
    school_database::DbSession::new(&mut **tx).query("administration.supply_catalog.services.update_metadata", "UPDATE supply_items SET name=$2,normalized_name=$3,description=$4,category_id=$5,unit=$6,normalized_unit=$7,image_file_id=$8,revision=revision+1 WHERE id=$1").bind(id).bind(&p.name).bind(normalize(&p.name)).bind(&p.description).bind(p.category_id).bind(&p.unit).bind(normalize(&p.unit)).bind(p.image).execute().await.map_err(db)?;
    Ok(())
}
pub async fn create_item(
    pool: &PgPool,
    actor: &ActorContext,
    payload: SupplyItemWrite,
) -> Result<SupplyItem, AppError> {
    require_action(actor, SupplyAction::Manage, None, false)?;
    let p = product(payload)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    files::lock_files(&mut tx, p.image).await?;
    if let Some(id) = p.image {
        files::attach(&mut tx, actor, id, FilePurpose::SupplyCatalogImage, None).await?;
    }
    let id = insert_product(&mut tx, actor, &p).await?;
    tx.commit().await?;
    Ok(read_item(pool, actor.user_id, id).await?.item)
}
pub async fn update_item(
    pool: &PgPool,
    platform: &FilePlatform,
    actor: &ActorContext,
    id: Uuid,
    payload: SupplyItemUpdate,
) -> Result<SupplyItem, AppError> {
    require_action(actor, SupplyAction::Manage, None, false)?;
    let p = Product {
        name: text(&payload.name, 200, true)?,
        description: text(&payload.description, 5000, false)?,
        category_id: payload.category_id,
        unit: text(&payload.unit, 80, true)?,
        image: payload.image_file_id,
        price: 0,
        shop: String::new(),
    };
    let scope = item_scope(pool, id).await?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    files::lock_files(&mut tx, scope.image_file_id.into_iter().chain(p.image)).await?;
    let locked = lock_item(&mut tx, id).await?;
    revision(locked.revision, payload.revision)?;
    if locked.image_file_id != scope.image_file_id {
        return Err(AppError::Conflict("รูปวัสดุเปลี่ยนแล้ว".into()));
    }
    if p.image != locked.image_file_id {
        if let Some(file) = p.image {
            files::attach(&mut tx, actor, file, FilePurpose::SupplyCatalogImage, None).await?;
        }
    }
    update_metadata(&mut tx, id, &p).await?;
    let work = files::prepare_delete(
        pool,
        &mut tx,
        locked.image_file_id.filter(|old| Some(*old) != p.image),
    )
    .await?;
    tx.commit().await?;
    files::finish_delete(platform, pool, work).await;
    Ok(read_item(pool, actor.user_id, id).await?.item)
}
pub async fn write_version(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    version_id: Option<Uuid>,
    payload: SupplyVersionWrite,
) -> Result<SupplyItemDetail, AppError> {
    require_action(actor, SupplyAction::Manage, None, false)?;
    let price = parse_price(&payload.price)?;
    let shop = text(&payload.shop, 300, true)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    let scope = lock_item(&mut tx, id).await?;
    if let Some(version_id) = version_id {
        let actual: i64 = school_database::DbSession::new(&mut *tx)
            .scalar(
                "administration.supply_catalog.services.write_version",
                "SELECT revision FROM supply_item_versions WHERE id=$1 AND item_id=$2 FOR UPDATE",
            )
            .bind(version_id)
            .bind(id)
            .fetch_optional()
            .await?
            .ok_or_else(missing)?;
        revision(actual, payload.revision)?;
        school_database::DbSession::new(&mut *tx).query("administration.supply_catalog.services.write_version", "UPDATE supply_item_versions SET price_satang=$2,shop=$3,revision=revision+1,updated_at=now() WHERE id=$1").bind(version_id).bind(price).bind(shop).execute().await?;
    } else {
        revision(scope.revision, payload.revision)?;
        insert_version(&mut tx, actor, id, price, &shop).await?;
    }
    school_database::DbSession::new(&mut *tx)
        .query(
            "administration.supply_catalog.services.write_version",
            "UPDATE supply_items SET revision=revision+1 WHERE id=$1",
        )
        .bind(id)
        .execute()
        .await?;
    tx.commit().await?;
    read_item(pool, actor.user_id, id).await
}
pub async fn set_favorite(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    payload: SupplyFavoriteWrite,
) -> Result<SupplyItem, AppError> {
    require_action(actor, SupplyAction::Read, None, false)?;
    item_scope(pool, id).await?;
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.set_favorite")
            .await?;
    if payload.favorite {
        school_database::DbSession::new(&mut *connection)
            .query(
                "administration.supply_catalog.services.set_favorite",
                "INSERT INTO supply_favorites(user_id,item_id)VALUES($1,$2)ON CONFLICT DO NOTHING",
            )
            .bind(actor.user_id)
            .bind(id)
            .execute()
            .await?;
    } else {
        school_database::DbSession::new(&mut *connection)
            .query(
                "administration.supply_catalog.services.set_favorite",
                "DELETE FROM supply_favorites WHERE user_id=$1 AND item_id=$2",
            )
            .bind(actor.user_id)
            .bind(id)
            .execute()
            .await?;
    }
    drop(connection);
    Ok(read_item(pool, actor.user_id, id).await?.item)
}
pub async fn append_version(
    pool: &PgPool,
    platform: &FilePlatform,
    actor: &ActorContext,
    id: Uuid,
    payload: SupplyVersionAppend,
) -> Result<SupplyItemDetail, AppError> {
    require_action(actor, SupplyAction::Manage, None, false)?;
    let price = parse_price(&payload.price)?;
    let shop = text(&payload.shop, 300, true)?;
    let scope = item_scope(pool, id).await?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    files::lock_files(
        &mut tx,
        scope.image_file_id.into_iter().chain(payload.image_file_id),
    )
    .await?;
    let locked = lock_item(&mut tx, id).await?;
    revision(locked.revision, payload.revision)?;
    if locked.image_file_id != scope.image_file_id {
        return Err(AppError::Conflict("รูปวัสดุเปลี่ยนแล้ว".into()));
    }
    if payload.image_file_id != locked.image_file_id {
        if let Some(file) = payload.image_file_id {
            files::attach(&mut tx, actor, file, FilePurpose::SupplyCatalogImage, None).await?;
        }
    }
    insert_version(&mut tx, actor, id, price, &shop).await?;
    school_database::DbSession::new(&mut *tx)
        .query(
            "administration.supply_catalog.services.append_version",
            "UPDATE supply_items SET image_file_id=$2,revision=revision+1 WHERE id=$1",
        )
        .bind(id)
        .bind(payload.image_file_id)
        .execute()
        .await?;
    let work = files::prepare_delete(
        pool,
        &mut tx,
        locked
            .image_file_id
            .filter(|old| Some(*old) != payload.image_file_id),
    )
    .await?;
    tx.commit().await?;
    files::finish_delete(platform, pool, work).await;
    read_item(pool, actor.user_id, id).await
}
#[derive(sqlx::FromRow)]
struct BundleRow {
    id: Uuid,
    name: String,
    shared: bool,
    owner_user_id: Uuid,
    revision: i64,
}
#[derive(sqlx::FromRow)]
struct BundleEntryRow {
    #[sqlx(flatten)]
    item: ItemRow,
    quantity: i32,
    bundle_id: Uuid,
}
fn total_money(value: i128) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}
async fn fetch_bundle_entries(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<BundleEntryRow>>, AppError> {
    let entries=school_database::DbSession::new(&mut **tx).query_as::<BundleEntryRow>("administration.supply_catalog.services.fetch_bundle_entries", sqlx::AssertSqlSafe(format!("SELECT item.*,e.quantity,e.bundle_id FROM({ITEM_SELECT})item JOIN supply_bundle_entries e ON e.item_id=item.id WHERE e.bundle_id=ANY($2) ORDER BY e.bundle_id,item.name,item.id"))).bind(user).bind(ids).fetch_all().await?;
    let mut grouped = std::collections::HashMap::<Uuid, Vec<BundleEntryRow>>::new();
    for entry in entries {
        grouped.entry(entry.bundle_id).or_default().push(entry);
    }
    Ok(grouped)
}
fn bundle_dto(row: BundleRow, entries: Vec<BundleEntryRow>) -> SupplyBundle {
    let mut total = 0_i128;
    let entries = entries
        .into_iter()
        .map(|e| {
            let subtotal = i128::from(e.item.price_satang) * i128::from(e.quantity);
            total += subtotal;
            SupplyBundleEntry {
                item: e.item.dto(),
                quantity: e.quantity,
                subtotal: total_money(subtotal),
            }
        })
        .collect();
    SupplyBundle {
        id: row.id,
        name: row.name,
        shared: row.shared,
        owner_user_id: row.owner_user_id,
        revision: row.revision,
        entries,
        total_price: total_money(total),
    }
}
pub async fn list_bundles(
    pool: &PgPool,
    actor: &ActorContext,
    query: SupplyPageQuery,
) -> Result<SupplyBundlePage, AppError> {
    require_action(actor, SupplyAction::Read, None, false)?;
    let (page, page_size, offset) = paging(query.page, query.page_size)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    school_database::DbSession::new(&mut *tx)
        .query(
            "administration.supply_catalog.services.list_bundles",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let total: i64 = school_database::DbSession::new(&mut *tx)
        .scalar(
            "administration.supply_catalog.services.list_bundles",
            "SELECT count(*) FROM supply_bundles WHERE shared OR owner_user_id=$1",
        )
        .bind(actor.user_id)
        .fetch_one()
        .await?;
    let rows=school_database::DbSession::new(&mut *tx).query_as::<BundleRow>("administration.supply_catalog.services.list_bundles", "SELECT id,name,shared,owner_user_id,revision FROM supply_bundles WHERE shared OR owner_user_id=$1 ORDER BY name,id LIMIT $2 OFFSET $3").bind(actor.user_id).bind(page_size).bind(offset).fetch_all().await?;
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let mut grouped = fetch_bundle_entries(&mut tx, actor.user_id, &ids).await?;
    let items = rows
        .into_iter()
        .map(|row| {
            let entries = grouped.remove(&row.id).unwrap_or_default();
            bundle_dto(row, entries)
        })
        .collect();
    tx.commit().await?;
    Ok(SupplyBundlePage {
        items,
        total,
        page,
        page_size,
    })
}
pub async fn write_bundle(
    pool: &PgPool,
    actor: &ActorContext,
    id: Option<Uuid>,
    payload: SupplyBundleWrite,
) -> Result<SupplyBundle, AppError> {
    require_action(actor, SupplyAction::BundleWrite, None, payload.shared)?;
    let name = text(&payload.name, 200, true)?;
    if payload.entries.is_empty() || payload.entries.len() > 100 {
        return Err(AppError::BadRequest("ชุดวัสดุต้องมี 1 ถึง 100 รายการ".into()));
    }
    let mut seen = std::collections::HashSet::new();
    for entry in &payload.entries {
        if !(1..=1000000).contains(&entry.quantity) || !seen.insert(entry.item_id) {
            return Err(AppError::BadRequest("จำนวนไม่ถูกต้องหรือวัสดุซ้ำในชุด".into()));
        }
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    let id = if let Some(id) = id {
        let row=school_database::DbSession::new(&mut *tx).query_as::<BundleRow>("administration.supply_catalog.services.write_bundle", "SELECT id,name,shared,owner_user_id,revision FROM supply_bundles WHERE id=$1 FOR UPDATE").bind(id).fetch_optional().await?.ok_or_else(missing)?;
        require_action(
            actor,
            SupplyAction::BundleWrite,
            Some(row.owner_user_id),
            row.shared,
        )?;
        require_action(
            actor,
            SupplyAction::BundleWrite,
            Some(row.owner_user_id),
            payload.shared,
        )?;
        revision(
            row.revision,
            payload
                .revision
                .ok_or_else(|| AppError::BadRequest("ต้องระบุ revision".into()))?,
        )?;
        school_database::DbSession::new(&mut *tx)
            .query(
                "administration.supply_catalog.services.write_bundle",
                "UPDATE supply_bundles SET name=$2,shared=$3,revision=revision+1 WHERE id=$1",
            )
            .bind(id)
            .bind(&name)
            .bind(payload.shared)
            .execute()
            .await?;
        school_database::DbSession::new(&mut *tx)
            .query(
                "administration.supply_catalog.services.write_bundle",
                "DELETE FROM supply_bundle_entries WHERE bundle_id=$1",
            )
            .bind(id)
            .execute()
            .await?;
        id
    } else {
        if payload.revision.is_some() {
            return Err(AppError::BadRequest("รายการใหม่ไม่ต้องระบุ revision".into()));
        }
        let id = Uuid::now_v7();
        school_database::DbSession::new(&mut *tx)
            .query(
                "administration.supply_catalog.services.write_bundle",
                "INSERT INTO supply_bundles(id,name,shared,owner_user_id)VALUES($1,$2,$3,$4)",
            )
            .bind(id)
            .bind(&name)
            .bind(payload.shared)
            .bind(actor.user_id)
            .execute()
            .await?;
        id
    };
    let item_ids = payload
        .entries
        .iter()
        .map(|entry| entry.item_id)
        .collect::<Vec<_>>();
    let quantities = payload
        .entries
        .iter()
        .map(|entry| entry.quantity)
        .collect::<Vec<_>>();
    school_database::DbSession::new(&mut *tx).query("administration.supply_catalog.services.write_bundle", "INSERT INTO supply_bundle_entries(bundle_id,item_id,quantity) SELECT $1,item_id,quantity FROM UNNEST($2::uuid[],$3::integer[]) AS e(item_id,quantity)").bind(id).bind(&item_ids).bind(&quantities).execute().await.map_err(db)?;
    let row = school_database::DbSession::new(&mut *tx)
        .query_as::<BundleRow>(
            "administration.supply_catalog.services.write_bundle",
            "SELECT id,name,shared,owner_user_id,revision FROM supply_bundles WHERE id=$1",
        )
        .bind(id)
        .fetch_one()
        .await?;
    let mut grouped = fetch_bundle_entries(&mut tx, actor.user_id, &[id]).await?;
    let result = bundle_dto(row, grouped.remove(&id).unwrap_or_default());
    tx.commit().await?;
    Ok(result)
}
pub async fn delete_bundle(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    payload: SupplyRevision,
) -> Result<(), AppError> {
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    let row = school_database::DbSession::new(&mut *tx).query_as::<BundleRow>("administration.supply_catalog.services.delete_bundle",
        "SELECT id,name,shared,owner_user_id,revision FROM supply_bundles WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional()
    .await?
    .ok_or_else(missing)?;
    require_action(
        actor,
        SupplyAction::BundleWrite,
        Some(row.owner_user_id),
        row.shared,
    )?;
    revision(row.revision, payload.revision)?;
    school_database::DbSession::new(&mut *tx)
        .query(
            "administration.supply_catalog.services.delete_bundle",
            "DELETE FROM supply_bundles WHERE id=$1",
        )
        .bind(id)
        .execute()
        .await?;
    tx.commit().await?;
    Ok(())
}
const PROPOSAL_SELECT:&str="SELECT id,name,description,category_id,unit,image_file_id,(price_satang/100)::text||'.'||lpad((price_satang%100)::text,2,'0') AS price,shop,source_file_id,source_url,status,proposed_by,reviewed_by,reviewed_at,rejection_reason,item_id,revision,created_at FROM supply_proposals";
async fn load_proposal(pool: &PgPool, id: Uuid) -> Result<SupplyProposal, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.load_proposal")
            .await?;
    load_proposal_on_connection(&mut connection, id).await
}

async fn load_proposal_on_connection(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<SupplyProposal, AppError> {
    school_database::DbSession::new(&mut *connection)
        .query_as(
            "administration.supply_catalog.services.load_proposal_on_connection",
            sqlx::AssertSqlSafe(format!("{PROPOSAL_SELECT} WHERE id=$1")),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(missing)
}
fn require_proposal(actor: &ActorContext, row: &SupplyProposal) -> Result<(), AppError> {
    if actor.has_permission(school_permissions::registry::codes::SUPPLY_CATALOG_APPROVE_SCHOOL) {
        require_action(actor, SupplyAction::Review, None, false)
    } else {
        require_action(actor, SupplyAction::Propose, Some(row.proposed_by), false)
    }
}
pub async fn get_proposal(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<SupplyProposal, AppError> {
    let row = load_proposal(pool, id).await?;
    require_proposal(actor, &row)?;
    Ok(row)
}
pub async fn list_proposals(
    pool: &PgPool,
    actor: &ActorContext,
    query: SupplyProposalQuery,
) -> Result<SupplyProposalPage, AppError> {
    let review = query.review.unwrap_or(false);
    require_action(
        actor,
        if review {
            SupplyAction::Review
        } else {
            SupplyAction::Propose
        },
        None,
        false,
    )?;
    let (page, page_size, offset) = paging(query.page, query.page_size)?;
    let filter = " WHERE (($2 AND status='pending') OR (NOT $2 AND proposed_by=$1))";
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.list_proposals")
            .await?;
    let total = school_database::DbSession::new(&mut *connection)
        .scalar(
            "administration.supply_catalog.services.list_proposals",
            sqlx::AssertSqlSafe(format!("SELECT count(*) FROM supply_proposals{filter}")),
        )
        .bind(actor.user_id)
        .bind(review)
        .fetch_one()
        .await?;
    let order = if review {
        "created_at ASC,id ASC"
    } else {
        "created_at DESC,id DESC"
    };
    let items = school_database::DbSession::new(&mut *connection)
        .query_as(
            "administration.supply_catalog.services.list_proposals",
            sqlx::AssertSqlSafe(format!(
                "{PROPOSAL_SELECT}{filter} ORDER BY {order} LIMIT $3 OFFSET $4"
            )),
        )
        .bind(actor.user_id)
        .bind(review)
        .bind(page_size)
        .bind(offset)
        .fetch_all()
        .await?;
    Ok(SupplyProposalPage {
        items,
        total,
        page,
        page_size,
    })
}
pub async fn create_proposal(
    pool: &PgPool,
    actor: &ActorContext,
    payload: SupplyProposalWrite,
) -> Result<SupplyProposal, AppError> {
    require_action(actor, SupplyAction::Propose, None, false)?;
    let p = product(SupplyItemWrite {
        name: payload.name,
        description: payload.description,
        category_id: payload.category_id,
        unit: payload.unit,
        image_file_id: payload.image_file_id,
        price: payload.price,
        shop: payload.shop,
    })?;
    let source_url = payload
        .source_url
        .map(|s| text(&s, 2048, true))
        .transpose()?;
    if let Some(value) = &source_url {
        let url = url::Url::parse(value).map_err(|_| AppError::BadRequest("URL ไม่ถูกต้อง".into()))?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(AppError::BadRequest(
                "URL ต้องเป็น HTTP หรือ HTTPS และไม่มีข้อมูลเข้าสู่ระบบ".into(),
            ));
        }
    }
    if source_url.is_some() && payload.source_file_id.is_some() {
        return Err(AppError::BadRequest(
            "เลือกภาพต้นทางหรือ URL อย่างใดอย่างหนึ่ง".into(),
        ));
    }
    if p.image.is_some() && p.image == payload.source_file_id {
        return Err(AppError::BadRequest(
            "รูปผลิตภัณฑ์และภาพต้นทางต้องเป็นคนละไฟล์".into(),
        ));
    }
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    files::lock_files(&mut tx, p.image.into_iter().chain(payload.source_file_id)).await?;
    if let Some(file) = p.image {
        files::attach(&mut tx, actor, file, FilePurpose::SupplyCatalogImage, None).await?;
    }
    if let Some(file) = payload.source_file_id {
        files::attach(
            &mut tx,
            actor,
            file,
            FilePurpose::SupplyProposalSource,
            None,
        )
        .await?;
    }
    let id = Uuid::now_v7();
    school_database::DbSession::new(&mut *tx).query("administration.supply_catalog.services.create_proposal", "INSERT INTO supply_proposals(id,name,description,category_id,unit,image_file_id,price_satang,shop,source_file_id,source_url,proposed_by)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)").bind(id).bind(p.name).bind(p.description).bind(p.category_id).bind(p.unit).bind(p.image).bind(p.price).bind(p.shop).bind(payload.source_file_id).bind(source_url).bind(actor.user_id).execute().await.map_err(db)?;
    tx.commit().await?;
    load_proposal(pool, id).await
}
async fn lock_proposal(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    expected: i64,
    snapshot: &SupplyProposal,
) -> Result<SupplyProposal, AppError> {
    let row = school_database::DbSession::new(&mut **tx)
        .query_as::<SupplyProposal>(
            "administration.supply_catalog.services.lock_proposal",
            sqlx::AssertSqlSafe(format!("{PROPOSAL_SELECT} WHERE id=$1 FOR UPDATE")),
        )
        .bind(id)
        .fetch_optional()
        .await?
        .ok_or_else(missing)?;
    revision(row.revision, expected)?;
    if row.status != "pending"
        || row.image_file_id != snapshot.image_file_id
        || row.source_file_id != snapshot.source_file_id
    {
        return Err(AppError::Conflict("คำเสนอได้รับการตรวจสอบหรือเปลี่ยนแล้ว".into()));
    }
    Ok(row)
}
pub async fn approve_proposal(
    pool: &PgPool,
    platform: &FilePlatform,
    actor: &ActorContext,
    id: Uuid,
    payload: SupplyProposalApprove,
) -> Result<SupplyItemDetail, AppError> {
    require_action(actor, SupplyAction::Review, None, false)?;
    let p = product(SupplyItemWrite {
        name: payload.name,
        description: payload.description,
        category_id: payload.category_id,
        unit: payload.unit,
        image_file_id: payload.image_file_id,
        price: payload.price,
        shop: payload.shop,
    })?;
    if payload.existing_item_id.is_some() != payload.existing_item_revision.is_some() {
        return Err(AppError::BadRequest("ต้องระบุ revision ของวัสดุที่เลือก".into()));
    }
    let snapshot = load_proposal(pool, id).await?;
    let target = if let Some(target) = payload.existing_item_id {
        Some(item_scope(pool, target).await?)
    } else {
        None
    };
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    let file_ids = snapshot
        .image_file_id
        .into_iter()
        .chain(snapshot.source_file_id)
        .chain(p.image)
        .chain(target.as_ref().and_then(|t| t.image_file_id));
    files::lock_files(&mut tx, file_ids).await?;
    let proposal = lock_proposal(&mut tx, id, payload.revision, &snapshot).await?;
    let target = if let Some(target_id) = payload.existing_item_id {
        let locked = lock_item(&mut tx, target_id).await?;
        revision(
            locked.revision,
            payload
                .existing_item_revision
                .ok_or_else(|| AppError::BadRequest("ต้องระบุ revision".into()))?,
        )?;
        if locked.image_file_id != target.as_ref().and_then(|t| t.image_file_id) {
            return Err(AppError::Conflict("รูปวัสดุเปลี่ยนแล้ว".into()));
        }
        Some(locked)
    } else {
        None
    };
    if p.image != target.as_ref().and_then(|t| t.image_file_id) {
        if let Some(file) = p.image {
            files::attach(
                &mut tx,
                actor,
                file,
                FilePurpose::SupplyCatalogImage,
                Some(id),
            )
            .await?;
        }
    }
    let item_id = if let Some(item_id) = payload.existing_item_id {
        update_metadata(&mut tx, item_id, &p).await?;
        insert_version(&mut tx, actor, item_id, p.price, &p.shop).await?;
        item_id
    } else {
        insert_product(&mut tx, actor, &p).await?
    };
    school_database::DbSession::new(&mut *tx).query("administration.supply_catalog.services.approve_proposal", "UPDATE supply_proposals SET status='approved',reviewed_by=$2,reviewed_at=now(),item_id=$3,image_file_id=NULL,source_file_id=NULL,source_url=NULL,revision=revision+1 WHERE id=$1").bind(id).bind(actor.user_id).bind(item_id).execute().await?;
    let retire = proposal
        .source_file_id
        .into_iter()
        .chain(proposal.image_file_id.filter(|old| Some(*old) != p.image))
        .chain(
            target
                .and_then(|t| t.image_file_id)
                .filter(|old| Some(*old) != p.image),
        );
    let work = files::prepare_delete(pool, &mut tx, retire).await?;
    tx.commit().await?;
    files::finish_delete(platform, pool, work).await;
    read_item(pool, actor.user_id, item_id).await
}
pub async fn reject_proposal(
    pool: &PgPool,
    platform: &FilePlatform,
    actor: &ActorContext,
    id: Uuid,
    payload: SupplyProposalReject,
) -> Result<SupplyProposal, AppError> {
    require_action(actor, SupplyAction::Review, None, false)?;
    let reason = text(&payload.reason, 2000, true)?;
    let snapshot = load_proposal(pool, id).await?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "administration.supply_transaction")
            .await?;
    files::lock_files(
        &mut tx,
        snapshot
            .image_file_id
            .into_iter()
            .chain(snapshot.source_file_id),
    )
    .await?;
    let row = lock_proposal(&mut tx, id, payload.revision, &snapshot).await?;
    school_database::DbSession::new(&mut *tx).query("administration.supply_catalog.services.reject_proposal", "UPDATE supply_proposals SET status='rejected',reviewed_by=$2,reviewed_at=now(),rejection_reason=$3,image_file_id=NULL,source_file_id=NULL,source_url=NULL,revision=revision+1 WHERE id=$1").bind(id).bind(actor.user_id).bind(reason).execute().await?;
    let work = files::prepare_delete(
        pool,
        &mut tx,
        row.image_file_id.into_iter().chain(row.source_file_id),
    )
    .await?;
    tx.commit().await?;
    files::finish_delete(platform, pool, work).await;
    load_proposal(pool, id).await
}
/// Advisory approximate-name search is capped before matching and never authorizes duplicate creation.
pub async fn duplicates(
    pool: &PgPool,
    actor: &ActorContext,
    query: SupplyDuplicateQuery,
) -> Result<Vec<SupplyItem>, AppError> {
    require_action(actor, SupplyAction::Read, None, false)?;
    let name = normalize(&text(&query.name, 200, true)?);
    let unit = normalize(&text(&query.unit, 80, true)?);
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "administration.duplicates")
            .await?;
    let rows=school_database::DbSession::new(&mut *connection).query_as::<ItemRow>("administration.supply_catalog.services.duplicates", sqlx::AssertSqlSafe(format!("{ITEM_SELECT} WHERE i.normalized_unit=$2 AND abs(char_length(i.normalized_name)-$3)<=8 ORDER BY CASE WHEN i.normalized_name=$4 THEN 0 WHEN strpos(i.normalized_name,$4)>0 THEN 1 ELSE 2 END,i.normalized_name,i.id LIMIT 200"))).bind(actor.user_id).bind(unit).bind(name.chars().count() as i32).bind(&name).fetch_all().await?;
    let mut matches = rows
        .into_iter()
        .filter_map(|row| {
            let normalized = normalize(&row.name);
            let distance = edit_distance(&normalized, &name);
            let threshold = (name.chars().count() / 4).clamp(1, 8);
            if normalized.contains(&name) || name.contains(&normalized) || distance <= threshold {
                Some((distance, row.dto()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    matches.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.name.cmp(&b.1.name)));
    Ok(matches.into_iter().take(25).map(|(_, item)| item).collect())
}
fn edit_distance(a: &str, b: &str) -> usize {
    let b = b.chars().collect::<Vec<_>>();
    let mut prev = (0..=b.len()).collect::<Vec<_>>();
    for (i, a) in a.chars().enumerate() {
        let mut curr = vec![i + 1; b.len() + 1];
        for (j, b) in b.iter().enumerate() {
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + usize::from(a != *b));
        }
        prev = curr;
    }
    prev[b.len()]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuzzy_distance_uses_unicode_characters() {
        assert_eq!(edit_distance("กระดาษ", "กระดาศ"), 1);
        assert_eq!(edit_distance("paper a4", "paper a5"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }
}

/// Coherent name/unit/price snapshot for an application-bound consumer. Locks follow
/// catalog writers: item before its selected price version. Consumers retain values.
#[derive(Debug, Clone)]
pub struct SupplyPriceSnapshot {
    pub item_id: Uuid,
    pub version_id: Uuid,
    pub revision: i64,
    pub name: String,
    pub unit: String,
    pub price_satang: i64,
}
pub async fn price_snapshot_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    item_id: Uuid,
    version_id: Uuid,
) -> Result<SupplyPriceSnapshot, AppError> {
    require_action(actor, SupplyAction::Read, None, false)?;
    let (id, name, unit): (Uuid, String, String) = school_database::DbSession::new(&mut **tx)
        .query_as(
            "administration.supply_catalog.services.price_snapshot_in_transaction",
            "SELECT id,name,unit FROM supply_items WHERE id=$1 FOR SHARE",
        )
        .bind(item_id)
        .fetch_optional()
        .await?
        .ok_or_else(missing)?;
    let (version_id, revision, price_satang): (Uuid, i64, i64) = school_database::DbSession::new(&mut **tx).query_as("administration.supply_catalog.services.price_snapshot_in_transaction",
        "SELECT id,revision,price_satang FROM supply_item_versions WHERE id=$1 AND item_id=$2 FOR SHARE",
    )
    .bind(version_id)
    .bind(id)
    .fetch_optional()
    .await?
    .ok_or_else(missing)?;
    Ok(SupplyPriceSnapshot {
        item_id: id,
        version_id,
        revision,
        name,
        unit,
        price_satang,
    })
}
