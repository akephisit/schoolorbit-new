use std::collections::{HashMap, HashSet};

use bigdecimal::BigDecimal;
use sqlx::{types::Json, FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::core::services::validate_canonical_decimal;
use crate::modules::academic::delivery::models::CourseGradingPolicy;
use crate::modules::academic::models::assessment::{
    AssessmentCoordinatorOption, AssessmentExamArrangement, AssessmentOfferingScopeRow,
    AssessmentPhase, AssessmentPhaseCode, AssessmentPhaseControl, AssessmentPhaseControlRow,
    AssessmentPlanDetail, AssessmentPlanListQuery, AssessmentPlanRow, AssessmentPlanSummary,
    AssessmentReadiness, AssessmentReadinessFinding, SaveAssessmentPhaseRequest,
    SaveAssessmentPlanRequest, UpdateAssessmentPhaseControlRequest,
};
use crate::permissions::registry::codes;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

#[derive(Debug, FromRow)]
struct AssessmentPlanSummaryRow {
    plan_id: Option<Uuid>,
    offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    subject_id: Uuid,
    subject_version_display_label: String,
    offering_code: String,
    offering_name: String,
    row_version: Option<i64>,
    assessment_coordinator_id: Option<Uuid>,
    assessment_coordinator_name: Option<String>,
    learning_group_ids: Vec<Uuid>,
    learning_group_count: i64,
    grading_policy: Json<CourseGradingPolicy>,
}

#[derive(Debug, FromRow)]
struct CoordinatorCandidateRow {
    offering_id: Uuid,
    teacher_id: Uuid,
    display_name: String,
    learning_group_count: i64,
    primary_learning_group_count: i64,
    primary_assignment_count: i64,
}

#[derive(Debug, Clone)]
struct CoordinatorCandidate {
    option: AssessmentCoordinatorOption,
    primary_assignment_count: i64,
}

pub fn default_phases() -> Vec<AssessmentPhase> {
    AssessmentPhaseCode::ALL
        .into_iter()
        .map(|phase_code| AssessmentPhase {
            id: None,
            phase_code,
            label: phase_code.label_th().to_string(),
            order: phase_code.order(),
            max_score: "0.00".to_string(),
            exam_arrangement: AssessmentExamArrangement::None,
            exam_duration_minutes: None,
            row_version: None,
        })
        .collect()
}

pub fn validate_plan_payload(payload: &SaveAssessmentPlanRequest) -> Result<(), AppError> {
    if payload.phases.len() != AssessmentPhaseCode::ALL.len() {
        return Err(AppError::ValidationError(
            "โครงสร้างคะแนนต้องมี 4 ช่วงมาตรฐานครบถ้วน".to_string(),
        ));
    }

    let zero = BigDecimal::from(0);
    let mut phase_codes = HashSet::new();
    let mut phase_ids = HashSet::new();
    for phase in &payload.phases {
        if !phase_codes.insert(phase.phase_code) {
            return Err(AppError::ValidationError(
                "โครงสร้างคะแนนต้องมี 4 ช่วงมาตรฐานโดยไม่ซ้ำกัน".to_string(),
            ));
        }
        if phase.id.is_some_and(|id| !phase_ids.insert(id)) {
            return Err(AppError::ValidationError(
                "ช่วงคะแนนอ้างอิงข้อมูลซ้ำกัน".to_string(),
            ));
        }

        let max_score = validate_canonical_decimal(&phase.max_score, 2)?;
        if max_score < zero {
            return Err(AppError::ValidationError(
                "คะแนนสูงสุดของแต่ละช่วงต้องไม่ติดลบ".to_string(),
            ));
        }

        if !phase.phase_code.supports_exam_arrangement()
            && phase.exam_arrangement != AssessmentExamArrangement::None
        {
            return Err(AppError::ValidationError(format!(
                "ช่วง{}ไม่ใช่ช่วงสอบกลางหรือปลายภาค",
                phase.phase_code.label_th()
            )));
        }
        if phase.exam_arrangement == AssessmentExamArrangement::None
            && phase.exam_duration_minutes.is_some()
        {
            return Err(AppError::ValidationError(format!(
                "ช่วง{}ที่ไม่จัดสอบต้องไม่มีระยะเวลาสอบ",
                phase.phase_code.label_th()
            )));
        }
        if phase
            .exam_duration_minutes
            .is_some_and(|duration| duration <= 0)
        {
            return Err(AppError::ValidationError(
                "ระยะเวลาสอบต้องมากกว่า 0 นาที".to_string(),
            ));
        }
    }

    if AssessmentPhaseCode::ALL
        .into_iter()
        .any(|phase_code| !phase_codes.contains(&phase_code))
    {
        return Err(AppError::ValidationError(
            "โครงสร้างคะแนนต้องมี 4 ช่วงมาตรฐานครบถ้วน".to_string(),
        ));
    }
    Ok(())
}

pub fn plan_readiness(
    payload: &SaveAssessmentPlanRequest,
    expected_total: &BigDecimal,
) -> Result<AssessmentReadiness, AppError> {
    validate_plan_payload(payload)?;
    let mut findings = Vec::new();
    if payload.assessment_coordinator_id.is_none() {
        findings.push(AssessmentReadinessFinding::MissingCoordinator);
    }

    let total = payload.phases.iter().try_fold(
        BigDecimal::from(0),
        |total, phase| -> Result<BigDecimal, AppError> {
            Ok(total + validate_canonical_decimal(&phase.max_score, 2)?)
        },
    )?;
    if total != *expected_total {
        findings.push(AssessmentReadinessFinding::TotalMismatch);
    }

    for phase in &payload.phases {
        if phase.exam_arrangement == AssessmentExamArrangement::InTimetable
            && phase.exam_duration_minutes.is_none()
        {
            match phase.phase_code {
                AssessmentPhaseCode::Midterm => {
                    findings.push(AssessmentReadinessFinding::MidtermMissingExamDuration)
                }
                AssessmentPhaseCode::Final => {
                    findings.push(AssessmentReadinessFinding::FinalMissingExamDuration)
                }
                AssessmentPhaseCode::BeforeMidterm | AssessmentPhaseCode::AfterMidterm => {}
            }
        }
    }

    Ok(AssessmentReadiness {
        ready: findings.is_empty(),
        findings,
        total_score: decimal_string(&total),
        expected_total_score: decimal_string(expected_total),
    })
}

pub async fn list_assessment_plans(
    pool: &PgPool,
    query: &AssessmentPlanListQuery,
    access: &AcademicResourceListFilter,
) -> Result<Vec<AssessmentPlanSummary>, AppError> {
    let owner_ids = access.allowed_organization_unit_ids();
    let rows: Vec<AssessmentPlanSummaryRow> = sqlx::query_as(
        r#"
        SELECT plan.id AS plan_id,
               offering.id AS offering_id,
               offering.academic_term_id,
               offering.academic_year_id,
               detail.subject_id,
               concat(
                   coalesce(version.name_th, version.name_en, offering.name_snapshot),
                   ' · v', version.version_no
               ) AS subject_version_display_label,
               offering.code_snapshot AS offering_code,
               offering.name_snapshot AS offering_name,
               plan.row_version,
               plan.assessment_coordinator_id,
               CASE WHEN coordinator.id IS NULL THEN NULL ELSE coalesce(
                   nullif(concat_ws(' ',
                       nullif(concat(coalesce(coordinator.title, ''), coordinator.first_name), ''),
                       nullif(coordinator.last_name, '')
                   ), ''),
                   coordinator.username
               ) END AS assessment_coordinator_name,
               ARRAY(
                   SELECT learning_group.id
                   FROM learning_groups learning_group
                   WHERE learning_group.learning_offering_id = offering.id
                     AND learning_group.status <> 'closed'
                   ORDER BY learning_group.code, learning_group.id
               ) AS learning_group_ids,
               (
                   SELECT count(*)
                   FROM learning_groups learning_group
                   WHERE learning_group.learning_offering_id = offering.id
                     AND learning_group.status <> 'closed'
               )::bigint AS learning_group_count,
               detail.grading_policy
        FROM learning_offerings offering
        JOIN course_offering_details detail ON detail.learning_offering_id = offering.id
        JOIN subject_versions version ON version.id = detail.subject_version_id
        LEFT JOIN course_assessment_plans plan ON plan.learning_offering_id = offering.id
        LEFT JOIN users coordinator ON coordinator.id = plan.assessment_coordinator_id
        WHERE offering.academic_term_id = $1
          AND offering.kind = 'course'
          AND ($2::uuid IS NULL OR detail.subject_id = $2)
          AND ($3::uuid IS NULL OR EXISTS (
              SELECT 1
              FROM learning_groups learning_group
              JOIN learning_group_teachers teacher
                ON teacher.learning_group_id = learning_group.id
              WHERE learning_group.learning_offering_id = offering.id
                AND teacher.teacher_id = $3
          ))
          AND ($4 OR offering.owning_organization_unit_id = ANY($5) OR EXISTS (
              SELECT 1
              FROM learning_groups learning_group
              JOIN learning_group_teachers teacher
                ON teacher.learning_group_id = learning_group.id
              WHERE learning_group.learning_offering_id = offering.id
                AND teacher.teacher_id = $6
          ))
        ORDER BY offering.code_snapshot, offering.id
        LIMIT 500
        "#,
    )
    .bind(query.academic_term_id)
    .bind(query.subject_id)
    .bind(query.instructor_id)
    .bind(access.includes_school_owned)
    .bind(owner_ids)
    .bind(access.assigned_actor_id)
    .fetch_all(pool)
    .await?;

    let plan_ids = rows
        .iter()
        .filter_map(|row| row.plan_id)
        .collect::<Vec<_>>();
    let offering_ids = rows.iter().map(|row| row.offering_id).collect::<Vec<_>>();
    let mut phases_by_plan = load_phases_by_plan(pool, &plan_ids).await?;
    let candidates_by_offering = load_coordinator_candidates(pool, &offering_ids).await?;

    let mut summaries = Vec::with_capacity(rows.len());
    for row in rows {
        let phases = row
            .plan_id
            .and_then(|plan_id| phases_by_plan.remove(&plan_id))
            .unwrap_or_else(default_phases);
        let candidates = candidates_by_offering
            .get(&row.offering_id)
            .cloned()
            .unwrap_or_default();
        let (suggested_id, suggested_name) =
            suggested_coordinator(&candidates, row.learning_group_count);
        let coordinator_is_candidate =
            row.assessment_coordinator_id.is_some_and(|coordinator_id| {
                candidates
                    .iter()
                    .any(|candidate| candidate.option.teacher_id == coordinator_id)
            });
        let (_, expected_total) = grading_policy_value(&row.grading_policy.0)?;
        let readiness = response_readiness(
            row.assessment_coordinator_id,
            coordinator_is_candidate,
            &phases,
            &expected_total,
        )?;
        if query.ready.is_some_and(|ready| readiness.ready != ready) {
            continue;
        }
        if query.exam_arrangement.is_some_and(|arrangement| {
            !phases.iter().any(|phase| {
                phase.phase_code.supports_exam_arrangement()
                    && phase.exam_arrangement == arrangement
            })
        }) {
            continue;
        }

        summaries.push(AssessmentPlanSummary {
            plan_id: row.plan_id,
            offering_id: row.offering_id,
            academic_term_id: row.academic_term_id,
            academic_year_id: row.academic_year_id,
            subject_id: row.subject_id,
            subject_version_display_label: row.subject_version_display_label,
            offering_code: row.offering_code,
            offering_name: row.offering_name,
            row_version: row.row_version,
            learning_group_ids: row.learning_group_ids,
            learning_group_count: row.learning_group_count,
            assessment_coordinator_id: row.assessment_coordinator_id,
            assessment_coordinator_name: row.assessment_coordinator_name,
            suggested_coordinator_id: suggested_id,
            suggested_coordinator_name: suggested_name,
            phases,
            readiness,
        });
    }
    Ok(summaries)
}

pub async fn get_plan_detail(
    pool: &PgPool,
    offering_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    fetch_plan_detail(pool, offering_id).await
}

pub async fn save_plan(
    pool: &PgPool,
    offering_id: Uuid,
    actor_user_id: Uuid,
    can_manage_school: bool,
    payload: SaveAssessmentPlanRequest,
) -> Result<AssessmentPlanDetail, AppError> {
    validate_plan_payload(&payload)?;
    let mut transaction = pool.begin().await?;
    let scope = resolve_offering_scope_in_tx(&mut transaction, offering_id, true).await?;
    let candidate_ids = load_candidate_ids_in_tx(&mut transaction, offering_id).await?;
    if payload
        .assessment_coordinator_id
        .is_some_and(|coordinator_id| !candidate_ids.contains(&coordinator_id))
    {
        return Err(AppError::ValidationError(
            "ผู้รับผิดชอบต้องเป็นครูที่กำลังสอนอย่างน้อยหนึ่งกลุ่มของรายวิชานี้".to_string(),
        ));
    }

    let existing = load_plan_in_tx(&mut transaction, offering_id).await?;
    if !can_manage_school {
        let allowed_coordinator = match &existing {
            Some(plan) => plan.assessment_coordinator_id == Some(actor_user_id),
            None => payload.assessment_coordinator_id == Some(actor_user_id),
        };
        if !allowed_coordinator || payload.assessment_coordinator_id != Some(actor_user_id) {
            return Err(AppError::Forbidden(
                "เฉพาะผู้รับผิดชอบโครงสร้างคะแนนหรือผู้ดูแลวิชาการเท่านั้นที่แก้ไขได้".to_string(),
            ));
        }
    }

    let plan_id = match existing {
        Some(plan) => {
            let expected_version = payload.row_version.ok_or_else(|| {
                AppError::Conflict("ต้องระบุ rowVersion ของแผนคะแนนเดิม".to_string())
            })?;
            if expected_version != plan.row_version {
                return Err(AppError::Conflict(
                    "โครงสร้างคะแนนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string(),
                ));
            }
            let updated = sqlx::query(
                r#"UPDATE course_assessment_plans
                   SET assessment_coordinator_id = $2,
                       row_version = row_version + 1,
                       updated_at = now()
                   WHERE id = $1 AND row_version = $3"#,
            )
            .bind(plan.id)
            .bind(payload.assessment_coordinator_id)
            .bind(expected_version)
            .execute(&mut *transaction)
            .await?;
            if updated.rows_affected() != 1 {
                return Err(AppError::Conflict(
                    "โครงสร้างคะแนนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string(),
                ));
            }
            plan.id
        }
        None => {
            if payload.row_version.is_some() {
                return Err(AppError::Conflict(
                    "ยังไม่มีโครงสร้างคะแนนสำหรับรายวิชาที่เปิดสอนนี้".to_string(),
                ));
            }
            let plan_id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO course_assessment_plans (
                       id, academic_term_id, subject_version_id, learning_offering_id,
                       academic_year_id, assessment_coordinator_id, row_version,
                       migration_provenance, created_at, updated_at
                   ) VALUES ($1, $2, $3, $4, $5, $6, 1, '{}'::jsonb, now(), now())"#,
            )
            .bind(plan_id)
            .bind(scope.academic_term_id)
            .bind(scope.subject_version_id)
            .bind(scope.offering_id)
            .bind(scope.academic_year_id)
            .bind(payload.assessment_coordinator_id)
            .execute(&mut *transaction)
            .await?;
            plan_id
        }
    };

    replace_plan_phases(&mut transaction, plan_id, actor_user_id, &payload.phases).await?;
    transaction.commit().await?;
    fetch_plan_detail(pool, offering_id).await
}

