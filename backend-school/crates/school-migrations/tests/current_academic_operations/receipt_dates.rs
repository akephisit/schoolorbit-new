use super::*;

async fn receipt_facts(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        r#"SELECT jsonb_build_object(
            'apply',(SELECT jsonb_agg(to_jsonb(r)-'starts_on' ORDER BY idempotency_key) FROM learning_delivery_apply_runs r),
            'preparation',(SELECT jsonb_agg(to_jsonb(r)-'offering_starts_on' ORDER BY id) FROM academic_term_preparation_runs r),
            'cutover',(SELECT to_jsonb(r) FROM academic_current_operations_migration_audit r WHERE migration_version=157))"#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn date_column_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM pg_attribute WHERE NOT attisdropped AND \
         ((attrelid='learning_delivery_apply_runs'::regclass AND attname='starts_on') OR \
          (attrelid='academic_term_preparation_runs'::regclass AND attname='offering_starts_on'))",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn receipt_date_cleanup_refuses_dependencies_atomically_and_preserves_accepted_facts_on_retry(
) {
    let pool = predecessor("current_ops_receipt_dates").await;
    apply_migrations_through(&pool, 157).await.unwrap();
    let apply_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO learning_delivery_apply_runs(
            idempotency_key,academic_term_id,request_hash,source_hash,offering_ids,group_ids,
            created_offering_count,created_group_count,actor_user_id,curriculum_sources,starts_on)
        SELECT $1,offering.academic_term_id,repeat('a',64),repeat('b',64),ARRAY[offering.id],ARRAY[groups.id],
            1,1,(SELECT id FROM users ORDER BY id LIMIT 1),
            '[{"editionId":"15800000-0000-0000-0000-000000000001","publicationId":"15800000-0000-0000-0000-000000000002","publicationNo":2,"revisionYear":2569,"editionName":"Accepted source"}]'::jsonb,
            '2026-05-10' FROM learning_offerings offering JOIN learning_groups groups ON groups.learning_offering_id=offering.id
        ORDER BY offering.id,groups.id LIMIT 1"#,
    )
    .bind(apply_id)
    .execute(&pool)
    .await
    .unwrap();
    let preparation_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO academic_term_preparation_runs(
            request_id,source_academic_term_id,source_academic_year_id,target_academic_term_id,target_academic_year_id,
            selected_modules,mappings,source_checksum,request_checksum,outcome,actor_user_id,offering_starts_on)
        SELECT $1,source.id,source.academic_year_id,target.id,target.academic_year_id,ARRAY['delivery'],
            '{"entities":[],"dates":[]}',repeat('c',64),repeat('d',64),
            jsonb_build_array(jsonb_build_object('module','delivery','createdCount',2,'targetIds',jsonb_build_array(offering.id,groups.id))),
            (SELECT id FROM users ORDER BY id LIMIT 1),'2026-06-01'
        FROM academic_terms source CROSS JOIN academic_terms target
        JOIN learning_offerings offering ON offering.academic_term_id=target.id
        JOIN learning_groups groups ON groups.learning_offering_id=offering.id
        WHERE source.id<>target.id ORDER BY source.id,target.id,offering.id,groups.id LIMIT 1"#,
    )
    .bind(preparation_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM learning_delivery_apply_runs WHERE idempotency_key=$1 AND starts_on IS NOT NULL) AND EXISTS(SELECT 1 FROM academic_term_preparation_runs WHERE request_id=$2 AND offering_starts_on IS NOT NULL)")
        .bind(apply_id).bind(preparation_id).fetch_one(&pool).await.unwrap());
    let before = receipt_facts(&pool).await;
    let current_before = current_facts(&pool).await;

    // A dependency on the second DROP must roll back the already executed first DROP.
    sqlx::raw_sql("CREATE VIEW receipt_date_dependency AS SELECT offering_starts_on FROM academic_term_preparation_runs")
        .execute(&pool).await.unwrap();
    let error = school_migrations::run_tenant_migrations_detailed(&pool)
        .await
        .unwrap_err();
    assert_eq!(error.diagnostic.version, Some(158));
    assert_eq!(error.diagnostic.sqlstate.as_deref(), Some("2BP01"));
    assert_eq!(date_column_count(&pool).await, 2);
    assert_eq!(before, receipt_facts(&pool).await);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(&pool)
            .await
            .unwrap(),
        157
    );
    sqlx::raw_sql("DROP VIEW receipt_date_dependency")
        .execute(&pool)
        .await
        .unwrap();

    canonical(&pool).await;
    assert_eq!(date_column_count(&pool).await, 0);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM pg_proc WHERE pronamespace=current_schema()::regnamespace AND ((prosrc LIKE '%learning_delivery_apply_runs%' AND prosrc LIKE '%starts_on%') OR (prosrc LIKE '%academic_term_preparation_runs%' AND prosrc LIKE '%offering_starts_on%'))")
        .fetch_one(&pool).await.unwrap(),0);
    assert_eq!(before, receipt_facts(&pool).await);
    assert_eq!(current_before, current_facts(&pool).await);
    assert!(sqlx::query(
        "UPDATE academic_term_preparation_runs SET outcome='[]' WHERE request_id=$1"
    )
    .bind(preparation_id)
    .execute(&pool)
    .await
    .is_err());
    canonical(&pool).await;
    assert_eq!(before, receipt_facts(&pool).await);
    assert_eq!(current_before, current_facts(&pool).await);
}
