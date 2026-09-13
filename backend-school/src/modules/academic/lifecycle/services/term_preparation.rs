use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::super::models::{
    ApplyTermPreparationInput, LifecycleFinding, LifecycleSeverity, PreviewTermPreparationInput,
    TermPreparationContext, TermPreparationDateRequirement, TermPreparationMappingKind,
    TermPreparationMappingOption, TermPreparationMappingRequirement, TermPreparationMappings,
    TermPreparationModule, TermPreparationModuleEvidence, TermPreparationModuleOutcome,
    TermPreparationOutcome, TermPreparationWorkspace,
};
use crate::{
    error::AppError,
    middleware::permission::ActorContext,
    modules::academic::{
        core::services::{lifecycle_guard, years_terms},
        delivery::services::offerings,
    },
    permissions::registry::codes,
};

const MAX_ENTITY_MAPPINGS: usize = 2_000;
const MAX_DATE_MAPPINGS: usize = 366;

#[derive(Debug, Clone, Serialize)]
pub(super) struct NormalizedPreparationInput {
    pub source_term_id: Uuid,
    pub target_term_id: Uuid,
    pub modules: Vec<TermPreparationModule>,
    pub mappings: TermPreparationMappings,
}

#[derive(Debug, Clone, FromRow)]
struct ContextRow {
    source_term_id: Uuid,
    source_year_id: Uuid,
    source_label: String,
    source_status: String,
    source_row_version: i64,
    source_start_date: NaiveDate,
    target_term_id: Uuid,
    target_year_id: Uuid,
    target_year_status: String,
    target_label: String,
    target_status: String,
    target_row_version: i64,
    target_start_date: NaiveDate,
    target_end_date: NaiveDate,
}

#[derive(Debug, Clone, FromRow)]
struct MappingSeed {
    source_id: Uuid,
    source_label: String,
    suggested_target_id: Option<Uuid>,
}

#[derive(Debug, Clone, FromRow)]
struct DateSeed {
    source_date: NaiveDate,
    source_label: String,
}

#[derive(Debug, Clone)]
struct PreviewState {
    workspace: TermPreparationWorkspace,
    delivery_preview: Option<crate::modules::academic::delivery::models::CurriculumOfferingPreview>,
}

pub(super) fn validate_checksum(value: &str) -> Result<(), AppError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(AppError::ValidationError("sourceChecksum ไม่ถูกต้อง".into()))
    }
}

pub(super) fn normalize_input(
    source_term_id: Uuid,
    target_term_id: Uuid,
    modules: &[TermPreparationModule],
    mappings: &TermPreparationMappings,
) -> Result<NormalizedPreparationInput, AppError> {
    if source_term_id.is_nil() || target_term_id.is_nil() || source_term_id == target_term_id {
        return Err(AppError::ValidationError(
            "ต้องเลือกภาคเรียนต้นทางและภาคเรียนเป้าหมายคนละภาคเรียน".into(),
        ));
    }
    if modules.is_empty() || modules.len() > TermPreparationModule::ALL.len() {
        return Err(AppError::ValidationError(
            "เลือกอย่างน้อยหนึ่งส่วนงานที่จะเตรียม".into(),
        ));
    }
    let mut normalized_modules = modules.to_vec();
    normalized_modules.sort_unstable();
    if normalized_modules
        .windows(2)
        .any(|window| window[0] == window[1])
    {
        return Err(AppError::ValidationError("ส่วนงานที่เลือกซ้ำกัน".into()));
    }
    if mappings.entities.len() > MAX_ENTITY_MAPPINGS || mappings.dates.len() > MAX_DATE_MAPPINGS {
        return Err(AppError::ValidationError("จำนวนการจับคู่มากเกินขอบเขต".into()));
    }
    let mut normalized_mappings = mappings.clone();
    normalized_mappings
        .entities
        .sort_by_key(|mapping| (mapping.kind, mapping.source_id, mapping.target_id));
    let mut source_keys = HashSet::new();
    let mut target_keys = HashSet::new();
    for mapping in &normalized_mappings.entities {
        if mapping.source_id.is_nil() || mapping.target_id.is_nil() {
            return Err(AppError::ValidationError("รหัสในการจับคู่ต้องไม่เป็นค่าว่าง".into()));
        }
        if !source_keys.insert((mapping.kind, mapping.source_id)) {
            return Err(AppError::ValidationError(
                "ข้อมูลต้นทางหนึ่งรายการจับคู่ปลายทางได้เพียงรายการเดียว".into(),
            ));
        }
        if !target_keys.insert((mapping.kind, mapping.target_id)) {
            return Err(AppError::ValidationError(
                "ข้อมูลปลายทางหนึ่งรายการรับการจับคู่ได้เพียงรายการเดียว".into(),
            ));
        }
    }
    normalized_mappings
        .dates
        .sort_by_key(|mapping| (mapping.source_date, mapping.target_date));
    let mut source_dates = HashSet::new();
    for mapping in &normalized_mappings.dates {
        if !source_dates.insert(mapping.source_date) {
            return Err(AppError::ValidationError(
                "วันที่ต้นทางหนึ่งวันจับคู่ได้เพียงวันเดียว".into(),
            ));
        }
    }
    Ok(NormalizedPreparationInput {
        source_term_id,
        target_term_id,
        modules: normalized_modules,
        mappings: normalized_mappings,
    })
}

pub async fn preview(
    pool: &PgPool,
    actor: &ActorContext,
    input: PreviewTermPreparationInput,
) -> Result<TermPreparationWorkspace, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL)?;
    let normalized = normalize_input(
        input.source_term_id,
        input.target_term_id,
        &input.modules,
        &input.mappings,
    )?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    lifecycle_guard::lock_transition_shared(&mut tx).await?;
    let state = build_workspace(&mut tx, &normalized).await?;
    tx.commit().await?;
    Ok(state.workspace)
}

