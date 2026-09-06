use super::*;
use bigdecimal::{num_bigint::BigInt, BigDecimal};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

#[derive(Clone)]
struct Fraction {
    numerator: BigInt,
    denominator: BigInt,
}
impl Fraction {
    fn new(numerator: BigInt, denominator: BigInt) -> Self {
        let (mut a, mut b) = (numerator.clone(), denominator.clone());
        while b != BigInt::from(0) {
            let next = &a % &b;
            a = b;
            b = next;
        }
        Self {
            numerator: numerator / &a,
            denominator: denominator / &a,
        }
    }
    fn mean(values: &[Self]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let sum = values
            .iter()
            .fold(Self::new(0.into(), 1.into()), |sum, value| {
                Self::new(
                    &sum.numerator * &value.denominator + &value.numerator * &sum.denominator,
                    &sum.denominator * &value.denominator,
                )
            });
        Some(Self::new(
            sum.numerator,
            sum.denominator * BigInt::from(values.len()),
        ))
    }
    fn wire(&self) -> ExactAverage {
        let decimal = (BigDecimal::from(self.numerator.clone())
            / BigDecimal::from(self.denominator.clone()))
        .with_scale(12)
        .normalized()
        .to_string();
        ExactAverage {
            numerator: self.numerator.to_string(),
            denominator: self.denominator.to_string(),
            decimal,
        }
    }
}

