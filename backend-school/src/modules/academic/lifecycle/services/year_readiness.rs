use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::{
        core::{
            models::{AcademicTermStatus as Term, YearTransitionAction as Action},
            services::year_transitions,
        },
        lifecycle::models::{LifecycleFinding, LifecycleSeverity, YearLifecycleWorkspace},
        results,
    },
    permissions::registry::codes,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn get_year_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
) -> Result<YearLifecycleWorkspace, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let workspace = year_workspace_in_transaction(&mut tx, actor, year).await?;
    tx.commit().await?;
    Ok(workspace)
}

pub(crate) async fn year_workspace_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    year: Uuid,
) -> Result<YearLifecycleWorkspace, AppError> {
    let context = year_transitions::read_context(tx, year).await?;
    let terms = year_transitions::read_terms(tx, year).await?;
    let coverage = results::services::annual_closure_coverage(tx, year).await?;
    let annual_url =
        crate::policies::academic_aggregate_access_policy::require_aggregate_read(actor)
            .is_ok()
            .then(|| format!("/staff/academic/results/annual?academicYearId={year}"));
    let mut findings = vec![];
    if !coverage.ready {
        findings.push(LifecycleFinding {
            code: "year.annual_missing".into(),
            severity: LifecycleSeverity::Blocking,
            count: coverage
                .students
                .iter()
                .filter(|row| !row.is_current)
                .count(),
            message: if coverage.students.is_empty() {
                "ยังไม่มีนักเรียนที่ใช้ตรวจผลสรุปรายปี"
            } else {
                "ผลสรุปรายปียังไม่ยืนยัน ไม่ครบ หรือข้อมูลต้นทางเปลี่ยนแปลง"
            }
            .into(),
            resolution_url: annual_url.clone(),
        });
    }
    let holds = coverage
        .students
        .iter()
        .filter(|row| row.is_current && row.hold_reason.is_some())
        .count();
    if holds > 0 {
        findings.push(LifecycleFinding {
            code: "year.annual_holds".into(),
            severity: LifecycleSeverity::Warning,
            count: holds,
            message: "มีผลค้างรายปีที่ตรวจและระบุเหตุผลแล้ว ต้องติดตามต่อหลังปิดปี".into(),
            resolution_url: annual_url,
        });
    }
    let open = terms
        .iter()
        .filter(|term| {
            matches!(term.status, Term::Active | Term::Closing)
                || (term.blocks_year_closure
                    && !matches!(term.status, Term::Closed | Term::Cancelled))
        })
        .count();
    if open > 0 || terms.is_empty() {
        findings.push(LifecycleFinding {
            code: "year.terms_open".into(),
            severity: LifecycleSeverity::Blocking,
            count: open,
            message: if terms.is_empty() {
                "ยังไม่มีภาคเรียนในปีการศึกษานี้"
            } else {
                "ยังมีภาคเรียนที่ต้องปิดก่อนปิดปีการศึกษา"
            }
            .into(),
            resolution_url: None,
        });
    }
    let optional = terms
        .iter()
        .filter(|term| {
            !term.blocks_year_closure && matches!(term.status, Term::Planning | Term::Ready)
        })
        .count();
    if optional > 0 {
        findings.push(LifecycleFinding {
            code: "year.optional_terms".into(),
            severity: LifecycleSeverity::Warning,
            count: optional,
            message: "มีภาคเรียนไม่บังคับที่ยังไม่เริ่มใช้งาน ปิดปีแล้วจะเก็บรายการไว้โดยไม่เปิดใช้งาน".into(),
            resolution_url: None,
        });
    }
    findings.sort_by(|a, b| a.code.cmp(&b.code));
    let can_close = coverage.ready
        && !findings
            .iter()
            .any(|row| row.severity == LifecycleSeverity::Blocking);
    // Permission-filtered links and actions are presentation, not source state.
    let finding_state: Vec<_> = findings
        .iter()
        .map(|row| (&row.code, row.severity, row.count))
        .collect();
    let source_checksum =
        super::checksum(&(&context, &terms, &coverage.source_checksum, finding_state))?;
    let available_actions = [Action::BeginClosing, Action::CancelClosing, Action::Close]
        .into_iter()
        .filter(|action| {
            actor.has_permission(year_transitions::action_permission(*action))
                && year_transitions::target_state(context.status, *action).is_ok()
        })
        .collect();
    Ok(YearLifecycleWorkspace {
        context,
        terms,
        coverage,
        findings,
        available_actions,
        can_close,
        source_checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::academic::{core, cutover_test_support::apply_migrations_through},
        permissions::registry::codes,
    };

    #[tokio::test]
    async fn year_lifecycle_readiness_distinguishes_missing_annual_results_and_optional_terms() {
        let pool = core::services_tests::prepare_core_fixture("year_lifecycle_readiness").await;
        apply_migrations_through(&pool, 70).await.unwrap();
        let (year, term): (Uuid, Uuid) =
            sqlx::query_as("SELECT academic_year_id,id FROM academic_terms WHERE status='active'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE user_type='staff' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let reader = ActorContext {
            user_id: id,
            permissions: vec![codes::ACADEMIC_LIFECYCLE_READ_SCHOOL.into()],
        };
        let ws = get_year_workspace(&pool, &reader, year).await.unwrap();
        let manager = ActorContext {
            user_id: id,
            permissions: vec![codes::WILDCARD.into()],
        };
        let privileged = get_year_workspace(&pool, &manager, year).await.unwrap();
        assert_eq!(ws.source_checksum, privileged.source_checksum);
        assert!(privileged
            .findings
            .iter()
            .filter(|row| row.code.starts_with("year.annual"))
            .all(|row| row.resolution_url.is_some()));
        assert!(!ws.can_close && !ws.coverage.ready && ws.available_actions.is_empty());
        assert!(ws
            .findings
            .iter()
            .any(|row| row.code == "year.annual_missing"));
        assert!(ws.findings.iter().any(|row| row.code == "year.terms_open"));
        assert!(ws
            .findings
            .iter()
            .filter(|row| row.code.starts_with("year.annual"))
            .all(|row| row.resolution_url.is_none()));
        let optional:Uuid=sqlx::query_scalar("SELECT id FROM academic_terms WHERE academic_year_id=$1 AND id<>$2 ORDER BY sequence_no LIMIT 1").bind(year).bind(term).fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE academic_terms SET status='closed',closed_on=start_date WHERE academic_year_id=$1").bind(year).execute(&pool).await.unwrap();
        sqlx::query("UPDATE academic_terms SET status='planning',closed_on=NULL,included_in_year_result=false,blocks_year_closure=false,term_type='summer' WHERE id=$1").bind(optional).execute(&pool).await.unwrap();
        let ws = get_year_workspace(&pool, &reader, year).await.unwrap();
        assert!(!ws.findings.iter().any(|row| row.code == "year.terms_open"));
        assert!(ws
            .findings
            .iter()
            .any(|row| row.code == "year.optional_terms"));
        sqlx::query("UPDATE academic_terms SET status='active' WHERE id=$1")
            .bind(optional)
            .execute(&pool)
            .await
            .unwrap();
        let ws = get_year_workspace(&pool, &reader, year).await.unwrap();
        assert!(ws.findings.iter().any(|row| row.code == "year.terms_open"));
        assert!(get_year_workspace(&pool, &reader, Uuid::new_v4())
            .await
            .is_err());
        let denied = ActorContext {
            user_id: id,
            permissions: vec![],
        };
        assert!(matches!(
            get_year_workspace(&pool, &denied, year).await,
            Err(AppError::Forbidden(_))
        ));
    }
}
