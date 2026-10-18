use school_authorization::ActorContext;
use school_permissions::registry::codes;
use school_workflow::{correspondence, process, process_models::*, work};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(name: &str) -> (PgPool, ActorContext, ActorContext, ActorContext) {
    fixture_with_connections(name, 5).await
}
async fn fixture_with_connections(
    name: &str,
    connections: u32,
) -> (PgPool, ActorContext, ActorContext, ActorContext) {
    let pool = school_test_db::create_named_test_pool_with_max_connections(name, connections).await;
    school_test_db::run_test_migrations(&pool).await;
    let mut users = Vec::new();
    for name in ["Creator", "First", "Second"] {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic',$3,'Fixture','staff','active')").bind(id).bind(id.to_string()).bind(name).execute(&pool).await.unwrap();
        users.push(ActorContext {
            user_id: id,
            permissions: vec![
                codes::CORRESPONDENCE_CREATE_OWN.into(),
                codes::CORRESPONDENCE_READ_OWN.into(),
                codes::CORRESPONDENCE_READ_ASSIGNED.into(),
                codes::CORRESPONDENCE_UPDATE_ASSIGNED.into(),
                codes::CORRESPONDENCE_SIGN_OWN.into(),
                codes::CORRESPONDENCE_DOWNLOAD_ASSIGNED.into(),
            ],
        });
    }
    (pool, users.remove(0), users.remove(0), users.remove(0))
}
fn create_request() -> CreateCorrespondenceRequest {
    CreateCorrespondenceRequest {
        request_id: Uuid::now_v7(),
        register: CorrespondenceRegister::Incoming,
        title: "บันทึกทดสอบ".into(),
        sender: "Synthetic source".into(),
        document_number: Some("SOURCE-1".into()),
        document_date: None,
        urgency: CorrespondenceUrgency::Normal,
        source_links: vec![],
        organization_unit_id: None,
        original_file_id: None,
        attachment_file_ids: vec![],
        content: "เนื้อหาทดสอบ น้ำ ผู้อนุมัติ".into(),
        annotations: vec![],
        template_id: None,
    }
}
fn recipient(id: Uuid) -> ProcessRecipient {
    ProcessRecipient {
        kind: ProcessRecipientKind::User,
        user_id: Some(id),
        organization_unit_id: None,
        position_code: None,
    }
}
fn step(
    kind: ProcessStepKind,
    policy: ProcessCompletionPolicy,
    users: &[Uuid],
) -> ProcessTemplateStep {
    ProcessTemplateStep {
        title: "Synthetic step".into(),
        kind,
        completion_policy: policy,
        recipients: users.iter().copied().map(recipient).collect(),
        due_after_days: Some(2),
        requires_signature: false,
        requires_comment: false,
        requires_evidence: false,
        requires_delegated_authority: false,
    }
}
async fn template(
    pool: &PgPool,
    creator: &ActorContext,
    steps: Vec<ProcessTemplateStep>,
) -> ProcessTemplate {
    let admin = ActorContext {
        user_id: creator.user_id,
        permissions: vec![codes::PROCESS_MANAGE_SCHOOL.into()],
    };
    process::save_template(
        pool,
        &admin,
        None,
        SaveProcessTemplateRequest {
            request_id: Uuid::now_v7(),
            expected_revision: None,
            code: format!("fixture_{}", Uuid::now_v7().simple()),
            title: "Fixture".into(),
            steps,
            active: true,
        },
    )
    .await
    .unwrap()
}
async fn send(
    pool: &PgPool,
    creator: &ActorContext,
    record: &CorrespondenceDetail,
    template: Uuid,
) -> CorrespondenceDetail {
    correspondence::send(
        pool,
        creator,
        record.record.id,
        SendCorrespondenceRequest {
            request_id: Uuid::now_v7(),
            expected_revision: record.record.revision,
            template_id: template,
            step_recipients: vec![],
            due_at: None,
        },
    )
    .await
    .unwrap()
}
fn action(revision: i64, kind: ProcessAction) -> ProcessActionRequest {
    ProcessActionRequest {
        request_id: Uuid::now_v7(),
        expected_revision: revision,
        action: kind,
        annotations: vec![],
        comment: None,
        evidence_file_ids: vec![],
        signature: None,
        reassign_user_id: None,
        reassign_from_user_id: None,
    }
}

