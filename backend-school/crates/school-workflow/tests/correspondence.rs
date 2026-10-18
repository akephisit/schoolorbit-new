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
            metadata: None,
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
            metadata: None,
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
            metadata: None,
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
            view: None,
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
    assert_eq!(result.counts.all, 3);
    assert_eq!(result.counts.drafts, 1);
    assert_eq!(result.counts.action_required, 1);
    assert_eq!(result.progress.len(), 3);
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
        metadata: None,
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

fn list_query(view: CorrespondenceListView) -> correspondence::CorrespondenceListQuery {
    correspondence::CorrespondenceListQuery {
        view: Some(view),
        cursor: None,
        limit: None,
        register: None,
        status: None,
        search: None,
    }
}

async fn inbox(pool: &PgPool, actor: &ActorContext) -> CorrespondenceList {
    correspondence::list(
        pool,
        actor,
        list_query(CorrespondenceListView::ActionRequired),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn list_counts_cover_authorized_results_before_view_cursor_and_page() {
    let (pool, creator, other, foreign) = fixture("correspondence_list_counts").await;
    let mut drafts = Vec::new();
    for index in 0..35 {
        let mut request = create_request();
        request.title = "queue_fixture draft".into();
        if index == 0 {
            request.register = CorrespondenceRegister::Outgoing;
        }
        drafts.push(
            correspondence::create(&pool, &creator, &MemoryValidator, request)
                .await
                .unwrap(),
        );
    }
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Acknowledge,
            ProcessCompletionPolicy::All,
            &[other.user_id],
        )],
    )
    .await;
    let mut request = create_request();
    request.title = "queue_fixture sent".into();
    let own = correspondence::create(&pool, &creator, &MemoryValidator, request.clone())
        .await
        .unwrap();
    send(&pool, &creator, &own, t.id).await;
    let assigned_template = template(
        &pool,
        &other,
        vec![step(
            ProcessStepKind::Review,
            ProcessCompletionPolicy::All,
            &[creator.user_id],
        )],
    )
    .await;
    let assigned = correspondence::create(&pool, &other, &MemoryValidator, request.clone())
        .await
        .unwrap();
    send(&pool, &other, &assigned, assigned_template.id).await;
    let hidden = correspondence::create(&pool, &foreign, &MemoryValidator, request)
        .await
        .unwrap();
    let mut query = list_query(CorrespondenceListView::All);
    query.search = Some("queue_fixture".into());
    let page = correspondence::list(&pool, &creator, query).await.unwrap();
    assert_eq!(page.items.len(), 30);
    assert_eq!(page.progress.len(), 30);
    assert_eq!(
        (
            page.counts.all,
            page.counts.drafts,
            page.counts.sent,
            page.counts.action_required
        ),
        (37, 35, 1, 1)
    );
    assert!(page.items.iter().all(|item| item.id != hidden.record.id));
    assert!(page
        .items
        .iter()
        .zip(&page.progress)
        .all(|(item, progress)| item.id == progress.correspondence_id));
    let mut next = list_query(CorrespondenceListView::All);
    next.cursor = page.next_cursor;
    let second = correspondence::list(&pool, &creator, next).await.unwrap();
    assert_eq!(second.items.len(), 7);
    assert!(second.next_cursor.is_none());
    assert_eq!(second.counts.all, 37);
    assert!(second
        .items
        .iter()
        .all(|item| !page.items.iter().any(|first| first.id == item.id)));
    let mut drafts_query = list_query(CorrespondenceListView::Drafts);
    drafts_query.limit = Some(10);
    let draft_page = correspondence::list(&pool, &creator, drafts_query)
        .await
        .unwrap();
    assert_eq!(draft_page.items.len(), 10);
    assert_eq!(draft_page.counts.drafts, 35);
    assert_eq!(draft_page.counts.all, 37);
    assert!(draft_page
        .progress
        .iter()
        .all(|p| p.can_revise && p.step_count == 0 && p.my_action.is_none()));
    let sent = correspondence::list(&pool, &creator, list_query(CorrespondenceListView::Sent))
        .await
        .unwrap();
    assert_eq!(
        sent.items.iter().map(|i| i.id).collect::<Vec<_>>(),
        vec![own.record.id]
    );
    assert_eq!(sent.counts.all, 37);
    assert_eq!(inbox(&pool, &creator).await.items[0].id, assigned.record.id);
    let mut filtered = list_query(CorrespondenceListView::All);
    filtered.register = Some(CorrespondenceRegister::Outgoing);
    filtered.status = Some(CorrespondenceStatus::Draft);
    let filtered = correspondence::list(&pool, &creator, filtered)
        .await
        .unwrap();
    assert_eq!(
        (
            filtered.counts.all,
            filtered.counts.drafts,
            filtered.counts.sent
        ),
        (1, 1, 0)
    );
    assert_eq!(filtered.items[0].id, drafts[0].record.id);
    let mut status_only = list_query(CorrespondenceListView::All);
    status_only.status = Some(CorrespondenceStatus::Draft);
    let status_only = correspondence::list(&pool, &creator, status_only)
        .await
        .unwrap();
    assert_eq!(
        (
            status_only.counts.all,
            status_only.counts.drafts,
            status_only.counts.sent,
            status_only.counts.action_required
        ),
        (35, 35, 0, 0)
    );
    assert_eq!(status_only.items.len(), 30);
    let selected = &drafts[18].record;
    for search in [
        selected.register_number.to_string(),
        format!("{}/{}", selected.register_number, selected.register_year),
    ] {
        let mut query = list_query(CorrespondenceListView::All);
        query.search = Some(search);
        let found = correspondence::list(&pool, &creator, query).await.unwrap();
        assert!(found.items.iter().any(|r| r.id == selected.id));
        assert_eq!(found.counts.all, 1);
    }
    let mut year = list_query(CorrespondenceListView::All);
    year.search = Some(selected.register_year.to_string());
    assert_eq!(
        correspondence::list(&pool, &creator, year)
            .await
            .unwrap()
            .counts
            .all,
        37
    );
    let mut empty = list_query(CorrespondenceListView::All);
    empty.search = Some("no such correspondence fixture".into());
    let empty = correspondence::list(&pool, &creator, empty).await.unwrap();
    assert!(empty.items.is_empty() && empty.progress.is_empty() && empty.next_cursor.is_none());
    assert_eq!(empty.counts.all, 0);
    let denied = ActorContext {
        user_id: creator.user_id,
        permissions: vec![],
    };
    let denied = correspondence::list(&pool, &denied, list_query(CorrespondenceListView::All))
        .await
        .unwrap();
    assert_eq!(denied.counts.all, 0);
    assert!(denied.progress.is_empty());
}

