use super::*;

#[tokio::test]
async fn retained_attendance_blocks_reopening_the_exact_term_and_its_year() {
    let pool = school_test_db::create_named_test_pool("attendance_retention_reopen_guard").await;
    school_test_db::run_test_migrations(&pool).await;
    let actor:Uuid=sqlx::query_scalar("INSERT INTO users(username,password_hash,first_name,last_name,user_type,status) VALUES('retention-owner','!','Retention','Fixture','staff','active') RETURNING id").fetch_one(&pool).await.unwrap();
    let year:Uuid=sqlx::query_scalar("INSERT INTO academic_years(year,name,status) VALUES(2998,'Retention','planning') RETURNING id").fetch_one(&pool).await.unwrap();
    let bell:Uuid=sqlx::query_scalar("INSERT INTO bell_schedules(id,academic_year_id,code,name,is_default) VALUES(uuidv7(),$1,'RETENTION','Retention',true) RETURNING id").bind(year).fetch_one(&pool).await.unwrap();
    let mut terms = Vec::new();
    for sequence in 1..=2 {
        let term:Uuid=sqlx::query_scalar("INSERT INTO academic_terms(school_days, academic_year_id,sequence_no,code,name,term_type,bell_schedule_id,status,included_in_year_result,blocks_year_closure) VALUES( 'MON,TUE,WED,THU,FRI',$1,$2,$3,'Retention','regular',$4,'closed',true,true) RETURNING id")
            .bind(year).bind(sequence).bind(format!("T{sequence}")).bind(bell).fetch_one(&pool).await.unwrap();
        terms.push(term);
    }
    let mut connection = pool.acquire().await.unwrap();
    require_reopen_allowed(&mut connection, year, Some(terms[0]))
        .await
        .unwrap();
    sqlx::query("INSERT INTO attendance_term_archives(academic_term_id,archived_by,record_count,counted_record_count,settings_snapshot) VALUES($1,$2,0,0,'{}')")
        .bind(terms[0]).bind(actor).execute(&mut *connection).await.unwrap();
    assert!(matches!(
        require_reopen_allowed(&mut connection, year, Some(terms[0])).await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        require_reopen_allowed(&mut connection, year, None).await,
        Err(AppError::Conflict(_))
    ));
    require_reopen_allowed(&mut connection, year, Some(terms[1]))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attendance_term_archives")
            .fetch_one(&mut *connection)
            .await
            .unwrap(),
        1
    );
}