async fn load_context(
    tx: &mut Transaction<'_, Postgres>,
    source_term_id: Uuid,
    target_term_id: Uuid,
) -> Result<ContextRow, AppError> {
    let context: ContextRow = sqlx::query_as(
        r#"SELECT source.id AS source_term_id,
                  source.academic_year_id AS source_year_id,
                  source_year.name || ' · ' || source.name AS source_label,
                  source.status::text AS source_status,
                  source.row_version AS source_row_version,
                  source.start_date AS source_start_date,
                  target.id AS target_term_id,
                  target.academic_year_id AS target_year_id,
                  target_year.status::text AS target_year_status,
                  target_year.name || ' · ' || target.name AS target_label,
                  target.status::text AS target_status,
                  target.row_version AS target_row_version,
                  target.start_date AS target_start_date,
                  COALESCE(target.planned_end_date, target_year.end_date) AS target_end_date
           FROM academic_terms source
           JOIN academic_years source_year ON source_year.id=source.academic_year_id
           CROSS JOIN academic_terms target
           JOIN academic_years target_year ON target_year.id=target.academic_year_id
           WHERE source.id=$1 AND target.id=$2"#,
    )
    .bind(source_term_id)
    .bind(target_term_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนต้นทางหรือภาคเรียนเป้าหมาย".into()))?;
    if context.source_status != "closed" {
        return Err(AppError::Conflict(
            "เตรียมภาคเรียนถัดไปได้เมื่อภาคเรียนต้นทางปิดแล้ว".into(),
        ));
    }
    if context.target_status != "planning" {
        return Err(AppError::Conflict(
            "ภาคเรียนเป้าหมายต้องอยู่ในสถานะวางแผน".into(),
        ));
    }
    if matches!(context.target_year_status.as_str(), "closed" | "archived") {
        return Err(AppError::Conflict(
            "ปีการศึกษาเป้าหมายปิดแล้วหรือเก็บถาวรแล้ว".into(),
        ));
    }
    if context.target_start_date <= context.source_start_date {
        return Err(AppError::Conflict(
            "ภาคเรียนเป้าหมายต้องเริ่มหลังภาคเรียนต้นทาง".into(),
        ));
    }
    Ok(context)
}

async fn count(
    tx: &mut Transaction<'_, Postgres>,
    sql: &str,
    term_id: Uuid,
) -> Result<usize, AppError> {
    let value: i64 = sqlx::query_scalar(sql)
        .bind(term_id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(value as usize)
}

async fn json_rows(
    tx: &mut Transaction<'_, Postgres>,
    sql: &str,
    term_id: Uuid,
) -> Result<Vec<serde_json::Value>, AppError> {
    let rows: Vec<sqlx::types::Json<serde_json::Value>> = sqlx::query_scalar(sql)
        .bind(term_id)
        .fetch_all(&mut **tx)
        .await?;
    Ok(rows.into_iter().map(|row| row.0).collect())
}

async fn module_source_fingerprint(
    tx: &mut Transaction<'_, Postgres>,
    term_id: Uuid,
    module: TermPreparationModule,
) -> Result<String, AppError> {
    let mut tables = Vec::new();
    match module {
        TermPreparationModule::Delivery => {}
        TermPreparationModule::Assessments => {
            tables.push((
                "plans",
                json_rows(
                    tx,
                    "SELECT to_jsonb(plan) FROM course_assessment_plans plan WHERE plan.academic_term_id=$1 ORDER BY plan.id",
                    term_id,
                )
                .await?,
            ));
            tables.push((
                "phases",
                json_rows(
                    tx,
                    "SELECT to_jsonb(phase) FROM course_assessment_phases phase JOIN course_assessment_plans plan ON plan.id=phase.plan_id WHERE plan.academic_term_id=$1 ORDER BY plan.id,phase.phase_code,phase.id",
                    term_id,
                )
                .await?,
            ));
        }
        TermPreparationModule::Timetable => {
            let selected = "SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1";
            tables.push((
                "version",
                json_rows(
                    tx,
                    &format!("SELECT to_jsonb(version) FROM academic_timetable_versions version WHERE version.id=({selected})"),
                    term_id,
                )
                .await?,
            ));
            tables.push((
                "targets",
                json_rows(
                    tx,
                    &format!("SELECT to_jsonb(target) FROM academic_timetable_version_targets target WHERE target.timetable_version_id=({selected}) ORDER BY target.learning_offering_id"),
                    term_id,
                )
                .await?,
            ));
            for (name, sql) in [
                ("blocks", "SELECT to_jsonb(block) FROM academic_timetable_blocks block WHERE block.timetable_version_id=(SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1) AND block.is_active ORDER BY block.id"),
                ("groups", "SELECT to_jsonb(block_group) FROM academic_timetable_block_groups block_group JOIN academic_timetable_blocks block ON block.id=block_group.block_id WHERE block.timetable_version_id=(SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1) AND block.is_active AND block_group.is_active ORDER BY block_group.id"),
                ("instructors", "SELECT to_jsonb(instructor) FROM academic_timetable_block_group_instructors instructor JOIN academic_timetable_block_groups block_group ON block_group.id=instructor.block_group_id JOIN academic_timetable_blocks block ON block.id=block_group.block_id WHERE block.timetable_version_id=(SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1) AND block.is_active AND block_group.is_active ORDER BY instructor.id"),
                ("homerooms", "SELECT to_jsonb(homeroom) FROM academic_timetable_block_homerooms homeroom JOIN academic_timetable_blocks block ON block.id=homeroom.block_id WHERE block.timetable_version_id=(SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1) AND block.is_active AND homeroom.is_active ORDER BY homeroom.id"),
                ("teachers", "SELECT to_jsonb(teacher) FROM academic_timetable_block_teachers teacher JOIN academic_timetable_blocks block ON block.id=teacher.block_id WHERE block.timetable_version_id=(SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1) AND block.is_active AND teacher.is_active ORDER BY teacher.id"),
            ] {
                tables.push((name, json_rows(tx, sql, term_id).await?));
            }
        }
        TermPreparationModule::Exams => {
            for (name, sql) in [
                ("rounds", "SELECT to_jsonb(exam_round) FROM academic_exam_rounds exam_round WHERE exam_round.academic_term_id=$1 ORDER BY exam_round.id"),
                ("days", "SELECT to_jsonb(day) FROM academic_exam_days day WHERE day.academic_term_id=$1 ORDER BY day.id"),
                ("gradeLevels", "SELECT to_jsonb(grade) FROM academic_exam_day_grade_levels grade JOIN academic_exam_days day ON day.id=grade.exam_day_id WHERE day.academic_term_id=$1 ORDER BY grade.exam_day_id,grade.grade_level_id"),
                ("blockedWindows", "SELECT to_jsonb(blocked_window) FROM academic_exam_day_blocked_windows blocked_window JOIN academic_exam_days day ON day.id=blocked_window.exam_day_id WHERE day.academic_term_id=$1 ORDER BY blocked_window.id"),
                ("roomAssignments", "SELECT to_jsonb(assignment) FROM academic_exam_day_room_assignments assignment WHERE assignment.academic_term_id=$1 ORDER BY assignment.id"),
                ("items", "SELECT to_jsonb(item) FROM academic_exam_schedule_items item WHERE item.academic_term_id=$1 ORDER BY item.id"),
                ("sessions", "SELECT to_jsonb(session) FROM academic_exam_sessions session JOIN academic_exam_schedule_items item ON item.id=session.exam_schedule_item_id WHERE item.academic_term_id=$1 ORDER BY session.id"),
            ] {
                tables.push((name, json_rows(tx, sql, term_id).await?));
            }
        }
        TermPreparationModule::Supervision => {
            tables.push((
                "cycles",
                json_rows(
                    tx,
                    "SELECT to_jsonb(cycle) FROM supervision_cycles cycle WHERE cycle.academic_term_id=$1 ORDER BY cycle.id",
                    term_id,
                )
                .await?,
            ));
            tables.push((
                "targets",
                json_rows(
                    tx,
                    "SELECT to_jsonb(target) FROM supervision_cycle_targets target JOIN supervision_cycles cycle ON cycle.id=target.cycle_id WHERE cycle.academic_term_id=$1 ORDER BY target.id",
                    term_id,
                )
                .await?,
            ));
        }
    }
    super::checksum(&tables)
}

async fn build_workspace(
    tx: &mut Transaction<'_, Postgres>,
    normalized: &NormalizedPreparationInput,
) -> Result<PreviewState, AppError> {
    let context = load_context(tx, normalized.source_term_id, normalized.target_term_id).await?;
    let mut findings = Vec::new();
    let mut evidence = Vec::new();
    let mut source_fingerprints = Vec::new();
    let mut delivery_preview = None;

    for module in &normalized.modules {
        let (source_count, target_existing_count, draft_count, summary) = match module {
            TermPreparationModule::Delivery => {
                let source = count(
                    tx,
                    "SELECT count(*) FROM learning_offerings WHERE academic_term_id=$1",
                    context.source_term_id,
                )
                .await?;
                let target = count(
                    tx,
                    "SELECT count(*) FROM learning_offerings WHERE academic_term_id=$1",
                    context.target_term_id,
                )
                .await?;
                match offerings::preview_term_preparation(tx, context.target_term_id).await {
                    Ok(preview) => {
                        let drafts = preview.proposals.len();
                        for proposal in &preview.proposals {
                            for conflict in &proposal.conflicts {
                                findings.push(blocking(
                                    format!("delivery.{}", conflict.code),
                                    conflict.message.clone(),
                                ));
                            }
                        }
                        delivery_preview = Some(preview);
                        (
                            source,
                            target,
                            drafts,
                            format!("เตรียม {} รายการจากหลักสูตรเป้าหมาย", drafts),
                        )
                    }
                    Err(error) => {
                        findings.push(blocking("delivery.target_curriculum", error.to_string()));
                        (source, target, 0, "ยังเตรียมจากหลักสูตรเป้าหมายไม่ได้".into())
                    }
                }
            }
            TermPreparationModule::Assessments => {
                let source = count(
                    tx,
                    "SELECT count(*) FROM course_assessment_plans WHERE academic_term_id=$1",
                    context.source_term_id,
                )
                .await?;
                let target = count(
                    tx,
                    "SELECT count(*) FROM course_assessment_plans WHERE academic_term_id=$1",
                    context.target_term_id,
                )
                .await?;
                if target > 0 {
                    findings.push(non_pristine(*module, target));
                }
                (
                    source,
                    target,
                    source,
                    format!("คัดลอกโครงสร้าง 4 ช่วง {} รายวิชา", source),
                )
            }
            TermPreparationModule::Timetable => {
                let source = count(tx, "SELECT count(*) FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published'", context.source_term_id).await?;
                let target = count(tx, "SELECT count(*) FROM academic_timetable_versions WHERE academic_term_id=$1 AND status <> 'cancelled'", context.target_term_id).await?;
                if target > 0 {
                    findings.push(non_pristine(*module, target));
                }
                (
                    source,
                    target,
                    usize::from(source > 0),
                    if source > 0 {
                        "สร้างแบบร่างหนึ่งรุ่นจากตารางที่เผยแพร่ล่าสุด".into()
                    } else {
                        "ต้นทางไม่มีตารางสอนที่เผยแพร่".into()
                    },
                )
            }
            TermPreparationModule::Exams => {
                let source = count(
                    tx,
                    "SELECT count(*) FROM academic_exam_rounds WHERE academic_term_id=$1",
                    context.source_term_id,
                )
                .await?;
                let target = count(
                    tx,
                    "SELECT count(*) FROM academic_exam_rounds WHERE academic_term_id=$1",
                    context.target_term_id,
                )
                .await?;
                if target > 0 {
                    findings.push(non_pristine(*module, target));
                }
                (
                    source,
                    target,
                    source,
                    format!("สร้างแบบร่างรอบสอบ {} รอบ", source),
                )
            }
            TermPreparationModule::Supervision => {
                let source = count(
                    tx,
                    "SELECT count(*) FROM supervision_cycles WHERE academic_term_id=$1",
                    context.source_term_id,
                )
                .await?;
                let target = count(
                    tx,
                    "SELECT count(*) FROM supervision_cycles WHERE academic_term_id=$1",
                    context.target_term_id,
                )
                .await?;
                if target > 0 {
                    findings.push(non_pristine(*module, target));
                }
                (
                    source,
                    target,
                    source,
                    format!("สร้างแบบร่างรอบนิเทศ {} รอบ", source),
                )
            }
        };
        evidence.push(TermPreparationModuleEvidence {
            module: *module,
            source_count,
            target_existing_count,
            draft_count,
            status: if target_existing_count > 0 && *module != TermPreparationModule::Delivery {
                LifecycleSeverity::Blocking
            } else {
                LifecycleSeverity::Ready
            },
            summary,
        });
        if *module != TermPreparationModule::Delivery {
            source_fingerprints.push((
                *module,
                module_source_fingerprint(tx, context.source_term_id, *module).await?,
            ));
        }
    }

    if normalized
        .modules
        .contains(&TermPreparationModule::Delivery)
        && delivery_preview.is_none()
    {
        return finalize_workspace(
            context,
            normalized,
            evidence,
            source_fingerprints,
            Vec::new(),
            Vec::new(),
            findings,
            None,
        );
    }

    // Later modules may be prepared in the same atomic command. In that case,
    // prospective deterministic Delivery IDs are valid mapping options before
    // the rows are inserted.
    if delivery_preview.is_none()
        && normalized
            .modules
            .iter()
            .any(|module| *module != TermPreparationModule::Delivery)
    {
        delivery_preview = offerings::preview_term_preparation(tx, context.target_term_id)
            .await
            .ok();
    }
    let prospective =
        prospective_delivery_options(context.target_term_id, delivery_preview.as_ref());
    let (mapping_requirements, mapping_findings) =
        mapping_requirements(tx, &context, normalized, &prospective).await?;
    findings.extend(mapping_findings);
    findings.extend(
        mapping_dependency_findings(
            tx,
            &context,
            normalized,
            &mapping_requirements,
            &prospective,
        )
        .await?,
    );
    let (date_requirements, date_findings) = date_requirements(tx, &context, normalized).await?;
    findings.extend(date_findings);
    finalize_workspace(
        context,
        normalized,
        evidence,
        source_fingerprints,
        mapping_requirements,
        date_requirements,
        findings,
        delivery_preview,
    )
}

fn finalize_workspace(
    context: ContextRow,
    normalized: &NormalizedPreparationInput,
    modules: Vec<TermPreparationModuleEvidence>,
    source_fingerprints: Vec<(TermPreparationModule, String)>,
    mapping_requirements: Vec<TermPreparationMappingRequirement>,
    date_requirements: Vec<TermPreparationDateRequirement>,
    mut findings: Vec<LifecycleFinding>,
    delivery_preview: Option<crate::modules::academic::delivery::models::CurriculumOfferingPreview>,
) -> Result<PreviewState, AppError> {
    findings.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then(left.message.cmp(&right.message))
    });
    findings.dedup_by(|left, right| left.code == right.code && left.message == right.message);
    let public_context = TermPreparationContext {
        source_term_id: context.source_term_id,
        source_year_id: context.source_year_id,
        source_label: context.source_label,
        source_status: context.source_status,
        target_term_id: context.target_term_id,
        target_year_id: context.target_year_id,
        target_label: context.target_label,
        target_status: context.target_status,
        target_start_date: context.target_start_date,
        target_end_date: context.target_end_date,
    };
    let source_checksum = super::checksum(&(
        normalized,
        context.source_row_version,
        context.target_row_version,
        delivery_preview
            .as_ref()
            .map(|preview| &preview.source_hash),
        &source_fingerprints,
        &modules,
        &mapping_requirements,
        &date_requirements,
        &findings,
    ))?;
    let can_apply = !findings
        .iter()
        .any(|finding| finding.severity == LifecycleSeverity::Blocking);
    Ok(PreviewState {
        workspace: TermPreparationWorkspace {
            context: public_context,
            modules,
            mapping_requirements,
            date_requirements,
            findings,
            can_apply,
            source_checksum,
        },
        delivery_preview,
    })
}

