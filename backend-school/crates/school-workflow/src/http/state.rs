use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct WorkflowHttpState {
    pub auth_runtime: AuthRuntime,
    pub work_event_channel:
        tokio::sync::broadcast::Sender<school_notifications::events::WorkChangeEvent>,
}

impl WorkflowHttpState {
    pub fn notify_work_items_changed(&self, tenant: &str) {
        let _ = self
            .work_event_channel
            .send(school_notifications::events::WorkChangeEvent::work_items_changed(tenant));
    }
    pub fn notify_workflow_window_changed(&self, tenant: &str) {
        let _ = self
            .work_event_channel
            .send(school_notifications::events::WorkChangeEvent::workflow_window_changed(tenant));
    }
}
