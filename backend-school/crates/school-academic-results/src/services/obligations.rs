use super::ResultContext;
use chrono::{DateTime, NaiveDate, Utc};
use school_academic_assessment::ports::CURRENT_OBLIGATION_GROUP_SCAN_LIMIT;
use school_academic_delivery::services::{audiences, roster_memberships};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

pub(super) fn captured_day() -> Result<NaiveDate, AppError> {
    bangkok_day(Utc::now())
}
fn bangkok_day(now: DateTime<Utc>) -> Result<NaiveDate, AppError> {
    now.checked_add_signed(chrono::Duration::hours(7))
        .map(|date| date.date_naive())
        .ok_or_else(|| {
            AppError::InternalServerError("Could not resolve Bangkok operation date".into())
        })
}
pub(super) async fn operation_date(
    connection: &mut PgConnection,
    context: &ResultContext,
    captured: NaiveDate,
) -> Result<NaiveDate, AppError> {
    let belongs: bool = DbSession::new(connection)
        .scalar(
            "academic_results.services.obligations.operation_date",
            "SELECT EXISTS(SELECT 1 FROM academic_terms WHERE id=$1 AND academic_year_id=$2)",
        )
        .bind(context.academic_term_id)
        .bind(context.academic_year_id)
        .fetch_one()
        .await?;
    if !belongs {
        return Err(AppError::ValidationError(
            "Academic term does not belong to selected year".into(),
        ));
    }
    Ok(captured)
}

pub(super) fn require_scan_budget(count: usize) -> Result<(), AppError> {
    if count > CURRENT_OBLIGATION_GROUP_SCAN_LIMIT {
        return Err(AppError::ValidationError(format!(
            "Current obligation scan exceeds {CURRENT_OBLIGATION_GROUP_SCAN_LIMIT} groups"
        )));
    }
    Ok(())
}
fn retain_candidate(explicitly_omitted: bool, established: bool) -> bool {
    !explicitly_omitted || established
}

/// Current selection only: omission cannot erase established learner obligations.
/// Unknown/mismatched candidates fail closed; returned IDs are sorted and unique.
pub async fn current_obligation_group_ids_on_connection(
    connection: &mut PgConnection,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    date: NaiveDate,
    candidate_group_ids: &[Uuid],
) -> Result<Vec<Uuid>, AppError> {
    if candidate_group_ids.is_empty() {
        return Ok(Vec::new());
    }
    let candidates: Vec<_> = candidate_group_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    require_scan_budget(candidates.len())?;
    let valid:Vec<Uuid>=DbSession::new(&mut *connection).scalar("academic_results.services.obligations.current_obligation_group_ids_on_connection",
        "SELECT g.id FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id WHERE g.id=ANY($1) AND g.academic_term_id=$2 AND g.academic_year_id=$3 AND o.academic_term_id=$2 AND o.academic_year_id=$3 ORDER BY g.id")
        .bind(&candidates).bind(academic_term_id).bind(academic_year_id).fetch_all().await?;
    if valid != candidates {
        return Err(AppError::Conflict(
            "Current obligation groups do not match the academic context".into(),
        ));
    }
    let scopes =
        audiences::dated_group_constraints_on_connection(connection, &candidates, date).await?;
    let omitted: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|id| {
            scopes
                .constraint_for_group(*id)
                .is_some_and(|scope| !scope.allows_group(*id))
        })
        .collect();
    if omitted.is_empty() {
        return Ok(candidates);
    }
    let mut history = roster_memberships::groups_with_established_history_on_connection(
        connection,
        academic_term_id,
        academic_year_id,
        &omitted,
    )
    .await?;
    history.extend(
        school_academic_assessment::evidence::groups_with_established_history_on_connection(
            connection,
            academic_term_id,
            academic_year_id,
            &omitted,
        )
        .await?,
    );
    history.extend(result_history(connection, academic_term_id, academic_year_id, &omitted).await?);
    let omitted: HashSet<_> = omitted.into_iter().collect();
    Ok(candidates
        .into_iter()
        .filter(|id| retain_candidate(omitted.contains(id), history.contains(id)))
        .collect())
}

