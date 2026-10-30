use std::collections::{BTreeMap, BTreeSet};

use school_authorization::ActorContext;
use school_database::DbSession;
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::{PgConnection, PgPool, types::Json};
use uuid::Uuid;

use super::{context, lifecycle_guard, parse_row_version, years_terms};
use crate::models::{
    AcademicContextSettings, AcademicContextSettingsMutationResponse,
    AcademicContextVisibilityInput, UpdateAcademicContextSettingsRequest,
};

pub async fn get_on_connection(
    connection: &mut PgConnection,
) -> Result<AcademicContextSettings, AppError> {
    DbSession::new(connection).query_as("academic_core.services.context_settings.get_on_connection",
        "SELECT default_academic_year_id,default_academic_term_id,row_version FROM academic_context_settings WHERE singleton")
        .fetch_one().await.map_err(Into::into)
}

fn validate_changes(
    changes: &[AcademicContextVisibilityInput],
    maximum: usize,
) -> Result<(), AppError> {
    if changes.len() > maximum
        || changes.iter().any(|change| change.id.is_nil())
        || changes
            .iter()
            .map(|change| change.id)
            .collect::<BTreeSet<_>>()
            .len()
            != changes.len()
    {
        return Err(AppError::ValidationError(
            "รายการแสดงบริบทไม่ถูกต้องหรือซ้ำกัน".into(),
        ));
    }
    Ok(())
}

fn validate_defaults(
    year: Option<Uuid>,
    term: Option<Uuid>,
    years: &BTreeMap<Uuid, bool>,
    terms: &BTreeMap<Uuid, (Uuid, bool)>,
) -> Result<(), AppError> {
    if let Some(year) = year {
        if years.get(&year) != Some(&true) {
            return Err(AppError::ValidationError(
                "ปีการศึกษาเริ่มต้นต้องเปิดให้แสดง".into(),
            ));
        }
    }
    if let Some(term) = term {
        if !matches!(terms.get(&term),Some((parent,true)) if Some(*parent)==year) {
            return Err(AppError::ValidationError(
                "ภาคเรียนเริ่มต้นต้องเปิดให้แสดงและอยู่ในปีเริ่มต้น".into(),
            ));
        }
    }
    Ok(())
}

