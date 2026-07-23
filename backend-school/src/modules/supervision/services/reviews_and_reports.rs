use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::supervision::models::{
    AcknowledgeObservationRequest, SupervisionCycleProgress, SupervisionEvaluator,
    SupervisionObservation, SupervisionObservationReview, SupervisionObservationStatus,
    SupervisionReviewEvaluatorResult, SupervisionReviewItemSummary, SupervisionReviewResponse,
    SupervisionTeacherStatusRow,
};

use super::evaluations::load_evaluator_submission_states;
use super::observations::{get_observation, set_observation_status};
use super::shared::{
    all_required_evaluators_submitted, average_submitted_evaluator_rating,
    parse_optional_observation_status, EvaluatorRatingInput, SupervisionObservationListAccess,
};
use super::templates::get_template;

#[derive(Debug, sqlx::FromRow)]
struct TeacherStatusOverviewRow {
    teacher_id: Uuid,
    teacher_display_name: String,
    organization_unit_names: Vec<String>,
    observation_id: Option<Uuid>,
    status: Option<String>,
    observed_at: Option<DateTime<Utc>>,
    lesson_title: Option<String>,
    evaluator_names: Vec<String>,
    average_rating: Option<f64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct SupervisionReviewResponseRow {
    evaluator_id: Uuid,
    template_item_id: Uuid,
    rating_score: Option<f64>,
    text_response: Option<String>,
}

pub async fn get_observation_review(
    pool: &PgPool,
    id: Uuid,
) -> Result<SupervisionObservationReview, AppError> {
    let observation = get_observation(pool, id).await?;
    let template = get_template(pool, observation.template_id).await?;
    let response_rows = load_observation_review_responses(pool, id).await?;
    let average_rating = observation.average_rating;
    let evaluator_results = build_review_evaluator_results(&observation.evaluators, response_rows);
    let item_summaries = build_review_item_summaries(&evaluator_results);

    Ok(SupervisionObservationReview {
        observation,
        template,
        evaluator_results,
        item_summaries,
        average_rating,
    })
}

pub async fn certify_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    let states = load_evaluator_submission_states(pool, observation_id).await?;
    if !all_required_evaluators_submitted(&states) {
        return Err(AppError::ValidationError(
            "ผู้ประเมินหลักยังส่งผลไม่ครบ".to_string(),
        ));
    }

    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Approved,
        "subject_group_certified",
        None,
    )
    .await
}

pub async fn approve_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
) -> Result<SupervisionObservation, AppError> {
    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Published,
        "academic_approved",
        None,
    )
    .await
}

pub async fn acknowledge_observation(
    pool: &PgPool,
    actor_user_id: Uuid,
    observation_id: Uuid,
    input: AcknowledgeObservationRequest,
) -> Result<SupervisionObservation, AppError> {
    let observation = get_observation(pool, observation_id).await?;
    if observation.observed_user_id != actor_user_id {
        return Err(AppError::Forbidden("รับทราบผลนิเทศของผู้อื่นไม่ได้".to_string()));
    }

    let action_kind = if input
        .comment
        .as_deref()
        .is_some_and(|comment| !comment.trim().is_empty())
    {
        "acknowledged_with_comment"
    } else {
        "acknowledged"
    };

    set_observation_status(
        pool,
        observation_id,
        actor_user_id,
        SupervisionObservationStatus::Completed,
        action_kind,
        input.comment,
    )
    .await
}