pub async fn list_phase_controls(
    pool: &PgPool,
    academic_term_id: Uuid,
) -> Result<Vec<AssessmentPhaseControl>, AppError> {
    let rows: Vec<AssessmentPhaseControlRow> = sqlx::query_as(
        r#"SELECT id, academic_term_id, academic_year_id, phase_code,
                  item_editing_enabled, score_entry_enabled, row_version
           FROM academic_assessment_phase_controls
           WHERE academic_term_id = $1
           ORDER BY CASE phase_code
               WHEN 'before_midterm' THEN 1
               WHEN 'midterm' THEN 2
               WHEN 'after_midterm' THEN 3
               WHEN 'final' THEN 4
               ELSE 99
           END"#,
    )
    .bind(academic_term_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(phase_control_from_row).collect()
}

pub async fn update_phase_control(
    pool: &PgPool,
    control_id: Uuid,
    actor_user_id: Uuid,
    payload: UpdateAssessmentPhaseControlRequest,
) -> Result<AssessmentPhaseControl, AppError> {
    let row: AssessmentPhaseControlRow = sqlx::query_as(
        r#"UPDATE academic_assessment_phase_controls
           SET item_editing_enabled = $2,
               score_entry_enabled = $3,
               row_version = row_version + 1,
               updated_by = $4,
               updated_at = now()
           WHERE id = $1 AND row_version = $5
           RETURNING id, academic_term_id, academic_year_id, phase_code,
                     item_editing_enabled, score_entry_enabled, row_version"#,
    )
    .bind(control_id)
    .bind(payload.item_editing_enabled)
    .bind(payload.score_entry_enabled)
    .bind(actor_user_id)
    .bind(payload.row_version)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Conflict("การตั้งค่าช่วงคะแนนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string()))?;
    phase_control_from_row(row)
}

pub fn require_phase_controls_read_access(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_READ_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT,
        codes::ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED,
        codes::ACADEMIC_ASSESSMENT_READ_SCHOOL,
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::LEARNING_OFFERING_READ_SCHOOL,
        codes::LEARNING_OFFERING_MANAGE_SCHOOL,
    ])
}

