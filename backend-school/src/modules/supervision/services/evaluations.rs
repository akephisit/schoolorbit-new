use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::supervision::models::{
    EvaluationResponseInput, ReplaceObservationEvaluatorsRequest, SaveEvaluationRequest,
    SupervisionEvaluatorAvailability, SupervisionEvaluatorConflict, SupervisionEvaluatorStatus,
    SupervisionObservation, SupervisionObservationStatus, SupervisionTemplateItemType,
};

use super::observations::{get_observation, insert_observation_action};
use super::shared::{
    all_required_evaluators_submitted, can_transition_observation_status,
    evaluator_conflict_status_codes, manager_can_edit_observation, normalize_evaluator_replacement,
    parse_template_item_type, EvaluatorReplacementState, EvaluatorSubmissionState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EvaluationItemSpec {
    pub(super) item_type: SupervisionTemplateItemType,
    pub(super) rating_min: i32,
    pub(super) rating_max: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct EvaluationResponseBulkRow {
    pub(super) template_item_id: Uuid,
    pub(super) rating_score: Option<f64>,
    pub(super) text_response: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct EvaluatorAvailabilityRow {
    pub(super) id: Uuid,
    pub(super) title: Option<String>,
    pub(super) first_name: String,
    pub(super) last_name: String,
    pub(super) conflict_observation_id: Option<Uuid>,
    pub(super) conflict_observed_display_name: Option<String>,
    pub(super) conflict_observed_at: Option<DateTime<Utc>>,
    pub(super) conflict_subject_name: Option<String>,
    pub(super) conflict_period_label: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct EvaluatorConflictRow {
    evaluator_display_name: Option<String>,
    observed_display_name: Option<String>,
    subject_name: Option<String>,
    period_label: Option<String>,
}

pub async fn replace_observation_evaluators(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: ReplaceObservationEvaluatorsRequest,
) -> Result<SupervisionObservation, AppError> {
    let current = get_observation(pool, observation_id).await?;
    if !manager_can_edit_observation(current.status) {
        return Err(AppError::ValidationError(
            "แก้ไขผู้ประเมินได้เฉพาะสถานะรออนุมัติ วางแผน หรือส่งกลับ".to_string(),
        ));
    }
    if input
        .evaluators
        .iter()
        .any(|evaluator| evaluator.evaluator_user_id == current.observed_user_id)
    {
        return Err(AppError::ValidationError(
            "ครูผู้ถูกนิเทศเป็นผู้ประเมินรายการของตนเองไม่ได้".to_string(),
        ));
    }
    let requested_evaluator_user_ids = input
        .evaluators
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect::<Vec<_>>();
    validate_evaluator_availability_for_observation(
        pool,
        observation_id,
        current.observed_at,
        &requested_evaluator_user_ids,
    )
    .await?;

    let existing_states = current
        .evaluators
        .iter()
        .map(|evaluator| EvaluatorReplacementState {
            evaluator_user_id: evaluator.evaluator_user_id,
            submitted: evaluator.status == SupervisionEvaluatorStatus::Submitted,
        })
        .collect::<Vec<_>>();
    let submitted_user_ids = existing_states
        .iter()
        .filter(|state| state.submitted)
        .map(|state| state.evaluator_user_id)
        .collect::<HashSet<_>>();
    let replacement = normalize_evaluator_replacement(&existing_states, input.evaluators)?;
    let insert_rows = replacement
        .into_iter()
        .filter(|evaluator| !submitted_user_ids.contains(&evaluator.evaluator_user_id))
        .collect::<Vec<_>>();

    let mut tx = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin replace supervision evaluators transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถเริ่มแก้ไขผู้ประเมินได้".to_string())
    })?;

    sqlx::query(
        r#"
        DELETE FROM supervision_evaluators
        WHERE observation_id = $1
          AND status <> 'submitted'
        "#,
    )
    .bind(observation_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| {
        tracing::error!(
            "Failed to clear non-submitted supervision evaluators: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถแก้ไขผู้ประเมินได้".to_string())
    })?;

    insert_supervision_evaluators(&mut tx, observation_id, &insert_rows).await?;

    tx.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit replace supervision evaluators transaction: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกผู้ประเมินได้".to_string())
    })?;

    insert_observation_action(
        pool,
        observation_id,
        Some(actor_user_id),
        "evaluators_updated",
        Some(current.status),
        Some(current.status),
        None,
    )
    .await?;

    get_observation(pool, observation_id).await
}

