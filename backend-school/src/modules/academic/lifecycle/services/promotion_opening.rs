use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::super::models::{
    PromotionDecisionInput, PromotionDecisionOutcome, PromotionItemStatus, PromotionRunStatus,
};
use crate::{
    error::AppError,
    modules::academic::{
        core::models::{HomeroomPlacementStatus, StudentAcademicYearStatus},
        results::services::corrections_after_annuals,
    },
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PromotionOpeningEvidence {
    pub source_students: usize,
    pub covered_students: usize,
    pub missing_students: usize,
    pub executing_runs: usize,
    pub inconsistent_receipts: usize,
    pub unresolved_impacts: usize,
    pub source_checksum: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct RunReference {
    id: Uuid,
    status: PromotionRunStatus,
    row_version: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct ReceiptReference {
    run_id: Uuid,
    item_id: Uuid,
    source_student_year_id: Uuid,
    student_id: Uuid,
    item_status: PromotionItemStatus,
    annual_revision_id: Option<Uuid>,
    #[sqlx(json(nullable))]
    decision: Option<PromotionDecisionInput>,
    target_student_year_id: Option<Uuid>,
    target_placement_id: Option<Uuid>,
    source_status: StudentAcademicYearStatus,
    actual_target_student_id: Option<Uuid>,
    actual_target_grade_level_id: Option<Uuid>,
    actual_target_study_program_id: Option<Uuid>,
    actual_target_status: Option<StudentAcademicYearStatus>,
    actual_placement_student_year_id: Option<Uuid>,
    actual_placement_homeroom_id: Option<Uuid>,
    actual_placement_status: Option<HomeroomPlacementStatus>,
}

pub(crate) async fn read_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    source_year_id: Uuid,
    target_year_id: Uuid,
) -> Result<PromotionOpeningEvidence, AppError> {
    if source_year_id.is_nil() || target_year_id.is_nil() || source_year_id == target_year_id {
        return Err(AppError::ValidationError(
            "ระบุปีต้นทางและปีปลายทางให้ถูกต้อง".into(),
        ));
    }
    let source_students: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM student_academic_years
         WHERE academic_year_id=$1 AND status='active' ORDER BY id LIMIT 10001",
    )
    .bind(source_year_id)
    .fetch_all(&mut **tx)
    .await?;
    if source_students.len() > 10_000 {
        return Err(AppError::ValidationError(
            "นักเรียนต้นทางเกิน 10,000 คน กรุณาติดต่อผู้ดูแลระบบ".into(),
        ));
    }
    let runs: Vec<RunReference> = sqlx::query_as(
        "SELECT id,status,row_version FROM academic_promotion_runs
         WHERE source_year_id=$1 AND target_year_id=$2 ORDER BY id LIMIT 501",
    )
    .bind(source_year_id)
    .bind(target_year_id)
    .fetch_all(&mut **tx)
    .await?;
    if runs.len() > 500 {
        return Err(AppError::ValidationError(
            "รอบเลื่อนชั้นระหว่างปีที่เลือกเกิน 500 รอบ กรุณาติดต่อผู้ดูแลระบบ".into(),
        ));
    }
    let run_ids: Vec<_> = runs.iter().map(|run| run.id).collect();
    let receipts: Vec<ReceiptReference> = if run_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            "WITH effective AS (
                 SELECT receipt.run_id,receipt.item_id,
                        item.student_academic_year_id AS source_student_year_id,
                        item.student_id,item.status AS item_status,item.annual_revision_id,
                        CASE WHEN resolution.resolution_kind='replace_decision'
                             THEN resolution.replacement_decision ELSE item.decision END AS decision,
                        CASE WHEN resolution.resolution_kind='replace_decision'
                             THEN NULLIF(resolution.outcome->>'targetStudentYearId','')::uuid
                             ELSE receipt.target_student_year_id END AS target_student_year_id,
                        CASE WHEN resolution.resolution_kind='replace_decision'
                             THEN NULLIF(resolution.outcome->>'targetPlacementId','')::uuid
                             ELSE receipt.target_placement_id END AS target_placement_id
                 FROM academic_promotion_execution_receipts receipt
                 JOIN academic_promotion_run_items item
                   ON item.id=receipt.item_id AND item.run_id=receipt.run_id
                 LEFT JOIN LATERAL (
                     SELECT resolution_kind,replacement_decision,outcome
                     FROM academic_promotion_impact_resolutions
                     WHERE item_id=receipt.item_id AND resolution_kind='replace_decision'
                     ORDER BY resolved_at DESC,id DESC LIMIT 1
                 ) resolution ON TRUE
                 WHERE receipt.run_id=ANY($1)
             )
             SELECT effective.run_id,effective.item_id,effective.source_student_year_id,
                    effective.student_id,effective.item_status,effective.annual_revision_id,
                    effective.decision,effective.target_student_year_id,effective.target_placement_id,
                    source.status AS source_status,target.student_id AS actual_target_student_id,
                    target.grade_level_id AS actual_target_grade_level_id,
                    target.study_program_id AS actual_target_study_program_id,
                    target.status AS actual_target_status,
                    placement.student_academic_year_id AS actual_placement_student_year_id,
                    placement.homeroom_id AS actual_placement_homeroom_id,
                    placement.status AS actual_placement_status
             FROM effective
             JOIN student_academic_years source ON source.id=effective.source_student_year_id
             LEFT JOIN student_academic_years target ON target.id=effective.target_student_year_id
             LEFT JOIN homeroom_placements placement ON placement.id=effective.target_placement_id
             ORDER BY effective.run_id,effective.item_id LIMIT 10001",
        )
        .bind(&run_ids)
        .fetch_all(&mut **tx)
        .await?
    };
    if receipts.len() > 10_000 {
        return Err(AppError::ValidationError(
            "หลักฐานดำเนินการเลื่อนชั้นเกิน 10,000 รายการ กรุณาติดต่อผู้ดูแลระบบ".into(),
        ));
    }

    let mut covered = BTreeSet::new();
    let mut inconsistent_receipts = 0;
    for receipt in &receipts {
        if receipt_is_consistent(receipt) {
            if source_students
                .binary_search(&receipt.source_student_year_id)
                .is_ok()
            {
                covered.insert(receipt.source_student_year_id);
            }
        } else {
            inconsistent_receipts += 1;
        }
    }
    let annual_owners =
        receipts
            .iter()
            .fold(BTreeMap::<Uuid, Vec<Uuid>>::new(), |mut owners, receipt| {
                if let Some(annual) = receipt.annual_revision_id {
                    owners.entry(annual).or_default().push(receipt.item_id);
                }
                owners
            });
    let annual_ids: Vec<_> = annual_owners.keys().copied().collect();
    let mut correction_evidence = Vec::new();
    for batch in annual_ids.chunks(500) {
        correction_evidence.extend(corrections_after_annuals(tx, source_year_id, batch).await?);
        if correction_evidence.len() > 10_000 {
            return Err(AppError::ValidationError(
                "ผลแก้ไขที่กระทบการเปิดปีเกิน 10,000 รายการ กรุณาติดต่อผู้ดูแลระบบ".into(),
            ));
        }
    }
    let resolved: BTreeSet<(Uuid, Uuid)> = if run_ids.is_empty() {
        BTreeSet::new()
    } else {
        sqlx::query_as(
            "SELECT item_id,correction_id FROM academic_promotion_impact_resolutions
             WHERE run_id=ANY($1) ORDER BY item_id,correction_id",
        )
        .bind(&run_ids)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .collect()
    };
    let mut unresolved_impacts = 0_usize;
    for evidence in &correction_evidence {
        let owners = annual_owners
            .get(&evidence.annual_revision_id)
            .ok_or_else(|| AppError::InternalServerError("ผลแก้ไขไม่ตรงกับหลักฐานเลื่อนชั้น".into()))?;
        for item_id in owners {
            if !resolved.contains(&(*item_id, evidence.correction.id)) {
                unresolved_impacts = unresolved_impacts.checked_add(1).ok_or_else(|| {
                    AppError::ValidationError("ผลแก้ไขที่กระทบการเปิดปีมีจำนวนไม่ถูกต้อง".into())
                })?;
            }
        }
    }
    let executing_runs = runs
        .iter()
        .filter(|run| run.status == PromotionRunStatus::Executing)
        .count();
    let source_checksum = super::checksum(&(
        &source_students,
        &runs,
        &receipts,
        &correction_evidence,
        &resolved,
    ))?;
    Ok(PromotionOpeningEvidence {
        source_students: source_students.len(),
        covered_students: covered.len(),
        missing_students: source_students.len().saturating_sub(covered.len()),
        executing_runs,
        inconsistent_receipts,
        unresolved_impacts,
        source_checksum,
    })
}