pub fn require_phase_controls_manage_access(actor: &ActorContext) -> Result<(), AppError> {
    actor.require_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::LEARNING_OFFERING_MANAGE_SCHOOL,
    ])
}

pub fn actor_can_manage_all_plans(actor: &ActorContext) -> bool {
    actor.has_any_permission(&[
        codes::ACADEMIC_ASSESSMENT_MANAGE_SCHOOL,
        codes::LEARNING_OFFERING_MANAGE_SCHOOL,
    ])
}

async fn resolve_offering_scope(
    pool: &PgPool,
    offering_id: Uuid,
) -> Result<AssessmentOfferingScopeRow, AppError> {
    sqlx::query_as(&offering_scope_sql(false))
        .bind(offering_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()))
}

async fn resolve_offering_scope_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
    require_writable: bool,
) -> Result<AssessmentOfferingScopeRow, AppError> {
    let scope: AssessmentOfferingScopeRow = sqlx::query_as(&offering_scope_sql(true))
        .bind(offering_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรายวิชาที่เปิดสอน".to_string()))?;
    if require_writable && matches!(scope.academic_term_status.as_str(), "closed" | "archived") {
        return Err(AppError::Conflict(
            "ภาคเรียนนี้ปิดแล้ว ไม่สามารถแก้ไขโครงสร้างคะแนนได้".to_string(),
        ));
    }
    Ok(scope)
}

fn offering_scope_sql(for_update: bool) -> String {
    format!(
        r#"SELECT offering.id AS offering_id,
                  offering.academic_term_id,
                  offering.academic_year_id,
                  term.status AS academic_term_status,
                  detail.subject_version_id,
                  detail.subject_id,
                  concat(
                      coalesce(version.name_th, version.name_en, offering.name_snapshot),
                      ' · v', version.version_no
                  ) AS subject_version_display_label,
                  offering.code_snapshot AS offering_code,
                  offering.name_snapshot AS offering_name,
                  detail.grading_policy
           FROM learning_offerings offering
           JOIN academic_terms term ON term.id = offering.academic_term_id
           JOIN course_offering_details detail ON detail.learning_offering_id = offering.id
           JOIN subject_versions version ON version.id = detail.subject_version_id
           WHERE offering.id = $1 AND offering.kind = 'course'{}"#,
        if for_update {
            " FOR UPDATE OF offering"
        } else {
            ""
        }
    )
}

fn grading_policy_value(
    policy: &CourseGradingPolicy,
) -> Result<(CourseGradingPolicy, BigDecimal), AppError> {
    let expected_total = validate_canonical_decimal(&policy.total_score, 2).map_err(|error| {
        tracing::error!(reason = "invalid_course_total_score_snapshot", ?error);
        AppError::InternalServerError("คะแนนรวมตามนโยบายไม่ถูกต้อง".to_string())
    })?;
    Ok((policy.clone(), expected_total))
}

async fn load_plan_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<Option<AssessmentPlanRow>, AppError> {
    Ok(sqlx::query_as(
        r#"SELECT plan.id, plan.assessment_coordinator_id,
                  CASE WHEN coordinator.id IS NULL THEN NULL ELSE coalesce(
                      nullif(concat_ws(' ',
                          nullif(concat(coalesce(coordinator.title, ''), coordinator.first_name), ''),
                          nullif(coordinator.last_name, '')
                      ), ''),
                      coordinator.username
                  ) END AS assessment_coordinator_name,
                  plan.row_version
           FROM course_assessment_plans plan
           LEFT JOIN users coordinator ON coordinator.id = plan.assessment_coordinator_id
           WHERE plan.learning_offering_id = $1
           FOR UPDATE OF plan"#,
    )
    .bind(offering_id)
    .fetch_optional(&mut **transaction)
    .await?)
}

async fn replace_plan_phases(
    transaction: &mut Transaction<'_, Postgres>,
    plan_id: Uuid,
    actor_user_id: Uuid,
    phases: &[SaveAssessmentPhaseRequest],
) -> Result<(), AppError> {
    let existing: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, phase_code FROM course_assessment_phases WHERE plan_id = $1 FOR UPDATE",
    )
    .bind(plan_id)
    .fetch_all(&mut **transaction)
    .await?;
    let existing_by_code = existing
        .into_iter()
        .map(|(id, code)| (code, id))
        .collect::<HashMap<_, _>>();

    for phase in phases {
        let existing_id = existing_by_code.get(phase.phase_code.as_str()).copied();
        if phase.id.is_some_and(|id| Some(id) != existing_id) {
            return Err(AppError::ValidationError(
                "ช่วงคะแนนไม่ได้อยู่ในโครงสร้างคะแนนนี้".to_string(),
            ));
        }
        let phase_id = existing_id.unwrap_or_else(Uuid::new_v4);
        let max_score = validate_canonical_decimal(&phase.max_score, 2)?;
        sqlx::query(
            r#"INSERT INTO course_assessment_phases (
                   id, plan_id, phase_code, max_score, exam_arrangement,
                   exam_duration_minutes, row_version, created_by, updated_by,
                   created_at, updated_at
               ) VALUES ($1, $2, $3, $4, $5, $6, 1, $7, $7, now(), now())
               ON CONFLICT (plan_id, phase_code) DO UPDATE
               SET max_score = EXCLUDED.max_score,
                   exam_arrangement = EXCLUDED.exam_arrangement,
                   exam_duration_minutes = EXCLUDED.exam_duration_minutes,
                   row_version = course_assessment_phases.row_version + 1,
                   updated_by = EXCLUDED.updated_by,
                   updated_at = now()"#,
        )
        .bind(phase_id)
        .bind(plan_id)
        .bind(phase.phase_code.as_str())
        .bind(max_score)
        .bind(phase.exam_arrangement.as_str())
        .bind(phase.exam_duration_minutes)
        .bind(actor_user_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn fetch_plan_detail(
    pool: &PgPool,
    offering_id: Uuid,
) -> Result<AssessmentPlanDetail, AppError> {
    let scope = resolve_offering_scope(pool, offering_id).await?;
    let (policy, expected_total) = grading_policy_value(&scope.grading_policy.0)?;
    let plan: Option<AssessmentPlanRow> = sqlx::query_as(
        r#"SELECT plan.id, plan.assessment_coordinator_id,
                  CASE WHEN coordinator.id IS NULL THEN NULL ELSE coalesce(
                      nullif(concat_ws(' ',
                          nullif(concat(coalesce(coordinator.title, ''), coordinator.first_name), ''),
                          nullif(coordinator.last_name, '')
                      ), ''),
                      coordinator.username
                  ) END AS assessment_coordinator_name,
                  plan.row_version
           FROM course_assessment_plans plan
           LEFT JOIN users coordinator ON coordinator.id = plan.assessment_coordinator_id
           WHERE plan.learning_offering_id = $1"#,
    )
    .bind(offering_id)
    .fetch_optional(pool)
    .await?;
    let learning_group_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM learning_groups
           WHERE learning_offering_id = $1 AND status <> 'closed'
           ORDER BY code, id"#,
    )
    .bind(offering_id)
    .fetch_all(pool)
    .await?;
    let candidates = load_coordinator_candidates(pool, &[offering_id])
        .await?
        .remove(&offering_id)
        .unwrap_or_default();
    let (suggested_id, suggested_name) =
        suggested_coordinator(&candidates, learning_group_ids.len() as i64);

    let (id, coordinator_id, coordinator_name, row_version, phases) = match plan {
        Some(plan) => {
            let phases = load_phases_by_plan(pool, &[plan.id])
                .await?
                .remove(&plan.id)
                .unwrap_or_default();
            (
                Some(plan.id),
                plan.assessment_coordinator_id,
                plan.assessment_coordinator_name,
                Some(plan.row_version),
                phases,
            )
        }
        None => (None, None, None, None, default_phases()),
    };
    let coordinator_is_candidate = coordinator_id.is_some_and(|selected_id| {
        candidates
            .iter()
            .any(|candidate| candidate.option.teacher_id == selected_id)
    });
    let readiness = response_readiness(
        coordinator_id,
        coordinator_is_candidate,
        &phases,
        &expected_total,
    )?;

    Ok(AssessmentPlanDetail {
        id,
        offering_id: scope.offering_id,
        academic_term_id: scope.academic_term_id,
        academic_year_id: scope.academic_year_id,
        subject_id: scope.subject_id,
        subject_version_display_label: scope.subject_version_display_label,
        offering_code: scope.offering_code,
        offering_name: scope.offering_name,
        grading_policy: policy,
        row_version,
        learning_group_ids,
        assessment_coordinator_id: coordinator_id,
        assessment_coordinator_name: coordinator_name,
        suggested_coordinator_id: suggested_id,
        suggested_coordinator_name: suggested_name,
        coordinator_candidates: candidates
            .into_iter()
            .map(|candidate| candidate.option)
            .collect(),
        phases,
        readiness,
    })
}

async fn load_phases_by_plan(
    pool: &PgPool,
    plan_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<AssessmentPhase>>, AppError> {
    if plan_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<crate::modules::academic::models::assessment::AssessmentPhaseRow> =
        sqlx::query_as(
            r#"SELECT id, plan_id, phase_code, max_score, exam_arrangement,
                      exam_duration_minutes, row_version
               FROM course_assessment_phases
               WHERE plan_id = ANY($1)
               ORDER BY plan_id,
                   CASE phase_code
                       WHEN 'before_midterm' THEN 1
                       WHEN 'midterm' THEN 2
                       WHEN 'after_midterm' THEN 3
                       WHEN 'final' THEN 4
                       ELSE 99
                   END"#,
        )
        .bind(plan_ids)
        .fetch_all(pool)
        .await?;
    let mut by_plan: HashMap<Uuid, Vec<AssessmentPhase>> = HashMap::new();
    for row in rows {
        let plan_id = row.plan_id;
        by_plan
            .entry(plan_id)
            .or_default()
            .push(phase_from_row(row)?);
    }
    Ok(by_plan)
}

fn phase_from_row(
    row: crate::modules::academic::models::assessment::AssessmentPhaseRow,
) -> Result<AssessmentPhase, AppError> {
    let phase_code = AssessmentPhaseCode::try_from(row.phase_code.as_str()).map_err(|reason| {
        tracing::error!(%reason, "invalid assessment phase code stored in database");
        AppError::InternalServerError("รหัสช่วงคะแนนในระบบไม่ถูกต้อง".to_string())
    })?;
    let exam_arrangement = AssessmentExamArrangement::try_from(row.exam_arrangement.as_str())
        .map_err(|reason| {
            tracing::error!(%reason, "invalid exam arrangement stored in database");
            AppError::InternalServerError("รูปแบบการสอบในระบบไม่ถูกต้อง".to_string())
        })?;
    Ok(AssessmentPhase {
        id: Some(row.id),
        phase_code,
        label: phase_code.label_th().to_string(),
        order: phase_code.order(),
        max_score: decimal_string(&row.max_score),
        exam_arrangement,
        exam_duration_minutes: row.exam_duration_minutes,
        row_version: Some(row.row_version),
    })
}

fn response_readiness(
    coordinator_id: Option<Uuid>,
    coordinator_is_candidate: bool,
    phases: &[AssessmentPhase],
    expected_total: &BigDecimal,
) -> Result<AssessmentReadiness, AppError> {
    if phases.len() != 4 {
        let total = phases.iter().try_fold(
            BigDecimal::from(0),
            |total, phase| -> Result<BigDecimal, AppError> {
                Ok(total + validate_canonical_decimal(&phase.max_score, 2)?)
            },
        )?;
        return Ok(AssessmentReadiness {
            ready: false,
            findings: vec![AssessmentReadinessFinding::MissingPhase],
            total_score: decimal_string(&total),
            expected_total_score: decimal_string(expected_total),
        });
    }
    let payload = SaveAssessmentPlanRequest {
        row_version: None,
        assessment_coordinator_id: coordinator_id,
        phases: phases
            .iter()
            .map(|phase| SaveAssessmentPhaseRequest {
                id: phase.id,
                phase_code: phase.phase_code,
                max_score: phase.max_score.clone(),
                exam_arrangement: phase.exam_arrangement,
                exam_duration_minutes: phase.exam_duration_minutes,
            })
            .collect(),
    };
    let mut readiness = plan_readiness(&payload, expected_total)?;
    if coordinator_id.is_some() && !coordinator_is_candidate {
        readiness
            .findings
            .insert(0, AssessmentReadinessFinding::CoordinatorNotCandidate);
        readiness.ready = false;
    }
    Ok(readiness)
}

async fn load_coordinator_candidates(
    pool: &PgPool,
    offering_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<CoordinatorCandidate>>, AppError> {
    if offering_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<CoordinatorCandidateRow> = sqlx::query_as(
        r#"SELECT offering.id AS offering_id,
                  teacher.teacher_id,
                  coalesce(
                      nullif(concat_ws(' ',
                          nullif(concat(coalesce(user_account.title, ''), user_account.first_name), ''),
                          nullif(user_account.last_name, '')
                      ), ''),
                      user_account.username
                  ) AS display_name,
                  count(DISTINCT learning_group.id)::bigint AS learning_group_count,
                  count(DISTINCT learning_group.id)
                      FILTER (WHERE teacher.role = 'primary')::bigint
                      AS primary_learning_group_count,
                  count(*) FILTER (WHERE teacher.role = 'primary')::bigint
                      AS primary_assignment_count
           FROM learning_offerings offering
           JOIN academic_terms term ON term.id = offering.academic_term_id
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = offering.id
            AND learning_group.status <> 'closed'
           JOIN learning_group_teachers teacher
             ON teacher.learning_group_id = learning_group.id
            AND teacher.starts_on <= LEAST(
                GREATEST(current_date, term.start_date), term.planned_end_date
            )
            AND (
                teacher.ends_on IS NULL
                OR teacher.ends_on >= LEAST(
                    GREATEST(current_date, term.start_date), term.planned_end_date
                )
            )
           JOIN users user_account
             ON user_account.id = teacher.teacher_id
            AND user_account.status = 'active'
           WHERE offering.id = ANY($1)
           GROUP BY offering.id, teacher.teacher_id, user_account.id
           ORDER BY offering.id, display_name, teacher.teacher_id"#,
    )
    .bind(offering_ids)
    .fetch_all(pool)
    .await?;
    let mut by_offering: HashMap<Uuid, Vec<CoordinatorCandidate>> = HashMap::new();
    for row in rows {
        by_offering
            .entry(row.offering_id)
            .or_default()
            .push(CoordinatorCandidate {
                option: AssessmentCoordinatorOption {
                    teacher_id: row.teacher_id,
                    display_name: row.display_name,
                    learning_group_count: row.learning_group_count,
                    primary_learning_group_count: row.primary_learning_group_count,
                },
                primary_assignment_count: row.primary_assignment_count,
            });
    }
    Ok(by_offering)
}

async fn load_candidate_ids_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    offering_id: Uuid,
) -> Result<HashSet<Uuid>, AppError> {
    Ok(sqlx::query_scalar(
        r#"SELECT DISTINCT teacher.teacher_id
           FROM learning_offerings offering
           JOIN academic_terms term ON term.id = offering.academic_term_id
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = offering.id
            AND learning_group.status <> 'closed'
           JOIN learning_group_teachers teacher
             ON teacher.learning_group_id = learning_group.id
            AND teacher.starts_on <= LEAST(
                GREATEST(current_date, term.start_date), term.planned_end_date
            )
            AND (
                teacher.ends_on IS NULL
                OR teacher.ends_on >= LEAST(
                    GREATEST(current_date, term.start_date), term.planned_end_date
                )
            )
           JOIN users user_account
             ON user_account.id = teacher.teacher_id
            AND user_account.status = 'active'
           WHERE offering.id = $1"#,
    )
    .bind(offering_id)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .collect())
}

fn suggested_coordinator(
    candidates: &[CoordinatorCandidate],
    total_learning_group_count: i64,
) -> (Option<Uuid>, Option<String>) {
    if total_learning_group_count == 0 {
        return (None, None);
    }
    let matches = candidates
        .iter()
        .filter(|candidate| {
            candidate.option.primary_learning_group_count == total_learning_group_count
                && candidate.primary_assignment_count == total_learning_group_count
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return (None, None);
    }
    (
        Some(matches[0].option.teacher_id),
        Some(matches[0].option.display_name.clone()),
    )
}

fn phase_control_from_row(
    row: AssessmentPhaseControlRow,
) -> Result<AssessmentPhaseControl, AppError> {
    let phase_code = AssessmentPhaseCode::try_from(row.phase_code.as_str()).map_err(|reason| {
        tracing::error!(%reason, "invalid assessment phase control code stored in database");
        AppError::InternalServerError("รหัสช่วงคะแนนในระบบไม่ถูกต้อง".to_string())
    })?;
    Ok(AssessmentPhaseControl {
        id: row.id,
        academic_term_id: row.academic_term_id,
        academic_year_id: row.academic_year_id,
        phase_code,
        label: phase_code.label_th().to_string(),
        order: phase_code.order(),
        item_editing_enabled: row.item_editing_enabled,
        score_entry_enabled: row.score_entry_enabled,
        row_version: row.row_version,
    })
}

fn decimal_string(value: &BigDecimal) -> String {
    value.normalized().to_string()
}

#[cfg(test)]
mod tests {
    use super::{default_phases, plan_readiness, validate_plan_payload};
    use crate::error::AppError;
    use crate::modules::academic::models::assessment::{
        AssessmentExamArrangement, AssessmentPhaseCode, AssessmentReadinessFinding,
        SaveAssessmentPhaseRequest, SaveAssessmentPlanRequest,
    };
    use bigdecimal::BigDecimal;
    use uuid::Uuid;

    fn phase(
        phase_code: AssessmentPhaseCode,
        score: &str,
        exam_arrangement: AssessmentExamArrangement,
        exam_duration_minutes: Option<i32>,
    ) -> SaveAssessmentPhaseRequest {
        SaveAssessmentPhaseRequest {
            id: None,
            max_score: score.to_string(),
            phase_code,
            exam_arrangement,
            exam_duration_minutes,
        }
    }

    fn complete_payload() -> SaveAssessmentPlanRequest {
        SaveAssessmentPlanRequest {
            row_version: None,
            assessment_coordinator_id: Some(Uuid::new_v4()),
            phases: vec![
                phase(
                    AssessmentPhaseCode::BeforeMidterm,
                    "30.00",
                    AssessmentExamArrangement::None,
                    None,
                ),
                phase(
                    AssessmentPhaseCode::Midterm,
                    "20.00",
                    AssessmentExamArrangement::InTimetable,
                    Some(60),
                ),
                phase(
                    AssessmentPhaseCode::AfterMidterm,
                    "20.00",
                    AssessmentExamArrangement::None,
                    None,
                ),
                phase(
                    AssessmentPhaseCode::Final,
                    "30.00",
                    AssessmentExamArrangement::OutsideTimetable,
                    None,
                ),
            ],
        }
    }

    #[test]
    fn fixed_phase_payload_accepts_temporary_under_or_over_allocation() {
        let expected = "100.00".parse::<BigDecimal>().unwrap();
        let mut payload = complete_payload();
        assert!(validate_plan_payload(&payload).is_ok());

        payload.phases[0].max_score = "10.00".to_string();
        assert!(validate_plan_payload(&payload).is_ok());
        assert!(!plan_readiness(&payload, &expected).unwrap().ready);

        payload.phases[0].max_score = "70.00".to_string();
        assert!(validate_plan_payload(&payload).is_ok());
        assert!(!plan_readiness(&payload, &expected).unwrap().ready);
    }

    #[test]
    fn readiness_is_derived_from_coordinator_total_and_exam_duration() {
        let expected = "100.00".parse::<BigDecimal>().unwrap();
        let mut payload = complete_payload();
        let ready = plan_readiness(&payload, &expected).unwrap();
        assert!(ready.ready);
        assert!(ready.findings.is_empty());

        payload.assessment_coordinator_id = None;
        payload.phases[1].exam_duration_minutes = None;
        let incomplete = plan_readiness(&payload, &expected).unwrap();
        assert!(!incomplete.ready);
        assert_eq!(
            incomplete.findings,
            vec![
                AssessmentReadinessFinding::MissingCoordinator,
                AssessmentReadinessFinding::MidtermMissingExamDuration,
            ]
        );
    }

    #[test]
    fn duplicate_or_noncanonical_phase_shape_is_rejected() {
        let mut payload = complete_payload();
        payload.phases[3].phase_code = AssessmentPhaseCode::Midterm;
        assert!(matches!(
            validate_plan_payload(&payload),
            Err(AppError::ValidationError(message)) if message.contains("4 ช่วง")
        ));

        let mut invalid_arrangement = complete_payload();
        invalid_arrangement.phases[0].exam_arrangement = AssessmentExamArrangement::InTimetable;
        assert!(matches!(
            validate_plan_payload(&invalid_arrangement),
            Err(AppError::ValidationError(message)) if message.contains("ก่อนกลางภาค")
        ));
    }

    #[test]
    fn defaults_are_four_system_owned_phase_rows() {
        let phases = default_phases();
        assert_eq!(phases.len(), 4);
        assert_eq!(
            phases
                .iter()
                .map(|phase| phase.phase_code)
                .collect::<Vec<_>>(),
            AssessmentPhaseCode::ALL
        );
        assert!(phases
            .iter()
            .all(|phase| phase.id.is_none() && phase.max_score == "0.00"));
    }
}
