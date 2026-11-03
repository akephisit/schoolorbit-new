use crate::models::*;
use school_academic_core::models::{AcademicYearStatus, GradeProgression, GradeProgressionKind};
use school_academic_core::services::{lifecycle_context, promotion_students, promotion_targets};
use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub async fn options(pool: &PgPool, actor: &ActorContext) -> Result<YearRolloverOptions, AppError> {
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_lifecycle.year_rollover.options",
    )
    .await?;
    let target_years=DbSession::new(&mut connection).query_as(
        "academic_lifecycle.year_rollover.options",
        "SELECT id AS academic_year_id,year,name,status,row_version FROM academic_years WHERE status='planning' ORDER BY year,id LIMIT 501",
    ).fetch_all().await?;
    let result = YearRolloverOptions { target_years };
    if result.target_years.len() > 500 {
        return Err(AppError::ValidationError("จำนวนปีการศึกษาเกินขอบเขต".into()));
    }
    Ok(result)
}

pub async fn preview(
    pool: &PgPool,
    actor: &ActorContext,
    input: YearRolloverPreviewInput,
) -> Result<YearRolloverPreview, AppError> {
    actor.require_permission(codes::ACADEMIC_PROMOTION_READ_SCHOOL)?;
    validate_input(&input)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_lifecycle.year_rollover.preview",
    )
    .await?;
    DbSession::new(&mut tx)
        .query(
            "academic_lifecycle.year_rollover.preview",
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY",
        )
        .execute()
        .await?;
    let result = preview_in_transaction(&mut tx, &input).await?;
    tx.commit().await?;
    Ok(result)
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub(super) struct AcceptedDestination {
    pub source_id: Uuid,
    #[sqlx(json)]
    pub decision: PromotionDecisionInput,
    pub target_student_year_id: Option<Uuid>,
    pub target_placement_id: Option<Uuid>,
}

pub(super) async fn accepted_destinations(
    tx: &mut Transaction<'_, Postgres>,
    source: Uuid,
    target: Uuid,
) -> Result<BTreeMap<Uuid, AcceptedDestination>, AppError> {
    let rows:Vec<AcceptedDestination>=DbSession::new(&mut **tx).query_as(
        "academic_lifecycle.year_rollover.accepted_destinations",
        "SELECT DISTINCT ON(item.student_academic_year_id) item.student_academic_year_id AS source_id,
         CASE WHEN correction.id IS NOT NULL THEN correction.replacement_decision ELSE item.decision END AS decision,
         CASE WHEN correction.id IS NOT NULL THEN NULLIF(correction.outcome->>'targetStudentYearId','')::uuid ELSE receipt.target_student_year_id END AS target_student_year_id,
         CASE WHEN correction.id IS NOT NULL THEN NULLIF(correction.outcome->>'targetPlacementId','')::uuid ELSE receipt.target_placement_id END AS target_placement_id
         FROM academic_promotion_execution_receipts receipt
         JOIN academic_promotion_run_items item ON item.id=receipt.item_id AND item.run_id=receipt.run_id
         LEFT JOIN LATERAL(SELECT id,replacement_decision,outcome FROM academic_promotion_impact_resolutions
           WHERE item_id=item.id AND resolution_kind='replace_decision' ORDER BY resolved_at DESC,id DESC LIMIT 1) correction ON true
         WHERE receipt.source_year_id=$1 AND receipt.target_year_id=$2
         ORDER BY item.student_academic_year_id,receipt.executed_at DESC,item.id DESC LIMIT 10001",
    ).bind(source).bind(target).fetch_all().await?;
    if rows.len() > 10_000 {
        return Err(AppError::ValidationError("หลักฐานนักเรียนเกินขอบเขต".into()));
    }
    Ok(rows.into_iter().map(|row| (row.source_id, row)).collect())
}