fn blocking(code: impl Into<String>, message: impl Into<String>) -> LifecycleFinding {
    LifecycleFinding {
        code: code.into(),
        severity: LifecycleSeverity::Blocking,
        count: 1,
        message: message.into(),
        resolution_url: None,
    }
}

fn non_pristine(module: TermPreparationModule, count: usize) -> LifecycleFinding {
    LifecycleFinding {
        code: format!("{}.target_not_pristine", module.as_str()),
        severity: LifecycleSeverity::Blocking,
        count,
        message: format!(
            "{} ของภาคเรียนเป้าหมายมีข้อมูลอยู่แล้ว ระบบจะไม่เขียนทับ",
            module.label_th()
        ),
        resolution_url: None,
    }
}

#[derive(Default)]
struct ProspectiveDeliveryOptions {
    offerings: Vec<TermPreparationMappingOption>,
    groups: Vec<TermPreparationMappingOption>,
    group_offering_ids: HashMap<Uuid, Uuid>,
}

fn prospective_delivery_options(
    target_term_id: Uuid,
    preview: Option<&crate::modules::academic::delivery::models::CurriculumOfferingPreview>,
) -> ProspectiveDeliveryOptions {
    let mut options = ProspectiveDeliveryOptions::default();
    let Some(preview) = preview else {
        return options;
    };
    for proposal in &preview.proposals {
        let offering_id = proposal.existing_offering_id.unwrap_or_else(|| {
            offerings::prepared_offering_id(
                target_term_id,
                proposal.resource_kind,
                proposal.catalog_version_id,
            )
        });
        options.offerings.push(TermPreparationMappingOption {
            id: offering_id,
            label: format!("{} · {}", proposal.code, proposal.name),
        });
        for group in &proposal.default_groups {
            let group_id = offerings::prepared_group_id(offering_id, &group.group_key);
            options.groups.push(TermPreparationMappingOption {
                id: group_id,
                label: group.name.clone(),
            });
            options.group_offering_ids.insert(group_id, offering_id);
        }
    }
    options
}