#[tokio::test]
async fn list_actions_follow_current_frozen_recipients_permissions_and_returned_author() {
    let (pool, creator, a, b) = fixture("correspondence_list_actions").await;
    let mut first = step(
        ProcessStepKind::Approve,
        ProcessCompletionPolicy::All,
        &[a.user_id, b.user_id],
    );
    first.due_after_days = None;
    let t = template(
        &pool,
        &creator,
        vec![
            first,
            step(
                ProcessStepKind::Review,
                ProcessCompletionPolicy::All,
                &[a.user_id],
            ),
        ],
    )
    .await;
    let draft = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let due = chrono::Utc::now() + chrono::Duration::days(14);
    let sent = correspondence::send(
        &pool,
        &creator,
        draft.record.id,
        SendCorrespondenceRequest {
            request_id: Uuid::now_v7(),
            expected_revision: draft.record.revision,
            template_id: t.id,
            step_recipients: vec![],
            due_at: Some(due),
        },
    )
    .await
    .unwrap();
    let initial = inbox(&pool, &a).await;
    assert_eq!(initial.counts.action_required, 1);
    let progress = &initial.progress[0];
    assert_eq!(
        (
            progress.current_step_index,
            progress.step_count,
            progress.my_action
        ),
        (Some(0), 2, Some(ProcessAction::Approve))
    );
    assert_eq!(progress.due_at, Some(due));
    let frozen_name = progress
        .pending_recipients
        .iter()
        .find(|r| r.id == a.user_id)
        .unwrap()
        .display_name
        .clone();
    sqlx::query("UPDATE users SET first_name='Changed after send' WHERE id=$1")
        .bind(a.user_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        inbox(&pool, &a).await.progress[0]
            .pending_recipients
            .iter()
            .find(|r| r.id == a.user_id)
            .unwrap()
            .display_name,
        frozen_name
    );
    let read_only = ActorContext {
        user_id: a.user_id,
        permissions: vec![codes::CORRESPONDENCE_READ_ASSIGNED.into()],
    };
    assert_eq!(inbox(&pool, &read_only).await.counts.action_required, 0);
    let manager_only = ActorContext {
        user_id: creator.user_id,
        permissions: vec![codes::CORRESPONDENCE_MANAGE_SCHOOL.into()],
    };
    assert_eq!(inbox(&pool, &manager_only).await.counts.action_required, 0);
    assert_eq!(inbox(&pool, &creator).await.counts.action_required, 0);
    let one = correspondence::act(
        &pool,
        &a,
        draft.record.id,
        &MemoryValidator,
        action(sent.record.revision, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(inbox(&pool, &a).await.counts.action_required, 0);
    assert_eq!(
        inbox(&pool, &b).await.progress[0].pending_recipients.len(),
        1
    );
    let mut reassign = action(one.record.revision, ProcessAction::Reassign);
    reassign.comment = Some("Transfer current task".into());
    reassign.reassign_from_user_id = Some(b.user_id);
    reassign.reassign_user_id = Some(creator.user_id);
    let reassigned = correspondence::act(&pool, &b, draft.record.id, &MemoryValidator, reassign)
        .await
        .unwrap();
    assert_eq!(inbox(&pool, &b).await.counts.action_required, 0);
    assert_eq!(
        inbox(&pool, &creator).await.progress[0].pending_recipients[0].id,
        creator.user_id
    );
    let next = correspondence::act(
        &pool,
        &creator,
        draft.record.id,
        &MemoryValidator,
        action(reassigned.record.revision, ProcessAction::Approve),
    )
    .await
    .unwrap();
    assert_eq!(
        inbox(&pool, &a).await.progress[0].current_step_index,
        Some(1)
    );
    let mut return_request = action(next.record.revision, ProcessAction::ReturnForRevision);
    return_request.comment = Some("Please revise".into());
    correspondence::act(&pool, &a, draft.record.id, &MemoryValidator, return_request)
        .await
        .unwrap();
    let returned = inbox(&pool, &creator).await;
    assert_eq!(returned.counts.action_required, 1);
    let progress = &returned.progress[0];
    assert!(progress.can_revise && progress.my_action.is_none());
    assert_eq!(
        (
            progress.current_step_index,
            progress.current_step_title.as_ref(),
            progress.due_at,
            progress.step_count
        ),
        (None, None, None, 2)
    );
    assert!(progress.pending_recipients.is_empty());
    assert_eq!(inbox(&pool, &a).await.counts.action_required, 0);
}

#[tokio::test]
async fn list_any_terminal_close_and_signature_permission_match_available_actions() {
    let (pool, creator, a, b) = fixture("correspondence_list_terminal").await;
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Acknowledge,
            ProcessCompletionPolicy::Any,
            &[a.user_id, b.user_id],
        )],
    )
    .await;
    let draft = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    let sent = send(&pool, &creator, &draft, t.id).await;
    // The DTO accepts omitted false defaults; SQL eligibility must match that snapshot.
    sqlx::query("UPDATE process_instances SET definition=definition #- '{steps,0,definition,requiresSignature}' WHERE id=$1")
        .bind(sent.instance.as_ref().unwrap().id).execute(&pool).await.unwrap();
    let defaulted = correspondence::detail(&pool, &a, draft.record.id)
        .await
        .unwrap();
    assert!(defaulted
        .available_actions
        .contains(&ProcessAction::Acknowledge));
    assert_eq!(
        inbox(&pool, &a).await.progress[0].my_action,
        Some(ProcessAction::Acknowledge)
    );
    let complete = correspondence::act(
        &pool,
        &a,
        draft.record.id,
        &MemoryValidator,
        action(sent.record.revision, ProcessAction::Acknowledge),
    )
    .await
    .unwrap();
    assert_eq!(inbox(&pool, &b).await.counts.action_required, 0);
    let close = inbox(&pool, &creator).await;
    assert_eq!(close.progress[0].my_action, Some(ProcessAction::Close));
    assert!(close.progress[0].pending_recipients.is_empty());
    assert!(close.progress[0].current_step_index.is_none());
    correspondence::act(
        &pool,
        &creator,
        draft.record.id,
        &MemoryValidator,
        action(complete.record.revision, ProcessAction::Close),
    )
    .await
    .unwrap();
    assert_eq!(inbox(&pool, &creator).await.counts.action_required, 0);
    let mut sign_step = step(
        ProcessStepKind::Review,
        ProcessCompletionPolicy::All,
        &[a.user_id],
    );
    sign_step.requires_signature = true;
    let t = template(&pool, &creator, vec![sign_step]).await;
    let draft = correspondence::create(&pool, &creator, &MemoryValidator, create_request())
        .await
        .unwrap();
    send(&pool, &creator, &draft, t.id).await;
    let no_sign = ActorContext {
        user_id: a.user_id,
        permissions: vec![codes::CORRESPONDENCE_UPDATE_ASSIGNED.into()],
    };
    assert_eq!(inbox(&pool, &no_sign).await.counts.action_required, 0);
    let all = correspondence::list(&pool, &no_sign, list_query(CorrespondenceListView::All))
        .await
        .unwrap();
    assert!(all.progress.iter().all(|p| p.my_action.is_none()));
    assert_eq!(
        inbox(&pool, &a).await.progress[0].my_action,
        Some(ProcessAction::Review)
    );
}