pub(super) async fn source_contexts(
    tx: &mut Transaction<'_, Postgres>,
    input: &YearRolloverPreviewInput,
) -> Result<Vec<promotion_students::PromotionStudentContext>, AppError> {
    let ids:Vec<Uuid>=DbSession::new(&mut **tx).scalar(
        "academic_lifecycle.year_rollover.source_contexts",
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 AND status IN ('active','completed') ORDER BY id LIMIT 10001",
    ).bind(input.source_year_id).fetch_all().await?;
    if ids.len() > 10_000 {
        return Err(AppError::ValidationError("นักเรียนต้นทางเกิน 10,000 คน".into()));
    }
    let mut sources = Vec::with_capacity(ids.len());
    for batch in ids.chunks(500) {
        sources.extend(
            promotion_students::read_promotion_students(
                tx,
                input.source_year_id,
                input.target_year_id,
                batch,
            )
            .await?,
        );
    }
    Ok(sources)
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct SourcePlacement {
    student_academic_year_id: Uuid,
    id: Uuid,
    homeroom_id: Uuid,
    name: String,
    start_date: chrono::NaiveDate,
    end_date: Option<chrono::NaiveDate>,
    status: String,
    row_version: i64,
}

pub(super) async fn preview_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    input: &YearRolloverPreviewInput,
) -> Result<YearRolloverPreview, AppError> {
    validate_input(input)?;
    let source_year = lifecycle_context::read_year_context(tx, input.source_year_id).await?;
    let target_year = lifecycle_context::read_year_context(tx, input.target_year_id).await?;
    if source_year.year >= target_year.year {
        return Err(AppError::ValidationError("ปีปลายทางต้องอยู่หลังปีต้นทาง".into()));
    }
    let sources = source_contexts(tx, input).await?;
    let accepted = accepted_destinations(tx, input.source_year_id, input.target_year_id).await?;
    let mut decisions: BTreeMap<_, _> = input
        .decisions
        .iter()
        .map(|row| (row.student_academic_year_id, row.decision.clone()))
        .collect();
    let grades:Vec<PromotionGradeReference>=DbSession::new(&mut **tx).query_as(
        "academic_lifecycle.year_rollover.preview_in_transaction",
        "SELECT id,level_type,year,is_active IS TRUE AS is_active FROM grade_levels ORDER BY level_type,year,id LIMIT 501",
    ).fetch_all().await?;
    let programs:Vec<YearRolloverProgramReference>=DbSession::new(&mut **tx).query_as(
        "academic_lifecycle.year_rollover.preview_in_transaction",
        "SELECT p.id,p.code,p.name_th AS name,level.id AS curriculum_level_id,level.name_th AS level_name,edition.name AS edition_name,COALESCE(level.grade_level_ids,'[]'::jsonb) AS grade_level_ids
         FROM published_study_programs p JOIN published_curriculum_levels level ON level.id=p.curriculum_level_id
         JOIN curriculum_editions edition ON edition.id=level.edition_id
         WHERE p.status='published' AND level.is_active AND edition.is_active IS TRUE AND edition.status='published'
         ORDER BY level.id,p.display_order,p.code,p.id LIMIT 5001",
    ).fetch_all().await?;
    let homerooms:Vec<YearRolloverHomeroomReference>=DbSession::new(&mut **tx).query_as(
        "academic_lifecycle.year_rollover.preview_in_transaction",
        "SELECT room.id,room.name,room.grade_level_id,room.study_program_id,room.capacity,
         count(DISTINCT placement.student_academic_year_id) AS occupied_count
         FROM homerooms room LEFT JOIN homeroom_placements placement ON placement.homeroom_id=room.id AND placement.status IN ('planned','current')
         WHERE room.academic_year_id=$1 AND room.is_active IS TRUE
         GROUP BY room.id ORDER BY room.name,room.id LIMIT 5001",
    ).bind(input.target_year_id).fetch_all().await?;
    let mappings:Vec<GradeProgression>=DbSession::new(&mut **tx).query_as(
        "academic_lifecycle.year_rollover.preview_in_transaction",
        "SELECT id,from_grade_level_id,to_grade_level_id,transition_kind,curriculum_level_id,is_active,created_at,updated_at
         FROM grade_level_progressions WHERE is_active ORDER BY id LIMIT 5001",
    ).fetch_all().await?;
    if grades.len() > 500
        || programs.len() > 5000
        || homerooms.len() > 5000
        || mappings.len() > 5000
    {
        return Err(AppError::ValidationError(
            "ข้อมูลอ้างอิงเกินขอบเขตการแสดงผล".into(),
        ));
    }
    let placements:Vec<SourcePlacement>=DbSession::new(&mut **tx).query_as(
        "academic_lifecycle.year_rollover.preview_in_transaction",
        "SELECT placement.student_academic_year_id,placement.id,placement.homeroom_id,room.name,placement.start_date,placement.end_date,placement.status,placement.row_version
         FROM homeroom_placements placement JOIN homerooms room ON room.id=placement.homeroom_id
         WHERE placement.academic_year_id=$1 ORDER BY placement.student_academic_year_id,
           CASE placement.status WHEN 'current' THEN 0 WHEN 'planned' THEN 1 ELSE 2 END,placement.start_date DESC,placement.id DESC LIMIT 100001",
    ).bind(input.source_year_id).fetch_all().await?;
    if placements.len() > 100_000 {
        return Err(AppError::ValidationError("ประวัติห้องต้นทางเกินขอบเขต".into()));
    }
    let inflight:bool=DbSession::new(&mut **tx).scalar(
        "academic_lifecycle.year_rollover.preview_in_transaction",
        "SELECT EXISTS(SELECT 1 FROM academic_promotion_runs WHERE source_year_id=$1 AND target_year_id=$2 AND status='executing') OR EXISTS(
          SELECT 1 FROM academic_promotion_execution_batches batch JOIN academic_promotion_runs run ON run.id=batch.run_id
          CROSS JOIN LATERAL unnest(batch.item_ids) pending(id)
          LEFT JOIN academic_promotion_execution_receipts receipt ON receipt.item_id=pending.id AND receipt.run_id=run.id
          WHERE run.source_year_id=$1 AND run.target_year_id=$2 AND receipt.item_id IS NULL)",
    ).bind(input.source_year_id).bind(input.target_year_id).fetch_one().await?;
    let today: chrono::NaiveDate = DbSession::new(&mut **tx)
        .scalar(
            "academic_lifecycle.year_rollover.preview_in_transaction",
            "SELECT (now() AT TIME ZONE 'Asia/Bangkok')::date",
        )
        .fetch_one()
        .await?;
    let other_successors:Vec<Uuid>=DbSession::new(&mut **tx).scalar(
        "academic_lifecycle.year_rollover.preview_in_transaction",
        "SELECT DISTINCT item.student_academic_year_id FROM academic_promotion_execution_receipts receipt
         JOIN academic_promotion_run_items item ON item.id=receipt.item_id AND item.run_id=receipt.run_id
         JOIN student_academic_years successor ON successor.id=receipt.target_student_year_id AND successor.status IN ('planned','active','completed','graduated')
         WHERE receipt.source_year_id=$1 AND receipt.target_year_id<>$2 ORDER BY item.student_academic_year_id LIMIT 10001",
    ).bind(input.source_year_id).bind(input.target_year_id).fetch_all().await?;
    let source_room: BTreeMap<_, _> = placements
        .iter()
        .rev()
        .map(|row| (row.student_academic_year_id, row))
        .collect();
    let mut findings = vec![];
    let mut block = |code: &str, count: usize, message: &str| {
        if count > 0 {
            findings.push(LifecycleFinding {
                code: code.into(),
                severity: LifecycleSeverity::Blocking,
                count,
                message: message.into(),
                resolution_url: None,
            });
        }
    };
    block(
        "rollover.source_year_open",
        usize::from(source_year.status != AcademicYearStatus::Closed),
        "ต้องปิดปีต้นทางก่อนยืนยันรายชื่อนักเรียนปีใหม่",
    );
    block(
        "rollover.target_year_not_planning",
        usize::from(target_year.status != AcademicYearStatus::Planning),
        "ปีปลายทางต้องอยู่ระหว่างเตรียมการ",
    );
    block(
        "rollover.other_successor_exists",
        other_successors.len(),
        "มีนักเรียนที่เตรียมปีถัดไปอีกปีไว้แล้ว ไม่สามารถสร้างปลายทางซ้ำจากปีต้นทางเดิม",
    );
    block(
        "rollover.execution_unresolved",
        usize::from(inflight),
        "มีงานเลื่อนชั้นเดิมที่ต้องกู้คืนให้เสร็จก่อนดำเนินการ",
    );
    block(
        "rollover.future_outcome_date",
        usize::from(input.outcome_effective_on > today),
        "วันสิ้นสุดต้นทางต้องเป็นวันที่เกิดขึ้นแล้ว",
    );
    block(
        "rollover.empty_roster",
        usize::from(sources.is_empty()),
        "ไม่มีนักเรียนต้นทางที่รอดำเนินการ",
    );
    let mut students = vec![];
    let mut new_targets = vec![];
    let mut prepared_targets = vec![];
    let mut room_additions = BTreeMap::<Uuid, i64>::new();
    for source in &sources {
        let decision = decisions.remove(&source.student_academic_year_id);
        let prior = accepted.get(&source.student_academic_year_id);
        if let Some(decision) = &decision {
            if source.existing_target_student_year_id.is_some() {
                let same = if let Some(prior) = prior.filter(|prior| {
                    prior.target_student_year_id == source.existing_target_student_year_id
                }) {
                    super::checksum(&prior.decision)? == super::checksum(decision)?
                } else {
                    false
                };
                if !same {
                    block(
                        "rollover.target_already_exists",
                        1,
                        "มีข้อมูลปีปลายทางแล้ว ต้องคงผลเดิมและตรวจประวัติก่อนเปลี่ยน",
                    );
                } else if let Some(prior) = prior {
                    if let Some(target) = prior.target_student_year_id {
                        prepared_targets.push(school_academic_core::services::promotion_finalization::PreparedPromotionTarget {
                            student_id:source.student_id,target_student_year_id:target,target_placement_id:prior.target_placement_id,
                            starts_on:input.target_placement_starts_on,grade_level_id:decision.target_grade_level_id,
                            study_program_id:decision.target_study_program_id,homeroom_id:decision.target_homeroom_id,
                        });
                    }
                }
            } else {
                if decision.outcome == PromotionDecisionOutcome::Conditional {
                    block(
                        "rollover.conditional_history_only",
                        1,
                        "ผลมีเงื่อนไขใช้ยืนยันปลายทางเดิมจากประวัติได้เท่านั้น",
                    );
                }
                new_targets.push((source, decision.core_command()));
                if let Some(room) = decision.target_homeroom_id {
                    *room_additions.entry(room).or_default() += 1;
                }
            }
        }
        let room = source_room.get(&source.student_academic_year_id).copied();
        students.push(YearRolloverStudent {
            student_academic_year_id: source.student_academic_year_id,
            student_id: source.student_id,
            student_code: source.student_code.clone(),
            student_name: source.student_name.clone(),
            source_grade_level_id: source.grade_level_id,
            source_study_program_id: source.study_program_id,
            source_homeroom_id: room.map(|row| row.homeroom_id),
            source_homeroom_name: room.map(|row| row.name.clone()),
            source_row_version: source.row_version,
            existing_target_student_year_id: source.existing_target_student_year_id,
            suggested_decision: prior
                .filter(|prior| prior.target_student_year_id.is_some())
                .map(|prior| prior.decision.clone())
                .or_else(|| suggest(source, &grades, &programs, &mappings)),
            decision,
        });
    }
    for batch in prepared_targets.chunks(500) {
        if let Err(error) =
            school_academic_core::services::promotion_finalization::verify_prepared_targets(
                tx,
                input.target_year_id,
                batch,
            )
            .await
        {
            match error {
                AppError::Conflict(message) | AppError::ValidationError(message) => {
                    block("rollover.target_changed", 1, &message)
                }
                other => return Err(other),
            }
        }
    }
    if !decisions.is_empty() {
        return Err(AppError::Conflict(
            "รายการนักเรียนเปลี่ยนแล้ว กรุณาตรวจตัวอย่างใหม่".into(),
        ));
    }
    block(
        "rollover.decision_missing",
        students.iter().filter(|row| row.decision.is_none()).count(),
        "ระบุผลของนักเรียนทุกรายก่อนยืนยัน",
    );
    block(
        "rollover.placement_date_invalid",
        placements
            .iter()
            .filter(|p| p.status == "current" && p.start_date > input.outcome_effective_on)
            .count(),
        "วันสิ้นสุดต้องไม่อยู่ก่อนวันเริ่มห้องปัจจุบัน",
    );
    for room in &homerooms {
        block(
            "rollover.room_capacity",
            usize::from(room.capacity.is_some_and(|cap| {
                room.occupied_count + room_additions.get(&room.id).copied().unwrap_or(0)
                    > i64::from(cap)
            })),
            "จำนวนที่จัดเพิ่มเกินความจุห้องปลายทาง",
        );
    }
    if target_year.status == AcademicYearStatus::Planning {
        for batch in new_targets.chunks(500) {
            let refs: Vec<_> = batch
                .iter()
                .map(|(source, decision)| (*source, decision))
                .collect();
            if let Err(error) =
                promotion_targets::preview_destinations(tx, input.target_year_id, &refs).await
            {
                match error {
                    AppError::Conflict(message) | AppError::ValidationError(message) => {
                        block("rollover.destination_invalid", 1, &message)
                    }
                    other => return Err(other),
                }
            }
        }
    }
    findings.sort_by(|a, b| a.code.cmp(&b.code).then(a.message.cmp(&b.message)));
    let can_confirm = findings.is_empty();
    let source_checksum = super::checksum(&(
        &source_year,
        &target_year,
        &sources,
        &placements,
        &accepted,
        &grades,
        &programs,
        &homerooms,
        &mappings,
        &students,
        input.outcome_effective_on,
        input.target_placement_starts_on,
        inflight,
        &other_successors,
        &findings,
    ))?;
    Ok(YearRolloverPreview {
        source_year,
        target_year,
        outcome_effective_on: input.outcome_effective_on,
        target_placement_starts_on: input.target_placement_starts_on,
        students,
        grades,
        programs,
        homerooms,
        findings,
        can_confirm,
        source_checksum,
    })
}

