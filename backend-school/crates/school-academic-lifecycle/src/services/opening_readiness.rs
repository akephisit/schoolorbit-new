use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use std::collections::BTreeSet;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::super::models::{LifecycleFinding, LifecycleSeverity, TermActivationWorkspace};
use school_academic_core::{
    models::{ActivationIssue, ActivationIssueCode, StudentAcademicYearStatus},
    services::{
        activation_context,
        opening_diagnostics::{self, DiagnosticResource},
    },
};
use school_academic_delivery::services::opening as delivery_opening;
use school_academic_timetable::services::timetable_service;
use school_permissions::registry::codes;

pub async fn get_activation_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
) -> Result<TermActivationWorkspace, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_lifecycle.services.opening_readiness.get_activation_workspace",
    )
    .await?;
    DbSession::new(&mut *tx)
        .query(
            "academic_lifecycle.services.opening_readiness.get_activation_workspace",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let workspace =
        activation_workspace_in_transaction(&mut tx, actor, academic_year_id, academic_term_id)
            .await?;
    tx.commit().await?;
    Ok(workspace)
}

pub(crate) async fn activation_workspace_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
) -> Result<TermActivationWorkspace, AppError> {
    Ok(
        activation_evidence_in_transaction(tx, actor, academic_year_id, academic_term_id)
            .await?
            .0,
    )
}

pub(crate) async fn activation_evidence_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
) -> Result<
    (
        TermActivationWorkspace,
        school_academic_core::models::ActivationState,
    ),
    AppError,
> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    if academic_year_id.is_nil() || academic_term_id.is_nil() {
        return Err(AppError::ValidationError(
            "ระบุปีการศึกษาและภาคเรียนให้ถูกต้อง".into(),
        ));
    }
    let state =
        activation_context::read_activation_state(tx, academic_year_id, academic_term_id).await?;
    let policy = super::opening_policy::read_in_transaction(tx).await?;
    let offerings =
        delivery_opening::current_for_term(tx, academic_year_id, academic_term_id).await?;
    let timetables = timetable_service::opening::current_for_term(
        tx,
        academic_year_id,
        academic_term_id,
        state.context.bell_schedule_id,
    )
    .await?;
    let promotion = if state.opens_year {
        match state.predecessor.as_ref() {
            Some(predecessor) => Some(
                super::promotion_opening::read_in_transaction(
                    tx,
                    predecessor.academic_year_id,
                    academic_year_id,
                )
                .await?,
            ),
            None => None,
        }
    } else {
        None
    };

    let diagnostics = opening_diagnostics::explain(&state);
    let has_bell_details = diagnostics
        .iter()
        .any(|item| item.resource == DiagnosticResource::BellSchedule);
    let has_room_details = diagnostics
        .iter()
        .any(|item| matches!(item.resource, DiagnosticResource::Homeroom(_)));
    let mut findings: Vec<LifecycleFinding> = state
        .issues
        .iter()
        .filter(|issue| {
            !(issue.code == ActivationIssueCode::BellSchedule && has_bell_details
                || issue.code == ActivationIssueCode::RoomCapacity && has_room_details)
        })
        .map(|issue| core_finding(actor, academic_year_id, academic_term_id, issue))
        .collect();
    for diagnostic in diagnostics {
        let resolution_url = match diagnostic.resource {
            DiagnosticResource::BellSchedule => ((actor
                .has_permission(codes::ACADEMIC_YEAR_READ_SCHOOL)
                || actor.has_permission(codes::ACADEMIC_YEAR_MANAGE_SCHOOL))
                && (actor.has_permission(codes::ACADEMIC_TERM_READ_SCHOOL)
                    || actor.has_permission(codes::ACADEMIC_TERM_MANAGE_SCHOOL)))
            .then(|| context_url("/staff/academic/core", academic_year_id, academic_term_id)),
            DiagnosticResource::Homeroom(id) => (actor.has_permission(codes::HOMEROOM_READ_SCHOOL)
                || actor.has_permission(codes::HOMEROOM_MANAGE_SCHOOL))
            .then(|| {
                format!(
                    "/staff/academic/homerooms?academicYearId={academic_year_id}&homeroomId={id}"
                )
            }),
        };
        findings.push(LifecycleFinding {
            code: diagnostic.code.into(),
            severity: LifecycleSeverity::Blocking,
            count: 1,
            message: diagnostic.message,
            resolution_url,
        });
    }
    let eligible_student_years: BTreeSet<_> = state
        .placements
        .iter()
        .filter(|placement| placement.eligible && placement.reference_valid)
        .map(|placement| placement.student_academic_year_id)
        .collect();
    let planned_students = state
        .students
        .iter()
        .filter(|student| student.status == StudentAcademicYearStatus::Planned)
        .count();
    let missing_placements = state
        .students
        .iter()
        .filter(|student| {
            student.status == StudentAcademicYearStatus::Planned
                && !eligible_student_years.contains(&student.id)
        })
        .count();
    if policy.require_homeroom_placements && missing_placements > 0 {
        findings.push(LifecycleFinding {
            code: "opening.homeroom_placement_missing".into(),
            severity: LifecycleSeverity::Blocking,
            count: missing_placements,
            message: "นักเรียนที่เตรียมไว้ยังไม่มีห้องประจำชั้นที่พร้อมเริ่มใช้".into(),
            resolution_url: core_resolution_url(
                actor,
                academic_year_id,
                academic_term_id,
                ActivationIssueCode::PlacementReference,
            ),
        });
    }
    if policy.require_offerings && offerings.count() == 0 {
        findings.push(LifecycleFinding {
            code: "opening.offering_missing".into(),
            severity: LifecycleSeverity::Blocking,
            count: 1,
            message: "ยังไม่มีรายการเปิดสอนในภาคเรียนนี้".into(),
            resolution_url: actor
                .has_permission(codes::LEARNING_OFFERING_READ_SCHOOL)
                .then(|| {
                    context_url(
                        "/staff/academic/delivery",
                        academic_year_id,
                        academic_term_id,
                    )
                }),
        });
    }
    if policy.require_timetable && timetables.count() == 0 {
        findings.push(LifecycleFinding {
            code: "opening.timetable_missing".into(),
            severity: LifecycleSeverity::Blocking,
            count: 1,
            message: "ยังไม่มีตารางสอนที่ใช้ตารางคาบของภาคเรียนนี้".into(),
            resolution_url: actor
                .has_permission(codes::ACADEMIC_TIMETABLE_READ_SCHOOL)
                .then(|| {
                    context_url(
                        "/staff/academic/timetable",
                        academic_year_id,
                        academic_term_id,
                    )
                }),
        });
    }
    if let Some(evidence) = &promotion {
        let promotion_url = actor
            .has_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)
            .then(|| format!("/staff/academic/promotion?academicYearId={academic_year_id}"));
        for (code, count, message) in [
            (
                "opening.promotion_decision_missing",
                evidence.missing_students,
                "ยังมีนักเรียนจากปีก่อนที่ไม่มีผลการเลื่อนชั้นหรือพักการตัดสินที่ดำเนินการแล้ว",
            ),
            (
                "opening.promotion_run_executing",
                evidence.executing_runs,
                "ยังมีรอบเลื่อนชั้นที่กำลังดำเนินการ",
            ),
            (
                "opening.promotion_receipt_inconsistent",
                evidence.inconsistent_receipts,
                "หลักฐานการเลื่อนชั้นไม่ตรงกับข้อมูลนักเรียนปลายทาง",
            ),
            (
                "opening.promotion_correction_unresolved",
                evidence.unresolved_impacts,
                "มีผลการเรียนที่แก้ไขหลังดำเนินการเลื่อนชั้นและยังไม่ได้จัดการผลกระทบ",
            ),
        ] {
            if count > 0 {
                findings.push(LifecycleFinding {
                    code: code.into(),
                    severity: if code == "opening.promotion_correction_unresolved" {
                        LifecycleSeverity::Warning
                    } else {
                        LifecycleSeverity::Blocking
                    },
                    count,
                    message: message.into(),
                    resolution_url: promotion_url.clone(),
                });
            }
        }
    }
    findings.sort_by(|left, right| left.code.cmp(&right.code));
    let finding_state: Vec<_> = findings
        .iter()
        .map(|finding| (&finding.code, finding.severity, finding.count))
        .collect();
    let source_checksum = super::checksum(&(
        &state.source_checksum,
        &policy,
        &offerings.source_checksum,
        &timetables.source_checksum,
        promotion
            .as_ref()
            .map(|evidence| evidence.source_checksum.as_str()),
        &finding_state,
    ))?;
    let workspace = TermActivationWorkspace {
        context: state.context.clone(),
        opens_year: state.opens_year,
        predecessor: state.predecessor.clone(),
        policy,
        planned_students,
        eligible_placements: eligible_student_years.len(),
        can_activate: findings
            .iter()
            .all(|finding| finding.severity != LifecycleSeverity::Blocking),
        findings,
        source_checksum,
    };
    Ok((workspace, state))
}