pub async fn cycle_progress(
    pool: &PgPool,
    cycle_id: Uuid,
) -> Result<SupervisionCycleProgress, AppError> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total_observations,
            COUNT(*) FILTER (WHERE status = 'requested') AS requested_count,
            COUNT(*) FILTER (WHERE status = 'planned') AS planned_count,
            COUNT(*) FILTER (WHERE status IN ('evaluators_submitted', 'under_review')) AS under_review_count,
            COUNT(*) FILTER (WHERE status = 'approved') AS approved_count,
            COUNT(*) FILTER (WHERE status = 'published') AS published_count,
            COUNT(*) FILTER (WHERE status IN ('acknowledged', 'completed')) AS completed_count,
            COUNT(*) FILTER (WHERE status = 'cancelled') AS cancelled_count
        FROM supervision_observations
        WHERE cycle_id = $1
        "#,
    )
    .bind(cycle_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision cycle progress: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงรายงานรอบนิเทศได้".to_string())
    })?;

    let average_rating = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        SELECT AVG(evaluator_average)::double precision
        FROM (
            SELECT AVG(r.rating_score)::double precision AS evaluator_average
            FROM supervision_observations o
            JOIN supervision_evaluators e ON e.observation_id = o.id
            JOIN supervision_evaluator_responses r ON r.evaluator_id = e.id
            JOIN supervision_template_items i ON i.id = r.template_item_id
            WHERE o.cycle_id = $1
              AND e.status = 'submitted'
              AND i.item_type = 'rating'
              AND r.rating_score IS NOT NULL
            GROUP BY e.id
        ) evaluator_averages
        "#,
    )
    .bind(cycle_id)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision cycle rating average: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงคะแนนเฉลี่ยรอบนิเทศได้".to_string())
    })?;

    Ok(SupervisionCycleProgress {
        cycle_id,
        total_observations: row.try_get("total_observations").unwrap_or(0),
        requested_count: row.try_get("requested_count").unwrap_or(0),
        planned_count: row.try_get("planned_count").unwrap_or(0),
        under_review_count: row.try_get("under_review_count").unwrap_or(0),
        approved_count: row.try_get("approved_count").unwrap_or(0),
        published_count: row.try_get("published_count").unwrap_or(0),
        completed_count: row.try_get("completed_count").unwrap_or(0),
        cancelled_count: row.try_get("cancelled_count").unwrap_or(0),
        average_rating,
    })
}

