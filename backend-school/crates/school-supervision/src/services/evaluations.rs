use school_database::DbSession;
use std::collections::{HashMap, HashSet};

use sqlx::{PgConnection, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::models::{
    EvaluationResponseInput, ReplaceObservationEvaluatorsRequest, SaveEvaluationRequest,
    SupervisionObservation, SupervisionObservationStatus, SupervisionTemplateItemType,
};
use school_errors::AppError;

use super::lifecycle::require_observation_write;
use super::observations::{
    get_observation, get_observation_with_connection, insert_observation_action,
};
use super::shared::{
    all_required_evaluators_submitted, can_view_observation_results, manager_can_edit_observation,
    normalize_evaluator_replacement, parse_template_item_type, EvaluatorReplacementState,
    EvaluatorSubmissionState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EvaluationItemSpec {
    pub(super) item_type: SupervisionTemplateItemType,
    pub(super) rating_min: i32,
    pub(super) rating_max: i32,
    pub(super) required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct EvaluationResponseBulkRow {
    pub(super) template_item_id: Uuid,
    pub(super) rating_score: Option<f64>,
    pub(super) text_response: Option<String>,
}

pub async fn replace_observation_evaluators(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: ReplaceObservationEvaluatorsRequest,
) -> Result<SupervisionObservation, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        &pool,
        "supervision.evaluations.replace_observation_evaluators",
    )
    .await?;
    require_observation_write(&mut tx, observation_id).await?;
    let current = get_observation_with_connection(&mut tx, observation_id).await?;
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
    let retained_user_ids = evaluator_submission_evidence_user_ids(&mut tx, observation_id).await?;
    let existing_states = current
        .evaluators
        .iter()
        .map(|evaluator| EvaluatorReplacementState {
            evaluator_user_id: evaluator.evaluator_user_id,
            retain: retained_user_ids.contains(&evaluator.evaluator_user_id),
            is_required: evaluator.is_required,
        })
        .collect::<Vec<_>>();
    let replacement = normalize_evaluator_replacement(&existing_states, input.evaluators)?;
    // Evaluators with submission evidence are retained even when omitted from the requested
    // replacement. Validate the complete accepted assignment set under its locks.
    let evaluator_user_ids = replacement
        .iter()
        .map(|e| e.evaluator_user_id)
        .collect::<Vec<_>>();
    let insert_rows = replacement
        .into_iter()
        .filter(|evaluator| !retained_user_ids.contains(&evaluator.evaluator_user_id))
        .collect::<Vec<_>>();
    let new_evaluator_user_ids = insert_rows
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect::<Vec<_>>();
    super::availability::validate_replacement(
        &mut tx,
        &current,
        &evaluator_user_ids,
        &new_evaluator_user_ids,
    )
    .await?;

    let retained_user_ids: Vec<_> = retained_user_ids.into_iter().collect();
    DbSession::new(&mut *tx)
        .query(
            "supervision.services.evaluations.replace_observation_evaluators",
            r#"
        DELETE FROM supervision_evaluators
        WHERE observation_id = $1
          AND evaluator_user_id <> ALL($2)
        "#,
        )
        .bind(observation_id)
        .bind(retained_user_ids)
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to clear replaceable supervision evaluators: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถแก้ไขผู้ประเมินได้".to_string())
        })?;

    insert_supervision_evaluators(&mut tx, observation_id, &insert_rows).await?;

    let action_id = insert_observation_action(
        &mut tx,
        observation_id,
        Some(actor_user_id),
        "evaluators_updated",
        Some(current.status),
        Some(current.status),
        None,
    )
    .await?;
    let published = publish_completed_evaluations(&mut tx, &current, actor_user_id).await?;
    super::notifications::evaluators_replaced(
        &mut tx,
        action_id,
        &current,
        &evaluator_user_ids,
        published,
    )
    .await?;

    tx.commit().await?;
    get_observation(pool, observation_id).await
}