fn core_finding(
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
    issue: &ActivationIssue,
) -> LifecycleFinding {
    let (suffix, message) = match issue.code {
        ActivationIssueCode::YearState => ("year_state", "สถานะปีการศึกษายังไม่พร้อมเปิด"),
        ActivationIssueCode::TermState => ("term_state", "สถานะภาคเรียนยังไม่พร้อมเปิด"),
        ActivationIssueCode::OtherRunningYear => {
            ("other_running_year", "ยังมีปีการศึกษาอื่นที่กำลังใช้งานหรือกำลังปิด")
        }
        ActivationIssueCode::OtherRunningTerm => {
            ("other_running_term", "ยังมีภาคเรียนอื่นที่กำลังใช้งานหรือกำลังปิด")
        }
        ActivationIssueCode::EarlierYearOpen => ("earlier_year_open", "ปีการศึกษาก่อนหน้ายังปิดไม่ครบ"),
        ActivationIssueCode::EarlierTermOpen => ("earlier_term_open", "ภาคเรียนก่อนหน้ายังปิดไม่ครบ"),
        ActivationIssueCode::NotFirstTerm => ("not_first_term", "ต้องเปิดภาคเรียนแรกของปีการศึกษาก่อน"),
        ActivationIssueCode::YearOverlap => ("year_overlap", "ข้อมูลปีการศึกษาขัดแย้งกับปีอื่น"),
        ActivationIssueCode::TermConfiguration => (
            "term_configuration",
            "วันเรียนของปีหรือการนับผลของภาคเรียนไม่ถูกต้อง",
        ),
        ActivationIssueCode::BellSchedule => ("bell_schedule", "ตารางคาบยังไม่มีช่วงเวลาที่ใช้ได้ในวันเรียน"),
        ActivationIssueCode::StudentReference => (
            "student_reference",
            "ข้อมูลนักเรียน ชั้น หรือแผนการเรียนปลายทางไม่ถูกต้อง",
        ),
        ActivationIssueCode::PlacementReference => (
            "placement_reference",
            "ข้อมูลห้องประจำชั้นของนักเรียนไม่ถูกต้องหรือซ้ำกัน",
        ),
        ActivationIssueCode::RoomCapacity => ("room_capacity", "จำนวนนักเรียนเกินความจุห้องประจำชั้น"),
        ActivationIssueCode::ExistingActiveEnrollment => (
            "existing_active_enrollment",
            "ปีที่กำลังวางแผนมีข้อมูลนักเรียนหรือห้องที่เปิดใช้งานแล้ว",
        ),
    };
    LifecycleFinding {
        code: format!("opening.{suffix}"),
        severity: LifecycleSeverity::Blocking,
        count: issue.count,
        message: message.into(),
        resolution_url: core_resolution_url(actor, year, term, issue.code),
    }
}

