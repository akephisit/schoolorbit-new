use super::{promotion_students, promotion_targets};
use crate::models::{
    AcademicYearStatus, PromotionDestinationCommand, PromotionDestinationOutcome as Outcome,
    PromotionWriteOutcome, StudentAcademicYearStatus,
};
use school_database::DbSession;
use school_errors::AppError;
use serde::Serialize;
use sqlx::{types::Json, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PromotionBatchDeclaration {
    pub source: promotion_students::PromotionStudentContext,
    pub decision: PromotionDestinationCommand,
    pub original_target_placement_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromotionBatchOutcome {
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub write: PromotionWriteOutcome,
}

#[derive(Serialize)]
struct AnnualInsert {
    id: Uuid,
    student_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
}
#[derive(Serialize)]
struct PlacementInsert {
    id: Uuid,
    student_academic_year_id: Uuid,
    homeroom_id: Uuid,
}
#[derive(Serialize)]
struct SourceUpdate {
    id: Uuid,
    status: &'static str,
    outcome: Outcome,
}

/// Canonical manual write operation. Lifecycle owns authorization, exact accepted
/// provenance, replay and the same transaction under the tenant transition lock.
/// All same-kind reads/writes are bounded sets, including source history/audits.
pub async fn apply_declarations(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    source_year: Uuid,
    target_year: Uuid,
    declarations: &[PromotionBatchDeclaration],
) -> Result<Vec<PromotionBatchOutcome>, AppError> {
    validate_batch(declarations)?;
    let (source_context, _) =
        promotion_students::validated_run_years(tx, source_year, target_year).await?;
    if source_context.status != AcademicYearStatus::Closed {
        return Err(AppError::Conflict(
            "ต้องปิดปีต้นทางก่อนยืนยันรายชื่อนักเรียนปีใหม่".into(),
        ));
    }
    let ids: Vec<_> = declarations
        .iter()
        .map(|d| d.source.student_academic_year_id)
        .collect();
    DbSession::new(&mut **tx).query("academic_core.promotion_batch.lock_sources",
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 AND id=ANY($2) ORDER BY id FOR UPDATE")
        .bind(source_year).bind(&ids).execute().await?;
    let actual =
        promotion_students::read_promotion_students(tx, source_year, target_year, &ids).await?;
    let mut actual: BTreeMap<_, _> = actual
        .into_iter()
        .map(|row| (row.student_academic_year_id, row))
        .collect();
    for declaration in declarations {
        let original = &declaration.source;
        let current = actual
            .remove(&original.student_academic_year_id)
            .ok_or_else(|| AppError::Conflict("รายชื่อนักเรียนเปลี่ยนแล้ว".into()))?;
        if current.student_id != original.student_id
            || current.row_version != original.row_version
            || current.grade_level_id != original.grade_level_id
            || current.study_program_id != original.study_program_id
            || current.status != original.status
            || current.existing_target_student_year_id != original.existing_target_student_year_id
            || current.existing_target_row_version != original.existing_target_row_version
        {
            return Err(AppError::Conflict(
                "ข้อมูลนักเรียนต้นทางหรือปลายทางเปลี่ยนแล้ว".into(),
            ));
        }
    }
    let new: Vec<_> = declarations
        .iter()
        .filter(|d| d.source.existing_target_student_year_id.is_none())
        .collect();
    if !new.is_empty() {
        let references: Vec<_> = new.iter().map(|d| (&d.source, &d.decision)).collect();
        promotion_targets::validate_destinations(tx, target_year, &references).await?;
    }
    let existing: Vec<_> = declarations
        .iter()
        .filter_map(|d| {
            d.source.existing_target_student_year_id.map(|id| {
                promotion_targets::PreparedPromotionTarget {
                    student_id: d.source.student_id,
                    target_student_year_id: id,
                }
            })
        })
        .collect();
    if !existing.is_empty() {
        promotion_targets::verify_prepared_targets(tx, target_year, &existing).await?;
    }
    let joining: Vec<_> = new
        .iter()
        .filter(|d| creates_target(d.decision.outcome))
        .map(|d| d.source.student_id)
        .collect();
    if !joining.is_empty() {
        let active:Vec<Uuid>=DbSession::new(&mut **tx).scalar("academic_core.promotion_batch.active_students",
            "SELECT id FROM users WHERE id=ANY($1) AND user_type='student' AND status='active' ORDER BY id FOR SHARE")
            .bind(&joining).fetch_all().await?;
        if active.len() != joining.len() {
            return Err(AppError::Conflict(
                "มีบัญชีนักเรียนที่ไม่ใช้งานแล้ว ต้องตรวจทะเบียนก่อนเตรียมปีใหม่".into(),
            ));
        }
    }
    check_room_capacity(tx, target_year, &new).await?;
    let mut annuals = vec![];
    let mut placements = vec![];
    let mut updates = vec![];
    let mut results = vec![];
    for declaration in declarations {
        let source = &declaration.source;
        let decision = &declaration.decision;
        let mut write = PromotionWriteOutcome {
            target_student_year_id: source.existing_target_student_year_id,
            target_placement_id: declaration.original_target_placement_id,
            source_row_version: source.row_version,
        };
        if source.existing_target_student_year_id.is_none() && creates_target(decision.outcome) {
            let (Some(grade), Some(program)) = (
                decision.target_grade_level_id,
                decision.target_study_program_id,
            ) else {
                return Err(AppError::ValidationError("ระบุชั้นและแผนปลายทางให้ครบ".into()));
            };
            let id = Uuid::now_v7();
            annuals.push(AnnualInsert {
                id,
                student_id: source.student_id,
                grade_level_id: grade,
                study_program_id: program,
            });
            write.target_student_year_id = Some(id);
            if let Some(room) = decision.target_homeroom_id {
                let placement = Uuid::now_v7();
                placements.push(PlacementInsert {
                    id: placement,
                    student_academic_year_id: id,
                    homeroom_id: room,
                });
                write.target_placement_id = Some(placement);
            }
        }
        if let Some(status) = final_status(decision.outcome) {
            updates.push(SourceUpdate {
                id: source.student_academic_year_id,
                status,
                outcome: decision.outcome,
            });
        }
        results.push(PromotionBatchOutcome {
            student_academic_year_id: source.student_academic_year_id,
            student_id: source.student_id,
            write,
        });
    }
    let finished: Vec<_> = updates.iter().map(|row| row.id).collect();
    if !annuals.is_empty() {
        DbSession::new(&mut **tx).query("academic_core.promotion_batch.insert_targets",
            "INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status)
             SELECT id,student_id,$2,grade_level_id,study_program_id,'planned' FROM jsonb_to_recordset($1)
             row(id uuid,student_id uuid,grade_level_id uuid,study_program_id uuid)")
            .bind(Json(&annuals)).bind(target_year).execute().await?;
    }
    if !placements.is_empty() {
        DbSession::new(&mut **tx).query("academic_core.promotion_batch.insert_placements",
            "INSERT INTO homeroom_placements(id,student_academic_year_id,academic_year_id,homeroom_id,placement_scope,status,enrollment_type)
             SELECT id,student_academic_year_id,$2,homeroom_id,'academic_year','planned','promotion' FROM jsonb_to_recordset($1)
             row(id uuid,student_academic_year_id uuid,homeroom_id uuid)")
            .bind(Json(&placements)).bind(target_year).execute().await?;
    }
    if !updates.is_empty() {
        DbSession::new(&mut **tx).query("academic_core.promotion_batch.finish_placements",
            "UPDATE homeroom_placements SET status=CASE WHEN status='planned' THEN 'cancelled' ELSE 'ended' END,
             row_version=row_version+1,updated_at=now()
             WHERE student_academic_year_id=ANY($1) AND status IN ('current','planned')")
            .bind(&finished).execute().await?;
        DbSession::new(&mut **tx).query("academic_core.promotion_batch.finish_sources",
            "UPDATE student_academic_years source SET status=row.status,row_version=source.row_version+1,updated_at=now()
             FROM jsonb_to_recordset($1) row(id uuid,status text) WHERE source.id=row.id AND source.status IS DISTINCT FROM row.status")
            .bind(Json(&updates)).execute().await?;
    }
    let versions: Vec<(Uuid, i64)> = DbSession::new(&mut **tx)
        .query_as(
            "academic_core.promotion_batch.final_versions",
            "SELECT id,row_version FROM student_academic_years WHERE id=ANY($1) ORDER BY id",
        )
        .bind(&ids)
        .fetch_all()
        .await?;
    let versions: BTreeMap<_, _> = versions.into_iter().collect();
    for result in &mut results {
        result.write.source_row_version = *versions
            .get(&result.student_academic_year_id)
            .ok_or_else(|| AppError::Conflict("ข้อมูลนักเรียนต้นทางไม่ครบ".into()))?;
    }
    if !updates.is_empty() {
        DbSession::new(&mut **tx).query("academic_core.promotion_batch.audit_sources",
            "INSERT INTO academic_audit_events(event_code,entity_type,entity_id,academic_year_id,actor_user_id,payload)
             SELECT 'student_academic_year.rollover_finalized','student_academic_year',row.id,$2,$3,
               jsonb_build_object('outcome',row.outcome,'targetYearId',$4::uuid)
             FROM jsonb_to_recordset($1) row(id uuid,outcome text)")
            .bind(Json(&updates)).bind(source_year).bind(actor).bind(target_year).execute().await?;
    }
    Ok(results)
}

async fn check_room_capacity(
    tx: &mut Transaction<'_, Postgres>,
    year: Uuid,
    declarations: &[&PromotionBatchDeclaration],
) -> Result<(), AppError> {
    let mut additions = BTreeMap::<Uuid, i64>::new();
    for row in declarations {
        if let Some(room) = row.decision.target_homeroom_id {
            *additions.entry(room).or_default() += 1;
        }
    }
    if additions.is_empty() {
        return Ok(());
    }
    let ids: Vec<_> = additions.keys().copied().collect();
    let rooms: Vec<(Uuid, Option<i32>)> = DbSession::new(&mut **tx)
        .query_as(
            "academic_core.promotion_batch.lock_rooms",
            "SELECT id,capacity FROM homerooms WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&ids)
        .fetch_all()
        .await?;
    let occupied:Vec<(Uuid,i64)>=DbSession::new(&mut **tx).query_as("academic_core.promotion_batch.room_occupancy",
        "SELECT homeroom_id,count(DISTINCT student_academic_year_id) FROM homeroom_placements WHERE academic_year_id=$1 AND homeroom_id=ANY($2) AND status IN ('planned','current') GROUP BY homeroom_id")
        .bind(year).bind(&ids).fetch_all().await?;
    let occupied: BTreeMap<_, _> = occupied.into_iter().collect();
    if rooms.len() != ids.len()
        || rooms.iter().any(|(room, capacity)| {
            capacity.is_some_and(|cap| {
                occupied.get(room).copied().unwrap_or(0) + additions.get(room).copied().unwrap_or(0)
                    > i64::from(cap)
            })
        })
    {
        return Err(AppError::Conflict("จำนวนที่จัดเพิ่มเกินความจุห้องปลายทาง".into()));
    }
    Ok(())
}

fn creates_target(outcome: Outcome) -> bool {
    matches!(
        outcome,
        Outcome::Promote | Outcome::Repeat | Outcome::Conditional
    )
}
fn final_status(outcome: Outcome) -> Option<&'static str> {
    match outcome {
        Outcome::Hold => None,
        Outcome::Graduate => Some("graduated"),
        Outcome::TransferOut => Some("withdrawn"),
        _ => Some("completed"),
    }
}
fn validate_batch(
    rows: &[PromotionBatchDeclaration],
) -> Result<(), AppError> {
    if rows.is_empty()
        || rows.len() > 500
        || rows
            .iter()
            .map(|row| row.source.student_academic_year_id)
            .collect::<BTreeSet<_>>()
            .len()
            != rows.len()
        || rows.iter().any(|row| {
            !matches!(
                row.source.status,
                StudentAcademicYearStatus::Active | StudentAcademicYearStatus::Completed
            ) || (row.source.existing_target_student_year_id.is_some()
                && !creates_target(row.decision.outcome))
                || (row.source.existing_target_student_year_id.is_none()
                    && row.decision.outcome == Outcome::Conditional)
        })
    {
        return Err(AppError::ValidationError(
            "ระบุผลและนักเรียนไม่ซ้ำครั้งละ 1–500 คน".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_outcomes_finish_source_history_without_turning_a_hold_into_enrollment() {
        assert_eq!(final_status(Outcome::Hold), None);
        assert!(!creates_target(Outcome::Hold));
        assert_eq!(final_status(Outcome::Graduate), Some("graduated"));
        assert_eq!(final_status(Outcome::TransferOut), Some("withdrawn"));
        assert_eq!(final_status(Outcome::Promote), Some("completed"));
        assert!(creates_target(Outcome::Repeat));
    }
}