pub async fn cycle_teacher_status(
    pool: &PgPool,
    access: SupervisionObservationListAccess,
    cycle_id: Uuid,
) -> Result<Vec<SupervisionTeacherStatusRow>, AppError> {
    if !access.school && access.organization_unit_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        SELECT u.id AS teacher_id,
               COALESCE(NULLIF(TRIM(CONCAT(COALESCE(u.title, ''), u.first_name, ' ', u.last_name)), ''), u.username)
                   AS teacher_display_name,
               COALESCE(units.organization_unit_names, ARRAY[]::text[]) AS organization_unit_names,
               latest.id AS observation_id,
               latest.status,
               latest.observed_at,
               NULLIF(
                   CONCAT_WS(
                       ' / ',
                       NULLIF(COALESCE(latest.manual_subject_name, latest.lesson_snapshot->>'subjectName'), ''),
                       NULLIF(COALESCE(latest.manual_period_label, latest.lesson_snapshot->>'periodLabel'), ''),
                       NULLIF(COALESCE(latest.manual_classroom_label, latest.lesson_snapshot->>'classroomLabel'), '')
                   ),
                   ''
               ) AS lesson_title,
               COALESCE(evaluators.evaluator_names, ARRAY[]::text[]) AS evaluator_names,
               rating.average_rating
        FROM users u
        LEFT JOIN LATERAL (
            SELECT ARRAY_AGG(DISTINCT sg.name_th ORDER BY sg.name_th) AS organization_unit_names
            FROM organization_members om
            JOIN organization_units ou ON ou.id = om.organization_unit_id
            JOIN subject_groups sg ON sg.id = ou.subject_group_id
            WHERE om.user_id = u.id
              AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
        ) units ON true
        LEFT JOIN LATERAL (
            SELECT o.*
            FROM supervision_observations o
            WHERE o.cycle_id =
        "#,
    );
    builder.push_bind(cycle_id);
    builder.push(
        r#"
              AND o.observed_user_id = u.id
              AND o.status <> 'cancelled'
            ORDER BY o.updated_at DESC, o.created_at DESC
            LIMIT 1
        ) latest ON true
        LEFT JOIN LATERAL (
            SELECT ARRAY_AGG(
                       COALESCE(
                           NULLIF(TRIM(CONCAT(COALESCE(eu.title, ''), eu.first_name, ' ', eu.last_name)), ''),
                           eu.username
                       )
                       ORDER BY e.is_required DESC, e.created_at
                   ) AS evaluator_names
            FROM supervision_evaluators e
            JOIN users eu ON eu.id = e.evaluator_user_id
            WHERE e.observation_id = latest.id
        ) evaluators ON latest.id IS NOT NULL
        LEFT JOIN LATERAL (
            SELECT AVG(evaluator_average)::double precision AS average_rating
            FROM (
                SELECT AVG(r.rating_score)::double precision AS evaluator_average
                FROM supervision_evaluators e
                JOIN supervision_evaluator_responses r ON r.evaluator_id = e.id
                JOIN supervision_template_items i ON i.id = r.template_item_id
                WHERE e.observation_id = latest.id
                  AND e.status = 'submitted'
                  AND i.item_type = 'rating'
                  AND r.rating_score IS NOT NULL
                GROUP BY e.id
            ) evaluator_averages
        ) rating ON latest.id IS NOT NULL
        WHERE u.user_type = 'staff'
          AND u.status = 'active'
        "#,
    );

    if !access.school {
        builder.push(
            r#"
          AND EXISTS (
              SELECT 1
              FROM organization_members om_scope
              WHERE om_scope.user_id = u.id
                AND om_scope.organization_unit_id = ANY(
            "#,
        );
        builder.push_bind(access.organization_unit_ids);
        builder.push(
            r#"
                )
                AND (om_scope.ended_at IS NULL OR om_scope.ended_at > CURRENT_DATE)
          )
            "#,
        );
    }

    builder.push(" ORDER BY u.first_name, u.last_name, u.username");

    let rows = builder
        .build_query_as::<TeacherStatusOverviewRow>()
        .fetch_all(pool)
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to load supervision teacher status overview: {}",
                error
            );
            AppError::InternalServerError("ไม่สามารถโหลดภาพรวมสถานะครูได้".to_string())
        })?;

    rows.into_iter()
        .map(teacher_status_from_row)
        .collect::<Result<Vec<_>, _>>()
}

async fn load_observation_review_responses(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Vec<SupervisionReviewResponseRow>, AppError> {
    sqlx::query_as::<_, SupervisionReviewResponseRow>(
        r#"
        SELECT evaluator_id,
               template_item_id,
               rating_score::double precision AS rating_score,
               text_response
        FROM supervision_evaluator_responses
        WHERE observation_id = $1
        ORDER BY updated_at
        "#,
    )
    .bind(observation_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision review responses: {}", error);
        AppError::InternalServerError("ไม่สามารถดึงคำตอบแบบประเมินนิเทศได้".to_string())
    })
}

fn build_review_evaluator_results(
    evaluators: &[SupervisionEvaluator],
    response_rows: Vec<SupervisionReviewResponseRow>,
) -> Vec<SupervisionReviewEvaluatorResult> {
    let mut responses_by_evaluator = HashMap::<Uuid, Vec<SupervisionReviewResponse>>::new();
    for row in response_rows {
        responses_by_evaluator
            .entry(row.evaluator_id)
            .or_default()
            .push(SupervisionReviewResponse {
                template_item_id: row.template_item_id,
                rating_score: row.rating_score,
                text_response: row.text_response,
            });
    }

    evaluators
        .iter()
        .map(|evaluator| {
            let responses = responses_by_evaluator
                .remove(&evaluator.id)
                .unwrap_or_default();
            let average_rating = average_rating_from_scores(
                responses
                    .iter()
                    .filter_map(|response| response.rating_score),
            );

            SupervisionReviewEvaluatorResult {
                evaluator_id: evaluator.id,
                evaluator_user_id: evaluator.evaluator_user_id,
                evaluator_display_name: evaluator.evaluator_display_name.clone(),
                role_label: evaluator.role_label.clone(),
                status: evaluator.status,
                submitted_at: evaluator.submitted_at,
                average_rating,
                responses,
            }
        })
        .collect()
}