#[tokio::test]
async fn idempotent_registration_acl_and_immutable_revisions() {
    let (pool, creator, other, _) = fixture("correspondence_registration").await;
    let request = create_request();
    let record = correspondence::create(&pool, &creator, &MemoryValidator, request.clone())
        .await
        .unwrap();
    let replay = correspondence::create(&pool, &creator, &MemoryValidator, request.clone())
        .await
        .unwrap();
    assert_eq!(record.record.id, replay.record.id);
    assert_eq!(record.record.register_number, replay.record.register_number);
    assert!(correspondence::detail(&pool, &other, record.record.id)
        .await
        .is_err());
    let mut divergent = request.clone();
    divergent.title = "Different".into();
    assert!(
        correspondence::create(&pool, &creator, &MemoryValidator, divergent)
            .await
            .is_err()
    );
    let revised = correspondence::revise(
        &pool,
        &creator,
        record.record.id,
        &MemoryValidator,
        ReviseCorrespondenceRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1,
            content: "Revised".into(),
            original_file_id: None,
            attachment_file_ids: vec![],
            annotations: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(revised.revisions.len(), 2);
    assert_eq!(revised.revisions[0].content, request.content);
    assert_ne!(
        revised.revisions[0].content_hash,
        revised.revisions[1].content_hash
    );
    assert!(sqlx::query(
        "UPDATE correspondence_document_revisions SET document=document WHERE correspondence_id=$1"
    )
    .bind(record.record.id)
    .execute(&pool)
    .await
    .is_err());
    let second = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    assert_eq!(
        second.record.register_number,
        record.record.register_number + 1
    );
}
#[tokio::test]
async fn every_future_recipient_resolves_before_any_send_and_snapshot_survives_status_change() {
    let (pool, creator, a, b) = fixture("correspondence_recipient_snapshot").await;
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let bad = template(
        &pool,
        &creator,
        vec![
            step(
                ProcessStepKind::Acknowledge,
                ProcessCompletionPolicy::All,
                &[a.user_id],
            ),
            step(
                ProcessStepKind::Approve,
                ProcessCompletionPolicy::Any,
                &[Uuid::now_v7()],
            ),
        ],
    )
    .await;
    assert!(correspondence::send(
        &pool,
        &creator,
        record.record.id,
        SendCorrespondenceRequest {
            request_id: Uuid::now_v7(),
            expected_revision: 1,
            template_id: bad.id,
            step_recipients: vec![],
            due_at: None
        }
    )
    .await
    .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM process_instances")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let good = template(
        &pool,
        &creator,
        vec![
            step(
                ProcessStepKind::Acknowledge,
                ProcessCompletionPolicy::All,
                &[a.user_id, b.user_id],
            ),
            step(
                ProcessStepKind::Approve,
                ProcessCompletionPolicy::Any,
                &[a.user_id, b.user_id],
            ),
        ],
    )
    .await;
    let sent = send(&pool, &creator, &record, good.id).await;
    let original = sent.instance.clone().unwrap();
    assert!(original.steps[0].due_at.is_some());
    assert!(original.steps[1].due_at.is_none());
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(b.user_id)
        .execute(&pool)
        .await
        .unwrap();
    let read = correspondence::act(
        &pool,
        &a,
        record.record.id,
        &MemoryValidator,
        action(sent.record.revision, ProcessAction::Read),
    )
    .await
    .unwrap();
    assert!(read.instance.as_ref().unwrap().steps[0]
        .recipients
        .iter()
        .all(|r| r.completed_at.is_none()));
    let first = correspondence::act(
        &pool,
        &a,
        record.record.id,
        &MemoryValidator,
        action(read.record.revision, ProcessAction::Acknowledge),
    )
    .await
    .unwrap();
    assert_eq!(first.instance.as_ref().unwrap().current_step, 0);
    let second = correspondence::act(
        &pool,
        &b,
        record.record.id,
        &MemoryValidator,
        action(first.record.revision, ProcessAction::Acknowledge),
    )
    .await
    .unwrap();
    assert_eq!(second.instance.as_ref().unwrap().current_step, 1);
    assert!(second.instance.as_ref().unwrap().steps[1].due_at.is_some());
    let finished = correspondence::act(
        &pool,
        &a,
        record.record.id,
        &MemoryValidator,
        action(second.record.revision, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(finished.record.status, CorrespondenceStatus::Completed);
    let jobs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_dispatch_jobs WHERE source_kind='process'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(jobs >= 4);
    let revoked = ActorContext {
        user_id: b.user_id,
        permissions: vec![],
    };
    assert!(correspondence::detail(&pool, &revoked, record.record.id)
        .await
        .is_err());
    assert!(
        work::list_my_work_items(&pool, &revoked, work::WorkItemFilter::default())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        work::get_my_work_counts(&pool, &revoked)
            .await
            .unwrap()
            .total,
        0
    );
}
#[tokio::test]
async fn concurrency_replay_and_resend_preserve_source_identity_and_snapshots() {
    let (pool, creator, a, b) = fixture("correspondence_concurrency").await;
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Review,
            ProcessCompletionPolicy::All,
            &[a.user_id, b.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &record, t.id).await;
    let request_a = action(sent.record.revision, ProcessAction::Review);
    let request_b = action(sent.record.revision, ProcessAction::Review);
    let (left, right) = tokio::join!(
        correspondence::act(
            &pool,
            &a,
            record.record.id,
            &MemoryValidator,
            request_a.clone()
        ),
        correspondence::act(
            &pool,
            &b,
            record.record.id,
            &MemoryValidator,
            request_b.clone()
        )
    );
    assert_ne!(left.is_ok(), right.is_ok());
    let (winner, request) = if left.is_ok() {
        (&a, request_a)
    } else {
        (&b, request_b)
    };
    let replay = correspondence::act(
        &pool,
        winner,
        record.record.id,
        &MemoryValidator,
        request.clone(),
    )
    .await
    .unwrap();
    let mut changed = request;
    changed.comment = Some("different payload".into());
    assert!(
        correspondence::act(&pool, winner, record.record.id, &MemoryValidator, changed)
            .await
            .is_err()
    );
    let remaining = if winner.user_id == a.user_id { &b } else { &a };
    let mut return_request = action(replay.record.revision, ProcessAction::ReturnForRevision);
    return_request.comment = Some("Please revise".into());
    let returned = correspondence::act(
        &pool,
        remaining,
        record.record.id,
        &MemoryValidator,
        return_request,
    )
    .await
    .unwrap();
    assert_eq!(
        returned.record.status,
        CorrespondenceStatus::RevisionRequested
    );
    let revised = correspondence::revise(
        &pool,
        &creator,
        record.record.id,
        &MemoryValidator,
        ReviseCorrespondenceRequest {
            request_id: Uuid::now_v7(),
            expected_revision: returned.record.revision,
            content: "new content".into(),
            original_file_id: None,
            attachment_file_ids: vec![],
            annotations: vec![],
        },
    )
    .await
    .unwrap();
    let resent = send(&pool, &creator, &revised, t.id).await;
    assert_eq!(sent.instance.unwrap().id, resent.instance.unwrap().id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM process_instances")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM process_instance_revisions")
            .fetch_one(&pool)
            .await
            .unwrap()
            >= 4
    );
}
#[tokio::test]
async fn signatures_bind_document_and_action_layer_and_read_cannot_write() {
    let (pool, creator, a, _) = fixture("correspondence_signatures").await;
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Approve,
            ProcessCompletionPolicy::All,
            &[a.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &record, t.id).await;
    let annotation = DocumentAnnotation {
        id: Uuid::now_v7(),
        kind: DocumentAnnotationKind::Text,
        page: 1,
        x: 0.1,
        y: 0.1,
        width: 0.5,
        height: 0.1,
        text: Some("ดำเนินการตามเสนอ".into()),
        color: "#111111".into(),
        strokes: vec![],
    };
    let reader = ActorContext {
        user_id: a.user_id,
        permissions: vec![
            codes::CORRESPONDENCE_READ_ASSIGNED.into(),
            codes::CORRESPONDENCE_SIGN_OWN.into(),
        ],
    };
    let mut read = action(sent.record.revision, ProcessAction::Read);
    read.annotations.push(annotation.clone());
    assert!(
        correspondence::act(&pool, &reader, record.record.id, &MemoryValidator, read)
            .await
            .is_err()
    );
    let mut approve = action(sent.record.revision, ProcessAction::Approve);
    approve.annotations.push(annotation);
    approve.signature = Some(SignatureInput {
        method: SignatureMethod::Drawn,
        asset_id: None,
        strokes: vec![InkStroke {
            points: vec![InkPoint { x: 0.0, y: 0.0 }, InkPoint { x: 1.0, y: 1.0 }],
            width: 0.01,
            color: "#111111".into(),
        }],
        page: 1,
        x: 0.2,
        y: 0.4,
        width: 0.3,
        height: 0.1,
        document_revision: sent.document.revision,
        content_hash: "wrong".into(),
    });
    assert!(correspondence::act(
        &pool,
        &a,
        record.record.id,
        &MemoryValidator,
        approve.clone()
    )
    .await
    .is_err());
    approve.signature.as_mut().unwrap().content_hash = sent.document.content_hash.clone();
    let signed = correspondence::act(
        &pool,
        &a,
        record.record.id,
        &MemoryValidator,
        approve.clone(),
    )
    .await
    .unwrap();
    assert_eq!(signed.signatures.len(), 1);
    assert_eq!(
        signed.signatures[0].signature.content_hash,
        sent.document.content_hash
    );
    assert_ne!(
        signed.signatures[0].signed_content_hash,
        sent.document.content_hash
    );
    assert_eq!(signed.history.last().unwrap().annotations.len(), 1);
    assert_eq!(
        signed.history.last().unwrap().document_revision,
        sent.document.revision
    );
    let replay = correspondence::act(&pool, &a, record.record.id, &MemoryValidator, approve)
        .await
        .unwrap();
    assert_eq!(replay.signatures.len(), 1);
    assert!(
        sqlx::query("DELETE FROM correspondence_signing_evidence WHERE correspondence_id=$1")
            .bind(record.record.id)
            .execute(&pool)
            .await
            .is_err()
    );
}

struct SyntheticSource;
#[async_trait::async_trait]
impl process::ProcessSourcePort for SyntheticSource {
    async fn validate_existing(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _actor: &ActorContext,
        instance: &ProcessInstance,
    ) -> Result<(), school_errors::AppError> {
        if instance.source.module_code == "synthetic" {
            Ok(())
        } else {
            Err(school_errors::AppError::Forbidden("wrong source".into()))
        }
    }

    async fn validate_start(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _actor: &ActorContext,
        source: &ProcessSource,
    ) -> Result<(), school_errors::AppError> {
        if source.module_code != "synthetic" {
            return Err(school_errors::AppError::Forbidden(
                "synthetic source denied".into(),
            ));
        }
        Ok(())
    }
    async fn validate_action(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _actor: &ActorContext,
        instance: &ProcessInstance,
        _request: &ProcessActionRequest,
    ) -> Result<(), school_errors::AppError> {
        if instance.status != ProcessStatus::Active {
            return Err(school_errors::AppError::Forbidden(
                "synthetic source is no longer active".into(),
            ));
        }
        Ok(())
    }
    async fn validate_delegated_authority(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _actor: &ActorContext,
        _instance: &ProcessInstance,
    ) -> Result<(), school_errors::AppError> {
        Ok(())
    }
    async fn completed(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _instance: &ProcessInstance,
    ) -> Result<(), school_errors::AppError> {
        Ok(())
    }
}
#[tokio::test]
async fn reusable_provider_retains_source_validation_and_completion() {
    let (pool, creator, a, _) = fixture("process_provider_reuse").await;
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Execute,
            ProcessCompletionPolicy::All,
            &[a.user_id],
        )],
    )
    .await;
    let source = ProcessSource {
        module_code: "synthetic".into(),
        resource_type: "request".into(),
        resource_id: Uuid::now_v7(),
    };
    let mut tx = pool.begin().await.unwrap();
    let input = process::StartProcessInput {
        request_id: Uuid::now_v7(),
        source: source.clone(),
        template_id: t.id,
        title: "Synthetic request".into(),
        action_path: "/staff/synthetic/request".into(),
        step_recipients: vec![],
        due_at: None,
    };
    let mut instance =
        process::start_in_transaction(&mut tx, &creator, &SyntheticSource, input.clone())
            .await
            .unwrap();
    let replay = process::start_in_transaction(&mut tx, &creator, &SyntheticSource, input.clone())
        .await
        .unwrap();
    assert_eq!(instance.id, replay.id);
    assert_eq!(
        process::completion_in_transaction(&mut tx, &source)
            .await
            .unwrap(),
        Some(ProcessStatus::Active)
    );
    let request = action(instance.revision, ProcessAction::Execute);
    instance =
        process::act_source_in_transaction(&mut tx, &a, &SyntheticSource, instance.id, &request)
            .await
            .unwrap();
    let replay =
        process::act_source_in_transaction(&mut tx, &a, &SyntheticSource, instance.id, &request)
            .await
            .unwrap();
    assert_eq!(replay.revision, instance.revision);
    let history = process::history_in_transaction(&mut tx, &source)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].actor_id, a.user_id);
    assert_eq!(history[0].action.action, ProcessAction::Execute);
    let replayed_start = process::start_in_transaction(&mut tx, &creator, &SyntheticSource, input)
        .await
        .unwrap();
    assert_eq!(replayed_start.status, ProcessStatus::Completed);
    assert_eq!(instance.status, ProcessStatus::Completed);
    assert_eq!(
        process::completion_in_transaction(&mut tx, &source)
            .await
            .unwrap(),
        Some(ProcessStatus::Completed)
    );
    tx.commit().await.unwrap();
    let legacy_actor = ActorContext {
        user_id: a.user_id,
        permissions: vec![],
    };
    let legacy = work::list_my_work_items(&pool, &legacy_actor, work::WorkItemFilter::default())
        .await
        .unwrap();
    assert!(legacy
        .iter()
        .any(|i| i.module_code == "synthetic" && i.source_resource_id == Some(source.resource_id)));
}