pub(super) async fn insert_supervision_evaluators(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    observation_id: Uuid,
    evaluators: &[crate::modules::supervision::models::EvaluatorAssignmentInput],
) -> Result<(), AppError> {
    if evaluators.is_empty() {
        return Ok(());
    }

    let evaluator_user_ids: Vec<Uuid> = evaluators
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect();
    let role_labels: Vec<Option<String>> = evaluators
        .iter()
        .map(|evaluator| evaluator.role_label.clone())
        .collect();
    let required_flags: Vec<bool> = evaluators
        .iter()
        .map(|evaluator| evaluator.is_required.unwrap_or(true))
        .collect();

    sqlx::query(
        r#"
        INSERT INTO supervision_evaluators (
            observation_id, evaluator_user_id, role_label, is_required
        )
        SELECT $1, evaluator_user_id, role_label, is_required
        FROM UNNEST($2::uuid[], $3::text[], $4::bool[])
             AS rows(evaluator_user_id, role_label, is_required)
        "#,
    )
    .bind(observation_id)
    .bind(&evaluator_user_ids)
    .bind(&role_labels)
    .bind(&required_flags)
    .execute(&mut **tx)
    .await
    .map_err(|error| {
        tracing::error!("Failed to assign supervision evaluators: {}", error);
        AppError::InternalServerError("ไม่สามารถกำหนดผู้ประเมินได้".to_string())
    })?;

    Ok(())
}

async fn save_my_evaluation(
    pool: &PgPool,
    evaluator_user_id: Uuid,
    observation_id: Uuid,
    input: SaveEvaluationRequest,
) -> Result<SupervisionObservation, AppError> {
    let evaluator = load_evaluator_for_user(pool, observation_id, evaluator_user_id).await?;
    if evaluator.status == "submitted" {
        return Err(AppError::ValidationError(
            "ส่งผลประเมินแล้ว ไม่สามารถแก้ไขได้".to_string(),
        ));
    }

    let responses = dedupe_evaluation_responses(input.responses);
    let template_item_ids = responses
        .iter()
        .map(|response| response.template_item_id)
        .collect::<Vec<_>>();
    let item_specs = load_evaluation_item_specs(pool, observation_id, &template_item_ids).await?;
    let response_rows = build_evaluation_response_bulk_rows(&responses, &item_specs)?;
    bulk_upsert_evaluation_responses(pool, observation_id, evaluator.id, &response_rows).await?;

    sqlx::query(
        r#"
        UPDATE supervision_evaluators
        SET status = 'draft'
        WHERE id = $1 AND status = 'assigned'
        "#,
    )
    .bind(evaluator.id)
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to mark supervision evaluation draft: {}", error);
        AppError::InternalServerError("ไม่สามารถบันทึกสถานะผลประเมินได้".to_string())
    })?;

    get_observation(pool, observation_id).await
}

pub async fn submit_my_evaluation(
    pool: &PgPool,
    evaluator_user_id: Uuid,
    observation_id: Uuid,
    input: SaveEvaluationRequest,
) -> Result<SupervisionObservation, AppError> {
    if !input.responses.is_empty() {
        save_my_evaluation(pool, evaluator_user_id, observation_id, input).await?;
    }

    let evaluator = load_evaluator_for_user(pool, observation_id, evaluator_user_id).await?;
    sqlx::query(
        r#"
        UPDATE supervision_evaluators
        SET status = 'submitted', submitted_at = now()
        WHERE id = $1
        "#,
    )
    .bind(evaluator.id)
    .execute(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to submit supervision evaluation: {}", error);
        AppError::InternalServerError("ไม่สามารถส่งผลประเมินได้".to_string())
    })?;

    let states = load_evaluator_submission_states(pool, observation_id).await?;
    if all_required_evaluators_submitted(&states) {
        let current = get_observation(pool, observation_id).await?;
        if can_transition_observation_status(
            current.status,
            SupervisionObservationStatus::EvaluatorsSubmitted,
        ) {
            sqlx::query(
                "UPDATE supervision_observations SET status = 'evaluators_submitted' WHERE id = $1",
            )
            .bind(observation_id)
            .execute(pool)
            .await
            .map_err(|error| {
                tracing::error!("Failed to mark supervision evaluators submitted: {}", error);
                AppError::InternalServerError("ไม่สามารถอัปเดตสถานะนิเทศได้".to_string())
            })?;
        }
    }

    insert_observation_action(
        pool,
        observation_id,
        Some(evaluator_user_id),
        "evaluator_submitted",
        None,
        None,
        None,
    )
    .await?;

    get_observation(pool, observation_id).await
}