fn build_review_item_summaries(
    evaluator_results: &[SupervisionReviewEvaluatorResult],
) -> Vec<SupervisionReviewItemSummary> {
    let mut scores_by_item = HashMap::<Uuid, Vec<f64>>::new();
    for evaluator in evaluator_results {
        for response in &evaluator.responses {
            if let Some(score) = response.rating_score {
                scores_by_item
                    .entry(response.template_item_id)
                    .or_default()
                    .push(score);
            }
        }
    }

    let mut summaries = scores_by_item
        .into_iter()
        .map(|(template_item_id, scores)| SupervisionReviewItemSummary {
            template_item_id,
            average_rating: average_rating_from_scores(scores.iter().copied()),
            response_count: scores.len() as i64,
        })
        .collect::<Vec<_>>();
    summaries.sort_by_key(|summary| summary.template_item_id);
    summaries
}

fn average_rating_from_scores(scores: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0.0;
    for score in scores {
        total += score;
        count += 1.0;
    }

    if count > 0.0 {
        Some(total / count)
    } else {
        None
    }
}

fn teacher_status_from_row(
    row: TeacherStatusOverviewRow,
) -> Result<SupervisionTeacherStatusRow, AppError> {
    let status = parse_optional_observation_status(row.status)?;
    Ok(SupervisionTeacherStatusRow {
        teacher_id: row.teacher_id,
        teacher_display_name: row.teacher_display_name,
        organization_unit_names: row.organization_unit_names,
        observation_id: row.observation_id,
        status,
        observed_at: row.observed_at,
        lesson_title: row.lesson_title,
        evaluator_names: row.evaluator_names,
        average_rating: row.average_rating,
        next_step_label: teacher_status_next_step_label(status),
    })
}

fn teacher_status_next_step_label(status: Option<SupervisionObservationStatus>) -> String {
    match status {
        None => "ยังไม่จองคาบนิเทศ",
        Some(SupervisionObservationStatus::Requested) => "รอหัวหน้าหน่วยงานอนุมัติคำขอ",
        Some(SupervisionObservationStatus::Planned | SupervisionObservationStatus::InProgress) => {
            "รอผู้ประเมินส่งผล"
        }
        Some(
            SupervisionObservationStatus::EvaluatorsSubmitted
            | SupervisionObservationStatus::UnderReview,
        ) => "รอหัวหน้ากลุ่มสาระรับรองผล",
        Some(SupervisionObservationStatus::Returned) => "รอครูแก้ไขคำขอ",
        Some(SupervisionObservationStatus::Approved) => "รอฝ่ายวิชาการอนุมัติผล",
        Some(SupervisionObservationStatus::Published) => "รอครูรับทราบผล",
        Some(
            SupervisionObservationStatus::Acknowledged | SupervisionObservationStatus::Completed,
        ) => "เสร็จสิ้น",
        Some(SupervisionObservationStatus::Cancelled) => "ยกเลิก",
    }
    .to_string()
}

