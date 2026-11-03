use super::super::models::{
    AcademicYearStatus, PromotionDestinationCommand, PromotionDestinationOutcome as Outcome,
    StudentAcademicYearStatus,
};
use super::promotion_students::PromotionStudentContext;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub async fn validate_destination(
    tx: &mut Transaction<'_, Postgres>,
    source: &PromotionStudentContext,
    target_year: Uuid,
    decision: &PromotionDestinationCommand,
) -> Result<(), AppError> {
    validate_destinations_with_owned_target(tx, target_year, &[(source, decision)], None, true)
        .await
}

pub async fn validate_reconciliation_destination(
    tx: &mut Transaction<'_, Postgres>,
    source: &PromotionStudentContext,
    target_year: Uuid,
    decision: &PromotionDestinationCommand,
    owned_target_student_year_id: Option<Uuid>,
) -> Result<(), AppError> {
    if source.existing_target_student_year_id != owned_target_student_year_id {
        return Err(AppError::Conflict(
            "ข้อมูลนักเรียนปีปลายทางไม่ได้เป็นของคำสั่งเลื่อนชั้นนี้".into(),
        ));
    }
    validate_destinations_with_owned_target(
        tx,
        target_year,
        &[(source, decision)],
        owned_target_student_year_id,
        true,
    )
    .await
}

/// Caller holds the tenant transition lock. Load and lock shared references once
/// per batch, not once per student. This command never writes enrollment/placement.
pub async fn validate_destinations(
    tx: &mut Transaction<'_, Postgres>,
    target_year: Uuid,
    items: &[(&PromotionStudentContext, &PromotionDestinationCommand)],
) -> Result<(), AppError> {
    validate_destinations_with_owned_target(tx, target_year, items, None, true).await
}

pub async fn preview_destinations(
    tx: &mut Transaction<'_, Postgres>,
    target_year: Uuid,
    items: &[(&PromotionStudentContext, &PromotionDestinationCommand)],
) -> Result<(), AppError> {
    validate_destinations_with_owned_target(tx, target_year, items, None, false).await
}