async fn mapping_requirements(
    tx: &mut Transaction<'_, Postgres>,
    context: &ContextRow,
    normalized: &NormalizedPreparationInput,
    prospective: &ProspectiveDeliveryOptions,
) -> Result<
    (
        Vec<TermPreparationMappingRequirement>,
        Vec<LifecycleFinding>,
    ),
    AppError,
> {
    let mut seeds: BTreeMap<
        (TermPreparationMappingKind, Uuid),
        (String, Option<Uuid>, BTreeSet<TermPreparationModule>),
    > = BTreeMap::new();
    for module in &normalized.modules {
        for (kind, rows) in module_mapping_seeds(tx, context, *module).await? {
            for row in rows {
                let entry = seeds.entry((kind, row.source_id)).or_insert_with(|| {
                    (row.source_label, row.suggested_target_id, BTreeSet::new())
                });
                entry.2.insert(*module);
            }
        }
    }
    let provided = normalized
        .mappings
        .entities
        .iter()
        .map(|mapping| ((mapping.kind, mapping.source_id), mapping.target_id))
        .collect::<HashMap<_, _>>();
    let mut options_cache: HashMap<TermPreparationMappingKind, Vec<TermPreparationMappingOption>> =
        HashMap::new();
    let mut requirements = Vec::new();
    let mut findings = Vec::new();
    for ((kind, source_id), (source_label, seed_suggestion, required_by)) in seeds {
        let options = if let Some(options) = options_cache.get(&kind) {
            options.clone()
        } else {
            let mut options = load_target_options(tx, context, kind).await?;
            match kind {
                TermPreparationMappingKind::LearningOffering => {
                    options.extend(prospective.offerings.clone())
                }
                TermPreparationMappingKind::LearningGroup => {
                    options.extend(prospective.groups.clone())
                }
                _ => {}
            }
            options
                .sort_by(|left, right| left.label.cmp(&right.label).then(left.id.cmp(&right.id)));
            options.dedup_by_key(|option| option.id);
            options_cache.insert(kind, options.clone());
            options
        };
        let label_suggestion = options
            .iter()
            .find(|option| option.label == source_label)
            .map(|option| option.id);
        let stable_suggestion =
            seed_suggestion.filter(|id| options.iter().any(|option| option.id == *id));
        let suggested_target_id = stable_suggestion.or(label_suggestion);
        let selected_target_id = provided
            .get(&(kind, source_id))
            .copied()
            .or(suggested_target_id);
        if selected_target_id.is_none() {
            findings.push(blocking(
                format!("mapping.{}.{}", kind.as_str(), source_id),
                format!("ยังไม่ได้จับคู่ {}", source_label),
            ));
        } else if selected_target_id
            .is_some_and(|selected| !options.iter().any(|option| option.id == selected))
        {
            findings.push(blocking(
                format!("mapping.{}.{}.invalid", kind.as_str(), source_id),
                format!("ปลายทางที่เลือกสำหรับ {} ใช้ไม่ได้ในภาคเรียนเป้าหมาย", source_label),
            ));
        }
        requirements.push(TermPreparationMappingRequirement {
            kind,
            source_id,
            source_label,
            required_by: required_by.into_iter().collect(),
            selected_target_id,
            suggested_target_id,
            target_options: options,
        });
    }
    Ok((requirements, findings))
}

async fn mapping_dependency_findings(
    tx: &mut Transaction<'_, Postgres>,
    context: &ContextRow,
    normalized: &NormalizedPreparationInput,
    requirements: &[TermPreparationMappingRequirement],
    prospective: &ProspectiveDeliveryOptions,
) -> Result<Vec<LifecycleFinding>, AppError> {
    let selected = requirements
        .iter()
        .filter_map(|requirement| {
            requirement
                .selected_target_id
                .map(|target_id| ((requirement.kind, requirement.source_id), target_id))
        })
        .collect::<HashMap<_, _>>();
    let mut findings = Vec::new();

    for requirement in requirements
        .iter()
        .filter(|requirement| requirement.kind == TermPreparationMappingKind::LearningGroup)
    {
        let Some(target_group_id) = requirement.selected_target_id else {
            continue;
        };
        let source_offering_id: Uuid =
            sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id=$1")
                .bind(requirement.source_id)
                .fetch_one(&mut **tx)
                .await?;
        let Some(target_offering_id) = selected
            .get(&(
                TermPreparationMappingKind::LearningOffering,
                source_offering_id,
            ))
            .copied()
        else {
            continue;
        };
        let target_group_offering_id = if let Some(offering_id) =
            prospective.group_offering_ids.get(&target_group_id)
        {
            Some(*offering_id)
        } else {
            sqlx::query_scalar(
                "SELECT learning_offering_id FROM learning_groups WHERE id=$1 AND academic_term_id=$2",
            )
            .bind(target_group_id)
            .bind(context.target_term_id)
            .fetch_optional(&mut **tx)
            .await?
        };
        if target_group_offering_id != Some(target_offering_id) {
            findings.push(blocking(
                format!("mapping.learning_group.{}.offering", requirement.source_id),
                format!(
                    "กลุ่มปลายทางของ {} ต้องอยู่ในรายการเปิดสอนที่จับคู่ไว้",
                    requirement.source_label
                ),
            ));
        }
    }

    if normalized.modules.contains(&TermPreparationModule::Exams)
        && !normalized
            .modules
            .contains(&TermPreparationModule::Assessments)
    {
        let source_phases: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT DISTINCT item.learning_offering_id,phase.phase_code
               FROM academic_exam_schedule_items item
               JOIN course_assessment_phases phase ON phase.id=item.assessment_phase_id
               WHERE item.academic_term_id=$1
               ORDER BY item.learning_offering_id,phase.phase_code"#,
        )
        .bind(context.source_term_id)
        .fetch_all(&mut **tx)
        .await?;
        for (source_offering_id, phase_code) in source_phases {
            let Some(target_offering_id) = selected
                .get(&(
                    TermPreparationMappingKind::LearningOffering,
                    source_offering_id,
                ))
                .copied()
            else {
                continue;
            };
            let exists: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(
                       SELECT 1
                       FROM course_assessment_plans plan
                       JOIN course_assessment_phases phase ON phase.plan_id=plan.id
                       WHERE plan.academic_term_id=$1
                         AND plan.learning_offering_id=$2
                         AND phase.phase_code=$3
                   )"#,
            )
            .bind(context.target_term_id)
            .bind(target_offering_id)
            .bind(&phase_code)
            .fetch_one(&mut **tx)
            .await?;
            if !exists {
                findings.push(blocking(
                    format!("exams.assessment_phase.{source_offering_id}.{phase_code}"),
                    "ตารางสอบต้องมีโครงสร้างคะแนนของรายวิชาเป้าหมายก่อน หรือเลือกเตรียมโครงสร้างคะแนนพร้อมกัน",
                ));
            }
        }
    }

    Ok(findings)
}