#[derive(Debug, sqlx::FromRow)]
struct EvaluatorForUserRow {
    id: Uuid,
    status: String,
}

async fn load_evaluator_for_user(
    pool: &PgPool,
    observation_id: Uuid,
    evaluator_user_id: Uuid,
) -> Result<EvaluatorForUserRow, AppError> {
    sqlx::query_as::<_, EvaluatorForUserRow>(
        r#"
        SELECT id, status
        FROM supervision_evaluators
        WHERE observation_id = $1 AND evaluator_user_id = $2
        "#,
    )
    .bind(observation_id)
    .bind(evaluator_user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision evaluator: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบผู้ประเมินได้".to_string())
    })?
    .ok_or_else(|| AppError::Forbidden("ไม่ได้รับมอบหมายให้ประเมินรายการนี้".to_string()))
}

pub(super) fn dedupe_evaluation_responses(
    responses: Vec<EvaluationResponseInput>,
) -> Vec<EvaluationResponseInput> {
    let mut ordered = Vec::<EvaluationResponseInput>::with_capacity(responses.len());
    let mut index_by_item = HashMap::<Uuid, usize>::with_capacity(responses.len());

    for response in responses {
        if let Some(index) = index_by_item.get(&response.template_item_id).copied() {
            ordered[index] = response;
        } else {
            index_by_item.insert(response.template_item_id, ordered.len());
            ordered.push(response);
        }
    }

    ordered
}

async fn load_evaluation_item_specs(
    pool: &PgPool,
    observation_id: Uuid,
    template_item_ids: &[Uuid],
) -> Result<HashMap<Uuid, EvaluationItemSpec>, AppError> {
    if template_item_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query(
        r#"
        SELECT i.id, i.item_type, t.rating_min, t.rating_max
        FROM supervision_template_items i
        JOIN supervision_template_sections s ON i.section_id = s.id
        JOIN supervision_templates t ON s.template_id = t.id
        JOIN supervision_observations o ON o.template_id = t.id
        WHERE o.id = $1 AND i.id = ANY($2)
        "#,
    )
    .bind(observation_id)
    .bind(template_item_ids)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision response item specs: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบหัวข้อประเมินได้".to_string())
    })?;

    let mut specs = HashMap::with_capacity(rows.len());
    for row in rows {
        let item_id: Uuid = row.try_get("id").map_err(|error| {
            tracing::error!("Failed to read supervision item id: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบหัวข้อประเมินได้".to_string())
        })?;
        let item_type: String = row.try_get("item_type").map_err(|error| {
            tracing::error!("Failed to read supervision item type: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบหัวข้อประเมินได้".to_string())
        })?;
        let rating_min: i32 = row.try_get("rating_min").map_err(|error| {
            tracing::error!("Failed to read supervision rating min: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบคะแนนประเมินได้".to_string())
        })?;
        let rating_max: i32 = row.try_get("rating_max").map_err(|error| {
            tracing::error!("Failed to read supervision rating max: {}", error);
            AppError::InternalServerError("ไม่สามารถตรวจสอบคะแนนประเมินได้".to_string())
        })?;

        specs.insert(
            item_id,
            EvaluationItemSpec {
                item_type: parse_template_item_type(&item_type)?,
                rating_min,
                rating_max,
            },
        );
    }

    Ok(specs)
}

pub(super) fn build_evaluation_response_bulk_rows(
    responses: &[EvaluationResponseInput],
    item_specs: &HashMap<Uuid, EvaluationItemSpec>,
) -> Result<Vec<EvaluationResponseBulkRow>, AppError> {
    let mut rows = Vec::with_capacity(responses.len());

    for response in responses {
        let spec = item_specs
            .get(&response.template_item_id)
            .ok_or_else(|| AppError::ValidationError("หัวข้อประเมินไม่อยู่ในแบบประเมินนี้".to_string()))?;

        match spec.item_type {
            SupervisionTemplateItemType::Rating => {
                if response
                    .text_response
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
                {
                    return Err(AppError::ValidationError(
                        "หัวข้อแบบคะแนนไม่รับคำตอบข้อความ".to_string(),
                    ));
                }
                if let Some(score) = response.rating_score {
                    if score < spec.rating_min as f64 || score > spec.rating_max as f64 {
                        return Err(AppError::ValidationError(
                            "คะแนนอยู่นอกช่วงที่แบบประเมินกำหนด".to_string(),
                        ));
                    }
                }
            }
            SupervisionTemplateItemType::Text => {
                if response.rating_score.is_some() {
                    return Err(AppError::ValidationError(
                        "หัวข้อแบบข้อความไม่รับคะแนน".to_string(),
                    ));
                }
            }
        }

        rows.push(EvaluationResponseBulkRow {
            template_item_id: response.template_item_id,
            rating_score: response.rating_score,
            text_response: response.text_response.clone(),
        });
    }

    Ok(rows)
}