#[tokio::test]
async fn draft_metadata_revision_is_atomic_author_only_and_preserves_registration() {
    let (pool, creator, other, _) = fixture("correspondence_draft_metadata").await;
    let mut create = create_request();
    create.organization_unit_id = Some(
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM organization_units WHERE is_active LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap(),
    );
    let draft = correspondence::create(&pool, &creator, &MemoryValidator, create)
        .await
        .unwrap();
    let request = ReviseCorrespondenceRequest {
        request_id: Uuid::now_v7(),
        expected_revision: draft.record.revision,
        content: "edited body".into(),
        original_file_id: None,
        attachment_file_ids: vec![],
        annotations: vec![],
        metadata: Some(CorrespondenceDraftMetadata {
            title: "Edited title".into(),
            sender: "Edited sender".into(),
            document_number: None,
            document_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 10, 18).unwrap()),
            urgency: CorrespondenceUrgency::Urgent,
        }),
    };
    let mut invalid = request.clone();
    invalid.metadata.as_mut().unwrap().title = " ".into();
    assert!(
        correspondence::revise(&pool, &creator, draft.record.id, &MemoryValidator, invalid)
            .await
            .is_err()
    );
    let mut stale = request.clone();
    stale.expected_revision = 0;
    assert!(
        correspondence::revise(&pool, &creator, draft.record.id, &MemoryValidator, stale)
            .await
            .is_err()
    );
    let unchanged = correspondence::detail(&pool, &creator, draft.record.id)
        .await
        .unwrap();
    assert_eq!(unchanged.record.revision, draft.record.revision);
    assert_eq!(unchanged.revisions.len(), 1);
    assert!(correspondence::revise(
        &pool,
        &other,
        draft.record.id,
        &MemoryValidator,
        request.clone()
    )
    .await
    .is_err());
    let read_only_author = ActorContext {
        user_id: creator.user_id,
        permissions: vec![codes::CORRESPONDENCE_READ_OWN.into()],
    };
    let read_only_list = correspondence::list(
        &pool,
        &read_only_author,
        list_query(CorrespondenceListView::Drafts),
    )
    .await
    .unwrap();
    assert!(!read_only_list.progress[0].can_revise);
    assert_eq!(read_only_list.counts.action_required, 0);
    assert!(correspondence::revise(
        &pool,
        &read_only_author,
        draft.record.id,
        &MemoryValidator,
        request.clone()
    )
    .await
    .is_err());
    let revised = correspondence::revise(
        &pool,
        &creator,
        draft.record.id,
        &MemoryValidator,
        request.clone(),
    )
    .await
    .unwrap();
    assert_eq!(revised.record.title, "Edited title");
    assert_eq!(revised.record.sender, "Edited sender");
    assert_eq!(revised.record.document_number, None);
    assert_eq!(revised.record.urgency, CorrespondenceUrgency::Urgent);
    assert_eq!(revised.document.content, "edited body");
    assert_eq!(
        (
            revised.record.register,
            revised.record.register_number,
            revised.record.register_year,
            revised.record.organization_unit_id,
            revised.record.created_by,
            revised.record.created_at
        ),
        (
            draft.record.register,
            draft.record.register_number,
            draft.record.register_year,
            draft.record.organization_unit_id,
            draft.record.created_by,
            draft.record.created_at
        )
    );
    let replay = correspondence::revise(
        &pool,
        &creator,
        draft.record.id,
        &MemoryValidator,
        request.clone(),
    )
    .await
    .unwrap();
    assert_eq!(replay.revisions.len(), 2);
    assert_eq!(replay.record.revision, revised.record.revision);
    let mut divergent = request.clone();
    divergent.metadata.as_mut().unwrap().sender = "different replay".into();
    assert!(correspondence::revise(
        &pool,
        &creator,
        draft.record.id,
        &MemoryValidator,
        divergent
    )
    .await
    .is_err());
    let t = template(
        &pool,
        &creator,
        vec![step(
            ProcessStepKind::Review,
            ProcessCompletionPolicy::All,
            &[other.user_id],
        )],
    )
    .await;
    let sent = send(&pool, &creator, &revised, t.id).await;
    let mut returned_action = action(sent.record.revision, ProcessAction::ReturnForRevision);
    returned_action.comment = Some("Revise body".into());
    let returned = correspondence::act(
        &pool,
        &other,
        draft.record.id,
        &MemoryValidator,
        returned_action,
    )
    .await
    .unwrap();
    let mut returned_request = request;
    returned_request.request_id = Uuid::now_v7();
    returned_request.expected_revision = returned.record.revision;
    assert!(correspondence::revise(
        &pool,
        &creator,
        draft.record.id,
        &MemoryValidator,
        returned_request.clone()
    )
    .await
    .is_err());
    returned_request.request_id = Uuid::now_v7();
    returned_request.metadata = None;
    let body_only = correspondence::revise(
        &pool,
        &creator,
        draft.record.id,
        &MemoryValidator,
        returned_request,
    )
    .await
    .unwrap();
    assert_eq!(body_only.record.title, "Edited title");
    assert_eq!(body_only.revisions.len(), 3);
}

#[test]
fn old_revision_requests_keep_serialized_payload_and_hash_without_metadata() {
    let legacy = serde_json::json!({
        "requestId": Uuid::now_v7(), "expectedRevision": 1, "content": "legacy body",
        "originalFileId": null, "attachmentFileIds": [], "annotations": []
    });
    let request: ReviseCorrespondenceRequest = serde_json::from_value(legacy.clone()).unwrap();
    assert!(request.metadata.is_none());
    assert_eq!(serde_json::to_value(&request).unwrap(), legacy);
    let legacy_bytes = format!(
        r#"{{"requestId":"{}","expectedRevision":1,"content":"legacy body","originalFileId":null,"attachmentFileIds":[],"annotations":[]}}"#,
        request.request_id
    );
    assert_eq!(serde_json::to_string(&request).unwrap(), legacy_bytes);
}
