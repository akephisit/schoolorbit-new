use super::super::models::{
    AcademicYearStatus, GradeProgression, GradeProgressionKind, StudentAcademicYearStatus,
};
use super::promotion_students::PromotionStudentContext;
use crate::{
    error::AppError,
    modules::academic::lifecycle::models::{
        PromotionDecisionInput, PromotionDecisionOutcome as Outcome,
    },
};
use sqlx::{types::Json, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(crate) async fn validate_destination(
    tx: &mut Transaction<'_, Postgres>,
    source: &PromotionStudentContext,
    target_year: Uuid,
    decision: &PromotionDecisionInput,
) -> Result<(), AppError> {
    validate_destinations_with_owned_target(tx, target_year, &[(source, decision)], None).await
}

pub(crate) async fn validate_reconciliation_destination(
    tx: &mut Transaction<'_, Postgres>,
    source: &PromotionStudentContext,
    target_year: Uuid,
    decision: &PromotionDecisionInput,
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
    )
    .await
}

/// Caller holds the tenant transition lock. Load and lock shared references once
/// per batch, not once per student. This command never writes enrollment/placement.
pub(crate) async fn validate_destinations(
    tx: &mut Transaction<'_, Postgres>,
    target_year: Uuid,
    items: &[(&PromotionStudentContext, &PromotionDecisionInput)],
) -> Result<(), AppError> {
    validate_destinations_with_owned_target(tx, target_year, items, None).await
}

async fn validate_destinations_with_owned_target(
    tx: &mut Transaction<'_, Postgres>,
    target_year: Uuid,
    items: &[(&PromotionStudentContext, &PromotionDecisionInput)],
    owned_target_student_year_id: Option<Uuid>,
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
    let status: Option<AcademicYearStatus> =
        sqlx::query_scalar("SELECT status FROM academic_years WHERE id=$1 FOR SHARE")
            .bind(target_year)
            .fetch_optional(&mut **tx)
            .await?;
    if status != Some(AcademicYearStatus::Planning) {
        return Err(AppError::Conflict("ปีปลายทางต้องอยู่ระหว่างวางแผน".into()));
    }
    sqlx::query("SELECT id FROM grade_level_progression_sets WHERE id=1 FOR SHARE")
        .execute(&mut **tx)
        .await?;
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
    let programs:Vec<ProgramReference>=sqlx::query_as(
        "SELECT program.id,version.curriculum_id,COALESCE(curriculum.grade_level_ids,'[]'::jsonb) AS grade_level_ids,
         (program.status='published' AND version.status='published' AND curriculum.is_active IS TRUE
          AND starts.start_date<=target.start_date AND (ends.end_date IS NULL OR ends.end_date>=target.end_date)) AS is_applicable
         FROM study_programs program JOIN curriculum_versions version ON version.id=program.curriculum_version_id
         JOIN curricula curriculum ON curriculum.id=version.curriculum_id
         JOIN academic_years starts ON starts.id=version.start_academic_year_id
         JOIN academic_years target ON target.id=$1 LEFT JOIN academic_years ends ON ends.id=version.end_academic_year_id
         WHERE program.id=ANY($2) ORDER BY program.id FOR SHARE OF program,version,curriculum"
    ).bind(target_year).bind(program_ids).fetch_all(&mut **tx).await?;
    let programs: BTreeMap<_, _> = programs.into_iter().map(|row| (row.id, row)).collect();
    let grades: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM grade_levels WHERE id=ANY($1) AND is_active IS TRUE ORDER BY id FOR SHARE",
    )
    .bind(&grade_ids)
    .fetch_all(&mut **tx)
    .await?;
    let grades: BTreeSet<_> = grades.into_iter().collect();
    let mappings:Vec<GradeProgression>=sqlx::query_as("SELECT id,from_grade_level_id,to_grade_level_id,transition_kind,curriculum_id,is_active,created_at,updated_at FROM grade_level_progressions WHERE from_grade_level_id=ANY($1) AND is_active ORDER BY id").bind(grade_ids).fetch_all(&mut **tx).await?;
    let rooms:Vec<RoomReference>=sqlx::query_as("SELECT id,academic_year_id,grade_level_id,study_program_id,is_active IS TRUE AS is_active FROM homerooms WHERE id=ANY($1) ORDER BY id FOR SHARE").bind(room_ids).fetch_all(&mut **tx).await?;
    let rooms: BTreeMap<_, _> = rooms.into_iter().map(|row| (row.id, row)).collect();
    let references = References {
        programs,
        grades,
        mappings,
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
    curriculum_id: Uuid,
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
    mappings: Vec<GradeProgression>,
    rooms: BTreeMap<Uuid, RoomReference>,
}

fn validate_reference(
    source: &PromotionStudentContext,
    target_year: Uuid,
    decision: &PromotionDecisionInput,
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
    let source_program = references
        .programs
        .get(&source.study_program_id)
        .ok_or_else(|| AppError::ValidationError("ไม่พบแผนการเรียนต้นทาง".into()))?;
    let mapped = references.mappings.iter().any(|mapping| {
        let kind_matches = match decision.outcome {
            Outcome::Graduate => mapping.transition_kind == GradeProgressionKind::Graduate,
            Outcome::Repeat => mapping.transition_kind == GradeProgressionKind::Repeat,
            Outcome::Promote => matches!(
                mapping.transition_kind,
                GradeProgressionKind::Promote | GradeProgressionKind::Exception
            ),
            Outcome::Conditional => matches!(
                mapping.transition_kind,
                GradeProgressionKind::Promote
                    | GradeProgressionKind::Repeat
                    | GradeProgressionKind::Exception
            ),
            Outcome::Hold | Outcome::TransferOut => false,
        };
        kind_matches
            && mapping.from_grade_level_id == source.grade_level_id
            && mapping.to_grade_level_id == decision.target_grade_level_id
            && mapping
                .curriculum_id
                .is_none_or(|id| id == source_program.curriculum_id)
    });
    if !mapped {
        return Err(AppError::ValidationError(
            "ไม่มีกฎการเลื่อนระดับชั้นที่ใช้ได้กับชั้นและหลักสูตรต้นทางนี้".into(),
        ));
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
            ))
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

#[cfg(test)]
#[path = "promotion_target_tests.rs"]
pub(super) mod tests;