async fn module_mapping_seeds(
    tx: &mut Transaction<'_, Postgres>,
    context: &ContextRow,
    module: TermPreparationModule,
) -> Result<Vec<(TermPreparationMappingKind, Vec<MappingSeed>)>, AppError> {
    let mut result = Vec::new();
    match module {
        TermPreparationModule::Delivery => {}
        TermPreparationModule::Assessments => {
            result.push((TermPreparationMappingKind::LearningOffering, sqlx::query_as(
                r#"SELECT source.id AS source_id,
                          source.code_snapshot || ' · ' || source.name_snapshot AS source_label,
                          (SELECT target.id FROM learning_offerings target
                           JOIN course_offering_details target_detail ON target_detail.learning_offering_id=target.id
                           WHERE target.academic_term_id=$2 AND target_detail.subject_id=source_detail.subject_id
                           ORDER BY target.id LIMIT 1) AS suggested_target_id
                   FROM course_assessment_plans plan
                   JOIN learning_offerings source ON source.id=plan.learning_offering_id
                   JOIN course_offering_details source_detail ON source_detail.learning_offering_id=source.id
                   WHERE plan.academic_term_id=$1 ORDER BY source.id"#,
            ).bind(context.source_term_id).bind(context.target_term_id).fetch_all(&mut **tx).await?));
            result.push((TermPreparationMappingKind::Teacher, sqlx::query_as(
                r#"SELECT DISTINCT coordinator.id AS source_id,
                          coalesce(nullif(concat_ws(' ', nullif(concat(coalesce(coordinator.title,''),coordinator.first_name),''), nullif(coordinator.last_name,'')),''), coordinator.username) AS source_label,
                          CASE WHEN coordinator.status='active' THEN coordinator.id END AS suggested_target_id
                   FROM course_assessment_plans plan
                   JOIN users coordinator ON coordinator.id=plan.assessment_coordinator_id
                   WHERE plan.academic_term_id=$1 ORDER BY source_id"#,
            ).bind(context.source_term_id).fetch_all(&mut **tx).await?));
        }
        TermPreparationModule::Timetable => {
            add_timetable_mapping_seeds(tx, context, &mut result).await?;
        }
        TermPreparationModule::Exams => {
            add_exam_mapping_seeds(tx, context, &mut result).await?;
        }
        TermPreparationModule::Supervision => {
            result.push((TermPreparationMappingKind::Teacher, sqlx::query_as(
                r#"SELECT DISTINCT staff.id AS source_id,
                          coalesce(nullif(concat_ws(' ', nullif(concat(coalesce(staff.title,''),staff.first_name),''), nullif(staff.last_name,'')),''), staff.username) AS source_label,
                          CASE WHEN staff.status='active' THEN staff.id END AS suggested_target_id
                   FROM supervision_cycles cycle
                   JOIN supervision_cycle_targets target ON target.cycle_id=cycle.id AND target.target_type='staff'
                   JOIN users staff ON staff.id=target.target_id
                   WHERE cycle.academic_term_id=$1 ORDER BY source_id"#,
            ).bind(context.source_term_id).fetch_all(&mut **tx).await?));
        }
    }
    Ok(result)
}

