//! The application binds the Supply Catalog provider; Budget never queries its tables.
use school_authorization::ActorContext;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
#[derive(Debug, Clone)]
pub struct BudgetCatalogPriceSnapshot {
    pub item_id: Uuid,
    pub version_id: Uuid,
    pub revision: i64,
    pub name: String,
    pub unit: String,
    pub price_satang: i64,
}
#[async_trait::async_trait]
pub trait BudgetCatalogPort: Send + Sync {
    async fn snapshot(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        item: Uuid,
        version: Uuid,
    ) -> Result<BudgetCatalogPriceSnapshot, AppError>;
}