async fn staged_file(pool: &PgPool, owner: Uuid, purpose: &str) -> (Uuid, Uuid) {
    let file = Uuid::now_v7();
    let version = Uuid::now_v7();
    sqlx::query("INSERT INTO files(id,owner_user_id,created_by,display_filename,purpose_code,visibility,lifecycle_status,retention_class,expires_at) VALUES($1,$2,$2,'synthetic.png',$3,'private','ready','temporary',now()+interval '1 hour')").bind(file).bind(owner).bind(purpose).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status) VALUES($1,$2,1,'r2','private','stored',$3,'image/png','png',100,repeat('a',64),'clean')").bind(version).bind(file).bind(format!("synthetic/{file}")).execute(pool).await.unwrap();
    sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
        .bind(file)
        .bind(version)
        .execute(pool)
        .await
        .unwrap();
    (file, version)
}
#[tokio::test]
async fn document_files_and_signature_assets_have_distinct_owner_and_evidence_acl() {
    use school_file_platform::{
        access::FilePolicyAction,
        platform_types::{FileLifecycleStatus, FilePurpose, FileVisibility},
        repository::PlatformFile,
    };
    use school_workflow::correspondence_files as files;
    let (pool, creator, a, b) = fixture("correspondence_file_acl").await;
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    assert!(files::authorize_create(
        &pool,
        &a,
        FilePurpose::CorrespondenceDocument,
        Some(record.record.id)
    )
    .await
    .is_err());
    assert!(files::authorize_create(
        &pool,
        &creator,
        FilePurpose::CorrespondenceDocument,
        Some(record.record.id)
    )
    .await
    .is_ok());
    let (file, version) = staged_file(&pool, creator.user_id, "correspondence_document").await;
    files::record_upload(&pool, creator.user_id, record.record.id, file)
        .await
        .unwrap();
    let revised = correspondence::revise(
        &pool,
        &creator,
        record.record.id,
        &MemoryValidator,
        ReviseCorrespondenceRequest {
            request_id: Uuid::now_v7(),
            expected_revision: record.record.revision,
            content: record.document.content.clone(),
            original_file_id: Some(file),
            attachment_file_ids: vec![],
            annotations: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(revised.document.file_snapshots[0].version_id, version);
    assert_eq!(revised.document.file_snapshots[0].checksum, "a".repeat(64));
    assert!(
        sqlx::query("UPDATE files SET lifecycle_status='delete_requested' WHERE id=$1")
            .bind(file)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE files SET current_version_id=NULL WHERE id=$1")
            .bind(file)
            .execute(&pool)
            .await
            .is_err()
    );
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Approve,
            ProcessCompletionPolicy::All,
            &[a.user_id, b.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &revised, t.id).await;
    assert!(files::authorize_create(
        &pool,
        &a,
        FilePurpose::CorrespondenceDocument,
        Some(record.record.id)
    )
    .await
    .is_ok());
    let (sig_file, _) = staged_file(&pool, a.user_id, "correspondence_signature").await;
    let asset = correspondence::create_signature(
        &pool,
        &a,
        CreateSignatureAssetRequest {
            request_id: Uuid::now_v7(),
            label: "Personal".into(),
            file_id: sig_file,
        },
    )
    .await
    .unwrap();
    assert!(correspondence::signatures(&pool, &b)
        .await
        .unwrap()
        .is_empty());
    assert!(files::authorize_create(
        &pool,
        &b,
        FilePurpose::CorrespondenceSignature,
        Some(a.user_id)
    )
    .await
    .is_err());
    let signature = SignatureInput {
        method: SignatureMethod::Image,
        asset_id: Some(asset.id),
        strokes: vec![],
        page: 1,
        x: 0.2,
        y: 0.4,
        width: 0.3,
        height: 0.1,
        document_revision: sent.document.revision,
        content_hash: sent.document.content_hash.clone(),
    };
    let mut stolen = action(sent.record.revision, ProcessAction::Approve);
    stolen.signature = Some(signature.clone());
    assert!(correspondence::act(
        &pool,
        &b,
        record.record.id,
        &MemoryValidator,
        stolen.clone()
    )
    .await
    .is_err());
    // Failed acceptance rolls back the receipt and all engine/history writes.
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM workflow_mutation_receipts WHERE request_id=$1"
        )
        .bind(stolen.request_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        correspondence::detail(&pool, &creator, record.record.id)
            .await
            .unwrap()
            .record
            .revision,
        sent.record.revision
    );
    let mut signed_request = action(sent.record.revision, ProcessAction::Approve);
    signed_request.signature = Some(signature);
    let signed = correspondence::act(
        &pool,
        &a,
        record.record.id,
        &MemoryValidator,
        signed_request,
    )
    .await
    .unwrap();
    assert_eq!(signed.signatures[0].file_id, Some(sig_file));
    let platform = PlatformFile {
        id: sig_file,
        owner_user_id: Some(a.user_id),
        purpose: FilePurpose::CorrespondenceSignature,
        visibility: FileVisibility::Private,
        lifecycle_status: FileLifecycleStatus::Ready,
        current_version: Some(1),
        display_filename: "synthetic.png".into(),
        detected_mime_type: "image/png".into(),
        byte_size: 100,
    };
    assert!(files::authorize_existing(
        &pool,
        &b,
        &platform,
        FilePolicyAction::Read,
        Some(record.record.id)
    )
    .await
    .is_ok());
    assert!(files::authorize_existing(
        &pool,
        &b,
        &platform,
        FilePolicyAction::Read,
        Some(a.user_id)
    )
    .await
    .is_err());
    assert!(files::authorize_existing(
        &pool,
        &b,
        &platform,
        FilePolicyAction::Read,
        Some(Uuid::now_v7())
    )
    .await
    .is_err());
    assert!(files::delete_guard(&pool, &a, &platform, Some(a.user_id))
        .await
        .is_err());
}

#[tokio::test]
async fn own_assigned_and_exact_manager_scopes_form_a_union_without_tree_inheritance() {
    let (pool, mut actor, creator, other) = fixture("correspondence_scope_union").await;
    let units:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active AND parent_unit_id IS NOT NULL ORDER BY id LIMIT 3").fetch_all(&pool).await.unwrap();
    assert_eq!(units.len(), 3);
    sqlx::query("INSERT INTO organization_members(id,user_id,organization_unit_id,position_code) VALUES($1,$2,$3,'member')").bind(Uuid::now_v7()).bind(actor.user_id).bind(units[0]).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code) SELECT $1,id,'member' FROM permissions WHERE code=$2 ON CONFLICT DO NOTHING").bind(units[0]).bind(codes::CORRESPONDENCE_READ_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    actor
        .permissions
        .push(codes::CORRESPONDENCE_READ_ORGANIZATION_UNIT.into());
    let mut own_request = create_request();
    own_request.organization_unit_id = Some(units[1]);
    let own = correspondence::create(&pool, &actor, &MemoryValidator, own_request)
        .await
        .unwrap();
    let mut managed_request = create_request();
    managed_request.organization_unit_id = Some(units[0]);
    let managed = correspondence::create(&pool, &creator, &MemoryValidator, managed_request)
        .await
        .unwrap();
    let mut foreign_request = create_request();
    foreign_request.organization_unit_id = Some(units[2]);
    let foreign = correspondence::create(&pool, &other, &MemoryValidator, foreign_request)
        .await
        .unwrap();
    let mut assigned_request = create_request();
    assigned_request.organization_unit_id = Some(units[2]);
    let assigned = correspondence::create(&pool, &creator, &MemoryValidator, assigned_request)
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Acknowledge,
            ProcessCompletionPolicy::All,
            &[actor.user_id],
        )],
    )
    .await;
    send(&pool, &creator, &assigned, t.id).await;
    let result = correspondence::list(
        &pool,
        &actor,
        correspondence::CorrespondenceListQuery {
            cursor: None,
            limit: None,
            register: None,
            status: None,
            search: None,
        },
    )
    .await
    .unwrap();
    let ids: Vec<_> = result.items.iter().map(|r| r.id).collect();
    for expected in [own.record.id, managed.record.id, assigned.record.id] {
        assert!(ids.contains(&expected));
        assert!(correspondence::require_read(&pool, &actor, expected)
            .await
            .is_ok());
    }
    assert!(!ids.contains(&foreign.record.id));
    assert!(
        correspondence::require_read(&pool, &actor, foreign.record.id)
            .await
            .is_err()
    );
    // Guaranteed parent/child fixture: an exact grant never traverses to its child.
    let(parent,child):(Uuid,Uuid)=sqlx::query_as("SELECT p.id,c.id FROM organization_units p JOIN organization_units c ON c.parent_unit_id=p.id WHERE p.is_active AND c.is_active AND c.id<>$1 ORDER BY p.id,c.id LIMIT 1").bind(units[0]).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_members(id,user_id,organization_unit_id,position_code) VALUES($1,$2,$3,'member') ON CONFLICT DO NOTHING").bind(Uuid::now_v7()).bind(actor.user_id).bind(parent).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code) SELECT $1,id,'member' FROM permissions WHERE code=$2 ON CONFLICT DO NOTHING").bind(parent).bind(codes::CORRESPONDENCE_READ_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    let mut r = create_request();
    r.organization_unit_id = Some(child);
    let child_record = correspondence::create(&pool, &creator, &MemoryValidator, r)
        .await
        .unwrap();
    assert!(
        correspondence::require_read(&pool, &actor, child_record.record.id)
            .await
            .is_err()
    );
}