pub(super) async fn evaluator_submission_evidence_user_ids(
    connection: &mut PgConnection,
    observation_id: Uuid,
) -> Result<HashSet<Uuid>, AppError> {
    let user_ids = DbSession::new(connection)
        .scalar(
            "supervision.evaluations.retained_evaluator_users",
            "SELECT evaluator_user_id FROM supervision_evaluators
             WHERE observation_id = $1 AND
                 (status = 'submitted' OR submission_reopened OR (status = 'draft' AND submitted_at IS NOT NULL))",
        )
        .bind(observation_id)
        .fetch_all()
        .await?;
    Ok(user_ids.into_iter().collect())
}

pub(super) async fn insert_supervision_evaluators(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    observation_id: Uuid,
    evaluators: &[crate::models::EvaluatorAssignmentInput],
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

    DbSession::new(&mut **tx)
        .query(
            "supervision.services.evaluations.insert_supervision_evaluators",
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
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to assign supervision evaluators: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถกำหนดผู้ประเมินได้".to_string())
        })?;

    Ok(())
}

pub async fn submit_my_evaluation(
    pool: &PgPool,
    evaluator_user_id: Uuid,
    observation_id: Uuid,
    input: SaveEvaluationRequest,
) -> Result<SupervisionObservation, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        &pool,
        "supervision.evaluations.submit_my_evaluation",
    )
    .await?;
    load_evaluator_for_user(&mut *tx, observation_id, evaluator_user_id).await?;
    require_observation_write(&mut tx, observation_id).await?;
    let evaluator = load_evaluator_for_user(&mut *tx, observation_id, evaluator_user_id).await?;
    let observation = get_observation_with_connection(&mut tx, observation_id).await?;
    let already_submitted = evaluator.status == "submitted";
    let released = can_view_observation_results(observation.status, false);
    if !(accepts_evaluations(observation.status) || already_submitted && released) {
        return Err(AppError::ValidationError(
            "ส่งผลประเมินได้เฉพาะรายการที่ยังเปิดรับผลประเมิน".into(),
        ));
    }
    let responses = dedupe_evaluation_responses(input.responses);
    if already_submitted {
        if !matches_submitted_responses(&mut tx, evaluator.id, &responses).await? {
            return Err(AppError::ValidationError(
                "ส่งผลประเมินแล้ว ไม่สามารถแก้ไขได้".into(),
            ));
        }
        tx.commit().await?;
        return Ok(observation);
    }

    let _: Uuid = DbSession::new(&mut tx)
        .scalar(
            "supervision.evaluations.lock_template_for_submission",
            "SELECT id FROM supervision_templates WHERE id = $1 FOR SHARE",
        )
        .bind(observation.template_id)
        .fetch_one()
        .await?;
    let template_item_ids: Vec<_> = responses
        .iter()
        .map(|response| response.template_item_id)
        .collect();
    let item_specs =
        load_evaluation_item_specs(&mut tx, observation_id, &template_item_ids).await?;
    let response_rows = build_evaluation_response_bulk_rows(&responses, &item_specs)?;
    require_completed_evaluation(&item_specs, &response_rows)?;
    bulk_upsert_evaluation_responses(&mut tx, observation_id, evaluator.id, &response_rows).await?;
    DbSession::new(&mut *tx)
        .query(
            "supervision.services.evaluations.submit_my_evaluation",
            r#"
        UPDATE supervision_evaluators
        SET status = 'submitted', submitted_at = now()
        WHERE id = $1
        "#,
        )
        .bind(evaluator.id)
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to submit supervision evaluation: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถส่งผลประเมินได้".to_string())
        })?;

    insert_observation_action(
        &mut tx,
        observation_id,
        Some(evaluator_user_id),
        "evaluator_submitted",
        None,
        None,
        None,
    )
    .await?;
    publish_completed_evaluations(&mut tx, &observation, evaluator_user_id).await?;

    tx.commit().await?;
    get_observation(pool, observation_id).await
}

