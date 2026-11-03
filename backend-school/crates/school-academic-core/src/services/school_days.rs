//! Core owns weekday presets and the interpretation of nullable bell-period days.
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

pub const ALL: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];

pub fn normalize(days: &[String]) -> Result<Vec<String>, AppError> {
    let normalized = ALL
        .iter()
        .filter(|supported| {
            days.iter()
                .any(|day| day.trim().eq_ignore_ascii_case(supported))
        })
        .map(|day| (*day).to_owned())
        .collect::<Vec<_>>();
    if normalized.is_empty() || normalized.len() != days.len() {
        return Err(AppError::ValidationError(
            "ต้องกำหนดวันเรียนที่ถูกต้องอย่างน้อยหนึ่งวันและห้ามซ้ำ".into(),
        ));
    }
    Ok(normalized)
}

/// Stored data must already contain canonical codes; malformed legacy values are
/// evidence for correction, never an empty set or an all-days fallback.
pub fn parse_stored(days: &str) -> Result<Vec<String>, AppError> {
    let values = days.split(',').map(str::to_owned).collect::<Vec<_>>();
    let normalized = normalize(&values)?;
    if values.iter().any(|day| !ALL.contains(&day.as_str())) {
        return Err(AppError::ValidationError(
            "รูปแบบวันเรียนที่บันทึกไว้ไม่ถูกต้อง".into(),
        ));
    }
    Ok(normalized)
}

pub async fn for_term_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<Vec<String>, AppError> {
    let stored: String = DbSession::new(connection)
        .scalar(
            "academic_core.services.school_days.for_term",
            "SELECT school_days FROM academic_terms WHERE id=$1",
        )
        .bind(term)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    parse_stored(&stored)
}

/// NULL is the historical wildcard: all school days of this exact term.
pub fn effective_period_days(
    stored: Option<&str>,
    term_days: &[String],
) -> Result<Vec<String>, AppError> {
    let term_days = normalize(term_days)?;
    let days = stored
        .map(parse_stored)
        .transpose()?
        .unwrap_or_else(|| term_days.clone());
    if days.iter().any(|day| !term_days.contains(day)) {
        return Err(AppError::ValidationError(
            "วันที่ใช้คาบเรียนต้องเป็นวันเรียนของภาคเรียน".into(),
        ));
    }
    Ok(days)
}

pub fn iso_weekdays(days: &[String]) -> Result<Vec<u32>, AppError> {
    Ok(normalize(days)?
        .iter()
        .filter_map(|day| {
            ALL.iter()
                .position(|value| value == day)
                .map(|index| index as u32 + 1)
        })
        .collect())
}

/// Exact-context eligibility for Timetable preview and writes. The caller holds
/// its normal term guard; this operation never mutates or infers a context.
pub async fn period_allows_day_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    schedule: Uuid,
    period: Uuid,
    day: &str,
) -> Result<bool, AppError> {
    let row: Option<(String, Option<Uuid>, Option<String>)> = DbSession::new(connection)
        .query_as("academic_core.services.school_days.period_allows_day",
            "SELECT term.school_days,period.id,period.applicable_days FROM academic_terms term LEFT JOIN bell_schedule_periods period ON period.id=$3 AND period.bell_schedule_id=$2 AND period.is_active WHERE term.id=$1")
        .bind(term).bind(schedule).bind(period).fetch_optional().await?;
    let (term_days, period_id, period_days) =
        row.ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    if period_id.is_none() {
        return Ok(false);
    }
    let term_days = parse_stored(&term_days)?;
    Ok(effective_period_days(period_days.as_deref(), &term_days)
        .is_ok_and(|days| days.iter().any(|value| value == day)))
}

pub(crate) async fn editable_term_ids(
    connection: &mut PgConnection,
) -> Result<Vec<Uuid>, AppError> {
    Ok(DbSession::new(connection).scalar("academic_core.services.school_days.editable_term_ids",
        "SELECT term.id FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id WHERE year.status IN ('planning','ready','active') AND term.status IN ('planning','ready') AND NOT EXISTS(SELECT 1 FROM academic_term_transition_receipts receipt WHERE receipt.academic_term_id=term.id AND receipt.action='activate') ORDER BY year.year,term.sequence_no,term.id")
        .fetch_all().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wildcard_uses_only_exact_term_days_and_malformed_is_not_wildcard() {
        let weekdays = vec!["MON".into(), "TUE".into()];
        assert_eq!(effective_period_days(None, &weekdays).unwrap(), weekdays);
        for malformed in ["", "{MON,TUE}", " MON ", "MON,MON", "SAT"] {
            assert!(effective_period_days(Some(malformed), &weekdays).is_err());
        }
        assert_eq!(
            iso_weekdays(&["SAT".into(), "MON".into()]).unwrap(),
            vec![1, 6]
        );
    }
}