fn validate_bands(bands: &[(i16, String)]) -> Result<Vec<(i16, BigDecimal)>, AppError> {
    let mut bands = bands
        .iter()
        .map(|(level, bound)| {
            BigDecimal::from_str(bound)
                .map(|bound| (*level, bound))
                .map_err(|_| {
                    AppError::ValidationError("Invalid aggregation policy boundary".into())
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    bands.sort_by_key(|b| b.0);
    if bands.len() != 4
        || bands[0] != (0, BigDecimal::from(0))
        || bands
            .iter()
            .enumerate()
            .any(|(i, b)| b.0 != i as i16 || b.1 > BigDecimal::from(3))
        || bands.windows(2).any(|w| w[0].1 >= w[1].1)
    {
        return Err(AppError::ValidationError(
            "Aggregation policy must have four increasing bands starting at zero".into(),
        ));
    }
    Ok(bands)
}

/// This pure calculation consumes effective criterion values. Task 7 can replace the
/// loader with correction-aware values without changing weighting or rounding semantics.
pub fn summarize_domains(
    rows: &[LockedCriterionValue],
    expected: &[Uuid],
    locked: &[(Uuid, LearnerEvaluationDomain)],
    bands: &[(i16, String)],
) -> Result<Vec<DomainEvaluationSummary>, AppError> {
    let bands = validate_bands(bands)?;
    let mut domains = vec![];
    for domain in [
        LearnerEvaluationDomain::DesirableCharacteristic,
        LearnerEvaluationDomain::ReadingThinkingWriting,
    ] {
        let mut subjects: BTreeMap<Uuid, Vec<LockedCriterionValue>> = BTreeMap::new();
        let mut catalog: BTreeMap<Uuid, Vec<Fraction>> = BTreeMap::new();
        for row in rows
            .iter()
            .filter(|r| r.domain == domain && locked.contains(&(r.subject_id, domain)))
        {
            LearnerEvaluationLevel::try_from(row.quality_level)
                .map_err(|e| AppError::ValidationError(e.into()))?;
            subjects
                .entry(row.subject_id)
                .or_default()
                .push(row.clone());
            if let Some(id) = row.school_criterion_id {
                catalog
                    .entry(id)
                    .or_default()
                    .push(Fraction::new(row.quality_level.into(), 1.into()));
            }
        }
        let mut subject_summaries = vec![];
        let mut averages = vec![];
        for (subject_id, mut criteria) in subjects {
            criteria.sort_by_key(|c| c.subject_term_criterion_id);
            let average = Fraction::mean(
                &criteria
                    .iter()
                    .map(|c| Fraction::new(c.quality_level.into(), 1.into()))
                    .collect::<Vec<_>>(),
            )
            .ok_or_else(|| {
                AppError::ValidationError("Locked subject has no criterion values".into())
            })?;
            subject_summaries.push(SubjectEvaluationSummary {
                subject_id,
                average: average.wire(),
                criteria,
            });
            averages.push(average);
        }
        let average = Fraction::mean(&averages);
        let quality_level = average.as_ref().and_then(|avg| {
            bands
                .iter()
                .rev()
                .find(|(_, bound)| {
                    BigDecimal::from(avg.numerator.clone())
                        >= bound * BigDecimal::from(avg.denominator.clone())
                })
                .map(|b| b.0)
        });
        let missing_subjects = expected
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|subject_id| {
                let reason = if !locked.contains(&(subject_id, domain)) {
                    Some("subject_domain_not_locked")
                } else if !subject_summaries.iter().any(|s| s.subject_id == subject_id) {
                    Some("student_not_in_locked_snapshot")
                } else {
                    None
                };
                reason.map(|reason| MissingSubject {
                    subject_id,
                    reason: reason.into(),
                })
            })
            .collect::<Vec<_>>();
        let catalog_criteria = catalog
            .into_iter()
            .filter_map(|(school_criterion_id, values)| {
                Fraction::mean(&values).map(|average| CatalogEvaluationSummary {
                    school_criterion_id,
                    average: average.wire(),
                })
            })
            .collect();
        domains.push(DomainEvaluationSummary {
            domain,
            average: average.map(|a| a.wire()),
            quality_level,
            complete: missing_subjects.is_empty(),
            missing_subjects,
            catalog_criteria,
            subjects: subject_summaries,
        });
    }
    Ok(domains)
}

pub async fn summary_for_actor(
    pool: &PgPool,
    actor: &ActorContext,
    ctx: &EvaluationContext,
    student: Uuid,
) -> Result<StudentEvaluationSummary, AppError> {
    crate::policies::learner_evaluation_access_policy::require_student_summary_access(
        pool,
        actor,
        ctx.academic_year_id,
        ctx.academic_term_id,
        student,
    )
    .await?;
    super::summarize_student_term(pool, ctx, student).await
}
pub async fn summarize_student_term(
    pool: &PgPool,
    ctx: &EvaluationContext,
    student: Uuid,
) -> Result<StudentEvaluationSummary, AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    validate_context(&mut tx, ctx).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM student_academic_years WHERE id=$1 AND academic_year_id=$2)",
    )
    .bind(student)
    .bind(ctx.academic_year_id)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        return Err(AppError::NotFound(
            "Student enrollment not found in this academic year".into(),
        ));
    }
    let expected:Vec<Uuid>=sqlx::query_scalar("SELECT DISTINCT d.subject_id FROM learning_group_students m JOIN learning_groups g ON g.id=m.learning_group_id JOIN course_offering_details d ON d.learning_offering_id=g.learning_offering_id WHERE m.student_academic_year_id=$1 AND m.academic_term_id=$2 AND m.academic_year_id=$3 AND m.membership_status='active' AND g.status<>'closed' UNION SELECT subject_id FROM subject_term_student_evaluations WHERE student_academic_year_id=$1 AND academic_term_id=$2 AND academic_year_id=$3 ORDER BY subject_id").bind(student).bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_all(&mut *tx).await?;
    let locked:Vec<(Uuid,LearnerEvaluationDomain)>=sqlx::query_as("SELECT subject_id,domain FROM subject_term_evaluation_locks WHERE academic_term_id=$1 AND academic_year_id=$2 AND subject_id=ANY($3)").bind(ctx.academic_term_id).bind(ctx.academic_year_id).bind(&expected).fetch_all(&mut *tx).await?;
    let rows = load_effective_values(&mut tx, ctx, student).await?;
    let policy_version_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_learner_evaluation_policy_versions WHERE lifecycle='active'",
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::ValidationError("No active learner evaluation aggregation policy".into())
    })?;
    let bands:Vec<(i16,String)>=sqlx::query_as("SELECT quality_level,lower_bound::text FROM academic_learner_evaluation_policy_bands WHERE policy_version_id=$1 ORDER BY quality_level").bind(policy_version_id).fetch_all(&mut *tx).await?;
    let domains = summarize_domains(&rows, &expected, &locked, &bands)?;
    tx.commit().await?;
    Ok(StudentEvaluationSummary {
        student_academic_year_id: student,
        policy_version_id,
        domains,
    })
}
async fn load_effective_values(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &EvaluationContext,
    student: Uuid,
) -> Result<Vec<LockedCriterionValue>, AppError> {
    // Immutable initial responses are the effective source until Task 7 adds corrections.
    // Criterion labels/catalog identities come from the lock snapshot, not today's catalog.
    Ok(sqlx::query_as(r#"SELECT r.id,r.subject_id,r.domain,r.subject_term_criterion_id,c."schoolCriterionId" AS school_criterion_id,c.name,r.quality_level,r.row_version
        FROM subject_term_student_evaluations r JOIN subject_term_evaluation_locks l ON l.id=r.evaluation_lock_id
        CROSS JOIN LATERAL jsonb_to_recordset(l.source_snapshot->'criteria') AS c(id uuid,"schoolCriterionId" uuid,name text)
        WHERE c.id=r.subject_term_criterion_id AND r.student_academic_year_id=$1 AND r.academic_term_id=$2 AND r.academic_year_id=$3
        ORDER BY r.domain,r.subject_id,r.subject_term_criterion_id"#).bind(student).bind(ctx.academic_term_id).bind(ctx.academic_year_id).fetch_all(&mut **tx).await?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_active_policy_does_not_silently_grade() {
        assert!(validate_bands(&[
            (0, "0".into()),
            (1, "1.5".into()),
            (2, "1".into()),
            (3, "2.5".into())
        ])
        .is_err());
        assert!(validate_bands(&[]).is_err());
    }
}