pub(super) async fn fetch_observation_average_rating(
    pool: &PgPool,
    observation_id: Uuid,
) -> Result<Option<f64>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT e.id AS evaluator_id,
               e.status,
               CASE WHEN i.item_type = 'rating'
                    THEN r.rating_score::double precision
                    ELSE NULL
               END AS rating_score
        FROM supervision_evaluators e
        LEFT JOIN supervision_evaluator_responses r ON r.evaluator_id = e.id
        LEFT JOIN supervision_template_items i ON i.id = r.template_item_id
        WHERE e.observation_id = $1
        "#,
    )
    .bind(observation_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        tracing::error!("Failed to load supervision observation average: {}", error);
        AppError::InternalServerError("ไม่สามารถคำนวณคะแนนเฉลี่ยนิเทศได้".to_string())
    })?;

    let mut inputs: HashMap<Uuid, EvaluatorRatingInput> = HashMap::new();
    for row in rows {
        let evaluator_id: Uuid = row.try_get("evaluator_id").map_err(|error| {
            tracing::error!("Failed to read supervision evaluator id: {}", error);
            AppError::InternalServerError("ไม่สามารถคำนวณคะแนนเฉลี่ยนิเทศได้".to_string())
        })?;
        let status: String = row.try_get("status").map_err(|error| {
            tracing::error!("Failed to read supervision evaluator status: {}", error);
            AppError::InternalServerError("ไม่สามารถคำนวณคะแนนเฉลี่ยนิเทศได้".to_string())
        })?;
        let rating_score: Option<f64> = row.try_get("rating_score").ok();

        let input = inputs.entry(evaluator_id).or_insert(EvaluatorRatingInput {
            submitted: status == "submitted",
            rating_scores: Vec::new(),
        });
        input.rating_scores.push(rating_score);
    }

    let ratings = inputs.into_values().collect::<Vec<_>>();
    Ok(average_submitted_evaluator_rating(&ratings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::supervision::models::SupervisionEvaluatorStatus;

    #[test]
    fn review_results_average_scores_by_evaluator_and_item() {
        let first_evaluator_id = Uuid::new_v4();
        let second_evaluator_id = Uuid::new_v4();
        let first_item_id = Uuid::new_v4();
        let second_item_id = Uuid::new_v4();
        let evaluators = vec![
            SupervisionEvaluator {
                id: first_evaluator_id,
                observation_id: Uuid::new_v4(),
                evaluator_user_id: Uuid::new_v4(),
                evaluator_display_name: Some("ผู้ประเมิน 1".to_string()),
                role_label: None,
                is_required: true,
                status: SupervisionEvaluatorStatus::Submitted,
                submitted_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            SupervisionEvaluator {
                id: second_evaluator_id,
                observation_id: Uuid::new_v4(),
                evaluator_user_id: Uuid::new_v4(),
                evaluator_display_name: Some("ผู้ประเมิน 2".to_string()),
                role_label: None,
                is_required: true,
                status: SupervisionEvaluatorStatus::Submitted,
                submitted_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ];

        let evaluator_results = build_review_evaluator_results(
            &evaluators,
            vec![
                SupervisionReviewResponseRow {
                    evaluator_id: first_evaluator_id,
                    template_item_id: first_item_id,
                    rating_score: Some(5.0),
                    text_response: None,
                },
                SupervisionReviewResponseRow {
                    evaluator_id: first_evaluator_id,
                    template_item_id: second_item_id,
                    rating_score: Some(3.0),
                    text_response: None,
                },
                SupervisionReviewResponseRow {
                    evaluator_id: second_evaluator_id,
                    template_item_id: first_item_id,
                    rating_score: Some(4.0),
                    text_response: None,
                },
            ],
        );
        let item_summaries = build_review_item_summaries(&evaluator_results);

        assert_eq!(evaluator_results.len(), 2);
        assert_eq!(evaluator_results[0].average_rating, Some(4.0));
        assert_eq!(evaluator_results[1].average_rating, Some(4.0));
        assert_eq!(
            item_summaries
                .iter()
                .find(|summary| summary.template_item_id == first_item_id)
                .map(|summary| (summary.average_rating, summary.response_count)),
            Some((Some(4.5), 2))
        );
        assert_eq!(
            item_summaries
                .iter()
                .find(|summary| summary.template_item_id == second_item_id)
                .map(|summary| (summary.average_rating, summary.response_count)),
            Some((Some(3.0), 1))
        );
    }
}
