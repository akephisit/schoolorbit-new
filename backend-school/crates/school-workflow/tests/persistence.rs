use school_workflow::models::{WorkflowWindowMetadata, WorkflowWindowStatus};
use school_workflow::workflow::{
    create_workflow_window, get_workflow_window, CreateWorkflowWindowInput, WorkflowWindowSchedule,
};

#[tokio::test]
async fn workflow_window_round_trips_through_the_crate_boundary() {
    let pool = school_test_db::create_named_test_pool("workflow_crate_persistence").await;
    school_test_db::run_test_migrations(&pool).await;

    let created = create_workflow_window(
        &pool,
        CreateWorkflowWindowInput {
            module_code: "academic".into(),
            workflow_code: "crate-boundary".into(),
            title: "Crate boundary".into(),
            description: None,
            organization_unit_id: None,
            managed_by_permission: "academic_lifecycle.manage.school".into(),
            schedule: WorkflowWindowSchedule {
                opens_at: None,
                due_at: None,
                closes_at: None,
            },
            metadata: WorkflowWindowMetadata::default(),
            created_by: None,
        },
    )
    .await
    .unwrap();

    let loaded = get_workflow_window(&pool, created.id).await.unwrap();
    assert_eq!(loaded.id, created.id);
    assert_eq!(loaded.status, WorkflowWindowStatus::Draft.as_str());
}
