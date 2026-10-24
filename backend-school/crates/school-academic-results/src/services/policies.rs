use super::*;
use crate::policy::results as access_policy;
use school_authorization::ActorContext;
use school_database::DbSession;
use sqlx::PgPool;
use uuid::Uuid;

const GRADES: [&str; 8] = ["0", "1", "1.5", "2", "2.5", "3", "3.5", "4"];

pub fn validate_policy(bands: &[GradingPolicyBand], total: &str) -> Result<(), AppError> {
    let total = decimal(total)?;
    if total < BigDecimal::from(0) || bands.len() != GRADES.len() {
        return Err(AppError::ValidationError(
            "A grading policy requires exactly eight ordered bands".into(),
        ));
    }
    let mut previous: Option<BigDecimal> = None;
    for (index, band) in bands.iter().enumerate() {
        if decimal(&band.grade)? != decimal(GRADES[index])? {
            return Err(AppError::ValidationError(
                "Grading policy outcomes must be exactly 0, 1, 1.5, 2, 2.5, 3, 3.5, and 4".into(),
            ));
        }
        let lower = decimal(&band.lower_bound)?;
        if (index == 0 && lower != BigDecimal::from(0))
            || lower > total
            || previous.as_ref().is_some_and(|value| value >= &lower)
        {
            return Err(AppError::ValidationError(
                "Grading policy lower bounds must start at zero and increase within the total"
                    .into(),
            ));
        }
        previous = Some(lower);
    }
    Ok(())
}

pub fn derive_grade(
    bands: &[GradingPolicyBand],
    score: &str,
    total: &str,
) -> Result<String, AppError> {
    validate_policy(bands, total)?;
    let score = decimal(score)?;
    let total = decimal(total)?;
    if score < BigDecimal::from(0) || score > total {
        return Err(AppError::ValidationError(
            "Calculated score must be within the assessment total".into(),
        ));
    }
    let band = bands
        .iter()
        .rev()
        .find(|band| decimal(&band.lower_bound).is_ok_and(|bound| score >= bound))
        .ok_or_else(|| AppError::ValidationError("Grading policy has no matching band".into()))?;
    Ok(decimal_wire(&decimal(&band.grade)?))
}

pub(super) async fn load_policy(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<GradingPolicyVersion, AppError> {
    let header: Option<(Uuid, i32, String, String, i64)> = DbSession::new(&mut **tx).query_as("academic_results.services.policies.load_policy",
        "SELECT id,version_no,name,lifecycle,row_version FROM academic_grading_policy_versions WHERE id=$1",
    )
    .bind(id)
    .fetch_optional()
    .await?;
    let (id, version_no, name, lifecycle, row_version) =
        header.ok_or_else(|| AppError::NotFound("Grading policy version not found".into()))?;
    let bands: Vec<GradingPolicyBand> = DbSession::new(&mut **tx).query_as::<(String, String)>("academic_results.services.policies.load_policy",
        "SELECT grade::text,lower_bound::text FROM academic_grading_policy_bands WHERE policy_version_id=$1 ORDER BY grade",
    )
    .bind(id)
    .fetch_all()
    .await?
    .into_iter()
    .map(|(grade, lower_bound)| GradingPolicyBand { grade, lower_bound })
    .collect();
    Ok(GradingPolicyVersion {
        id,
        version_no,
        name,
        lifecycle,
        row_version,
        bands,
    })
}

pub(super) async fn active_policy(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<GradingPolicyVersion, AppError> {
    let id: Uuid = DbSession::new(&mut **tx)
        .scalar(
            "academic_results.services.policies.active_policy",
            "SELECT id FROM academic_grading_policy_versions WHERE lifecycle='active'",
        )
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::ValidationError("No active grading policy".into()))?;
    load_policy(tx, id).await
}

pub async fn list_policies(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
) -> Result<Vec<GradingPolicyVersion>, AppError> {
    access_policy::list_access(pool, actor).await?;
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    let ids: Vec<Uuid> = DbSession::new(&mut *tx)
        .scalar(
            "academic_results.services.policies.list_policies",
            "SELECT id FROM academic_grading_policy_versions ORDER BY version_no DESC LIMIT 101",
        )
        .fetch_all()
        .await?;
    if ids.len() > 100 {
        return Err(AppError::ValidationError(
            "Grading policy history exceeds 100 versions".into(),
        ));
    }
    let mut policies = Vec::with_capacity(ids.len());
    for id in ids {
        policies.push(load_policy(&mut tx, id).await?);
    }
    tx.commit().await?;
    Ok(policies)
}

pub async fn create_policy(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    input: GradingPolicyInput,
) -> Result<GradingPolicyVersion, AppError> {
    if !access_policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "School result management permission is required".into(),
        ));
    }
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::ValidationError(
            "Grading policy name is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    let minimum_total: Option<String> = DbSession::new(&mut *tx).scalar("academic_results.services.policies.create_policy",
        "SELECT min(assessment_total_score)::text FROM course_offering_details WHERE academic_term_id=$1 AND academic_year_id=$2",
    )
    .bind(context.academic_term_id)
    .bind(context.academic_year_id)
    .fetch_one()
    .await?;
    validate_policy(&input.bands, minimum_total.as_deref().unwrap_or("100"))?;
    DbSession::new(&mut *tx)
        .query(
            "academic_results.services.policies.create_policy",
            "SELECT id FROM academic_grading_policy_versions ORDER BY version_no FOR UPDATE",
        )
        .execute()
        .await?;
    let version_no: i32 = DbSession::new(&mut *tx)
        .scalar(
            "academic_results.services.policies.create_policy",
            "SELECT COALESCE(max(version_no),0)+1 FROM academic_grading_policy_versions",
        )
        .fetch_one()
        .await?;
    let id: Uuid = DbSession::new(&mut *tx).scalar("academic_results.services.policies.create_policy",
        "INSERT INTO academic_grading_policy_versions (version_no,name,created_by) VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(version_no)
    .bind(name)
    .bind(actor.user_id)
    .fetch_one()
    .await?;
    let grades = input
        .bands
        .iter()
        .map(|band| decimal(&band.grade))
        .collect::<Result<Vec<_>, _>>()?;
    let bounds = input
        .bands
        .iter()
        .map(|band| decimal(&band.lower_bound))
        .collect::<Result<Vec<_>, _>>()?;
    DbSession::new(&mut *tx).query("academic_results.services.policies.create_policy",
        "INSERT INTO academic_grading_policy_bands (policy_version_id,grade,lower_bound) SELECT $1,b.grade,b.lower_bound FROM unnest($2::numeric[],$3::numeric[]) b(grade,lower_bound)",
    )
    .bind(id)
    .bind(grades)
    .bind(bounds)
    .execute()
    .await?;
    let policy = load_policy(&mut tx, id).await?;
    tx.commit().await?;
    Ok(policy)
}