fn core_resolution_url(
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
    code: ActivationIssueCode,
) -> Option<String> {
    use ActivationIssueCode as Code;
    let has = |read, manage| actor.has_permission(read) || actor.has_permission(manage);
    let year_access = has(
        codes::ACADEMIC_YEAR_READ_SCHOOL,
        codes::ACADEMIC_YEAR_MANAGE_SCHOOL,
    );
    let term_access = has(
        codes::ACADEMIC_TERM_READ_SCHOOL,
        codes::ACADEMIC_TERM_MANAGE_SCHOOL,
    );
    let (path, allowed) = match code {
        Code::YearState | Code::OtherRunningYear | Code::EarlierYearOpen | Code::YearOverlap => {
            ("/staff/academic/year-lifecycle", year_access)
        }
        Code::TermState | Code::OtherRunningTerm | Code::EarlierTermOpen | Code::NotFirstTerm => {
            ("/staff/academic/year-lifecycle", term_access)
        }
        Code::TermConfiguration => ("/staff/academic/year-lifecycle", year_access && term_access),
        Code::BellSchedule => ("/staff/academic/core", year_access && term_access),
        Code::StudentReference | Code::PlacementReference | Code::ExistingActiveEnrollment => {
            return has(
                codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL,
                codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL,
            )
            .then(|| format!("/staff/academic/student-years?academicYearId={year}"));
        }
        Code::RoomCapacity => (
            "/staff/academic/homerooms",
            has(codes::HOMEROOM_READ_SCHOOL, codes::HOMEROOM_MANAGE_SCHOOL),
        ),
    };
    allowed.then(|| context_url(path, year, term))
}

fn context_url(path: &str, year: Uuid, term: Uuid) -> String {
    format!("{path}?academicYearId={year}&academicTermId={term}")
}

#[cfg(test)]
mod resolution_tests {
    use super::*;

    #[test]
    fn readiness_resolution_uses_the_resource_owner_and_its_read_or_manage_permission() {
        let year = Uuid::new_v4();
        let term = Uuid::new_v4();
        for (code, read, manage, path) in [
            (
                ActivationIssueCode::YearState,
                codes::ACADEMIC_YEAR_READ_SCHOOL,
                codes::ACADEMIC_YEAR_MANAGE_SCHOOL,
                "/staff/academic/year-lifecycle",
            ),
            (
                ActivationIssueCode::TermState,
                codes::ACADEMIC_TERM_READ_SCHOOL,
                codes::ACADEMIC_TERM_MANAGE_SCHOOL,
                "/staff/academic/year-lifecycle",
            ),
            (
                ActivationIssueCode::RoomCapacity,
                codes::HOMEROOM_READ_SCHOOL,
                codes::HOMEROOM_MANAGE_SCHOOL,
                "/staff/academic/homerooms",
            ),
        ] {
            let mut actor = ActorContext {
                user_id: Uuid::new_v4(),
                permissions: vec![],
            };
            assert!(core_resolution_url(&actor, year, term, code).is_none());
            for permission in [read, manage] {
                actor.permissions = vec![permission.into()];
                assert_eq!(
                    core_resolution_url(&actor, year, term, code),
                    Some(context_url(path, year, term))
                );
            }
        }
        let mut actor = ActorContext {
            user_id: Uuid::new_v4(),
            permissions: vec![codes::ACADEMIC_YEAR_READ_SCHOOL.into()],
        };
        for code in [
            ActivationIssueCode::TermConfiguration,
            ActivationIssueCode::BellSchedule,
        ] {
            assert!(core_resolution_url(&actor, year, term, code).is_none());
        }
        actor
            .permissions
            .push(codes::ACADEMIC_TERM_MANAGE_SCHOOL.into());
        assert_eq!(
            core_resolution_url(&actor, year, term, ActivationIssueCode::TermConfiguration),
            Some(context_url("/staff/academic/year-lifecycle", year, term))
        );
        assert_eq!(
            core_resolution_url(&actor, year, term, ActivationIssueCode::BellSchedule),
            Some(context_url("/staff/academic/core", year, term))
        );
    }
}