fn accepts_evaluations(status: SupervisionObservationStatus) -> bool {
    matches!(
        status,
        SupervisionObservationStatus::Planned | SupervisionObservationStatus::InProgress
    )
}

/// The caller holds the observation write lock for the evaluation or roster change.
async fn publish_completed_evaluations(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    observation: &SupervisionObservation,
    actor_user_id: Uuid,
) -> Result<bool, AppError> {
    if !accepts_evaluations(observation.status)
        || !all_required_evaluators_submitted(
            &load_evaluator_submission_states(tx, observation.id).await?,
        )
    {
        return Ok(false);
    }

    let published = SupervisionObservationStatus::Published;
    DbSession::new(&mut **tx)
        .query(
            "supervision.evaluations.publish_completed_evaluations",
            "UPDATE supervision_observations SET status = $2 WHERE id = $1",
        )
        .bind(observation.id)
        .bind(published.as_str())
        .execute()
        .await?;
    let action_id = insert_observation_action(
        tx,
        observation.id,
        Some(actor_user_id),
        "result_published",
        Some(observation.status),
        Some(published),
        None,
    )
    .await?;
    super::notifications::status_changed(tx, action_id, observation, published).await?;
    Ok(true)
}

#[derive(Debug, sqlx::FromRow)]
struct EvaluatorForUserRow {
    id: Uuid,
    status: String,
}

async fn load_evaluator_for_user(
    connection: &mut PgConnection,
    observation_id: Uuid,
    evaluator_user_id: Uuid,
) -> Result<EvaluatorForUserRow, AppError> {
    DbSession::new(&mut *connection)
        .query_as::<EvaluatorForUserRow>(
            "supervision.services.evaluations.load_evaluator_for_user",
            r#"
        SELECT id, status
        FROM supervision_evaluators
        WHERE observation_id = $1 AND evaluator_user_id = $2
        "#,
        )
        .bind(observation_id)
        .bind(evaluator_user_id)
        .fetch_optional()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision evaluator: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบผู้ประเมินได้".to_string())
        })?
        .ok_or_else(|| AppError::Forbidden("ไม่ได้รับมอบหมายให้ประเมินรายการนี้".to_string()))
}

async fn matches_submitted_responses(
    connection: &mut PgConnection,
    evaluator_id: Uuid,
    rows: &[EvaluationResponseInput],
) -> Result<bool, AppError> {
    if rows.is_empty() {
        return Ok(true);
    }

    let item_ids: Vec<_> = rows.iter().map(|row| row.template_item_id).collect();
    let scores: Vec<_> = rows.iter().map(|row| row.rating_score).collect();
    let texts: Vec<_> = rows
        .iter()
        .map(|row| row.text_response.as_deref())
        .collect();
    Ok(DbSession::new(connection)
        .scalar(
            "supervision.evaluations.matches_submitted_responses",
            r#"
            SELECT NOT EXISTS (
                SELECT 1
                FROM UNNEST($2::uuid[], $3::double precision[], $4::text[])
                     AS incoming(item_id, score, answer)
                LEFT JOIN supervision_evaluator_responses saved
                  ON saved.evaluator_id = $1 AND saved.template_item_id = incoming.item_id
                WHERE saved.id IS NULL
                   OR saved.rating_score IS DISTINCT FROM round(incoming.score::numeric, 2)
                   OR saved.text_response IS DISTINCT FROM incoming.answer
            )
            "#,
        )
        .bind(evaluator_id)
        .bind(item_ids)
        .bind(scores)
        .bind(texts)
        .fetch_one()
        .await?)
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
    connection: &mut PgConnection,
    observation_id: Uuid,
    template_item_ids: &[Uuid],
) -> Result<HashMap<Uuid, EvaluationItemSpec>, AppError> {
    let rows = DbSession::new(&mut *connection)
        .query_as::<(Uuid, String, i32, i32, bool)>(
            "supervision.services.evaluations.load_evaluation_item_specs",
            r#"
        SELECT i.id, i.item_type, t.rating_min, t.rating_max, i.required
        FROM supervision_template_items i
        JOIN supervision_template_sections s ON i.section_id = s.id
        JOIN supervision_templates t ON s.template_id = t.id
        JOIN supervision_observations o ON o.template_id = t.id
        WHERE o.id = $1 AND (i.id = ANY($2) OR i.required)
        "#,
        )
        .bind(observation_id)
        .bind(template_item_ids)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision response item specs: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบหัวข้อประเมินได้".to_string())
        })?;

    rows.into_iter()
        .map(|(item_id, item_type, rating_min, rating_max, required)| {
            Ok((
                item_id,
                EvaluationItemSpec {
                    item_type: parse_template_item_type(&item_type)?,
                    rating_min,
                    rating_max,
                    required,
                },
            ))
        })
        .collect()
}