async fn add_timetable_mapping_seeds(
    tx: &mut Transaction<'_, Postgres>,
    context: &ContextRow,
    result: &mut Vec<(TermPreparationMappingKind, Vec<MappingSeed>)>,
) -> Result<(), AppError> {
    let source_cte = "WITH source_version AS (SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id DESC LIMIT 1) ";
    result.push((TermPreparationMappingKind::LearningOffering, sqlx::query_as(&(source_cte.to_owned() +
        r#"SELECT DISTINCT offering.id AS source_id, offering.code_snapshot || ' · ' || offering.name_snapshot AS source_label,
                  (SELECT target.id FROM learning_offerings target
                   LEFT JOIN course_offering_details td ON td.learning_offering_id=target.id
                   LEFT JOIN activity_offering_details ta ON ta.learning_offering_id=target.id
                   LEFT JOIN course_offering_details sd ON sd.learning_offering_id=offering.id
                   LEFT JOIN activity_offering_details sa ON sa.learning_offering_id=offering.id
                   WHERE target.academic_term_id=$2 AND ((td.subject_id=sd.subject_id AND sd.subject_id IS NOT NULL) OR (ta.activity_id=sa.activity_id AND sa.activity_id IS NOT NULL))
                   ORDER BY target.id LIMIT 1) AS suggested_target_id
           FROM source_version
           JOIN academic_timetable_version_targets version_target
             ON version_target.timetable_version_id=source_version.id
           JOIN learning_offerings offering ON offering.id=version_target.learning_offering_id
           ORDER BY source_id"#))
        .bind(context.source_term_id).bind(context.target_term_id).fetch_all(&mut **tx).await?));
    result.push((TermPreparationMappingKind::LearningGroup, sqlx::query_as(&(source_cte.to_owned() +
        r#"SELECT DISTINCT learning_group.id AS source_id, learning_group.name AS source_label, NULL::uuid AS suggested_target_id
           FROM source_version JOIN academic_timetable_block_groups block_group ON true
           JOIN academic_timetable_blocks block ON block.id=block_group.block_id AND block.timetable_version_id=source_version.id
           JOIN learning_groups learning_group ON learning_group.id=block_group.learning_group_id
           WHERE block_group.is_active ORDER BY source_id"#))
        .bind(context.source_term_id).fetch_all(&mut **tx).await?));
    result.push((TermPreparationMappingKind::Teacher, sqlx::query_as(&(source_cte.to_owned() +
        r#"SELECT DISTINCT teacher.id AS source_id,
                  coalesce(nullif(concat_ws(' ', nullif(concat(coalesce(teacher.title,''),teacher.first_name),''), nullif(teacher.last_name,'')),''), teacher.username) AS source_label,
                  CASE WHEN teacher.status='active' THEN teacher.id END AS suggested_target_id
           FROM source_version JOIN (
               SELECT block.timetable_version_id,instructor.instructor_id AS teacher_id
               FROM academic_timetable_blocks block JOIN academic_timetable_block_groups bg ON bg.block_id=block.id
               JOIN academic_timetable_block_group_instructors instructor ON instructor.block_group_id=bg.id
               UNION ALL
               SELECT block.timetable_version_id,target.teacher_id FROM academic_timetable_blocks block
               JOIN academic_timetable_block_teachers target ON target.block_id=block.id WHERE target.is_active
           ) used ON used.timetable_version_id=source_version.id
           JOIN users teacher ON teacher.id=used.teacher_id ORDER BY source_id"#))
        .bind(context.source_term_id).fetch_all(&mut **tx).await?));
    result.push((TermPreparationMappingKind::Room, sqlx::query_as(&(source_cte.to_owned() +
        r#"SELECT DISTINCT room.id AS source_id, coalesce(room.code || ' · ','') || room.name_th AS source_label,
                  CASE WHEN room.status='ACTIVE' THEN room.id END AS suggested_target_id
           FROM source_version JOIN (
               SELECT block.timetable_version_id,bg.room_id FROM academic_timetable_blocks block JOIN academic_timetable_block_groups bg ON bg.block_id=block.id WHERE bg.room_id IS NOT NULL AND bg.is_active
               UNION ALL SELECT block.timetable_version_id,bh.room_id FROM academic_timetable_blocks block JOIN academic_timetable_block_homerooms bh ON bh.block_id=block.id WHERE bh.room_id IS NOT NULL AND bh.is_active
           ) used ON used.timetable_version_id=source_version.id JOIN rooms room ON room.id=used.room_id ORDER BY source_id"#))
        .bind(context.source_term_id).fetch_all(&mut **tx).await?));
    result.push((TermPreparationMappingKind::Homeroom, sqlx::query_as(&(source_cte.to_owned() +
        r#"SELECT DISTINCT homeroom.id AS source_id, homeroom.name AS source_label,
                  (SELECT target.id FROM homerooms target WHERE target.academic_year_id=$2 AND target.is_active AND target.name=homeroom.name AND target.grade_level_id=homeroom.grade_level_id ORDER BY target.id LIMIT 1) AS suggested_target_id
           FROM source_version JOIN academic_timetable_blocks block ON block.timetable_version_id=source_version.id
           JOIN academic_timetable_block_homerooms bh ON bh.block_id=block.id AND bh.is_active
           JOIN homerooms homeroom ON homeroom.id=bh.homeroom_id ORDER BY source_id"#))
        .bind(context.source_term_id).bind(context.target_year_id).fetch_all(&mut **tx).await?));
    result.push((TermPreparationMappingKind::BellPeriod, sqlx::query_as(&(source_cte.to_owned() +
        r#"SELECT DISTINCT period.id AS source_id,
                  period.order_index::text || ' · ' || coalesce(period.name, to_char(period.start_time,'HH24:MI')) AS source_label,
                  (SELECT target_period.id FROM academic_terms target_term JOIN bell_schedule_periods target_period ON target_period.bell_schedule_id=target_term.bell_schedule_id
                   WHERE target_term.id=$2 AND target_period.is_active AND target_period.order_index=period.order_index ORDER BY target_period.id LIMIT 1) AS suggested_target_id
           FROM source_version JOIN academic_timetable_blocks block ON block.timetable_version_id=source_version.id
           JOIN bell_schedule_periods period ON period.id=block.bell_schedule_period_id WHERE block.is_active ORDER BY source_id"#))
        .bind(context.source_term_id).bind(context.target_term_id).fetch_all(&mut **tx).await?));
    Ok(())
}

async fn add_exam_mapping_seeds(
    tx: &mut Transaction<'_, Postgres>,
    context: &ContextRow,
    result: &mut Vec<(TermPreparationMappingKind, Vec<MappingSeed>)>,
) -> Result<(), AppError> {
    result.push((TermPreparationMappingKind::LearningOffering, sqlx::query_as(
        r#"SELECT DISTINCT offering.id AS source_id, offering.code_snapshot || ' · ' || offering.name_snapshot AS source_label,
                  (SELECT target.id FROM learning_offerings target
                   JOIN course_offering_details td ON td.learning_offering_id=target.id
                   JOIN course_offering_details sd ON sd.learning_offering_id=offering.id
                   WHERE target.academic_term_id=$2 AND td.subject_id=sd.subject_id ORDER BY target.id LIMIT 1) AS suggested_target_id
           FROM academic_exam_schedule_items item JOIN learning_offerings offering ON offering.id=item.learning_offering_id
           WHERE item.academic_term_id=$1 ORDER BY source_id"#,
    ).bind(context.source_term_id).bind(context.target_term_id).fetch_all(&mut **tx).await?));
    result.push((TermPreparationMappingKind::LearningGroup, sqlx::query_as(
        r#"SELECT DISTINCT learning_group.id AS source_id, learning_group.name AS source_label, NULL::uuid AS suggested_target_id
           FROM academic_exam_schedule_items item JOIN learning_groups learning_group ON learning_group.id=item.learning_group_id
           WHERE item.academic_term_id=$1 ORDER BY source_id"#,
    ).bind(context.source_term_id).fetch_all(&mut **tx).await?));
    result.push((TermPreparationMappingKind::Homeroom, sqlx::query_as(
        r#"SELECT DISTINCT homeroom.id AS source_id, homeroom.name AS source_label,
                  (SELECT target.id FROM homerooms target WHERE target.academic_year_id=$2 AND target.is_active AND target.name=homeroom.name AND target.grade_level_id=homeroom.grade_level_id ORDER BY target.id LIMIT 1) AS suggested_target_id
           FROM (
               SELECT item.homeroom_id
               FROM academic_exam_schedule_items item
               WHERE item.academic_term_id=$1
               UNION
               SELECT assignment.homeroom_id
               FROM academic_exam_day_room_assignments assignment
               WHERE assignment.academic_term_id=$1
           ) used
           JOIN homerooms homeroom ON homeroom.id=used.homeroom_id
           ORDER BY source_id"#,
    ).bind(context.source_term_id).bind(context.target_year_id).fetch_all(&mut **tx).await?));
    result.push((TermPreparationMappingKind::Room, sqlx::query_as(
        r#"SELECT DISTINCT room.id AS source_id, coalesce(room.code || ' · ','') || room.name_th AS source_label,
                  CASE WHEN room.status='ACTIVE' THEN room.id END AS suggested_target_id
           FROM academic_exam_day_room_assignments assignment
           JOIN academic_exam_days day ON day.id=assignment.exam_day_id JOIN rooms room ON room.id=assignment.room_id
           WHERE day.academic_term_id=$1 ORDER BY source_id"#,
    ).bind(context.source_term_id).fetch_all(&mut **tx).await?));
    Ok(())
}

async fn load_target_options(
    tx: &mut Transaction<'_, Postgres>,
    context: &ContextRow,
    kind: TermPreparationMappingKind,
) -> Result<Vec<TermPreparationMappingOption>, AppError> {
    let rows: Vec<(Uuid, String)> = match kind {
        TermPreparationMappingKind::LearningOffering => sqlx::query_as(
            "SELECT id,code_snapshot || ' · ' || name_snapshot FROM learning_offerings WHERE academic_term_id=$1 AND status <> 'closed' ORDER BY 2,id LIMIT 500",
        ).bind(context.target_term_id).fetch_all(&mut **tx).await?,
        TermPreparationMappingKind::LearningGroup => sqlx::query_as(
            "SELECT id,name FROM learning_groups WHERE academic_term_id=$1 AND status <> 'closed' ORDER BY name,id LIMIT 2000",
        ).bind(context.target_term_id).fetch_all(&mut **tx).await?,
        TermPreparationMappingKind::Teacher => sqlx::query_as(
            r#"SELECT id,coalesce(nullif(concat_ws(' ', nullif(concat(coalesce(title,''),first_name),''), nullif(last_name,'')),''),username) FROM users WHERE status='active' ORDER BY 2,id LIMIT 2000"#,
        ).fetch_all(&mut **tx).await?,
        TermPreparationMappingKind::Homeroom => sqlx::query_as(
            "SELECT id,name FROM homerooms WHERE academic_year_id=$1 AND is_active ORDER BY name,id LIMIT 500",
        ).bind(context.target_year_id).fetch_all(&mut **tx).await?,
        TermPreparationMappingKind::Room => sqlx::query_as(
            "SELECT id,coalesce(code || ' · ','') || name_th FROM rooms WHERE status='ACTIVE' ORDER BY 2,id LIMIT 2000",
        ).fetch_all(&mut **tx).await?,
        TermPreparationMappingKind::BellPeriod => sqlx::query_as(
            r#"SELECT period.id,period.order_index::text || ' · ' || coalesce(period.name,to_char(period.start_time,'HH24:MI')) FROM academic_terms term JOIN bell_schedule_periods period ON period.bell_schedule_id=term.bell_schedule_id WHERE term.id=$1 AND period.is_active ORDER BY period.order_index,period.id"#,
        ).bind(context.target_term_id).fetch_all(&mut **tx).await?,
        TermPreparationMappingKind::AssessmentPlan => sqlx::query_as(
            r#"SELECT plan.id,offering.code_snapshot || ' · ' || offering.name_snapshot FROM course_assessment_plans plan JOIN learning_offerings offering ON offering.id=plan.learning_offering_id WHERE plan.academic_term_id=$1 ORDER BY 2,plan.id"#,
        ).bind(context.target_term_id).fetch_all(&mut **tx).await?,
    };
    Ok(rows
        .into_iter()
        .map(|(id, label)| TermPreparationMappingOption { id, label })
        .collect())
}

async fn date_requirements(
    tx: &mut Transaction<'_, Postgres>,
    context: &ContextRow,
    normalized: &NormalizedPreparationInput,
) -> Result<(Vec<TermPreparationDateRequirement>, Vec<LifecycleFinding>), AppError> {
    let mut seeds: BTreeMap<NaiveDate, (String, BTreeSet<TermPreparationModule>)> = BTreeMap::new();
    if normalized.modules.contains(&TermPreparationModule::Exams) {
        let rows: Vec<DateSeed> = sqlx::query_as(
            "SELECT DISTINCT exam_date AS source_date, 'วันสอบ ' || to_char(exam_date,'DD/MM/YYYY') AS source_label FROM academic_exam_days WHERE academic_term_id=$1 ORDER BY source_date",
        ).bind(context.source_term_id).fetch_all(&mut **tx).await?;
        for row in rows {
            seeds
                .entry(row.source_date)
                .or_insert((row.source_label, BTreeSet::new()))
                .1
                .insert(TermPreparationModule::Exams);
        }
    }
    if normalized
        .modules
        .contains(&TermPreparationModule::Supervision)
    {
        let rows: Vec<DateSeed> = sqlx::query_as(
            r#"SELECT DISTINCT source_date,'รอบนิเทศ ' || to_char(source_date,'DD/MM/YYYY') AS source_label FROM (
                   SELECT starts_at::date AS source_date FROM supervision_cycles WHERE academic_term_id=$1
                   UNION SELECT ends_at::date FROM supervision_cycles WHERE academic_term_id=$1
                   UNION SELECT booking_opens_at::date FROM supervision_cycles WHERE academic_term_id=$1 AND booking_opens_at IS NOT NULL
                   UNION SELECT booking_closes_at::date FROM supervision_cycles WHERE academic_term_id=$1 AND booking_closes_at IS NOT NULL
               ) dates ORDER BY source_date"#,
        ).bind(context.source_term_id).fetch_all(&mut **tx).await?;
        for row in rows {
            seeds
                .entry(row.source_date)
                .or_insert((row.source_label, BTreeSet::new()))
                .1
                .insert(TermPreparationModule::Supervision);
        }
    }
    let provided = normalized
        .mappings
        .dates
        .iter()
        .map(|mapping| (mapping.source_date, mapping.target_date))
        .collect::<HashMap<_, _>>();
    let offset = context
        .target_start_date
        .signed_duration_since(context.source_start_date);
    let mut requirements = Vec::new();
    let mut findings = Vec::new();
    for (source_date, (source_label, required_by)) in seeds {
        let suggested = source_date
            .checked_add_signed(offset)
            .filter(|date| *date >= context.target_start_date && *date <= context.target_end_date);
        let selected = provided.get(&source_date).copied().or(suggested);
        if selected.is_none() {
            findings.push(blocking(
                format!("mapping.date.{source_date}"),
                format!("ยังไม่ได้จับคู่ {source_label}"),
            ));
        } else if selected
            .is_some_and(|date| date < context.target_start_date || date > context.target_end_date)
        {
            findings.push(blocking(
                format!("mapping.date.{source_date}.outside_target"),
                format!("วันที่ปลายทางของ {source_label} ต้องอยู่ในภาคเรียนเป้าหมาย"),
            ));
        }
        requirements.push(TermPreparationDateRequirement {
            source_date,
            source_label,
            required_by: required_by.into_iter().collect(),
            selected_target_date: selected,
            suggested_target_date: suggested,
        });
    }
    Ok((requirements, findings))
}

pub async fn apply(
    pool: &PgPool,
    actor: &ActorContext,
    input: ApplyTermPreparationInput,
) -> Result<TermPreparationOutcome, AppError> {
    actor.require_permission(codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL)?;
    validate_checksum(&input.source_checksum)?;
    if input.request_id.is_nil() {
        return Err(AppError::ValidationError("requestId ไม่ถูกต้อง".into()));
    }
    let normalized = normalize_input(
        input.source_term_id,
        input.target_term_id,
        &input.modules,
        &input.mappings,
    )?;
    let request_checksum = super::checksum(&(
        actor.user_id,
        input.request_id,
        &normalized,
        &input.source_checksum,
    ))?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;
    lifecycle_guard::lock_transition(&mut tx).await?;
    if let Some(replay) =
        replay(&mut tx, input.request_id, actor.user_id, &request_checksum).await?
    {
        tx.commit().await?;
        return Ok(replay);
    }
    let state = build_workspace(&mut tx, &normalized).await?;
    if state.workspace.source_checksum != input.source_checksum {
        return Err(AppError::Conflict(
            "ข้อมูลต้นทางหรือการจับคู่เปลี่ยนแล้ว กรุณาตรวจตัวอย่างใหม่".into(),
        ));
    }
    if !state.workspace.can_apply {
        return Err(AppError::Conflict(
            "ยังมีข้อมูลที่ต้องแก้หรือจับคู่ก่อนเตรียมภาคเรียน".into(),
        ));
    }
    let resolved = ResolvedMappings::from_workspace(&state.workspace)?;
    let mut outcomes = Vec::new();
    for module in &normalized.modules {
        let outcome = match module {
            TermPreparationModule::Delivery => {
                let preview = state
                    .delivery_preview
                    .as_ref()
                    .ok_or_else(|| AppError::Conflict("ไม่พบตัวอย่าง Delivery ที่พร้อมใช้".into()))?;
                let result = offerings::apply_term_preparation(&mut tx, preview).await?;
                let mut ids = result.offering_ids;
                ids.extend(result.group_ids);
                TermPreparationModuleOutcome {
                    module: *module,
                    created_count: result.created_offering_count + result.created_group_count,
                    target_ids: ids,
                }
            }
            TermPreparationModule::Assessments => {
                apply_assessments(&mut tx, actor.user_id, &state.workspace.context, &resolved)
                    .await?
            }
            TermPreparationModule::Timetable => {
                apply_timetable(&mut tx, actor.user_id, &state.workspace.context, &resolved).await?
            }
            TermPreparationModule::Exams => {
                apply_exams(&mut tx, actor.user_id, &state.workspace.context, &resolved).await?
            }
            TermPreparationModule::Supervision => {
                apply_supervision(&mut tx, actor.user_id, &state.workspace.context, &resolved)
                    .await?
            }
        };
        outcomes.push(outcome);
    }
    let run_id = Uuid::new_v4();
    let created_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        r#"INSERT INTO academic_term_preparation_runs(
               id,request_id,source_academic_term_id,source_academic_year_id,
               target_academic_term_id,target_academic_year_id,selected_modules,mappings,
               source_checksum,request_checksum,outcome,actor_user_id
           ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING created_at"#,
    )
    .bind(run_id)
    .bind(input.request_id)
    .bind(state.workspace.context.source_term_id)
    .bind(state.workspace.context.source_year_id)
    .bind(state.workspace.context.target_term_id)
    .bind(state.workspace.context.target_year_id)
    .bind(
        normalized
            .modules
            .iter()
            .map(|module| module.as_str())
            .collect::<Vec<_>>(),
    )
    .bind(sqlx::types::Json(&normalized.mappings))
    .bind(&input.source_checksum)
    .bind(&request_checksum)
    .bind(sqlx::types::Json(&outcomes))
    .bind(actor.user_id)
    .fetch_one(&mut *tx)
    .await?;
    let result = TermPreparationOutcome {
        run_id,
        request_id: input.request_id,
        source_term_id: state.workspace.context.source_term_id,
        target_term_id: state.workspace.context.target_term_id,
        modules: outcomes,
        created_at,
    };
    years_terms::append_audit(
        &mut tx,
        "academic_term.prepared",
        "academic_term",
        state.workspace.context.target_term_id,
        Some(state.workspace.context.target_year_id),
        Some(state.workspace.context.target_term_id),
        actor.user_id,
        &result,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
    actor_id: Uuid,
    request_checksum: &str,
) -> Result<Option<TermPreparationOutcome>, AppError> {
    let row: Option<(Uuid, Uuid, Uuid, Uuid, Uuid, String, sqlx::types::Json<Vec<TermPreparationModuleOutcome>>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id,request_id,source_academic_term_id,target_academic_term_id,actor_user_id,request_checksum,outcome,created_at FROM academic_term_preparation_runs WHERE request_id=$1",
    ).bind(request_id).fetch_optional(&mut **tx).await?;
    let Some((
        run_id,
        request_id,
        source_term_id,
        target_term_id,
        stored_actor_id,
        stored_checksum,
        sqlx::types::Json(modules),
        created_at,
    )) = row
    else {
        return Ok(None);
    };
    if stored_actor_id != actor_id || stored_checksum != request_checksum {
        return Err(AppError::Conflict(
            "requestId ถูกใช้กับคำขอเตรียมภาคเรียนอื่นแล้ว".into(),
        ));
    }
    Ok(Some(TermPreparationOutcome {
        run_id,
        request_id,
        source_term_id,
        target_term_id,
        modules,
        created_at,
    }))
}

struct ResolvedMappings {
    entities: HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
    dates: HashMap<NaiveDate, NaiveDate>,
}

impl ResolvedMappings {
    fn from_workspace(workspace: &TermPreparationWorkspace) -> Result<Self, AppError> {
        let entities = workspace
            .mapping_requirements
            .iter()
            .map(|requirement| {
                requirement
                    .selected_target_id
                    .map(|target| ((requirement.kind, requirement.source_id), target))
                    .ok_or_else(|| {
                        AppError::Conflict(format!("ยังไม่ได้จับคู่ {}", requirement.source_label))
                    })
            })
            .collect::<Result<HashMap<_, _>, _>>()?;
        let dates = workspace
            .date_requirements
            .iter()
            .map(|requirement| {
                requirement
                    .selected_target_date
                    .map(|target| (requirement.source_date, target))
                    .ok_or_else(|| {
                        AppError::Conflict(format!("ยังไม่ได้จับคู่ {}", requirement.source_label))
                    })
            })
            .collect::<Result<HashMap<_, _>, _>>()?;
        Ok(Self { entities, dates })
    }

    fn entity(&self, kind: TermPreparationMappingKind, source: Uuid) -> Result<Uuid, AppError> {
        self.entities
            .get(&(kind, source))
            .copied()
            .ok_or_else(|| AppError::Conflict(format!("ไม่พบการจับคู่ {} {source}", kind.as_str())))
    }
}

// Module-owned apply helpers are kept below while their SQL boundaries remain
// narrow: configuration only, no roster, score, result, observation or outcome
// tables are referenced.
async fn apply_assessments(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    context: &TermPreparationContext,
    mappings: &ResolvedMappings,
) -> Result<TermPreparationModuleOutcome, AppError> {
    #[derive(FromRow)]
    struct SourcePlan {
        id: Uuid,
        offering_id: Uuid,
        coordinator_id: Option<Uuid>,
    }
    let plans: Vec<SourcePlan> = sqlx::query_as("SELECT id,learning_offering_id AS offering_id,assessment_coordinator_id AS coordinator_id FROM course_assessment_plans WHERE academic_term_id=$1 ORDER BY id")
        .bind(context.source_term_id).fetch_all(&mut **tx).await?;
    let mut ids = Vec::new();
    for plan in plans {
        let target_offering = mappings.entity(
            TermPreparationMappingKind::LearningOffering,
            plan.offering_id,
        )?;
        let (subject_version_id, target_year_id): (Uuid, Uuid) = sqlx::query_as("SELECT detail.subject_version_id,offering.academic_year_id FROM course_offering_details detail JOIN learning_offerings offering ON offering.id=detail.learning_offering_id WHERE offering.id=$1 AND offering.academic_term_id=$2")
            .bind(target_offering).bind(context.target_term_id).fetch_optional(&mut **tx).await?.ok_or_else(|| AppError::Conflict("รายการเปิดสอนเป้าหมายของโครงสร้างคะแนนไม่พร้อม".into()))?;
        let coordinator = plan
            .coordinator_id
            .map(|id| mappings.entity(TermPreparationMappingKind::Teacher, id))
            .transpose()?;
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO course_assessment_plans(id,academic_term_id,subject_version_id,learning_offering_id,academic_year_id,assessment_coordinator_id,row_version,migration_provenance,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,1,'{}'::jsonb,now(),now())")
            .bind(id).bind(context.target_term_id).bind(subject_version_id).bind(target_offering).bind(target_year_id).bind(coordinator).execute(&mut **tx).await?;
        sqlx::query(r#"INSERT INTO course_assessment_phases(id,plan_id,phase_code,max_score,exam_arrangement,exam_duration_minutes,row_version,created_by,updated_by,created_at,updated_at)
                       SELECT gen_random_uuid(),$1,phase_code,max_score,exam_arrangement,exam_duration_minutes,1,$2,$2,now(),now() FROM course_assessment_phases WHERE plan_id=$3"#)
            .bind(id).bind(actor).bind(plan.id).execute(&mut **tx).await?;
        ids.push(id);
    }
    Ok(TermPreparationModuleOutcome {
        module: TermPreparationModule::Assessments,
        created_count: ids.len(),
        target_ids: ids,
    })
}

async fn apply_timetable(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    context: &TermPreparationContext,
    mappings: &ResolvedMappings,
) -> Result<TermPreparationModuleOutcome, AppError> {
    crate::modules::academic::services::timetable_version_service::apply_term_preparation(
        tx,
        actor,
        context,
        &mappings.entities,
    )
    .await
}

async fn apply_exams(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    context: &TermPreparationContext,
    mappings: &ResolvedMappings,
) -> Result<TermPreparationModuleOutcome, AppError> {
    crate::modules::academic::services::exam_schedule_service::apply_term_preparation(
        tx,
        actor,
        context,
        &mappings.entities,
        &mappings.dates,
    )
    .await
}

async fn apply_supervision(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    context: &TermPreparationContext,
    mappings: &ResolvedMappings,
) -> Result<TermPreparationModuleOutcome, AppError> {
    crate::modules::supervision::services::apply_term_preparation(
        tx,
        actor,
        context,
        &mappings.entities,
        &mappings.dates,
    )
    .await
}
