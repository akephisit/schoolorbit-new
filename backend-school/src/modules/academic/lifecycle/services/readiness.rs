use super::super::models::TermLifecycleWorkspace;
use super::super::models::{LifecycleFinding, LifecycleSeverity, PendingTermWork};
use crate::modules::academic::{
    core::{models::TermTransitionAction as Action, services::term_transitions},
    delivery, results,
    services::{exam_schedule_service, timetable_version_service},
};
use crate::{error::AppError, middleware::permission::ActorContext, permissions::registry::codes};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) async fn workspace_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
) -> Result<TermLifecycleWorkspace, AppError> {
    let context = term_transitions::read_context(tx, year, term).await?;
    let result_context = results::models::ResultContext {
        academic_year_id: year,
        academic_term_id: term,
    };
    let coverage = results::services::term_closure_coverage(tx, &result_context).await?;
    let delivery = delivery::services::pending_term_work(tx, year, term).await?;
    let timetable = timetable_version_service::pending_term_work(tx, year, term).await?;
    let exams = exam_schedule_service::pending_term_work(tx, year, term).await?;
    let supervision =
        crate::modules::supervision::services::pending_term_work(tx, year, term).await?;
    let link = |permission: &str, path: &str| {
        actor
            .has_permission(permission)
            .then(|| format!("{path}?academicYearId={year}&academicTermId={term}"))
    };
    let mut findings = Vec::new();
    let aggregate_url = crate::policies::academic_aggregate_access_policy::require_aggregate_read(
        actor,
    )
    .is_ok()
    .then(|| {
        format!("/staff/academic/results/aggregates?academicYearId={year}&academicTermId={term}")
    });
    if !coverage.ready {
        findings.push(LifecycleFinding {
            code: "results.incomplete".into(),
            severity: LifecycleSeverity::Blocking,
            count: coverage
                .students
                .iter()
                .filter(|student| !student.is_current)
                .count(),
            message: if coverage.students.is_empty() {
                "ยังไม่มีข้อมูลนักเรียนที่ใช้ตรวจผลสรุปภาคเรียน"
            } else {
                "ผลสรุปนักเรียนยังไม่ล็อก ไม่ครบ หรือมีข้อมูลต้นทางเปลี่ยนแปลง"
            }
            .into(),
            resolution_url: aggregate_url.clone(),
        });
    }
    let holds = coverage
        .students
        .iter()
        .filter(|student| student.hold_reason.is_some())
        .count();
    if holds > 0 {
        findings.push(LifecycleFinding {
            code: "results.reviewed_holds".into(),
            severity: LifecycleSeverity::Warning,
            count: holds,
            message: "มีผลค้างที่ฝ่ายวิชาการตรวจและระบุเหตุผลแล้ว ต้องติดตามต่อหลังปิดภาคเรียน".into(),
            resolution_url: aggregate_url,
        });
    }
    for (code, message, work, url) in [
        (
            "delivery",
            "มีร่างการปรับรายการเปิดสอนที่ยังไม่เสร็จ",
            &delivery,
            link(
                codes::LEARNING_OFFERING_READ_SCHOOL,
                "/staff/academic/delivery",
            ),
        ),
        (
            "timetable",
            "มีร่างตารางสอนที่ยังไม่เผยแพร่",
            &timetable,
            link(
                codes::ACADEMIC_TIMETABLE_READ_SCHOOL,
                "/staff/academic/timetable",
            ),
        ),
        (
            "exams",
            "มีรอบสอบร่างที่ยังไม่เผยแพร่",
            &exams,
            link(
                codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL,
                "/staff/academic/exam-schedules",
            ),
        ),
        (
            "supervision",
            "มีรายการนิเทศในภาคเรียนนี้ที่ยังไม่เสร็จ",
            &supervision,
            link(
                codes::SUPERVISION_READ_SCHOOL,
                "/staff/academic/supervision",
            ),
        ),
    ] {
        findings.extend(operational_findings(code, message, work, url));
    }
    findings.sort_by(|a, b| a.code.cmp(&b.code));
    let available_actions = [
        Action::MarkReady,
        Action::BeginClosing,
        Action::CancelClosing,
        Action::Close,
        Action::Reopen,
        Action::Cancel,
        Action::Activate,
    ]
    .into_iter()
    .filter(|action| {
        actor.has_permission(term_transitions::action_permission(*action))
            && term_transitions::target_state(context.term_status, context.year_status, *action)
                .is_ok()
    })
    .collect();
    let source_checksum = super::checksum(&(
        &context,
        &coverage.source_checksum,
        &delivery,
        &timetable,
        &exams,
        &supervision,
    ))?;
    Ok(TermLifecycleWorkspace {
        context,
        findings,
        coverage,
        available_actions,
        source_checksum,
    })
}

fn operational_findings(
    code: &str,
    message: &str,
    work: &[PendingTermWork],
    url: Option<String>,
) -> Vec<LifecycleFinding> {
    [
        (true, LifecycleSeverity::Blocking, "blocking"),
        (false, LifecycleSeverity::Warning, "warning"),
    ]
    .into_iter()
    .filter_map(|(blocking, severity, suffix)| {
        let count = work
            .iter()
            .filter(|item| item.blocks_closure == blocking)
            .count();
        (count > 0).then(|| LifecycleFinding {
            code: format!("{code}.{suffix}"),
            severity,
            count,
            message: message.into(),
            resolution_url: url.clone(),
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn operational_findings_do_not_invent_work_and_separate_blockers_from_warnings() {
        assert!(operational_findings("delivery", "งานค้าง", &[], None).is_empty());
        let work = vec![
            PendingTermWork {
                id: Uuid::from_u128(1),
                revision: "1".into(),
                blocks_closure: true,
            },
            PendingTermWork {
                id: Uuid::from_u128(2),
                revision: "2".into(),
                blocks_closure: false,
            },
            PendingTermWork {
                id: Uuid::from_u128(3),
                revision: "1".into(),
                blocks_closure: true,
            },
        ];
        let findings = operational_findings(
            "delivery",
            "งานค้าง",
            &work,
            Some("/staff/academic/delivery".into()),
        );
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].code, "delivery.blocking");
        assert_eq!(findings[0].severity, LifecycleSeverity::Blocking);
        assert_eq!(findings[0].count, 2);
        assert_eq!(findings[1].code, "delivery.warning");
        assert_eq!(findings[1].severity, LifecycleSeverity::Warning);
        assert_eq!(findings[1].count, 1);
        assert!(findings
            .iter()
            .all(|finding| finding.resolution_url.as_deref() == Some("/staff/academic/delivery")));
        assert!(operational_findings("delivery", "งานค้าง", &work, None)
            .iter()
            .all(|finding| finding.resolution_url.is_none()));
    }
}