fn receipt_is_consistent(receipt: &ReceiptReference) -> bool {
    if receipt.item_status != PromotionItemStatus::Executed || receipt.annual_revision_id.is_none()
    {
        return false;
    }
    let Some(decision) = receipt.decision.as_ref() else {
        return false;
    };
    let no_target = receipt.target_student_year_id.is_none()
        && receipt.target_placement_id.is_none()
        && receipt.actual_target_student_id.is_none()
        && receipt.actual_target_status.is_none()
        && receipt.actual_placement_student_year_id.is_none();
    match decision.outcome {
        PromotionDecisionOutcome::Hold => {
            receipt.source_status == StudentAcademicYearStatus::Active && no_target
        }
        PromotionDecisionOutcome::Graduate => {
            receipt.source_status == StudentAcademicYearStatus::Graduated && no_target
        }
        PromotionDecisionOutcome::TransferOut => {
            receipt.source_status == StudentAcademicYearStatus::Withdrawn && no_target
        }
        PromotionDecisionOutcome::Promote
        | PromotionDecisionOutcome::Repeat
        | PromotionDecisionOutcome::Conditional => {
            receipt.source_status == StudentAcademicYearStatus::Active
                && receipt.target_student_year_id.is_some()
                && receipt.actual_target_student_id == Some(receipt.student_id)
                && receipt.actual_target_grade_level_id == decision.target_grade_level_id
                && receipt.actual_target_study_program_id == decision.target_study_program_id
                && receipt.actual_target_status == Some(StudentAcademicYearStatus::Planned)
                && placement_is_consistent(receipt)
        }
    }
}

