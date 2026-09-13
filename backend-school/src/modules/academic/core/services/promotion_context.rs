use sqlx::{Postgres, Transaction};
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::academic::{
        core::models::{GradeProgression, GradeProgressionKind, GradeProgressionSet},
        lifecycle::models::{PromotionPolicyOptions, PromotionRuleInput, PromotionSuccessOutcome},
    },
};

/// Minimal school-office reference data. No student, staff, or privileged
/// curriculum-management payload is needed to read a reviewed promotion policy.
pub(crate) async fn policy_options(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<PromotionPolicyOptions, AppError> {
    let grades: Vec<crate::modules::academic::lifecycle::models::PromotionGradeReference> = sqlx::query_as(
        "SELECT id,level_type,year,is_active IS TRUE AS is_active FROM grade_levels ORDER BY CASE level_type WHEN 'kindergarten' THEN 1 WHEN 'primary' THEN 2 WHEN 'secondary' THEN 3 ELSE 4 END,year,id LIMIT 501"
    ).fetch_all(&mut **tx).await?;
    let programs: Vec<crate::modules::academic::lifecycle::models::PromotionProgramReference> = sqlx::query_as(
        "SELECT program.id,program.code,program.name_th AS name,version.curriculum_id,curriculum.name_th AS curriculum_name,version.version_name,program.status FROM study_programs program JOIN curriculum_versions version ON version.id=program.curriculum_version_id JOIN curricula curriculum ON curriculum.id=version.curriculum_id ORDER BY curriculum.name_th,version.version_name,program.code,program.id LIMIT 5001"
    ).fetch_all(&mut **tx).await?;
    let row_version =
        sqlx::query_scalar("SELECT row_version FROM grade_level_progression_sets WHERE id=1")
            .fetch_one(&mut **tx)
            .await?;
    let progressions: Vec<GradeProgression>=sqlx::query_as("SELECT id,from_grade_level_id,to_grade_level_id,transition_kind,curriculum_id,is_active,created_at,updated_at FROM grade_level_progressions ORDER BY from_grade_level_id,transition_kind,id LIMIT 5001")
        .fetch_all(&mut **tx).await?;
    if grades.len() > 500 || programs.len() > 5000 || progressions.len() > 5000 {
        return Err(AppError::ValidationError(
            "ข้อมูลอ้างอิงเกินขอบเขตการแสดงผล กรุณาติดต่อผู้ดูแลระบบ".into(),
        ));
    }
    Ok(PromotionPolicyOptions {
        grades,
        programs,
        progression_set: GradeProgressionSet {
            row_version,
            progressions,
        },
    })
}

/// Caller owns authorization and transaction. The set lock also serializes this
/// snapshot with Core's replace-progressions command; no mutable row IDs escape.
pub(crate) async fn validate_promotion_rules(
    tx: &mut Transaction<'_, Postgres>,
    rules: &[PromotionRuleInput],
) -> Result<i64, AppError> {
    let version = sqlx::query_scalar(
        "SELECT row_version FROM grade_level_progression_sets WHERE id=1 FOR SHARE",
    )
    .fetch_one(&mut **tx)
    .await?;
    let grades: Vec<Uuid> = rules
        .iter()
        .flat_map(|rule| [Some(rule.from_grade_level_id), rule.target_grade_level_id])
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let found: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE id=ANY($1) ORDER BY id FOR SHARE")
            .bind(&grades)
            .fetch_all(&mut **tx)
            .await?;
    if found.len() != grades.len() {
        return Err(AppError::ValidationError("ไม่พบระดับชั้นในเกณฑ์เลื่อนชั้น".into()));
    }
    let programs: Vec<Uuid> = rules
        .iter()
        .flat_map(|rule| {
            [
                Some(rule.from_study_program_id),
                rule.target_study_program_id,
            ]
        })
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let found: Vec<(Uuid,Uuid)> = sqlx::query_as("SELECT program.id,version.curriculum_id FROM study_programs program JOIN curriculum_versions version ON version.id=program.curriculum_version_id WHERE program.id=ANY($1) ORDER BY program.id FOR SHARE OF program,version")
        .bind(&programs).fetch_all(&mut **tx).await?;
    if found.len() != programs.len() {
        return Err(AppError::ValidationError(
            "ไม่พบแผนการเรียนในเกณฑ์เลื่อนชั้น".into(),
        ));
    }
    let curricula: HashMap<Uuid, Uuid> = found.into_iter().collect();
    let progressions: Vec<GradeProgression> = sqlx::query_as("SELECT id,from_grade_level_id,to_grade_level_id,transition_kind,curriculum_id,is_active,created_at,updated_at FROM grade_level_progressions WHERE is_active AND from_grade_level_id=ANY($1) ORDER BY id")
        .bind(&grades).fetch_all(&mut **tx).await?;
    for rule in rules {
        let curriculum = curricula
            .get(&rule.from_study_program_id)
            .ok_or_else(|| AppError::ValidationError("ไม่พบหลักสูตรต้นทาง".into()))?;
        let kind = match rule.success_outcome {
            PromotionSuccessOutcome::Promote => GradeProgressionKind::Promote,
            PromotionSuccessOutcome::Graduate => GradeProgressionKind::Graduate,
        };
        let applicable: Vec<_> = progressions
            .iter()
            .filter(|mapping| {
                mapping.from_grade_level_id == rule.from_grade_level_id
                    && mapping.transition_kind == kind
                    && mapping.curriculum_id.is_none_or(|id| id == *curriculum)
            })
            .collect();
        if applicable.len() != 1 || applicable[0].to_grade_level_id != rule.target_grade_level_id {
            return Err(AppError::ValidationError(
                "กฎการเลื่อนระดับชั้นยังไม่ตรงกับเกณฑ์ หรือมีหลายกฎที่ใช้ได้ กรุณาตรวจการตั้งค่าลำดับชั้น".into(),
            ));
        }
    }
    Ok(version)
}