async fn validate_destinations_with_owned_target(
    tx: &mut Transaction<'_, Postgres>,
    target_year: Uuid,
    items: &[(&PromotionStudentContext, &PromotionDestinationCommand)],
    owned_target_student_year_id: Option<Uuid>,
    lock_references: bool,
) -> Result<(), AppError> {
    if items.is_empty()
        || items.len() > 500
        || items
            .iter()
            .map(|(source, _)| source.student_academic_year_id)
            .collect::<BTreeSet<_>>()
            .len()
            != items.len()
    {
        return Err(AppError::ValidationError(
            "ตรวจปลายทางได้ครั้งละ 1–500 คน โดยไม่ซ้ำนักเรียน".into(),
        ));
    }
    let status: Option<AcademicYearStatus> = DbSession::new(&mut **tx)
        .scalar(
            "academic_core.services.promotion_targets.validate_destinations_with_owned_target",
            if lock_references {
                "SELECT status FROM academic_years WHERE id=$1 FOR SHARE"
            } else {
                "SELECT status FROM academic_years WHERE id=$1"
            },
        )
        .bind(target_year)
        .fetch_optional()
        .await?;
    if status != Some(AcademicYearStatus::Planning) {
        return Err(AppError::Conflict("ปีปลายทางต้องอยู่ระหว่างวางแผน".into()));
    }
    let program_ids: Vec<Uuid> = items
        .iter()
        .flat_map(|(source, decision)| {
            [
                Some(source.study_program_id),
                decision.target_study_program_id,
            ]
        })
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let grade_ids: Vec<Uuid> = items
        .iter()
        .flat_map(|(source, decision)| {
            [Some(source.grade_level_id), decision.target_grade_level_id]
        })
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let room_ids: Vec<Uuid> = items
        .iter()
        .filter_map(|(_, decision)| decision.target_homeroom_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let programs:Vec<ProgramReference>=DbSession::new(&mut **tx).query_as("academic_core.services.promotion_targets.validate_destinations_with_owned_target",
        sqlx::AssertSqlSafe(reference_sql("SELECT program.id,COALESCE(version.grade_level_ids,'[]'::jsonb) AS grade_level_ids,
         (program.status='published' AND curriculum.status='published' AND (curriculum.is_active IS TRUE AND version.is_active)
) AS is_applicable
         FROM published_study_programs program JOIN published_curriculum_levels version ON version.id=program.curriculum_level_id
         JOIN curriculum_editions curriculum ON curriculum.id=version.edition_id
         WHERE program.id=ANY($1) ORDER BY program.id FOR SHARE OF curriculum",lock_references))
    ).bind(program_ids).fetch_all().await?;
    let programs: BTreeMap<_, _> = programs.into_iter().map(|row| (row.id, row)).collect();
    let grades: Vec<Uuid> = DbSession::new(&mut **tx).scalar("academic_core.services.promotion_targets.validate_destinations_with_owned_target",
        sqlx::AssertSqlSafe(reference_sql("SELECT id FROM grade_levels WHERE id=ANY($1) AND is_active IS TRUE ORDER BY id FOR SHARE",lock_references)),
    )
    .bind(&grade_ids)
    .fetch_all()
    .await?;
    let grades: BTreeSet<_> = grades.into_iter().collect();
    let rooms:Vec<RoomReference>=DbSession::new(&mut **tx).query_as("academic_core.services.promotion_targets.validate_destinations_with_owned_target", sqlx::AssertSqlSafe(reference_sql("SELECT id,academic_year_id,grade_level_id,study_program_id,is_active IS TRUE AS is_active FROM homerooms WHERE id=ANY($1) ORDER BY id FOR SHARE",lock_references))).bind(room_ids).fetch_all().await?;
    let rooms: BTreeMap<_, _> = rooms.into_iter().map(|row| (row.id, row)).collect();
    let references = References {
        programs,
        grades,
        rooms,
    };
    for (source, decision) in items {
        validate_reference(
            source,
            target_year,
            decision,
            &references,
            owned_target_student_year_id,
        )?;
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ProgramReference {
    id: Uuid,
    grade_level_ids: Json<Vec<Uuid>>,
    is_applicable: bool,
}
#[derive(sqlx::FromRow)]
struct RoomReference {
    id: Uuid,
    academic_year_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
    is_active: bool,
}
struct References {
    programs: BTreeMap<Uuid, ProgramReference>,
    grades: BTreeSet<Uuid>,
    rooms: BTreeMap<Uuid, RoomReference>,
}

fn validate_reference(
    source: &PromotionStudentContext,
    target_year: Uuid,
    decision: &PromotionDestinationCommand,
    references: &References,
    owned_target_student_year_id: Option<Uuid>,
) -> Result<(), AppError> {
    let creates_target = matches!(
        decision.outcome,
        Outcome::Promote | Outcome::Repeat | Outcome::Conditional
    );
    if !creates_target
        && (decision.target_grade_level_id.is_some()
            || decision.target_study_program_id.is_some()
            || decision.target_homeroom_id.is_some())
    {
        return Err(AppError::ValidationError(
            "ผลนี้ต้องไม่ระบุชั้น แผน หรือห้องปลายทาง".into(),
        ));
    }
    if decision.outcome == Outcome::Hold {
        return Ok(());
    }
    if !matches!(
        source.status,
        StudentAcademicYearStatus::Active | StudentAcademicYearStatus::Completed
    ) {
        return Err(AppError::Conflict(
            "นักเรียนต้นทางยังไม่เริ่มเรียนหรือพ้นสภาพแล้ว ให้ตรวจสอบสถานะก่อนดำเนินการ".into(),
        ));
    }
    if source.existing_target_student_year_id.is_some()
        && source.existing_target_student_year_id != owned_target_student_year_id
    {
        return Err(AppError::Conflict(
            "มีข้อมูลนักเรียนปีปลายทางจากงานอื่นแล้ว ต้องตรวจสอบก่อน ไม่สามารถเขียนทับได้".into(),
        ));
    }
    if decision.outcome == Outcome::TransferOut {
        return Ok(());
    }
    if decision.outcome == Outcome::Graduate {
        return Ok(());
    }
    let (grade, program) = match (
        decision.target_grade_level_id,
        decision.target_study_program_id,
    ) {
        (Some(grade), Some(program)) if !grade.is_nil() && !program.is_nil() => (grade, program),
        _ => {
            return Err(AppError::ValidationError(
                "ระบุระดับชั้นและแผนปลายทางให้ครบ".into(),
            ));
        }
    };
    if (decision.outcome == Outcome::Repeat && grade != source.grade_level_id)
        || (decision.outcome == Outcome::Promote && grade == source.grade_level_id)
    {
        return Err(AppError::ValidationError(
            "ระดับชั้นปลายทางไม่ตรงกับผลที่เลือก".into(),
        ));
    }
    if !references.grades.contains(&grade)
        || !references
            .programs
            .get(&program)
            .is_some_and(|row| row.is_applicable && row.grade_level_ids.contains(&grade))
    {
        return Err(AppError::ValidationError(
            "แผนปลายทางต้องเผยแพร่แล้ว ครอบคลุมปีและระดับชั้นที่เลือก".into(),
        ));
    }
    if let Some(room) = decision.target_homeroom_id {
        if !references.rooms.get(&room).is_some_and(|row| {
            row.is_active
                && row.academic_year_id == target_year
                && row.grade_level_id == grade
                && row.study_program_id == program
        }) {
            return Err(AppError::ValidationError(
                "ห้องปลายทางต้องเปิดใช้งานและอยู่ในปี ระดับชั้น และแผนการเรียนที่เลือก".into(),
            ));
        }
    }
    Ok(())
}

fn reference_sql(sql: &str, lock: bool) -> String {
    if lock {
        sql.to_owned()
    } else {
        sql.replace(" FOR SHARE OF curriculum", "")
            .replace(" FOR SHARE", "")
    }
}

#[cfg(test)]
mod reference_tests {
    use super::*;
    #[test]
    fn read_only_preview_uses_the_same_projection_without_row_locks() {
        assert_eq!(
            reference_sql("SELECT id FROM refs FOR SHARE", false),
            "SELECT id FROM refs"
        );
        assert_eq!(
            reference_sql("SELECT id FROM refs FOR SHARE OF curriculum", false),
            "SELECT id FROM refs"
        );
        assert_eq!(
            reference_sql("SELECT id FROM refs FOR SHARE", true),
            "SELECT id FROM refs FOR SHARE"
        );
    }
}

/// Stable annual identities from accepted Lifecycle receipts. Later planned-year
/// grade/program/room edits remain owned and audited by Core.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreparedPromotionTarget {
    pub student_id: Uuid,
    pub target_student_year_id: Uuid,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct PreparedPromotionTargetState {
    pub student_academic_year_id: Uuid,
    pub row_version: i64,
    pub status: crate::models::StudentAcademicYearStatus,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
    pub study_program_name: String,
    pub placement_id: Option<Uuid>,
    pub placement_row_version: Option<i64>,
    pub placement_starts_on: Option<chrono::NaiveDate>,
    pub homeroom_id: Option<Uuid>,
    pub homeroom_name: Option<String>,
    pub homeroom_row_version: Option<i64>,
}

/// Reaffirm original intent while reading the separately audited current target
/// projection in one bounded snapshot. No historical placement is rewritten.
pub async fn verify_prepared_targets(
    tx: &mut Transaction<'_, Postgres>,
    target_year: Uuid,
    targets: &[PreparedPromotionTarget],
) -> Result<Vec<PreparedPromotionTargetState>, AppError> {
    if targets.is_empty()
        || targets.len() > 500
        || targets
            .iter()
            .map(|t| t.target_student_year_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != targets.len()
    {
        return Err(AppError::ValidationError(
            "ตรวจปลายทางที่เตรียมไว้ได้ครั้งละ 1–500 คนโดยไม่ซ้ำ".into(),
        ));
    }
    let states:Vec<PreparedPromotionTargetState>=DbSession::new(&mut **tx).query_as("academic_core.promotion_targets.verify_prepared_targets",
        "SELECT student_year.id AS student_academic_year_id,student_year.row_version,student_year.status,student_year.grade_level_id,student_year.study_program_id,
         program.name_th AS study_program_name,placement.id AS placement_id,placement.row_version AS placement_row_version,
         placement.start_date AS placement_starts_on,room.id AS homeroom_id,room.name AS homeroom_name,room.row_version AS homeroom_row_version
         FROM jsonb_to_recordset($1) expected(student_id uuid,target_student_year_id uuid)
         JOIN student_academic_years student_year ON student_year.id=expected.target_student_year_id
           AND student_year.student_id=expected.student_id AND student_year.academic_year_id=$2 AND student_year.status IN ('planned','withdrawn')
         JOIN published_study_programs program ON program.id=student_year.study_program_id
         LEFT JOIN LATERAL(SELECT id,homeroom_id,start_date,row_version FROM homeroom_placements
           WHERE student_academic_year_id=student_year.id AND status IN ('planned','current')
           ORDER BY CASE status WHEN 'current' THEN 0 ELSE 1 END,start_date DESC NULLS LAST,id DESC LIMIT 1) placement ON true
         LEFT JOIN homerooms room ON room.id=placement.homeroom_id ORDER BY student_year.id")
        .bind(sqlx::types::Json(targets)).bind(target_year).fetch_all().await?;
    if states.len() != targets.len() {
        return Err(AppError::Conflict(
            "ข้อมูลประจำปีปลายทางเปลี่ยนแล้ว กรุณาตรวจสอบประวัติก่อนยืนยัน".into(),
        ));
    }
    Ok(states)
}
