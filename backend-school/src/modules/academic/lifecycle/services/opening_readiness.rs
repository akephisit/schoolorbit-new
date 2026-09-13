use std::collections::BTreeSet;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::super::models::{LifecycleFinding, LifecycleSeverity, TermActivationWorkspace};
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::{
        core::{
            models::{ActivationIssue, ActivationIssueCode, StudentAcademicYearStatus},
            services::activation_context,
        },
        delivery,
        services::timetable_version_service,
    },
    permissions::registry::codes,
};

pub async fn get_activation_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    academic_year_id: Uuid,
    academic_term_id: Uuid,
) -> Result<TermActivationWorkspace, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_READ_SCHOOL)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
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
        crate::modules::academic::core::models::ActivationState,
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
    let offerings = delivery::services::opening::published_for_term(
        tx,
        academic_year_id,
        academic_term_id,
        state.context.term_start_date,
    )
    .await?;
    let timetables = timetable_version_service::opening::published_for_term(
        tx,
        academic_year_id,
        academic_term_id,
        state.context.term_start_date,
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

    let mut findings: Vec<LifecycleFinding> = state
        .issues
        .iter()
        .map(|issue| core_finding(actor, academic_year_id, academic_term_id, issue))
        .collect();
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
            message: "มีนักเรียนที่วางแผนเปิดปีแต่ยังไม่มีห้องประจำชั้นที่มีผลในวันเปิดภาค".into(),
            resolution_url: actor
                .has_permission(codes::STUDENT_READ_SCHOOL)
                .then(|| format!("/staff/academic/students?academicYearId={academic_year_id}")),
        });
    }
    if policy.require_published_offerings && offerings.count() == 0 {
        findings.push(LifecycleFinding {
            code: "opening.published_offering_missing".into(),
            severity: LifecycleSeverity::Blocking,
            count: 1,
            message: "ยังไม่มีรายการเปิดสอนที่เผยแพร่และมีผลในวันเปิดภาคเรียน".into(),
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
    if policy.require_published_timetable && timetables.count() == 0 {
        findings.push(LifecycleFinding {
            code: "opening.published_timetable_missing".into(),
            severity: LifecycleSeverity::Blocking,
            count: 1,
            message: "ยังไม่มีรุ่นตารางสอนที่เผยแพร่และใช้ตารางคาบของภาคเรียนนี้".into(),
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
                    severity: LifecycleSeverity::Blocking,
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
        ActivationIssueCode::YearOverlap => ("year_overlap", "ช่วงวันที่ของปีการศึกษาทับกับปีอื่น"),
        ActivationIssueCode::TermConfiguration => {
            ("term_configuration", "ข้อมูลวันที่หรือการนับผลของภาคเรียนไม่ถูกต้อง")
        }
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
        resolution_url: actor
            .has_permission(codes::ACADEMIC_YEAR_READ_SCHOOL)
            .then(|| context_url("/staff/academic/core", year, term)),
    }
}

fn context_url(path: &str, year: Uuid, term: Uuid) -> String {
    format!("{path}?academicYearId={year}&academicTermId={term}")
}
