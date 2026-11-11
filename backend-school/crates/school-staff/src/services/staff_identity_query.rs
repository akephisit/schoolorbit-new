use crate::career::StaffPersonnelType;
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// Pool entry for consumers without an existing transaction.
pub async fn active_staff_display_name(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<String>, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "staff.file_transfer.staff_display_name",
    )
    .await?;
    active_staff_display_name_on_connection(&mut connection, user_id).await
}

/// Current active Staff name, read on the caller's existing connection or transaction.
pub async fn active_staff_display_name_on_connection(
    connection: &mut PgConnection,
    user_id: Uuid,
) -> Result<Option<String>, AppError> {
    Ok(DbSession::new(connection)
        .scalar(
            "staff.services.staff_identity_query.active_staff_display_name_on_connection",
            "SELECT display_name FROM user_display_names WHERE id=$1 AND user_type='staff' AND status='active'",
        )
        .bind(user_id)
        .fetch_optional()
        .await?)
}

/// Effective career classification at the consumer's explicit civil date.
pub async fn effective_personnel_type_on_connection(
    connection: &mut PgConnection,
    user_id: Uuid,
    date: NaiveDate,
) -> Result<Option<StaffPersonnelType>, AppError> {
    let row: Option<Option<String>> = DbSession::new(connection)
        .scalar(
            "staff.services.staff_identity_query.effective_personnel_type_on_connection",
            "SELECT personnel_type FROM staff_career_history WHERE user_id=$1 AND kind='personnel_type' AND (effective_date<=$2 OR effective_date IS NULL) ORDER BY effective_date DESC NULLS LAST,created_at DESC,id DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(date)
        .fetch_optional()
        .await?;
    row.flatten().map(parse_personnel_type).transpose()
}

fn parse_personnel_type(value: String) -> Result<StaffPersonnelType, AppError> {
    value
        .parse()
        .map_err(|_| AppError::ValidationError("ประเภทบุคลากรไม่ถูกต้อง".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_persisted_personnel_type_keeps_the_original_validation_error() {
        assert!(matches!(
            parse_personnel_type("unknown".into()),
            Err(AppError::ValidationError(message)) if message == "ประเภทบุคลากรไม่ถูกต้อง"
        ));
    }

    #[tokio::test]
    async fn staff_identity_providers_share_uncommitted_caller_state_and_rollback() {
        let pool = school_test_db::create_named_test_pool_with_max_connections(
            "staff_identity_transaction",
            1,
        )
        .await;
        school_test_db::run_test_migrations(&pool).await;
        let user = Uuid::now_v7();
        let date: NaiveDate = "2026-11-11".parse().unwrap();
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','Identity','Fixture','staff','active')")
            .bind(user)
            .bind(user.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO staff_career_history(user_id,kind,personnel_type,effective_date,source) VALUES($1,'personnel_type','civil_servant',$2,'staff_entry')")
            .bind(user)
            .bind(date)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(
            active_staff_display_name_on_connection(&mut tx, user)
                .await
                .unwrap(),
            Some("Identity Fixture".into())
        );
        assert_eq!(
            effective_personnel_type_on_connection(&mut tx, user, date)
                .await
                .unwrap(),
            Some(StaffPersonnelType::CivilServant)
        );
        tx.rollback().await.unwrap();
        let mut connection = pool.acquire().await.unwrap();
        assert_eq!(
            active_staff_display_name_on_connection(&mut connection, user)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            effective_personnel_type_on_connection(&mut connection, user, date)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn staff_identity_providers_preserve_eligibility_and_effective_career_order() {
        let pool = school_test_db::create_named_test_pool("staff_identity_effective").await;
        school_test_db::run_test_migrations(&pool).await;
        let user = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,'synthetic','Effective','Fixture','staff','active')")
            .bind(user)
            .bind(user.to_string())
            .execute(&pool)
            .await
            .unwrap();
        let mut connection = pool.acquire().await.unwrap();
        assert_eq!(
            active_staff_display_name_on_connection(&mut connection, Uuid::now_v7())
                .await
                .unwrap(),
            None
        );
        for (user_type, status, eligible) in [
            ("staff", "active", true),
            ("staff", "inactive", false),
            ("student", "active", false),
        ] {
            sqlx::query("UPDATE users SET user_type=$2,status=$3 WHERE id=$1")
                .bind(user)
                .bind(user_type)
                .bind(status)
                .execute(&mut *connection)
                .await
                .unwrap();
            assert_eq!(
                active_staff_display_name_on_connection(&mut connection, user)
                    .await
                    .unwrap()
                    .is_some(),
                eligible
            );
            drop(connection);
            assert_eq!(
                active_staff_display_name(&pool, user)
                    .await
                    .unwrap()
                    .is_some(),
                eligible
            );
            connection = pool.acquire().await.unwrap();
        }
        sqlx::query("UPDATE users SET user_type='staff',status='active' WHERE id=$1")
            .bind(user)
            .execute(&mut *connection)
            .await
            .unwrap();
        for (id, personnel_type, effective_date, created_at) in [
            (1_u128, Some("civil_servant"), None, "2026-01-04T00:00:00Z"),
            (2, Some("government_employee"), Some("2026-01-01"), "2026-01-01T00:00:00Z"),
            (3, Some("contract_employee"), Some("2026-01-01"), "2026-01-02T00:00:00Z"),
            (4, Some("permanent_employee"), Some("2026-01-01"), "2026-01-02T00:00:00Z"),
            (5, None, Some("2026-02-01"), "2026-01-03T00:00:00Z"),
            (6, Some("other"), Some("2026-12-01"), "2026-01-03T00:00:00Z"),
        ] {
            sqlx::query("INSERT INTO staff_career_history(id,user_id,kind,personnel_type,effective_date,created_at,source) VALUES($1,$2,'personnel_type',$3,$4,$5,'staff_entry')")
                .bind(Uuid::from_u128(id))
                .bind(user)
                .bind(personnel_type)
                .bind(effective_date.map(|value| value.parse::<NaiveDate>().unwrap()))
                .bind(created_at.parse::<chrono::DateTime<chrono::Utc>>().unwrap())
                .execute(&mut *connection)
                .await
                .unwrap();
        }
        for (date, expected) in [
            ("2025-12-31", Some(StaffPersonnelType::CivilServant)),
            ("2026-01-01", Some(StaffPersonnelType::PermanentEmployee)),
            ("2026-11-11", None),
            ("2026-12-01", Some(StaffPersonnelType::Other)),
        ] {
            assert_eq!(
                effective_personnel_type_on_connection(&mut connection, user, date.parse().unwrap())
                    .await
                    .unwrap(),
                expected
            );
        }
    }
}