async fn result_history(
    connection: &mut PgConnection,
    term: Uuid,
    year: Uuid,
    groups: &[Uuid],
) -> Result<HashSet<Uuid>, AppError> {
    let rows:Vec<Uuid>=DbSession::new(connection).scalar("academic_results.services.obligations.result_history",r#"
        SELECT g.id FROM learning_groups g
        LEFT JOIN course_offering_details course ON course.learning_offering_id=g.learning_offering_id
        WHERE g.id=ANY($1) AND g.academic_term_id=$2 AND g.academic_year_id=$3 AND (
            EXISTS(SELECT 1 FROM learning_group_result_overrides value WHERE value.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM learning_group_result_confirmations value WHERE value.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM academic_course_results value WHERE value.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM academic_activity_evaluations value WHERE value.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM academic_activity_result_confirmations value WHERE value.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM academic_activity_result_locks value WHERE value.learning_group_id=g.id)
            OR EXISTS(SELECT 1 FROM academic_activity_results value WHERE value.learning_group_id=g.id)
            -- Subject locks remain conservative even for empty/legacy JSON source groups.
            -- FK-preserved result rows also retain every correction referring to them.
            OR EXISTS(SELECT 1 FROM academic_course_result_locks value WHERE value.subject_id=course.subject_id AND value.academic_term_id=$2 AND value.academic_year_id=$3)
        ) ORDER BY g.id
    "#).bind(groups).bind(term).bind(year).fetch_all().await?;
    Ok(rows.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn operation_date_retains_captured_day_and_requires_exact_context() {
        let pool = school_test_db::create_named_test_pool("results_operation_date_context").await;
        let mut connection = pool.acquire().await.unwrap();
        sqlx::raw_sql("CREATE TEMP TABLE academic_terms(id uuid PRIMARY KEY,academic_year_id uuid NOT NULL)")
            .execute(&mut *connection).await.unwrap();
        let mut context = ResultContext { academic_year_id: Uuid::new_v4(), academic_term_id: Uuid::new_v4() };
        sqlx::query("INSERT INTO academic_terms VALUES($1,$2)").bind(context.academic_term_id).bind(context.academic_year_id).execute(&mut *connection).await.unwrap();
        for captured in [NaiveDate::from_ymd_opt(2025,1,1).unwrap(),NaiveDate::from_ymd_opt(2030,1,1).unwrap()] {
            assert_eq!(operation_date(&mut connection,&context,captured).await.unwrap(),captured);
        }
        context.academic_year_id=Uuid::new_v4();
        assert!(matches!(operation_date(&mut connection,&context,NaiveDate::from_ymd_opt(2025,1,1).unwrap()).await,Err(AppError::ValidationError(_))));
    }

    #[test]
    fn omission_excludes_only_unused_explicit_groups() {
        assert!(retain_candidate(false, false));
        assert!(retain_candidate(false, true));
        assert!(retain_candidate(true, true));
        assert!(!retain_candidate(true, false));
    }
    #[test]
    fn request_budget_refuses_overflow_instead_of_truncating() {
        assert!(require_scan_budget(CURRENT_OBLIGATION_GROUP_SCAN_LIMIT).is_ok());
        assert!(require_scan_budget(CURRENT_OBLIGATION_GROUP_SCAN_LIMIT + 1).is_err());
    }
    #[test]
    fn bangkok_midnight_uses_one_captured_day() {
        let before = DateTime::parse_from_rfc3339("2026-10-28T16:59:59Z")
            .unwrap()
            .with_timezone(&Utc);
        let after = DateTime::parse_from_rfc3339("2026-10-28T17:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            bangkok_day(before).unwrap(),
            NaiveDate::from_ymd_opt(2026, 10, 28).unwrap()
        );
        assert_eq!(
            bangkok_day(after).unwrap(),
            NaiveDate::from_ymd_opt(2026, 10, 29).unwrap()
        );

    }
}
