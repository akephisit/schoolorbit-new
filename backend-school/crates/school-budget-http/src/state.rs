use school_auth::runtime::AuthRuntime;
use school_budget::ports::BudgetCatalogPort;
use school_notifications::events::WorkChangeEvent;

#[derive(Clone)]
pub struct BudgetHttpState {
    pub auth_runtime: AuthRuntime,
    pub catalog: &'static dyn BudgetCatalogPort,
    pub work_event_channel: tokio::sync::broadcast::Sender<WorkChangeEvent>,
}
impl BudgetHttpState {
    pub(crate) fn after_commit(&self, tenant: &str) {
        school_notifications::runtime::wake_after_commit(tenant);
        if let Err(error) = self
            .work_event_channel
            .send(WorkChangeEvent::work_items_changed(tenant))
        {
            tracing::debug!(%error,"budget_work_signal_without_subscribers");
        }
    }
}