pub(super) fn validate_input(input: &YearRolloverPreviewInput) -> Result<(), AppError> {
    if input.source_year_id.is_nil()
        || input.target_year_id.is_nil()
        || input.source_year_id == input.target_year_id
        || input.decisions.len() > 10_000
        || input.outcome_effective_on >= input.target_placement_starts_on
        || input
            .decisions
            .iter()
            .map(|row| row.student_academic_year_id)
            .collect::<BTreeSet<_>>()
            .len()
            != input.decisions.len()
    {
        return Err(AppError::ValidationError(
            "ระบุปี วันสิ้นสุดต้นทาง วันเริ่มปลายทาง และนักเรียนไม่ซ้ำให้ถูกต้อง".into(),
        ));
    }
    for row in &input.decisions {
        let d = &row.decision;
        if row.student_academic_year_id.is_nil()
            || (d.condition.is_some() && d.outcome != PromotionDecisionOutcome::Conditional)
            || [
                d.target_grade_level_id,
                d.target_study_program_id,
                d.target_homeroom_id,
            ]
            .into_iter()
            .flatten()
            .any(|id| id.is_nil())
            || [&d.reason, &d.condition].into_iter().flatten().any(|text| {
                text.trim().is_empty()
                    || text.chars().count() > 1000
                    || school_academic_core::services::student_years::contains_thirteen_digit_run(
                        text,
                    )
            })
        {
            return Err(AppError::ValidationError(
                "ผลการจัดนักเรียนหรือเหตุผลไม่ถูกต้อง".into(),
            ));
        }
    }
    Ok(())
}