fn placement_is_consistent(receipt: &ReceiptReference) -> bool {
    let Some(decision) = receipt.decision.as_ref() else {
        return false;
    };
    match decision.target_homeroom_id {
        None => {
            receipt.target_placement_id.is_none()
                && receipt.actual_placement_student_year_id.is_none()
                && receipt.actual_placement_homeroom_id.is_none()
                && receipt.actual_placement_status.is_none()
        }
        Some(homeroom) => {
            receipt.target_placement_id.is_some()
                && receipt.actual_placement_student_year_id == receipt.target_student_year_id
                && receipt.actual_placement_homeroom_id == Some(homeroom)
                && receipt.actual_placement_status == Some(HomeroomPlacementStatus::Planned)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executed_receipt(outcome: PromotionDecisionOutcome) -> ReceiptReference {
        ReceiptReference {
            run_id: Uuid::new_v4(),
            item_id: Uuid::new_v4(),
            source_student_year_id: Uuid::new_v4(),
            student_id: Uuid::new_v4(),
            item_status: PromotionItemStatus::Executed,
            annual_revision_id: Some(Uuid::new_v4()),
            decision: Some(PromotionDecisionInput {
                outcome,
                target_grade_level_id: None,
                target_study_program_id: None,
                target_homeroom_id: None,
                reason: None,
                condition: None,
            }),
            target_student_year_id: None,
            target_placement_id: None,
            source_status: StudentAcademicYearStatus::Active,
            actual_target_student_id: None,
            actual_target_grade_level_id: None,
            actual_target_study_program_id: None,
            actual_target_status: None,
            actual_placement_student_year_id: None,
            actual_placement_homeroom_id: None,
            actual_placement_status: None,
        }
    }

    #[test]
    fn terminal_receipts_require_the_exact_source_and_target_state() {
        let hold = executed_receipt(PromotionDecisionOutcome::Hold);
        assert!(receipt_is_consistent(&hold));

        let mut graduate = executed_receipt(PromotionDecisionOutcome::Graduate);
        assert!(!receipt_is_consistent(&graduate));
        graduate.source_status = StudentAcademicYearStatus::Graduated;
        assert!(receipt_is_consistent(&graduate));

        let mut transferred = executed_receipt(PromotionDecisionOutcome::TransferOut);
        transferred.source_status = StudentAcademicYearStatus::Withdrawn;
        transferred.target_student_year_id = Some(Uuid::new_v4());
        assert!(!receipt_is_consistent(&transferred));
    }

    #[test]
    fn continuing_receipts_require_the_receipt_owned_planned_target_and_placement() {
        let mut receipt = executed_receipt(PromotionDecisionOutcome::Promote);
        let target_student_year_id = Uuid::new_v4();
        let grade_level_id = Uuid::new_v4();
        let study_program_id = Uuid::new_v4();
        let homeroom_id = Uuid::new_v4();
        receipt.target_student_year_id = Some(target_student_year_id);
        receipt.actual_target_student_id = Some(receipt.student_id);
        receipt.actual_target_grade_level_id = Some(grade_level_id);
        receipt.actual_target_study_program_id = Some(study_program_id);
        receipt.actual_target_status = Some(StudentAcademicYearStatus::Planned);
        receipt.target_placement_id = Some(Uuid::new_v4());
        receipt.actual_placement_student_year_id = Some(target_student_year_id);
        receipt.actual_placement_homeroom_id = Some(homeroom_id);
        receipt.actual_placement_status = Some(HomeroomPlacementStatus::Planned);
        let decision = receipt.decision.as_mut().expect("decision");
        decision.target_grade_level_id = Some(grade_level_id);
        decision.target_study_program_id = Some(study_program_id);
        decision.target_homeroom_id = Some(homeroom_id);

        assert!(receipt_is_consistent(&receipt));

        receipt.actual_target_status = Some(StudentAcademicYearStatus::Active);
        assert!(!receipt_is_consistent(&receipt));
    }
}
