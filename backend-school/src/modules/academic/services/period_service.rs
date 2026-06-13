use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    AcademicPeriod, CreatePeriodRequest, PeriodQuery, ReorderPeriodsRequest, UpdatePeriodRequest,
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

    for item in &payload.items {
        sqlx::query(
            "UPDATE academic_periods SET order_index = $1 WHERE id = $2 AND academic_year_id = $3",
        )
        .bind(item.order_index)
        .bind(item.id)
        .bind(payload.academic_year_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update period: {}", e)))?;
    }

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
    use super::*;

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
}
