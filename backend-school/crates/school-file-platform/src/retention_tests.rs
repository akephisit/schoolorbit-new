use super::*;
use crate::repository::FileRepository;

#[tokio::test]
async fn retention_manifest_keeps_missing_pending_failed_and_held_files_distinct() {
    let pool = school_test_db::create_named_test_pool("files_retention_manifest").await;
    school_test_db::run_test_migrations(&pool).await;
    let file:Uuid=sqlx::query_scalar("INSERT INTO files(display_filename,purpose_code,visibility,lifecycle_status,retention_class) VALUES('retention.jpg','attendance_evidence','private','ready','legal_hold') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let version:Uuid=sqlx::query_scalar("INSERT INTO file_versions(file_id,version_number,provider_code,storage_class,storage_status,object_key,detected_mime_type,canonical_extension,byte_size,checksum,scan_status) VALUES($1,1,'r2','private','stored',$2,'image/jpeg','jpg',100,repeat('a',64),'clean') RETURNING id")
        .bind(file).bind(format!("tenants/{}/attendance/evidence/{file}/v1/original.jpg",Uuid::new_v4())).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE files SET current_version_id=$2 WHERE id=$1")
        .bind(file)
        .bind(version)
        .execute(&pool)
        .await
        .unwrap();
    let missing = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        require_retention_allowed(&mut tx, &[file]).await,
        Err(AppError::Conflict(_))
    ));
    let initial = deletion_progress(&mut tx, &[file, missing, file])
        .await
        .unwrap();
    assert_eq!((initial.pending, initial.failed), (2, 0));
    let empty = deletion_progress(&mut tx, &[]).await.unwrap();
    assert_eq!((empty.pending, empty.failed), (0, 0));
    tx.rollback().await.unwrap();
    sqlx::query("UPDATE files SET retention_class='standard' WHERE id=$1")
        .bind(file)
        .execute(&pool)
        .await
        .unwrap();
    let repository = crate::repository::SqlFileRepository::new(pool.clone());
    let mut tx = pool.begin().await.unwrap();
    require_retention_allowed(&mut tx, &[file]).await.unwrap();
    repository
        .request_delete_in_transaction(&mut tx, file)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("UPDATE file_operations SET status='failed',last_error_code='synthetic_delete_failure' WHERE file_id=$1 AND operation_type='delete_object'")
        .bind(file).execute(&pool).await.unwrap();
    let mut connection = pool.acquire().await.unwrap();
    let failed = deletion_progress(&mut connection, &[file]).await.unwrap();
    assert_eq!((failed.pending, failed.failed), (1, 1));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT storage_status FROM file_versions WHERE id=$1")
            .bind(version)
            .fetch_one(&mut *connection)
            .await
            .unwrap(),
        "delete_requested"
    );
    drop(connection);
    let replacement = repository.request_delete(file).await.unwrap();
    assert_eq!(replacement.len(), 1);
    let mut connection = pool.acquire().await.unwrap();
    let retrying = deletion_progress(&mut connection, &[file]).await.unwrap();
    assert_eq!((retrying.pending, retrying.failed), (1, 0));
    drop(connection);
    repository
        .mark_delete_succeeded(&replacement[0])
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    let completed = deletion_progress(&mut connection, &[file]).await.unwrap();
    assert_eq!((completed.pending, completed.failed), (0, 0));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM file_operations WHERE file_id=$1 AND status='failed'"
        )
        .bind(file)
        .fetch_one(&mut *connection)
        .await
        .unwrap(),
        1
    );
}