struct MemoryValidator;
fn memory_png() -> Vec<u8> {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        2,
        image::Rgba([250, 250, 250, 255]),
    ));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}
#[async_trait::async_trait]
impl school_workflow::document_export::DocumentValidationPort for MemoryValidator {
    async fn validate(
        &self,
        document: &CorrespondenceDocumentRevision,
        annotations: &[DocumentAnnotation],
        signature: Option<&SignatureInput>,
        label: &str,
    ) -> Result<(), school_errors::AppError> {
        let original = document.original_file_id.map(|_| memory_png());
        school_workflow::document_export::validate_document_bytes(
            document,
            original.as_deref(),
            annotations,
            signature,
            label,
        )
    }
}

#[tokio::test]
async fn invalid_source_pages_and_glyphs_fail_before_acceptance() {
    let (pool, creator, a, _) = fixture("document_preflight_acceptance").await;
    let mut invalid = create_request();
    invalid.content = "Unsupported glyph 🚀".into();
    let rejected_id = invalid.request_id;
    assert!(
        correspondence::create(&pool, &creator, &MemoryValidator, invalid)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM correspondence_records")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM workflow_mutation_receipts WHERE request_id=$1"
        )
        .bind(rejected_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Approve,
            ProcessCompletionPolicy::All,
            &[a.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &record, t.id).await;
    let mut request = action(sent.record.revision, ProcessAction::Approve);
    request.annotations.push(DocumentAnnotation {
        id: Uuid::now_v7(),
        kind: DocumentAnnotationKind::Text,
        page: 99,
        x: 0.1,
        y: 0.1,
        width: 0.3,
        height: 0.1,
        text: Some("invalid page".into()),
        color: "#111111".into(),
        strokes: vec![],
    });
    let id = request.request_id;
    assert!(
        correspondence::act(&pool, &a, record.record.id, &MemoryValidator, request)
            .await
            .is_err()
    );
    assert_eq!(
        correspondence::detail(&pool, &creator, record.record.id)
            .await
            .unwrap()
            .record
            .revision,
        sent.record.revision
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM workflow_mutation_receipts WHERE request_id=$1"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let mut signature = action(sent.record.revision, ProcessAction::Approve);
    signature.signature = Some(SignatureInput {
        method: SignatureMethod::Drawn,
        asset_id: None,
        strokes: vec![InkStroke {
            points: vec![InkPoint { x: 0.0, y: 0.0 }, InkPoint { x: 1.0, y: 1.0 }],
            width: 0.01,
            color: "#111111".into(),
        }],
        page: 99,
        x: 0.2,
        y: 0.2,
        width: 0.3,
        height: 0.1,
        document_revision: sent.document.revision,
        content_hash: sent.document.content_hash,
    });
    assert!(
        correspondence::act(&pool, &a, record.record.id, &MemoryValidator, signature)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn exact_manager_can_recover_an_inactive_recipient_without_acting_as_them_acceptance() {
    let (pool, creator, a, b) = fixture("manager_recovery_acceptance").await;
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Approve,
            ProcessCompletionPolicy::All,
            &[b.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &record, t.id).await;
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(b.user_id)
        .execute(&pool)
        .await
        .unwrap();
    let manager = ActorContext {
        user_id: a.user_id,
        permissions: vec![codes::CORRESPONDENCE_MANAGE_SCHOOL.into()],
    };
    let detail = correspondence::detail(&pool, &manager, record.record.id)
        .await
        .unwrap();
    assert_eq!(detail.available_actions, vec![ProcessAction::Reassign]);
    assert!(correspondence::act(
        &pool,
        &manager,
        record.record.id,
        &MemoryValidator,
        action(sent.record.revision, ProcessAction::Approve)
    )
    .await
    .is_err());
    let mut request = action(sent.record.revision, ProcessAction::Reassign);
    request.comment = Some("Recover inactive staff task".into());
    request.reassign_from_user_id = Some(b.user_id);
    request.reassign_user_id = Some(creator.user_id);
    let result = correspondence::act(&pool, &manager, record.record.id, &MemoryValidator, request)
        .await
        .unwrap();
    let history = result.history.last().unwrap();
    assert_eq!(history.actor_id, manager.user_id);
    assert_eq!(history.reassigned_from_user_id, Some(b.user_id));
    assert_eq!(history.reassigned_to_user_id, Some(creator.user_id));
    assert_eq!(result.record.status, CorrespondenceStatus::InProgress);
    assert!(result.instance.as_ref().unwrap().steps[0]
        .recipients
        .iter()
        .any(|r| r.user_id == creator.user_id
            && r.reassigned_from == Some(b.user_id)
            && r.completed_at.is_none()));
    let item = result.instance.as_ref().unwrap().steps[0]
        .work_item_id
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM work_item_assignees WHERE work_item_id=$1 AND user_id=$2"
        )
        .bind(item)
        .bind(b.user_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "dismissed"
    );
    let template_manager = ActorContext {
        user_id: a.user_id,
        permissions: vec![codes::PROCESS_MANAGE_SCHOOL.into()],
    };
    assert!(
        correspondence::detail(&pool, &template_manager, record.record.id)
            .await
            .is_err()
    );
}

struct RacingValidator<'a> {
    pool: &'a PgPool,
    file: Uuid,
    next: Uuid,
}
#[async_trait::async_trait]
impl school_workflow::document_export::DocumentValidationPort for RacingValidator<'_> {
    async fn validate(
        &self,
        document: &CorrespondenceDocumentRevision,
        annotations: &[DocumentAnnotation],
        signature: Option<&SignatureInput>,
        label: &str,
    ) -> Result<(), school_errors::AppError> {
        MemoryValidator
            .validate(document, annotations, signature, label)
            .await?;
        sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
            .bind(self.file)
            .bind(self.next)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
#[tokio::test]
async fn source_version_changed_after_render_is_rejected_at_acceptance() {
    let (pool, creator, _, _) = fixture("source_version_acceptance").await;
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let (file, _) = staged_file(&pool, creator.user_id, "correspondence_document").await;
    school_workflow::correspondence_files::record_upload(
        &pool,
        creator.user_id,
        record.record.id,
        file,
    )
    .await
    .unwrap();
    let next = Uuid::now_v7();
    sqlx::query("INSERT INTO file_versions(id,file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status) VALUES($1,$2,2,'r2','private','stored',$3,'image/png','png',100,repeat('a',64),'clean')").bind(next).bind(file).bind(format!("synthetic/{file}/2")).execute(&pool).await.unwrap();
    let request = ReviseCorrespondenceRequest {
        request_id: Uuid::now_v7(),
        expected_revision: record.record.revision,
        content: record.document.content.clone(),
        original_file_id: Some(file),
        attachment_file_ids: vec![],
        annotations: vec![],
    };
    let request_id = request.request_id;
    assert!(correspondence::revise(
        &pool,
        &creator,
        record.record.id,
        &RacingValidator {
            pool: &pool,
            file,
            next
        },
        request
    )
    .await
    .is_err());
    assert_eq!(
        correspondence::detail(&pool, &creator, record.record.id)
            .await
            .unwrap()
            .document
            .revision,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT retention_class FROM files WHERE id=$1")
            .bind(file)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "temporary"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM workflow_mutation_receipts WHERE request_id=$1"
        )
        .bind(request_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn task_notification_eligibility_follows_canonical_assignment_cancellation() {
    let (pool, creator, a, b) = fixture("process_notification_validity").await;
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Approve,
            ProcessCompletionPolicy::All,
            &[a.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &record, t.id).await;
    let instance = sent.instance.as_ref().unwrap().id;
    let source:Uuid=sqlx::query_scalar("SELECT id FROM process_notification_sources WHERE instance_id=$1 AND recipient_id=$2 AND work_item_id IS NOT NULL").bind(instance).bind(a.user_id).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
        .bind(a.user_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    assert!(
        process::authorize_notification_on_connection(&mut connection, source, a.user_id)
            .await
            .unwrap(),
        "recipient snapshot survives account status changes"
    );
    drop(connection);
    let mut delivery_guard = pool.begin().await.unwrap();
    assert!(
        process::authorize_notification_on_connection(&mut delivery_guard, source, a.user_id)
            .await
            .unwrap()
    );
    let blocked = sqlx::query("SELECT id FROM process_instances WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(instance)
        .fetch_one(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        blocked
            .as_database_error()
            .and_then(|e| e.code())
            .as_deref(),
        Some("55P03")
    );
    delivery_guard.rollback().await.unwrap();

    let manager = ActorContext {
        user_id: creator.user_id,
        permissions: vec![codes::CORRESPONDENCE_MANAGE_SCHOOL.into()],
    };
    let mut request = action(sent.record.revision, ProcessAction::Reassign);
    request.reassign_from_user_id = Some(a.user_id);
    request.reassign_user_id = Some(b.user_id);
    request.comment = Some("recover inactive assignment".into());
    let reassigned =
        correspondence::act(&pool, &manager, record.record.id, &MemoryValidator, request)
            .await
            .unwrap();
    let replacement:Uuid=sqlx::query_scalar("SELECT id FROM process_notification_sources WHERE instance_id=$1 AND recipient_id=$2 AND work_item_id IS NOT NULL ORDER BY id DESC LIMIT 1").bind(instance).bind(b.user_id).fetch_one(&pool).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    assert!(
        !process::authorize_notification_on_connection(&mut connection, source, a.user_id)
            .await
            .unwrap()
    );
    assert!(
        process::authorize_notification_on_connection(&mut connection, replacement, b.user_id)
            .await
            .unwrap()
    );
    drop(connection);
    let finished = correspondence::act(
        &pool,
        &b,
        record.record.id,
        &MemoryValidator,
        action(reassigned.record.revision, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(finished.record.status, CorrespondenceStatus::Completed);
    let outcome:Uuid=sqlx::query_scalar("SELECT id FROM process_notification_sources WHERE instance_id=$1 AND recipient_id=$2 AND work_item_id IS NULL ORDER BY id DESC LIMIT 1").bind(instance).bind(creator.user_id).fetch_one(&pool).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    assert!(!process::authorize_notification_on_connection(
        &mut connection,
        replacement,
        b.user_id
    )
    .await
    .unwrap());
    assert!(process::authorize_notification_on_connection(
        &mut connection,
        outcome,
        creator.user_id
    )
    .await
    .unwrap());
}

#[tokio::test]
async fn manager_options_allow_recovery_without_granting_record_mutation() {
    let (pool, creator, a, b) = fixture("manager_recovery_options").await;
    for permission in [
        codes::CORRESPONDENCE_MANAGE_SCHOOL,
        codes::CORRESPONDENCE_MANAGE_ORGANIZATION_UNIT,
    ] {
        let manager = ActorContext {
            user_id: a.user_id,
            permissions: vec![permission.into()],
        };
        let options = correspondence::options(
            &pool,
            &manager,
            correspondence::CorrespondenceOptionsQuery { search: None },
        )
        .await
        .unwrap();
        assert!(options.users.iter().any(|u| u.id == creator.user_id));
        assert!(!options.organizations.is_empty());
    }
    for permissions in [vec![], vec![codes::CORRESPONDENCE_READ_ASSIGNED.into()]] {
        let reader = ActorContext {
            user_id: a.user_id,
            permissions,
        };
        assert!(correspondence::options(
            &pool,
            &reader,
            correspondence::CorrespondenceOptionsQuery { search: None },
        )
        .await
        .is_err());
    }
    let record = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Approve,
            ProcessCompletionPolicy::All,
            &[b.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &record, t.id).await;
    let unscoped_manager = ActorContext {
        user_id: a.user_id,
        permissions: vec![codes::CORRESPONDENCE_MANAGE_ORGANIZATION_UNIT.into()],
    };
    let mut request = action(sent.record.revision, ProcessAction::Reassign);
    request.comment = Some("Attempt outside exact managed unit".into());
    request.reassign_from_user_id = Some(b.user_id);
    request.reassign_user_id = Some(creator.user_id);
    assert!(correspondence::act(
        &pool,
        &unscoped_manager,
        record.record.id,
        &MemoryValidator,
        request
    )
    .await
    .is_err());
}

#[tokio::test]
async fn exact_unit_manager_action_uses_one_connection() {
    let (pool, creator, a, b) = fixture_with_connections("unit_manager_one_connection", 1).await;
    let unit: Uuid =
        sqlx::query_scalar("SELECT id FROM organization_units WHERE is_active ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO organization_members(id,user_id,organization_unit_id,position_code) VALUES($1,$2,$3,'member')").bind(Uuid::now_v7()).bind(a.user_id).bind(unit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code) SELECT $1,id,'member' FROM permissions WHERE code=$2 ON CONFLICT DO NOTHING").bind(unit).bind(codes::CORRESPONDENCE_MANAGE_ORGANIZATION_UNIT).execute(&pool).await.unwrap();
    let mut input = create_request();
    input.organization_unit_id = Some(unit);
    let record = correspondence::create(&pool, &creator, &MemoryValidator, input)
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Approve,
            ProcessCompletionPolicy::All,
            &[b.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &record, t.id).await;
    let manager = ActorContext {
        user_id: a.user_id,
        permissions: vec![codes::CORRESPONDENCE_MANAGE_ORGANIZATION_UNIT.into()],
    };
    let mut request = action(sent.record.revision, ProcessAction::Reassign);
    request.comment = Some("Recover exact unit assignment".into());
    request.reassign_from_user_id = Some(b.user_id);
    request.reassign_user_id = Some(creator.user_id);
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        correspondence::act(&pool, &manager, record.record.id, &MemoryValidator, request),
    )
    .await
    .expect("locked action must not acquire another pool connection")
    .unwrap();
    assert_eq!(result.history.last().unwrap().actor_id, a.user_id);
    assert_eq!(
        result.history.last().unwrap().reassigned_to_user_id,
        Some(creator.user_id)
    );
    assert_eq!(result.available_actions, vec![ProcessAction::Reassign]);
}

#[tokio::test]
async fn accepted_annotation_ids_are_unique_across_base_and_history_but_replay_succeeds() {
    let (pool, creator, a, _) = fixture("annotation_committed_identity").await;
    let base = DocumentAnnotation {
        id: Uuid::now_v7(),
        kind: DocumentAnnotationKind::Text,
        page: 1,
        x: 0.1,
        y: 0.1,
        width: 0.6,
        height: 0.1,
        text: Some("Base instruction".into()),
        color: "#111111".into(),
        strokes: vec![],
    };
    let mut input = create_request();
    input.annotations.push(base.clone());
    let record = correspondence::create(&pool, &creator, &MemoryValidator, input)
        .await
        .unwrap();
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Approve,
            ProcessCompletionPolicy::All,
            &[a.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &record, t.id).await;
    let mut first = action(sent.record.revision, ProcessAction::Report);
    first.comment = Some("Report with instruction".into());
    first.annotations.push(DocumentAnnotation {
        text: Some("Attempt to reuse base identity".into()),
        ..base.clone()
    });
    assert!(
        correspondence::act(&pool, &a, record.record.id, &MemoryValidator, first.clone())
            .await
            .is_err()
    );
    first.annotations[0].id = Uuid::now_v7();
    first.annotations[0].text = Some("Accepted instruction".into());
    let accepted =
        correspondence::act(&pool, &a, record.record.id, &MemoryValidator, first.clone())
            .await
            .unwrap();
    let replay = correspondence::act(&pool, &a, record.record.id, &MemoryValidator, first.clone())
        .await
        .unwrap();
    assert_eq!(replay.record.revision, accepted.record.revision);
    assert_eq!(replay.history.len(), accepted.history.len());
    let mut different = action(accepted.record.revision, ProcessAction::Report);
    different.comment = Some("Different report".into());
    different.annotations = first.annotations.clone();
    different.annotations[0].text = Some("Must not replace accepted instruction".into());
    let rejected_request_id = different.request_id;
    assert!(correspondence::act(
        &pool,
        &a,
        record.record.id,
        &MemoryValidator,
        different.clone()
    )
    .await
    .is_err());
    let current = correspondence::detail(&pool, &a, record.record.id)
        .await
        .unwrap();
    assert_eq!(current.record.revision, accepted.record.revision);
    assert_eq!(current.history.len(), accepted.history.len());
    assert_eq!(
        current.history.last().unwrap().annotations[0].text,
        first.annotations[0].text
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM workflow_mutation_receipts WHERE request_id=$1"
        )
        .bind(rejected_request_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    different.annotations[0].id = Uuid::now_v7();
    let next = correspondence::act(&pool, &a, record.record.id, &MemoryValidator, different)
        .await
        .unwrap();
    assert_eq!(next.history.len(), accepted.history.len() + 1);
    assert_eq!(next.document.annotations[0].id, base.id);
}