async fn bulk_upsert_evaluation_responses(
    pool: &PgPool,
    observation_id: Uuid,
    evaluator_id: Uuid,
    rows: &[EvaluationResponseBulkRow],
) -> Result<(), AppError> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::new(
        r#"
        INSERT INTO supervision_evaluator_responses (
            observation_id, evaluator_id, template_item_id, rating_score, text_response
        )
        "#,
    );
    builder.push_values(rows, |mut row_builder, row| {
        row_builder
            .push_bind(observation_id)
            .push_bind(evaluator_id)
            .push_bind(row.template_item_id)
            .push_bind(row.rating_score)
            .push_unseparated("::numeric")
            .push_bind(&row.text_response);
    });
    builder.push(
        r#"
        ON CONFLICT (evaluator_id, template_item_id)
        DO UPDATE SET
            rating_score = EXCLUDED.rating_score,
            text_response = EXCLUDED.text_response,
            updated_at = now()
        "#,
    );

    builder.build().execute(pool).await.map_err(|error| {
        tracing::error!(
            "Failed to bulk upsert supervision evaluation responses: {}",
            error
        );
        AppError::InternalServerError("ไม่สามารถบันทึกผลประเมินได้".to_string())
    })?;

    Ok(())
}

pub(super) async fn load_evaluator_submission_states(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Vec<EvaluatorSubmissionState>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT is_required, status
        FROM supervision_evaluators
        WHERE observation_id = $1
        "#,
    )
    .bind(observation_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision evaluator states: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบสถานะผู้ประเมินได้".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| EvaluatorSubmissionState {
            is_required: row.try_get("is_required").unwrap_or(false),
            submitted: row
                .try_get::<String, _>("status")
                .map(|status| status == "submitted")
                .unwrap_or(false),
        })
        .collect())
}

pub(super) fn evaluator_availability_from_row(
    row: EvaluatorAvailabilityRow,
) -> SupervisionEvaluatorAvailability {
    let name = format!("{} {}", row.first_name, row.last_name);
    let conflict = row
        .conflict_observation_id
        .zip(row.conflict_observed_at)
        .map(
            |(observation_id, observed_at)| SupervisionEvaluatorConflict {
                observation_id,
                observed_display_name: row.conflict_observed_display_name.clone(),
                observed_at,
                lesson_title: conflict_lesson_title(
                    row.conflict_subject_name.as_deref(),
                    row.conflict_period_label.as_deref(),
                ),
            },
        );
    let conflict_reason = conflict.as_ref().map(|conflict| {
        let observed_name = conflict
            .observed_display_name
            .as_deref()
            .unwrap_or("ครูอีกคน");
        let lesson = conflict.lesson_title.as_deref().unwrap_or("คาบเดียวกัน");
        format!("มีงานนิเทศ {observed_name} ({lesson}) ในช่วงเวลาเดียวกัน")
    });

    SupervisionEvaluatorAvailability {
        id: row.id,
        name,
        title: row.title,
        available: conflict.is_none(),
        conflict_reason,
        conflict,
    }
}

fn conflict_lesson_title(subject_name: Option<&str>, period_label: Option<&str>) -> Option<String> {
    match (
        subject_name.filter(|value| !value.trim().is_empty()),
        period_label.filter(|value| !value.trim().is_empty()),
    ) {
        (Some(subject), Some(period)) => Some(format!("{subject} / {period}")),
        (Some(subject), None) => Some(subject.to_string()),
        (None, Some(period)) => Some(period.to_string()),
        (None, None) => None,
    }
}