fn require_completed_evaluation(
    item_specs: &HashMap<Uuid, EvaluationItemSpec>,
    responses: &[EvaluationResponseBulkRow],
) -> Result<(), AppError> {
    let answered: HashSet<_> = responses
        .iter()
        .filter(|response| {
            response.rating_score.is_some()
                || response
                    .text_response
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
        })
        .map(|response| response.template_item_id)
        .collect();
    if item_specs
        .iter()
        .any(|(id, spec)| spec.required && !answered.contains(id))
    {
        return Err(AppError::ValidationError(
            "กรุณาตอบหัวข้อประเมินที่จำเป็นให้ครบ".into(),
        ));
    }
    Ok(())
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
                    if !(spec.rating_min as f64..=spec.rating_max as f64).contains(&score) {
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
    tx: &mut sqlx::Transaction<'_, Postgres>,
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

    DbSession::new(&mut **tx)
        .prepared_query(
            "supervision.services.evaluations.bulk_upsert_evaluation_responses",
            builder.build(),
        )
        .execute()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to bulk upsert supervision evaluation responses: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถบันทึกผลประเมินได้".to_string())
        })?;

    Ok(())
}

async fn load_evaluator_submission_states(
    connection: &mut PgConnection,
    observation_id: Uuid,
) -> Result<Vec<EvaluatorSubmissionState>, AppError> {
    let rows: Vec<(bool, String)> = DbSession::new(&mut *connection)
        .query_as(
            "supervision.services.evaluations.load_evaluator_submission_states",
            r#"
        SELECT is_required, status
        FROM supervision_evaluators
        WHERE observation_id = $1
        "#,
        )
        .bind(observation_id)
        .fetch_all()
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision evaluator states: {}",
                school_database::error_class(&error)
            );
            AppError::InternalServerError("ไม่สามารถตรวจสอบสถานะผู้ประเมินได้".to_string())
        })?;

    Ok(rows
        .into_iter()
        .map(|(is_required, status)| EvaluatorSubmissionState {
            is_required,
            submitted: status == "submitted",
        })
        .collect())
}

pub(super) async fn validate_evaluator_availability_for_observation(
    connection: &mut PgConnection,
    observation: &SupervisionObservation,
    evaluator_user_ids: &[Uuid],
) -> Result<(), AppError> {
    let windows = observation
        .periods
        .iter()
        .map(super::availability::Window::from)
        .collect::<Vec<_>>();
    super::availability::validate(
        connection,
        observation.observed_user_id,
        observation.cycle_id,
        Some(observation.id),
        &windows,
        evaluator_user_ids,
    )
    .await
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
                    required: true,
                },
            ),
            (
                text_item_id,
                EvaluationItemSpec {
                    item_type: SupervisionTemplateItemType::Text,
                    rating_min: 1,
                    rating_max: 5,
                    required: true,
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
