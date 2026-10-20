//! Application composition translates the Supply Catalog provider into Budget's narrow port.
use school_authorization::ActorContext;
use school_budget::ports::{BudgetCatalogPort, BudgetCatalogPriceSnapshot};
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
pub struct ApplicationBudgetCatalog;
pub static BUDGET_CATALOG: ApplicationBudgetCatalog = ApplicationBudgetCatalog;
#[async_trait::async_trait]
impl BudgetCatalogPort for ApplicationBudgetCatalog {
    async fn snapshot(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        actor: &ActorContext,
        item: Uuid,
        version: Uuid,
    ) -> Result<BudgetCatalogPriceSnapshot, AppError> {
        let s = school_administration::supply_catalog::services::price_snapshot_in_transaction(
            tx, actor, item, version,
        )
        .await?;
        Ok(BudgetCatalogPriceSnapshot {
            item_id: s.item_id,
            version_id: s.version_id,
            revision: s.revision,
            name: s.name,
            unit: s.unit,
            price_satang: s.price_satang,
        })
    }
}
