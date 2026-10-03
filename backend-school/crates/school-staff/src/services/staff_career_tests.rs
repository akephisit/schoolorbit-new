use super::personnel_tests::migrate_through;
use super::staff_career_service;
use crate::career::*;
use crate::personnel::StaffAcademicRank;
use school_errors::AppError;
use school_test_db::{create_named_test_pool, create_named_test_pool_with_max_connections};
use sqlx::PgPool;
use uuid::Uuid;

fn rank_input(rank: Option<StaffAcademicRank>, date: Option<&str>) -> StaffCareerEntryInput {
    StaffCareerEntryInput {
        fact: StaffCareerFact::AcademicRank { value: rank },
        effective_date: date.map(|value| value.parse().unwrap()),
        order_date: None,
        order_number: None,
        note: None,
    }
}

fn reference(entry: &StaffCareerEntry) -> StaffCareerReference {
    StaffCareerReference {
        id: entry.id,
        revision: entry.revision,
    }
}

async fn career_fixture(name: &str) -> (PgPool, Uuid, Uuid) {
    let pool = create_named_test_pool_with_max_connections(name, 5).await;
    migrate_through(&pool, 84).await.unwrap();
    let user = staff_user(&pool).await;
    let actor = staff_user(&pool).await;
    sqlx::query("INSERT INTO staff_info(user_id,academic_rank) VALUES($1,'none')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    migrate_through(&pool, 85).await.unwrap();
    (pool, user, actor)
}

async fn current_change(
    pool: &PgPool,
    user: Uuid,
    actor: Uuid,
    input: UpdateStaffCareerRequest,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    staff_career_service::patch_current_career(&mut tx, user, actor, &input).await?;
    tx.commit().await?;
    Ok(())
}

async fn history(pool: &PgPool, user: Uuid) -> StaffCareerHistoryPage {
    staff_career_service::list_staff_career_history(pool, user, StaffCareerHistoryQuery::default())
        .await
        .unwrap()
}

#[tokio::test]
async fn staff_career_current_and_history_commit_together() {
    let (pool, user, actor) = career_fixture("career_domain_current").await;
    let original = history(&pool, user).await.current.academic_rank.unwrap();
    let mut promoted = rank_input(Some(StaffAcademicRank::Proficient), Some("2023-05-01"));
    promoted.order_date = Some("2023-05-15".parse().unwrap());
    current_change(
        &pool,
        user,
        actor,
        UpdateStaffCareerRequest {
            changes: vec![StaffCareerCurrentChange {
                expected_current: Some(reference(&original)),
                entry: promoted.clone(),
                correction_reason: None,
            }],
        },
    )
    .await
    .unwrap();
    let page = history(&pool, user).await;
    assert_eq!(page.items.len(), 2);
    assert!(page.items.iter().any(|entry| entry.id == original.id
        && entry.effective_date.is_none()
        && !entry.is_current));
    let current = page.current.academic_rank.unwrap();
    assert_eq!(current.fact, promoted.fact);
    assert_eq!(current.effective_date, promoted.effective_date);
    let unchanged_count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    current_change(
        &pool,
        user,
        actor,
        UpdateStaffCareerRequest {
            changes: vec![StaffCareerCurrentChange {
                expected_current: Some(reference(&current)),
                entry: promoted.clone(),
                correction_reason: None,
            }],
        },
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, unchanged_count);
    promoted.order_number = Some("12/2566".into());
    let change = StaffCareerCurrentChange {
        expected_current: Some(reference(&current)),
        entry: promoted.clone(),
        correction_reason: None,
    };
    assert!(matches!(
        current_change(
            &pool,
            user,
            actor,
            UpdateStaffCareerRequest {
                changes: vec![change.clone()]
            }
        )
        .await,
        Err(AppError::BadRequest(_))
    ));
    current_change(
        &pool,
        user,
        actor,
        UpdateStaffCareerRequest {
            changes: vec![StaffCareerCurrentChange {
                correction_reason: Some("เติมตามคำสั่ง".into()),
                ..change
            }],
        },
    )
    .await
    .unwrap();
    let corrected = history(&pool, user).await.current.academic_rank.unwrap();
    assert_eq!(corrected.id, current.id);
    assert_eq!(corrected.revision, 2);
    current_change(
        &pool,
        user,
        actor,
        UpdateStaffCareerRequest {
            changes: vec![StaffCareerCurrentChange {
                expected_current: Some(reference(&corrected)),
                entry: rank_input(None, None),
                correction_reason: None,
            }],
        },
    )
    .await
    .unwrap();
    let cleared = history(&pool, user).await;
    assert_eq!(cleared.items.len(), 3);
    assert_eq!(
        cleared.current.academic_rank.unwrap().fact,
        StaffCareerFact::AcademicRank { value: None }
    );
}

#[tokio::test]
async fn staff_career_correction_conflict_and_audit_failure() {
    let (pool, user, actor) = career_fixture("career_domain_conflict").await;
    let original = history(&pool, user).await.current.academic_rank.unwrap();
    let change = UpdateStaffCareerRequest {
        changes: vec![StaffCareerCurrentChange {
            expected_current: Some(reference(&original)),
            entry: rank_input(Some(StaffAcademicRank::Proficient), Some("2023-05-01")),
            correction_reason: None,
        }],
    };
    let (first, second) = tokio::join!(
        current_change(&pool, user, actor, change.clone()),
        current_change(&pool, user, actor, change)
    );
    assert_eq!(
        [&first, &second]
            .iter()
            .filter(|result| result.is_ok())
            .count(),
        1
    );
    assert!(
        matches!(first, Err(AppError::Conflict(_))) || matches!(second, Err(AppError::Conflict(_)))
    );
    assert!(matches!(
        staff_career_service::correct_staff_career_history(
            &pool,
            user,
            original.id,
            actor,
            CorrectStaffCareerHistoryRequest {
                expected_revision: original.revision,
                expected_is_current: true,
                entry: rank_input(Some(StaffAcademicRank::None), Some("2020-01-01")),
                reason: "เติมวันที่".into(),
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let before = history(&pool, user).await;
    let current = before.current.academic_rank.as_ref().unwrap();
    sqlx::query("ALTER TABLE audit_logs RENAME TO career_hidden_audit_logs")
        .execute(&pool)
        .await
        .unwrap();
    let result = staff_career_service::correct_staff_career_history(
        &pool,
        user,
        current.id,
        actor,
        CorrectStaffCareerHistoryRequest {
            expected_revision: current.revision,
            expected_is_current: true,
            entry: rank_input(Some(StaffAcademicRank::Proficient), Some("2023-05-02")),
            reason: "แก้ตามคำสั่ง".into(),
        },
    )
    .await;
    assert!(result.is_err());
    sqlx::query("ALTER TABLE career_hidden_audit_logs RENAME TO audit_logs")
        .execute(&pool)
        .await
        .unwrap();
    let after = history(&pool, user).await;
    assert_eq!(after.current, before.current);
    assert_eq!(after.items, before.items);
}

#[tokio::test]
async fn staff_career_retry_and_cursor_scope() {
    let (pool, user, actor) = career_fixture("career_domain_retry").await;
    let original = history(&pool, user).await.current.academic_rank.unwrap();
    current_change(
        &pool,
        user,
        actor,
        UpdateStaffCareerRequest {
            changes: vec![StaffCareerCurrentChange {
                expected_current: Some(reference(&original)),
                entry: rank_input(Some(StaffAcademicRank::Proficient), Some("2023-05-01")),
                correction_reason: None,
            }],
        },
    )
    .await
    .unwrap();
    let request = CreateStaffCareerHistoryRequest {
        id: Uuid::new_v4(),
        entry: rank_input(Some(StaffAcademicRank::None), Some("2020-01-01")),
    };
    let first =
        staff_career_service::append_staff_career_history(&pool, user, actor, request.clone())
            .await
            .unwrap();
    let first_count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let retry =
        staff_career_service::append_staff_career_history(&pool, user, actor, request.clone())
            .await
            .unwrap();
    assert_eq!(first.id, retry.id);
    assert_eq!(first.revision, retry.revision);
    let retried_count: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(first_count, retried_count);
    assert!(matches!(
        staff_career_service::append_staff_career_history(
            &pool,
            user,
            actor,
            CreateStaffCareerHistoryRequest {
                entry: rank_input(Some(StaffAcademicRank::Expert), Some("2020-01-01")),
                ..request.clone()
            }
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        staff_career_service::append_staff_career_history(&pool, actor, actor, request).await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        staff_career_service::append_staff_career_history(
            &pool,
            user,
            actor,
            CreateStaffCareerHistoryRequest {
                id: Uuid::new_v4(),
                entry: rank_input(Some(StaffAcademicRank::Expert), Some("2024-01-01"))
            }
        )
        .await,
        Err(AppError::BadRequest(_))
    ));
    let first_page = staff_career_service::list_staff_career_history(
        &pool,
        user,
        StaffCareerHistoryQuery {
            cursor: None,
            page_size: Some(1),
        },
    )
    .await
    .unwrap();
    assert_eq!(first_page.items.len(), 1);
    let second_page = staff_career_service::list_staff_career_history(
        &pool,
        user,
        StaffCareerHistoryQuery {
            cursor: first_page.next_cursor,
            page_size: Some(1),
        },
    )
    .await
    .unwrap();
    assert_eq!(second_page.items[0].id, first.id);
    let third_page = staff_career_service::list_staff_career_history(
        &pool,
        user,
        StaffCareerHistoryQuery {
            cursor: second_page.next_cursor,
            page_size: Some(1),
        },
    )
    .await
    .unwrap();
    assert_eq!(third_page.items[0].id, original.id);
    assert!(third_page.next_cursor.is_none());
    assert!(matches!(
        staff_career_service::list_staff_career_history(
            &pool,
            actor,
            StaffCareerHistoryQuery {
                cursor: Some(first.id),
                page_size: Some(20)
            }
        )
        .await,
        Err(AppError::BadRequest(_))
    ));
}

#[tokio::test]
async fn staff_career_preserves_unrelated_profile_patch() {
    let (pool, user, actor) = career_fixture("career_domain_patch").await;
    let before = history(&pool, user).await;
    let request = serde_json::from_value(serde_json::json!({"staff_info":{"education_level":"master","major":"วิทยาศาสตร์","teaching_license_expiry":"2030-01-01"}})).unwrap();
    super::staff_service::update_staff(&pool, user, request, actor)
        .await
        .unwrap();
    let after = history(&pool, user).await;
    assert_eq!(before.items, after.items);
    assert_eq!(before.current, after.current);
    let actual: (Option<String>,Option<String>,Option<String>) = sqlx::query_as("SELECT education_level,major,teaching_license_expiry::text FROM staff_info WHERE user_id=$1").bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(
        actual,
        (
            Some("master".into()),
            Some("วิทยาศาสตร์".into()),
            Some("2030-01-01".into())
        )
    );
}

async fn staff_user(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status,hired_date) VALUES($1,$2,'synthetic','Fixture','Person','staff','active','2020-01-01')")
        .bind(id).bind(id.to_string()).execute(pool).await.unwrap();
    id
}

async fn old_snapshot(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT (to_jsonb(info)-ARRAY['personnel_type','current_personnel_type_history_id','current_job_position_history_id','current_academic_rank_history_id'])::text FROM staff_info info ORDER BY user_id")
        .fetch_all(pool).await.unwrap()
}

#[tokio::test]
async fn staff_career_schema_preserves_legacy_without_dates() {
    let pool = create_named_test_pool("career_schema_preserves").await;
    migrate_through(&pool, 84).await.unwrap();
    let first = staff_user(&pool).await;
    let second = staff_user(&pool).await;
    let third = staff_user(&pool).await;
    let custom = Uuid::new_v4();
    sqlx::query("INSERT INTO staff_job_positions(id,code,name,is_active,is_selectable,display_order,created_at,updated_at) VALUES($1,$2,'ตำแหน่งเดิม',false,false,99,NOW(),NOW())")
        .bind(custom).bind(format!("custom_{}",custom.simple())).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO staff_info(user_id,job_position_id,academic_rank,education_level,major,university,employment_type,teaching_license_number,teaching_license_expiry,metadata) VALUES($1,(SELECT id FROM staff_job_positions WHERE code='teacher'),'none','master','วิทยาศาสตร์','สถาบันทดสอบ','contract','synthetic-license','2030-01-01','{\"fixture\":true}'),($2,$3,'not_applicable',NULL,NULL,NULL,NULL,NULL,NULL,'{}'),($4,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,'{}')")
        .bind(first).bind(second).bind(custom).bind(third).execute(&pool).await.unwrap();
    let before = old_snapshot(&pool).await;
    migrate_through(&pool, 85).await.unwrap();
    assert_eq!(old_snapshot(&pool).await, before);
    let values: (i64,i64,i64) = sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE effective_date IS NOT NULL OR order_date IS NOT NULL OR order_number IS NOT NULL OR created_by IS NOT NULL),count(*) FILTER(WHERE source<>'existing_record') FROM staff_career_history")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(values, (4, 0, 0));
    let types: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_info WHERE personnel_type IS NOT NULL OR current_personnel_type_history_id IS NOT NULL")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(types, 0);
    let entries: Vec<String> = sqlx::query_scalar(
        "SELECT to_jsonb(history)::text FROM staff_career_history history ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    migrate_through(&pool, 85).await.unwrap();
    let retried: Vec<String> = sqlx::query_scalar(
        "SELECT to_jsonb(history)::text FROM staff_career_history history ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(entries, retried);
}

#[tokio::test]
async fn staff_career_schema_rejects_inconsistent_projection() {
    let pool = create_named_test_pool("career_schema_integrity").await;
    migrate_through(&pool, 84).await.unwrap();
    let first = staff_user(&pool).await;
    let second = staff_user(&pool).await;
    sqlx::query(
        "INSERT INTO staff_info(user_id,academic_rank) VALUES($1,'none'),($2,'proficient')",
    )
    .bind(first)
    .bind(second)
    .execute(&pool)
    .await
    .unwrap();
    migrate_through(&pool, 85).await.unwrap();
    let first_entry: Uuid = sqlx::query_scalar(
        "SELECT current_academic_rank_history_id FROM staff_info WHERE user_id=$1",
    )
    .bind(first)
    .fetch_one(&pool)
    .await
    .unwrap();
    let second_entry: Uuid = sqlx::query_scalar(
        "SELECT current_academic_rank_history_id FROM staff_info WHERE user_id=$1",
    )
    .bind(second)
    .fetch_one(&pool)
    .await
    .unwrap();
    for invalid in [
        "UPDATE staff_info SET academic_rank='expert' WHERE user_id=$1 AND $2::uuid IS NOT NULL AND $3::uuid IS NOT NULL",
        "UPDATE staff_info SET current_academic_rank_history_id=NULL WHERE user_id=$1 AND $2::uuid IS NOT NULL AND $3::uuid IS NOT NULL",
        "UPDATE staff_info SET current_academic_rank_history_id=$2 WHERE user_id=$1 AND $3::uuid IS NOT NULL",
        "UPDATE staff_info SET current_job_position_history_id=$3 WHERE user_id=$1 AND $2::uuid IS NOT NULL",
        "UPDATE staff_info SET personnel_type='civil_servant' WHERE user_id=$1 AND $2::uuid IS NOT NULL AND $3::uuid IS NOT NULL",
        "UPDATE staff_career_history SET academic_rank='expert' WHERE id=$3 AND $1::uuid IS NOT NULL AND $2::uuid IS NOT NULL",
        "UPDATE staff_career_history SET user_id=$1 WHERE id=$2 AND $3::uuid IS NOT NULL",
        "DELETE FROM staff_career_history WHERE id=$3 AND $1::uuid IS NOT NULL AND $2::uuid IS NOT NULL",
    ] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query(invalid).bind(first).bind(second_entry).bind(first_entry).execute(&mut *tx).await.unwrap();
        assert!(tx.commit().await.is_err(), "{invalid}");
    }
    // A coordinated update succeeds; direct old-binary writes fail at commit.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("UPDATE staff_career_history SET academic_rank='expert' WHERE id=$1")
        .bind(first_entry)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE staff_info SET academic_rank='expert' WHERE user_id=$1")
        .bind(first)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM staff_career_history WHERE user_id=$1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn staff_career_schema_failed_import_rolls_back_and_retries() {
    let pool = create_named_test_pool("career_schema_retry").await;
    migrate_through(&pool, 84).await.unwrap();
    let user = staff_user(&pool).await;
    sqlx::query("UPDATE users SET user_type='student' WHERE id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO staff_info(user_id,academic_rank) VALUES($1,'none')")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    let before = old_snapshot(&pool).await;
    let failure = migrate_through(&pool, 85).await.unwrap_err();
    assert!(failure.to_string().contains("STAFF_CAREER_IMPORT_INVALID"));
    let table_exists: bool = sqlx::query_scalar(
        "SELECT to_regclass(current_schema()||'.staff_career_history') IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!table_exists);
    assert_eq!(old_snapshot(&pool).await, before);
    let version: i64 = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version, 84);
    sqlx::query("UPDATE users SET user_type='staff' WHERE id=$1")
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    migrate_through(&pool, 85).await.unwrap();
    assert_eq!(old_snapshot(&pool).await, before);
}

#[tokio::test]
async fn staff_career_schema_empty_provisioning_succeeds() {
    let pool = create_named_test_pool("career_schema_empty").await;
    migrate_through(&pool, 85).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM staff_career_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn staff_career_rejects_foreign_concurrent_id() {
    let (pool, user, actor) = career_fixture("career_id_race").await;
    let input = CreateStaffCareerHistoryRequest {
        id: Uuid::new_v4(),
        entry: rank_input(Some(StaffAcademicRank::None), None),
    };
    // An uncommitted foreign ID is invisible to the pre-read; insertion must still return 409.
    let mut owner = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO staff_career_history(id,user_id,kind,academic_rank,source) VALUES($1,$2,'academic_rank','none','staff_entry')").bind(input.id).bind(user).execute(&mut *owner).await.unwrap();
    let worker_pool = pool.clone();
    let worker = tokio::spawn(async move {
        staff_career_service::append_staff_career_history(&worker_pool, actor, actor, input).await
    });
    let mut waiting = false;
    for _ in 0..200 {
        waiting = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event='transactionid' AND query LIKE 'INSERT INTO staff_career_history%')").fetch_one(&pool).await.unwrap();
        if waiting {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        waiting,
        "the competing insertion must reach the unique constraint"
    );
    owner.commit().await.unwrap();
    assert!(matches!(worker.await.unwrap(), Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn staff_career_inactive_metadata_and_kind_validation() {
    let pool = create_named_test_pool("career_inactive_metadata").await;
    migrate_through(&pool, 84).await.unwrap();
    let user = staff_user(&pool).await;
    let actor = staff_user(&pool).await;
    let position: Uuid = sqlx::query_scalar("UPDATE staff_job_positions SET is_active=false,is_selectable=false WHERE code='teacher' RETURNING id").fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO staff_info(user_id,job_position_id) VALUES($1,$2)")
        .bind(user)
        .bind(position)
        .execute(&pool)
        .await
        .unwrap();
    migrate_through(&pool, 85).await.unwrap();
    let before = history(&pool, user).await.current.job_position.unwrap();
    let mut entry = rank_input(None, Some("2020-01-01"));
    entry.fact = StaffCareerFact::JobPosition {
        value: Some(position),
    };
    staff_career_service::correct_staff_career_history(
        &pool,
        user,
        before.id,
        actor,
        CorrectStaffCareerHistoryRequest {
            expected_revision: 1,
            expected_is_current: true,
            entry: entry.clone(),
            reason: "เติมวันที่ตามคำสั่ง".into(),
        },
    )
    .await
    .unwrap();
    let corrected = history(&pool, user).await.current.job_position.unwrap();
    assert_eq!(corrected.revision, 2);
    assert_eq!(corrected.job_position.unwrap().name, "ครู");
    let invalid_assignment = staff_career_service::append_staff_career_history(
        &pool,
        actor,
        actor,
        CreateStaffCareerHistoryRequest {
            id: Uuid::new_v4(),
            entry,
        },
    )
    .await;
    assert!(matches!(invalid_assignment, Err(AppError::BadRequest(_))));
    let invalid_kind = staff_career_service::correct_staff_career_history(
        &pool,
        user,
        before.id,
        actor,
        CorrectStaffCareerHistoryRequest {
            expected_revision: 2,
            expected_is_current: true,
            entry: rank_input(Some(StaffAcademicRank::None), None),
            reason: "ผิดชนิด".into(),
        },
    )
    .await;
    assert!(matches!(invalid_kind, Err(AppError::BadRequest(_))));
    let future = staff_career_service::append_staff_career_history(
        &pool,
        user,
        actor,
        CreateStaffCareerHistoryRequest {
            id: Uuid::new_v4(),
            entry: rank_input(None, Some("2999-01-01")),
        },
    )
    .await;
    assert!(matches!(future, Err(AppError::BadRequest(_))));
}

#[tokio::test]
async fn staff_career_history_query_plan() {
    let (pool, user, _) = career_fixture("career_query_plan").await;
    sqlx::query("INSERT INTO staff_career_history(user_id,kind,academic_rank,effective_date,source) SELECT $1,'academic_rank','none','2010-01-01'::date + n,'staff_entry' FROM generate_series(1,5000) n").bind(user).execute(&pool).await.unwrap();
    sqlx::query("ANALYZE staff_career_history")
        .execute(&pool)
        .await
        .unwrap();
    let sql = format!(
        "EXPLAIN (ANALYZE,BUFFERS,FORMAT JSON) {}",
        staff_career_service::history_read_sql()
    );
    let sqlx::types::Json(plan): sqlx::types::Json<serde_json::Value> =
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user)
            .bind(None::<Uuid>)
            .bind(21_i64)
            .fetch_one(&pool)
            .await
            .unwrap();
    println!("career query synthetic EXPLAIN: {}", plan);
    assert_eq!(history(&pool, user).await.items.len(), 20);
}