pub async fn update(
    pool: &PgPool,
    actor: &ActorContext,
    request: UpdateAcademicContextSettingsRequest,
) -> Result<AcademicContextSettingsMutationResponse, AppError> {
    actor.require_permission(codes::ACADEMIC_YEAR_MANAGE_SCHOOL)?;
    actor.require_permission(codes::ACADEMIC_TERM_MANAGE_SCHOOL)?;
    parse_row_version(request.row_version)?;
    validate_changes(&request.years, 100)?;
    validate_changes(&request.terms, 2000)?;
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "academic_core.services.context_settings.update",
    )
    .await?;
    lifecycle_guard::lock_transition_shared(&mut tx).await?;
    let before: AcademicContextSettings=DbSession::new(&mut *tx).query_as("academic_core.services.context_settings.lock",
        "SELECT default_academic_year_id,default_academic_term_id,row_version FROM academic_context_settings WHERE singleton FOR UPDATE")
        .fetch_one().await?;
    if before.row_version != request.row_version {
        return Err(AppError::Conflict("การแสดงบริบทเปลี่ยนแล้ว กรุณาโหลดใหม่".into()));
    }
    // Resolve parent identities before locks; parents precede children in every Core writer.
    let term_ids = request
        .terms
        .iter()
        .map(|change| change.id)
        .chain(request.default_academic_term_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let parent_ids: Vec<Uuid> = DbSession::new(&mut *tx)
        .scalar(
            "academic_core.services.context_settings.resolve_parents",
            "SELECT DISTINCT academic_year_id FROM academic_terms WHERE id=ANY($1)",
        )
        .bind(&term_ids)
        .fetch_all()
        .await?;
    let year_ids = request
        .years
        .iter()
        .map(|change| change.id)
        .chain(request.default_academic_year_id)
        .chain(parent_ids)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let year_rows: Vec<(Uuid, bool)> = DbSession::new(&mut *tx)
        .query_as(
            "academic_core.services.context_settings.lock_years",
            "SELECT id,is_visible FROM academic_years WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&year_ids)
        .fetch_all()
        .await?;
    let term_rows: Vec<(Uuid,Uuid,bool)>=DbSession::new(&mut *tx).query_as("academic_core.services.context_settings.lock_terms",
        "SELECT id,academic_year_id,is_visible FROM academic_terms WHERE id=ANY($1) ORDER BY id FOR UPDATE").bind(&term_ids).fetch_all().await?;
    let mut years = year_rows.into_iter().collect::<BTreeMap<_, _>>();
    let mut terms = term_rows
        .into_iter()
        .map(|(id, parent, visible)| (id, (parent, visible)))
        .collect::<BTreeMap<_, _>>();
    for change in &request.years {
        *years
            .get_mut(&change.id)
            .ok_or_else(|| AppError::NotFound("ไม่พบปีการศึกษาที่เปลี่ยนการแสดง".into()))? =
            change.is_visible;
    }
    for change in &request.terms {
        terms
            .get_mut(&change.id)
            .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียนที่เปลี่ยนการแสดง".into()))?
            .1 = change.is_visible;
    }
    validate_defaults(
        request.default_academic_year_id,
        request.default_academic_term_id,
        &years,
        &terms,
    )?;
    DbSession::new(&mut *tx).query("academic_core.services.context_settings.update_years",
        "UPDATE academic_years year SET is_visible=change.\"isVisible\",row_version=year.row_version+1,updated_at=now() FROM jsonb_to_recordset($1) change(id uuid,\"isVisible\" boolean) WHERE year.id=change.id AND year.is_visible IS DISTINCT FROM change.\"isVisible\"")
        .bind(Json(&request.years)).execute().await?;
    DbSession::new(&mut *tx).query("academic_core.services.context_settings.update_terms",
        "UPDATE academic_terms term SET is_visible=change.\"isVisible\",row_version=term.row_version+1,updated_at=now() FROM jsonb_to_recordset($1) change(id uuid,\"isVisible\" boolean) WHERE term.id=change.id AND term.is_visible IS DISTINCT FROM change.\"isVisible\"")
        .bind(Json(&request.terms)).execute().await?;
    let settings:AcademicContextSettings=DbSession::new(&mut *tx).query_as("academic_core.services.context_settings.write",
        "UPDATE academic_context_settings SET default_academic_year_id=$1,default_academic_term_id=$2,row_version=row_version+1,updated_by=$3,updated_at=now() WHERE singleton RETURNING default_academic_year_id,default_academic_term_id,row_version")
        .bind(request.default_academic_year_id).bind(request.default_academic_term_id).bind(actor.user_id).fetch_one().await?;
    DbSession::new(&mut *tx).query("academic_core.services.context_settings.audit",
        "INSERT INTO audit_logs(user_id,action,entity_type,entity_id,old_values,new_values,description) VALUES($1,'update','academic_context_settings',NULL,$2,$3,'academic context visibility and defaults')")
        .bind(actor.user_id).bind(Json(&before)).bind(Json(&request)).execute().await?;
    let changed_year_ids = request
        .years
        .iter()
        .map(|change| change.id)
        .collect::<Vec<_>>();
    let changed_term_ids = request
        .terms
        .iter()
        .map(|change| change.id)
        .collect::<Vec<_>>();
    let changed_years = years_terms::get_years_by_ids(&mut tx, &changed_year_ids).await?;
    let changed_terms = years_terms::get_terms_by_ids(&mut tx, &changed_term_ids).await?;
    let context_options = context::list_options_on_connection(&mut tx).await?;
    tx.commit().await?;
    Ok(AcademicContextSettingsMutationResponse {
        settings,
        years: changed_years,
        terms: changed_terms,
        context_options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_settings_put_requires_explicit_nullable_defaults() {
        let input = serde_json::json!({"rowVersion":1,"defaultAcademicYearId":null,"defaultAcademicTermId":null,"years":[],"terms":[]});
        assert!(
            serde_json::from_value::<UpdateAcademicContextSettingsRequest>(input.clone()).is_ok()
        );
        for field in ["defaultAcademicYearId", "defaultAcademicTermId"] {
            let mut missing = input.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<UpdateAcademicContextSettingsRequest>(missing).is_err()
            );
        }
    }
    #[test]
    fn defaults_require_visible_matching_context() {
        let year = Uuid::new_v4();
        let other = Uuid::new_v4();
        let term = Uuid::new_v4();
        let mut years = BTreeMap::from([(year, true), (other, true)]);
        let mut terms = BTreeMap::from([(term, (year, true))]);
        assert!(validate_defaults(None, None, &years, &terms).is_ok());
        assert!(validate_defaults(Some(year), None, &years, &terms).is_ok());
        assert!(validate_defaults(Some(year), Some(term), &years, &terms).is_ok());
        assert!(validate_defaults(None, Some(term), &years, &terms).is_err());
        assert!(validate_defaults(Some(other), Some(term), &years, &terms).is_err());
        years.insert(year, false);
        assert!(validate_defaults(Some(year), Some(term), &years, &terms).is_err());
        years.insert(year, true);
        terms.insert(term, (year, false));
        assert!(validate_defaults(Some(year), Some(term), &years, &terms).is_err());
    }
    #[test]
    fn visibility_commands_reject_duplicates_and_unbounded_inputs() {
        let change = AcademicContextVisibilityInput {
            id: Uuid::new_v4(),
            is_visible: true,
        };
        assert!(validate_changes(&[], 100).is_ok());
        assert!(validate_changes(std::slice::from_ref(&change), 100).is_ok());
        assert!(validate_changes(&[change.clone(), change], 100).is_err());
        assert!(
            validate_changes(
                &[AcademicContextVisibilityInput {
                    id: Uuid::nil(),
                    is_visible: false
                }],
                100
            )
            .is_err()
        );
    }
}