pub async fn activate_policy(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    id: Uuid,
    row_version: i64,
) -> Result<GradingPolicyVersion, AppError> {
    if !access_policy::can_manage_school(actor) {
        return Err(AppError::Forbidden(
            "School result management permission is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    validate_context(&mut tx, context).await?;
    lifecycle_guard::lock_transition_shared(&mut tx).await?;
    // The grading policy is global, so activation must serialize with every writer that can
    // change a course result source. Those writers all lock their offering before group/source
    // rows; taking every course offering in the same order prevents a stale confirmation from
    // being locked while the policy changes concurrently.
    let course_offerings: Vec<Uuid> = DbSession::new(&mut *tx)
        .scalar(
            "academic_results.services.policies.activate_policy",
            r#"SELECT offering.id
           FROM learning_offerings offering
           JOIN course_offering_details detail ON detail.learning_offering_id=offering.id
           ORDER BY offering.id
           LIMIT 10001
           FOR UPDATE OF offering"#,
        )
        .fetch_all()
        .await?;
    if course_offerings.len() > 10000 {
        return Err(AppError::ValidationError(
            "Grading policy activation exceeds 10000 course offerings".into(),
        ));
    }
    DbSession::new(&mut *tx)
        .query(
            "academic_results.services.policies.activate_policy",
            "SELECT id FROM academic_grading_policy_versions ORDER BY version_no FOR UPDATE",
        )
        .execute()
        .await?;
    let target: Option<(String, i64)> = DbSession::new(&mut *tx)
        .query_as(
            "academic_results.services.policies.activate_policy",
            "SELECT lifecycle,row_version FROM academic_grading_policy_versions WHERE id=$1",
        )
        .bind(id)
        .fetch_optional()
        .await?;
    let (lifecycle, actual_version) =
        target.ok_or_else(|| AppError::NotFound("Grading policy version not found".into()))?;
    if actual_version != row_version {
        return Err(conflict());
    }
    if lifecycle != "draft" {
        return Err(AppError::Conflict(
            "An activated grading policy version is immutable".into(),
        ));
    }
    DbSession::new(&mut *tx).query("academic_results.services.policies.activate_policy",
        "UPDATE academic_grading_policy_versions SET lifecycle='retired',row_version=row_version+1,updated_at=now() WHERE lifecycle='active'",
    )
    .execute()
    .await?;
    let changed = DbSession::new(&mut *tx).query("academic_results.services.policies.activate_policy",
        "UPDATE academic_grading_policy_versions SET lifecycle='active',activated_at=now(),row_version=row_version+1,updated_at=now() WHERE id=$1 AND lifecycle='draft' AND row_version=$2",
    )
    .bind(id)
    .bind(row_version)
    .execute()
    .await?;
    if changed.rows_affected() != 1 {
        return Err(conflict());
    }
    // Keep the confirmation identity and its monotonic revision. Locked subjects retain
    // their policy snapshot and their teacher confirmation unchanged.
    DbSession::new(&mut *tx)
        .query(
            "academic_results.services.policies.activate_policy",
            r#"UPDATE learning_group_result_confirmations confirmation
           SET row_version=row_version+1,
               source_snapshot=jsonb_set(source_snapshot,'{invalidated}','true'::jsonb)
           WHERE NOT EXISTS (
               SELECT 1 FROM academic_course_result_locks locked
               WHERE locked.subject_id=confirmation.subject_id
                 AND locked.academic_term_id=confirmation.academic_term_id
                 AND locked.academic_year_id=confirmation.academic_year_id
           )"#,
        )
        .execute()
        .await?;
    let policy = load_policy(&mut tx, id).await?;
    tx.commit().await?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_policy_requires_the_standard_order() {
        let bands = GRADES
            .iter()
            .zip(["0", "50", "55", "60", "65", "70", "75", "80"])
            .map(|(grade, lower_bound)| GradingPolicyBand {
                grade: (*grade).into(),
                lower_bound: lower_bound.into(),
            })
            .collect::<Vec<_>>();
        assert!(validate_policy(&bands, "100").is_ok());
        assert_eq!(derive_grade(&bands, "55", "100").unwrap(), "1.50");
    }
}