pub(super) async fn validate_evaluator_availability_for_observation(
    pool: &PgPool,
    observation_id: Uuid,
    observed_at: DateTime<Utc>,
    evaluator_user_ids: &[Uuid],
) -> Result<(), AppError> {
    if evaluator_user_ids.is_empty() {
        return Ok(());
    }

    let conflict_statuses = evaluator_conflict_status_codes()
        .iter()
        .map(|status| (*status).to_string())
        .collect::<Vec<_>>();
    let conflict = sqlx::query_as::<_, EvaluatorConflictRow>(
        r#"
        SELECT NULLIF(TRIM(CONCAT(COALESCE(evaluator.title, ''), evaluator.first_name, ' ', evaluator.last_name)), '')
                   AS evaluator_display_name,
               NULLIF(TRIM(CONCAT(COALESCE(observed.title, ''), observed.first_name, ' ', observed.last_name)), '')
                   AS observed_display_name,
               COALESCE(NULLIF(o.manual_subject_name, ''), NULLIF(o.lesson_snapshot->>'subjectName', ''))
                   AS subject_name,
               COALESCE(NULLIF(o.manual_period_label, ''), NULLIF(o.lesson_snapshot->>'periodLabel', ''))
                   AS period_label
        FROM supervision_evaluators e
        JOIN supervision_observations o ON o.id = e.observation_id
        JOIN users evaluator ON evaluator.id = e.evaluator_user_id
        JOIN users observed ON observed.id = o.observed_user_id
        WHERE e.evaluator_user_id = ANY($1::uuid[])
          AND o.id <> $2
          AND o.observed_at = $3
          AND o.status = ANY($4::text[])
        ORDER BY o.approved_at DESC NULLS LAST, o.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(evaluator_user_ids)
    .bind(observation_id)
    .bind(observed_at)
    .bind(&conflict_statuses)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to validate supervision evaluator availability: {}", error);
        AppError::InternalServerError("ไม่สามารถตรวจสอบผู้ประเมินที่ว่างได้".to_string())
    })?;

    if let Some(conflict) = conflict {
        let evaluator_name = conflict
            .evaluator_display_name
            .as_deref()
            .unwrap_or("ผู้ประเมินที่เลือก");
        let observed_name = conflict
            .observed_display_name
            .as_deref()
            .unwrap_or("ครูอีกคน");
        let lesson = conflict_lesson_title(
            conflict.subject_name.as_deref(),
            conflict.period_label.as_deref(),
        )
        .unwrap_or_else(|| "คาบเดียวกัน".to_string());

        return Err(AppError::ValidationError(format!(
            "{evaluator_name} มีงานนิเทศ {observed_name} ({lesson}) ในช่วงเวลาเดียวกัน"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_evaluation_responses_keep_the_latest_answer() {
        let item_id = Uuid::new_v4();
        let responses = dedupe_evaluation_responses(vec![
            EvaluationResponseInput {
                template_item_id: item_id,
                rating_score: Some(2.0),
                text_response: None,
            },
            EvaluationResponseInput {
                template_item_id: item_id,
                rating_score: Some(5.0),
                text_response: None,
            },
        ]);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].template_item_id, item_id);
        assert_eq!(responses[0].rating_score, Some(5.0));
    }
    #[test]
    fn evaluation_bulk_rows_validate_item_type_and_rating_range() {
        let rating_item_id = Uuid::new_v4();
        let text_item_id = Uuid::new_v4();
        let specs = HashMap::from([
            (
                rating_item_id,
                EvaluationItemSpec {
                    item_type: SupervisionTemplateItemType::Rating,
                    rating_min: 1,
                    rating_max: 5,
                },
            ),
            (
                text_item_id,
                EvaluationItemSpec {
                    item_type: SupervisionTemplateItemType::Text,
                    rating_min: 1,
                    rating_max: 5,
                },
            ),
        ]);

        let rows = build_evaluation_response_bulk_rows(
            &[
                EvaluationResponseInput {
                    template_item_id: rating_item_id,
                    rating_score: Some(4.0),
                    text_response: None,
                },
                EvaluationResponseInput {
                    template_item_id: text_item_id,
                    rating_score: None,
                    text_response: Some("จัดกิจกรรมได้ต่อเนื่อง".to_string()),
                },
            ],
            &specs,
        )
        .expect("valid bulk rows");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].rating_score, Some(4.0));
        assert_eq!(rows[1].text_response.as_deref(), Some("จัดกิจกรรมได้ต่อเนื่อง"));

        let invalid = build_evaluation_response_bulk_rows(
            &[EvaluationResponseInput {
                template_item_id: rating_item_id,
                rating_score: Some(6.0),
                text_response: None,
            }],
            &specs,
        );

        assert!(
            matches!(invalid, Err(AppError::ValidationError(message)) if message == "คะแนนอยู่นอกช่วงที่แบบประเมินกำหนด")
        );
    }
}
