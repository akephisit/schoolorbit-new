use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    AcademicPeriod, CreatePeriodRequest, PeriodQuery, ReorderPeriodsRequest, TimetablePeriod,
    UpdatePeriodRequest,
};
use chrono::NaiveTime;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_periods(
    pool: &PgPool,
    query: PeriodQuery,
) -> Result<Vec<AcademicPeriod>, AppError> {
    let mut sql = String::from("SELECT * FROM academic_periods WHERE 1=1");
    let mut idx = 0u32;

    if query.academic_year_id.is_some() {
        idx += 1;
        sql.push_str(&format!(" AND academic_year_id = ${idx}"));
    }

    if query.active_only.unwrap_or(false) {
        sql.push_str(" AND is_active = true");
    }

    sql.push_str(" ORDER BY order_index ASC");

    let mut q = sqlx::query_as::<_, AcademicPeriod>(&sql);
    if let Some(year_id) = query.academic_year_id {
        q = q.bind(year_id);
    }
    q.fetch_all(pool).await.map_err(|e| {
        tracing::error!("Failed to fetch periods: {}", e);
        AppError::InternalServerError("Failed to fetch periods".to_string())
    })
}

pub async fn list_active_periods_for_semester(
    pool: &PgPool,
    academic_semester_id: Uuid,
) -> Result<Vec<TimetablePeriod>, AppError> {
    sqlx::query_as::<_, TimetablePeriod>(
        r#"SELECT period.id,
                  period.name,
                  period.start_time,
                  period.end_time,
                  period.order_index
           FROM academic_periods period
           JOIN academic_semesters semester
             ON semester.academic_year_id = period.academic_year_id
           WHERE semester.id = $1
             AND period.is_active = true
           ORDER BY period.order_index, period.id"#,
    )
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to fetch timetable periods: {}", error);
        AppError::InternalServerError("Failed to fetch timetable periods".to_string())
    })
}

pub async fn create_period(
    pool: &PgPool,
    payload: CreatePeriodRequest,
) -> Result<AcademicPeriod, AppError> {
    let start_time = parse_period_time(&payload.start_time)?;
    let end_time = parse_period_time(&payload.end_time)?;
    validate_period_time_range(start_time, end_time)?;

    // Auto-assign order_index = MAX + 1 ถ้าไม่ส่งมา
    let order_index = match payload.order_index {
        Some(idx) => idx,
        None => {
            let next: Option<i32> = sqlx::query_scalar(
                "SELECT COALESCE(MAX(order_index), 0) + 1 FROM academic_periods WHERE academic_year_id = $1",
            )
            .bind(payload.academic_year_id)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to compute next order_index: {}", e)))?;
            next.unwrap_or(1)
        }
    };

    sqlx::query_as::<_, AcademicPeriod>(
        r#"
        INSERT INTO academic_periods (
            academic_year_id, name, start_time, end_time, order_index, applicable_days
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(payload.academic_year_id)
    .bind(normalized_period_name(payload.name))
    .bind(start_time)
    .bind(end_time)
    .bind(order_index)
    .bind(payload.applicable_days)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create period: {}", e);
        let msg = if e.to_string().contains("valid_time_range") {
            "เวลาจบต้องมากกว่าเวลาเริ่ม"
        } else if e.to_string().contains("unique_period_per_year") {
            "ลำดับคาบซ้ำกับที่มีอยู่แล้ว"
        } else {
            "ไม่สามารถสร้างคาบเรียนได้"
        };
        AppError::BadRequest(msg.to_string())
    })
}

pub async fn update_period(
    pool: &PgPool,
    id: Uuid,
    payload: UpdatePeriodRequest,
) -> Result<AcademicPeriod, AppError> {
    let start_time = if let Some(ref st) = payload.start_time {
        Some(parse_period_time(st)?)
    } else {
        None
    };

    let end_time = if let Some(ref et) = payload.end_time {
        Some(parse_period_time(et)?)
    } else {
        None
    };

    // name: ถ้า field ไม่ส่งมา → คงเดิม; ถ้าส่ง "" → clear เป็น NULL; ส่งค่า → set
    // ใช้ flag separate เพราะ COALESCE แยก "ไม่ส่ง" กับ "ส่ง NULL" ไม่ได้
    let name_set = payload.name.is_some();
    let name_value = normalized_period_name(payload.name);

    sqlx::query_as::<_, AcademicPeriod>(
        r#"
        UPDATE academic_periods SET
            name = CASE WHEN $2 THEN $3 ELSE name END,
            start_time = COALESCE($4, start_time),
            end_time = COALESCE($5, end_time),
            order_index = COALESCE($6, order_index),
            applicable_days = COALESCE($7, applicable_days),
            is_active = COALESCE($8, is_active),
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(name_set)
    .bind(name_value)
    .bind(start_time)
    .bind(end_time)
    .bind(payload.order_index)
    .bind(payload.applicable_days)
    .bind(payload.is_active)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::NotFound("Period not found".to_string()))
}

