use super::*;

#[tokio::test]
async fn post_cutover_new_term_initializes_and_advances_current_delivery_revision() {
    let pool = predecessor("current_new_term_revision").await;
    canonical(&pool).await;
    let term = Uuid::new_v4();
    let offering = Uuid::new_v4();
    let group = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    let (source, year): (Uuid, Uuid) =
        sqlx::query_as("SELECT id,academic_year_id FROM learning_offerings WHERE kind='course' ORDER BY id LIMIT 1")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    sqlx::query("INSERT INTO academic_terms SELECT (jsonb_populate_record(NULL::academic_terms,to_jsonb(t)||jsonb_build_object('id',$1::uuid,'sequence_no',(SELECT max(sequence_no)+1 FROM academic_terms WHERE academic_year_id=t.academic_year_id),'code',$1::text))).* FROM academic_terms t JOIN learning_offerings o ON o.academic_term_id=t.id WHERE o.id=$2")
        .bind(term).bind(source).execute(&mut *tx).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM academic_delivery_state WHERE academic_term_id=$1"
        )
        .bind(term)
        .fetch_one(&mut *tx)
        .await
        .unwrap(),
        0
    );
    sqlx::query("INSERT INTO learning_offerings SELECT (jsonb_populate_record(NULL::learning_offerings,to_jsonb(o)||jsonb_build_object('id',$2::uuid,'academic_term_id',$3::uuid))).* FROM learning_offerings o WHERE id=$1")
        .bind(source).bind(offering).bind(term).execute(&mut *tx).await.unwrap();
    let read = "SELECT row_version FROM academic_delivery_state WHERE academic_term_id=$1 AND academic_year_id=$2";
    assert_eq!(
        sqlx::query_scalar::<_, i64>(read)
            .bind(term)
            .bind(year)
            .fetch_one(&mut *tx)
            .await
            .unwrap(),
        2
    );
    let mut revision = 2;
    sqlx::query("INSERT INTO course_offering_details SELECT (jsonb_populate_record(NULL::course_offering_details,to_jsonb(d)||jsonb_build_object('learning_offering_id',$2::uuid,'academic_term_id',$3::uuid))).* FROM course_offering_details d WHERE learning_offering_id=$1")
        .bind(source).bind(offering).bind(term).execute(&mut *tx).await.unwrap();
    let next = sqlx::query_scalar::<_, i64>(read)
        .bind(term)
        .bind(year)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(next > revision);
    revision = next;
    sqlx::query(
        "UPDATE learning_offerings SET weekly_period_target=weekly_period_target+1 WHERE id=$1",
    )
    .bind(offering)
    .execute(&mut *tx)
    .await
    .unwrap();
    let next = sqlx::query_scalar::<_, i64>(read)
        .bind(term)
        .bind(year)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(next > revision);
    revision = next;
    sqlx::query("INSERT INTO learning_groups(id,learning_offering_id,academic_term_id,academic_year_id,code,name,registration_open) VALUES($1,$2,$3,$4,'NEW','New current group',false)").bind(group).bind(offering).bind(term).bind(year).execute(&mut *tx).await.unwrap();
    let next = sqlx::query_scalar::<_, i64>(read)
        .bind(term)
        .bind(year)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(next > revision);
    revision = next;
    sqlx::query("UPDATE learning_groups SET name='Edited current group' WHERE id=$1")
        .bind(group)
        .execute(&mut *tx)
        .await
        .unwrap();
    let next = sqlx::query_scalar::<_, i64>(read)
        .bind(term)
        .bind(year)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(next > revision);
    revision = next;
    tx.commit().await.unwrap();
    let mut rollback = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM learning_groups WHERE id=$1")
        .bind(group)
        .execute(&mut *rollback)
        .await
        .unwrap();
    assert!(
        sqlx::query_scalar::<_, i64>(read)
            .bind(term)
            .bind(year)
            .fetch_one(&mut *rollback)
            .await
            .unwrap()
            > revision
    );
    rollback.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(read)
            .bind(term)
            .bind(year)
            .fetch_one(&pool)
            .await
            .unwrap(),
        revision
    );
    canonical(&pool).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(read)
            .bind(term)
            .bind(year)
            .fetch_one(&pool)
            .await
            .unwrap(),
        revision
    );
}
