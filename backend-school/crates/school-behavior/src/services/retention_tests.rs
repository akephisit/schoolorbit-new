use super::*;

#[tokio::test]
async fn retention_chunks_resume_the_same_receipt_without_resetting_points_or_cycles() {
    let pool = school_test_db::create_named_test_pool("behavior_retention_chunk_replay").await;
    school_test_db::run_test_migrations(&pool).await;
    let actor:Uuid=sqlx::query_scalar("INSERT INTO users(username,password_hash,first_name,last_name,user_type,status) VALUES('retention-behavior','!','Retention','Fixture','staff','active') RETURNING id").fetch_one(&pool).await.unwrap();
    let year:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,status) VALUES(2998,'Retention','planning') RETURNING id").fetch_one(&pool).await.unwrap();
    let bell:Uuid=sqlx::query_scalar("INSERT INTO bell_schedules(id,academic_year_id,code,name,is_default) VALUES(uuidv7(),$1,'RETENTION','Retention',true) RETURNING id").bind(year).fetch_one(&pool).await.unwrap();
    let term:Uuid=sqlx::query_scalar("INSERT INTO academic_terms(school_days, academic_year_id,sequence_no,code,name,term_type,bell_schedule_id,status,included_in_year_result,blocks_year_closure) VALUES( 'MON,TUE,WED,THU,FRI',$1,1,'RETENTION','Retention','regular',$2,'closed',true,true) RETURNING id").bind(year).bind(bell).fetch_one(&pool).await.unwrap();
    let cycle: Uuid =
        sqlx::query_scalar("SELECT id FROM behavior_cycles WHERE side='merit' AND state='current'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let student = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) VALUES($1,'!','Retention pupil','Fixture','student','active')").bind(student).execute(&pool).await.unwrap();
    let total:Uuid=sqlx::query_scalar("INSERT INTO behavior_daily_totals(student_id,date,academic_term_id,grade_level_id,side,source,cycle_id,points) VALUES($1,CURRENT_DATE,$2,(SELECT id FROM grade_levels ORDER BY id LIMIT 1),'merit','arrival',$3,20001) RETURNING id")
        .bind(student).bind(term).bind(cycle).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO behavior_entries(total_id,source_key,source_generation,source_revision,source_status,raw_points,item_name,points) SELECT $1,'retention-source-'||n,$2,1,'present',1,'Synthetic contribution',1 FROM generate_series(1,20001) n")
        .bind(total).bind(Uuid::now_v7()).execute(&pool).await.unwrap();
    let request = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    let accepted = retain_in_transaction(&mut tx, actor, request, term)
        .await
        .unwrap();
    assert_eq!(accepted.status, "pending");
    tx.commit().await.unwrap();
    assert!(cleanup::run_cleanup_jobs(&pool, 1).await.unwrap());
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM behavior_entries WHERE total_id=$1")
            .bind(total)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 15001);
    let mut tx = pool.begin().await.unwrap();
    let replay = retain_in_transaction(&mut tx, actor, request, term)
        .await
        .unwrap();
    assert_eq!(replay.id, accepted.id);
    assert_eq!(replay.entries_deleted, 5000);
    tx.commit().await.unwrap();
    assert!(!cleanup::run_cleanup_jobs(&pool, 8).await.unwrap());
    let mut tx = pool.begin().await.unwrap();
    let completed = retain_in_transaction(&mut tx, actor, request, term)
        .await
        .unwrap();
    assert_eq!(completed.id, accepted.id);
    assert_eq!(completed.status, "completed");
    let preserved: (i64, Uuid, bool) = sqlx::query_as(
        "SELECT points,cycle_id,details_purged FROM behavior_daily_totals WHERE id=$1",
    )
    .bind(total)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(preserved, (20001, cycle, true));
    assert_eq!(entry_count(&mut tx, term).await.unwrap(), 0);
    assert!(matches!(
        retain_in_transaction(&mut tx, Uuid::now_v7(), request, term).await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
}