fn suggest(
    source: &promotion_students::PromotionStudentContext,
    grades: &[PromotionGradeReference],
    programs: &[YearRolloverProgramReference],
    mappings: &[GradeProgression],
) -> Option<PromotionDecisionInput> {
    let grade = grades
        .iter()
        .find(|grade| grade.id == source.grade_level_id)?;
    if explicit_boundary(&grade.level_type, grade.year) {
        return None;
    }
    let source_program = programs.iter().find(|p| p.id == source.study_program_id)?;
    let applicable: Vec<_> = mappings
        .iter()
        .filter(|m| {
            m.from_grade_level_id == grade.id
                && m.transition_kind == GradeProgressionKind::Promote
                && m.curriculum_level_id
                    .is_none_or(|id| id == source_program.curriculum_level_id)
        })
        .collect();
    if applicable.len() != 1 {
        return None;
    }
    let target = applicable[0].to_grade_level_id?;
    if !source_program.grade_level_ids.contains(&target) {
        return None;
    }
    Some(PromotionDecisionInput {
        outcome: PromotionDecisionOutcome::Promote,
        target_grade_level_id: Some(target),
        target_study_program_id: Some(source_program.id),
        target_homeroom_id: None,
        reason: None,
        condition: None,
    })
}

fn explicit_boundary(level: &str, year: i32) -> bool {
    matches!((level, year), ("primary", 6) | ("secondary", 3 | 6))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primary_six_and_secondary_boundaries_never_implicitly_graduate_or_promote() {
        for (level, year) in [("primary", 6), ("secondary", 3), ("secondary", 6)] {
            assert!(explicit_boundary(level, year));
        }
        assert!(!explicit_boundary("primary", 5));
    }
    #[test]
    fn manual_input_requires_distinct_years_and_actual_ordered_dates() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
        let mut input = YearRolloverPreviewInput {
            source_year_id: Uuid::new_v4(),
            target_year_id: Uuid::new_v4(),
            outcome_effective_on: date,
            target_placement_starts_on: date.succ_opt().unwrap(),
            decisions: vec![],
        };
        assert!(validate_input(&input).is_ok());
        input.target_placement_starts_on = date;
        assert!(validate_input(&input).is_err());
    }

    #[test]
    fn manual_reasons_reject_plain_formatted_and_thai_national_identifiers() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
        let mut input = YearRolloverPreviewInput {
            source_year_id: Uuid::new_v4(),
            target_year_id: Uuid::new_v4(),
            outcome_effective_on: date,
            target_placement_starts_on: date.succ_opt().unwrap(),
            decisions: vec![YearRolloverStudentInput {
                student_academic_year_id: Uuid::new_v4(),
                decision: PromotionDecisionInput {
                    outcome: PromotionDecisionOutcome::Hold,
                    target_grade_level_id: None,
                    target_study_program_id: None,
                    target_homeroom_id: None,
                    reason: None,
                    condition: None,
                },
            }],
        };
        for reason in ["1234567890123", "1-2345-67890-12-3", "๑๒๓๔๕๖๗๘๙๐๑๒๓"]
        {
            input.decisions[0].decision.reason = Some(reason.into());
            assert!(validate_input(&input).is_err());
        }
        input.decisions[0].decision.reason = Some("โรงเรียนยืนยันตามทะเบียนภายนอก".into());
        assert!(validate_input(&input).is_ok());
    }
}