pub async fn delete_period(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM academic_periods WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("foreign key constraint") {
                AppError::BadRequest("Cannot delete period that is used in timetable".to_string())
            } else {
                AppError::InternalServerError("Failed to delete period".to_string())
            }
        })?;
    Ok(())
}

fn period_reorder_arrays(payload: &ReorderPeriodsRequest) -> (Vec<Uuid>, Vec<i32>) {
    (
        payload.items.iter().map(|item| item.id).collect(),
        payload.items.iter().map(|item| item.order_index).collect(),
    )
}

async fn bulk_update_period_order(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    academic_year_id: Uuid,
    ids: &[Uuid],
    order_indexes: &[i32],
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        r#"UPDATE academic_periods AS period
           SET order_index = updates.order_index, updated_at = NOW()
           FROM UNNEST($1::uuid[], $2::int4[]) AS updates(id, order_index)
           WHERE period.id = updates.id AND period.academic_year_id = $3"#,
    )
    .bind(ids)
    .bind(order_indexes)
    .bind(academic_year_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to update periods: {}", e)))?;

    Ok(())
}

/// Batch update order_index หลายแถวใน transaction เดียว
/// ใช้ SET CONSTRAINTS DEFERRED เพื่อเลี่ยง unique constraint ชนระหว่าง update
pub async fn reorder_periods(
    pool: &PgPool,
    payload: ReorderPeriodsRequest,
) -> Result<usize, AppError> {
    if payload.items.is_empty() {
        return Ok(0);
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Transaction failed: {}", e)))?;

    sqlx::query("SET CONSTRAINTS unique_period_per_year DEFERRED")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to defer constraint: {}", e)))?;

    let (ids, order_indexes) = period_reorder_arrays(&payload);
    bulk_update_period_order(&mut tx, payload.academic_year_id, &ids, &order_indexes).await?;

    tx.commit().await.map_err(|e| {
        let msg = if e.to_string().contains("unique_period_per_year") {
            "ลำดับคาบซ้ำกัน — ตรวจสอบ payload".to_string()
        } else {
            format!("Failed to commit reorder: {}", e)
        };
        AppError::BadRequest(msg)
    })?;

    Ok(payload.items.len())
}

fn parse_period_time(value: &str) -> Result<NaiveTime, AppError> {
    let is_hour_minute = value.len() == 5
        && value.as_bytes()[2] == b':'
        && value
            .chars()
            .enumerate()
            .all(|(index, char)| index == 2 || char.is_ascii_digit());
    if !is_hour_minute {
        return Err(AppError::BadRequest(
            "Invalid time format (use HH:MM)".to_string(),
        ));
    }

    NaiveTime::parse_from_str(value, "%H:%M")
        .map_err(|_| AppError::BadRequest("Invalid time format (use HH:MM)".to_string()))
}

fn validate_period_time_range(start_time: NaiveTime, end_time: NaiveTime) -> Result<(), AppError> {
    if end_time <= start_time {
        return Err(AppError::BadRequest("เวลาจบต้องมากกว่าเวลาเริ่ม".to_string()));
    }
    Ok(())
}

fn normalized_period_name(name: Option<String>) -> Option<String> {
    name.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI32, Ordering};

    use crate::test_helpers::{create_test_pool, run_test_migrations};

    use super::*;

    static NEXT_TIMETABLE_GRID_YEAR: AtomicI32 = AtomicI32::new(50_000);

    async fn migrated_pool() -> PgPool {
        let pool = create_test_pool().await;
        run_test_migrations(&pool).await;
        pool
    }

    #[tokio::test]
    async fn list_active_periods_for_semester_returns_configured_slots_without_entries() {
        let pool = migrated_pool().await;
        let year = NEXT_TIMETABLE_GRID_YEAR.fetch_add(2, Ordering::Relaxed);
        let academic_year_id = Uuid::new_v4();
        let other_academic_year_id = Uuid::new_v4();
        let semester_id = Uuid::new_v4();

        for (id, year_value, name) in [
            (academic_year_id, year, "Grid Year"),
            (other_academic_year_id, year + 1, "Other Grid Year"),
        ] {
            sqlx::query(
                "INSERT INTO academic_years (id, year, name, start_date, end_date)
                 VALUES ($1, $2, $3, '9600-01-01', '9600-12-31')",
            )
            .bind(id)
            .bind(year_value)
            .bind(name)
            .execute(&pool)
            .await
            .expect("academic year fixture should insert");
        }

        sqlx::query(
            "INSERT INTO academic_semesters
                (id, academic_year_id, term, name, start_date, end_date)
             VALUES ($1, $2, '1', 'Grid Semester', '9600-01-01', '9600-06-30')",
        )
        .bind(semester_id)
        .bind(academic_year_id)
        .execute(&pool)
        .await
        .expect("semester fixture should insert");

        let first_period_id = Uuid::new_v4();
        let inactive_period_id = Uuid::new_v4();
        let third_period_id = Uuid::new_v4();
        let other_year_period_id = Uuid::new_v4();
        for (id, year_id, name, order_index, is_active) in [
            (third_period_id, academic_year_id, "คาบ 3", 3, true),
            (inactive_period_id, academic_year_id, "คาบ 2", 2, false),
            (first_period_id, academic_year_id, "คาบ 1", 1, true),
            (
                other_year_period_id,
                other_academic_year_id,
                "คาบต่างปี",
                1,
                true,
            ),
        ] {
            sqlx::query(
                "INSERT INTO academic_periods
                    (id, academic_year_id, name, start_time, end_time, order_index, is_active)
                 VALUES ($1, $2, $3, '08:00'::time, '08:50'::time, $4, $5)",
            )
            .bind(id)
            .bind(year_id)
            .bind(name)
            .bind(order_index)
            .bind(is_active)
            .execute(&pool)
            .await
            .expect("period fixture should insert");
        }

        let periods = list_active_periods_for_semester(&pool, semester_id)
            .await
            .expect("configured timetable periods should load");

        assert_eq!(
            periods.iter().map(|period| period.id).collect::<Vec<_>>(),
            vec![first_period_id, third_period_id]
        );
        assert_eq!(
            periods
                .iter()
                .map(|period| period.name.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("คาบ 1"), Some("คาบ 3")]
        );
    }

    #[test]
    fn parse_period_time_accepts_hour_minute_values() {
        assert_eq!(
            parse_period_time("08:30").unwrap(),
            NaiveTime::from_hms_opt(8, 30, 0).unwrap()
        );
    }

    #[test]
    fn parse_period_time_rejects_invalid_values() {
        assert!(matches!(
            parse_period_time("8:30"),
            Err(AppError::BadRequest(message)) if message.contains("HH:MM")
        ));
    }

    #[test]
    fn validate_period_time_range_requires_end_after_start() {
        let start = NaiveTime::from_hms_opt(8, 30, 0).unwrap();
        let end = NaiveTime::from_hms_opt(8, 30, 0).unwrap();

        assert!(matches!(
            validate_period_time_range(start, end),
            Err(AppError::BadRequest(message)) if message == "เวลาจบต้องมากกว่าเวลาเริ่ม"
        ));
    }

    #[test]
    fn normalized_period_name_treats_blank_names_as_none() {
        assert_eq!(normalized_period_name(Some("  ".to_string())), None);
        assert_eq!(
            normalized_period_name(Some("คาบ 1".to_string())),
            Some("คาบ 1".to_string())
        );
        assert_eq!(normalized_period_name(None), None);
    }

    #[test]
    fn period_reorder_arrays_preserve_payload_order() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let payload = ReorderPeriodsRequest {
            academic_year_id: Uuid::new_v4(),
            items: vec![
                crate::modules::academic::models::timetable::ReorderPeriodItem {
                    id: first,
                    order_index: 2,
                },
                crate::modules::academic::models::timetable::ReorderPeriodItem {
                    id: second,
                    order_index: 1,
                },
            ],
        };

        let (ids, order_indexes) = period_reorder_arrays(&payload);

        assert_eq!(ids, vec![first, second]);
        assert_eq!(order_indexes, vec![2, 1]);
    }
}
