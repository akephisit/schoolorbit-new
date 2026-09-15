use super::year_transitions;
use crate::models::{
    LifecycleFinding, LifecycleSeverity, YearLifecycleWorkspace, YearTransitionAction as Action,
};
use school_academic_core::models::AcademicTermStatus as Term;
use school_academic_results::{self as results, policy::aggregate};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
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
    let annual_url = aggregate::require_aggregate_read(actor)
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
