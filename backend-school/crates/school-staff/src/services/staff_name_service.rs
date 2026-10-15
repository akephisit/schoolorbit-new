use crate::models::{CreateStaffRequest, UpdateStaffRequest};
use school_auth::person_names::{normalize_name_part, require_complete_name};
use school_errors::AppError;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(super) fn normalize_create_names(input: &mut CreateStaffRequest) -> Result<(), AppError> {
    for (part, limit) in [
        (&mut input.title, 50),
        (&mut input.title_en, 50),
        (&mut input.first_name_en, 100),
        (&mut input.last_name_en, 100),
    ] {
        if let Some(value) = part {
            *value = normalize_name_part(value, limit)?;
        }
    }
    input.first_name = normalize_name_part(&input.first_name, 100)?;
    input.last_name = normalize_name_part(&input.last_name, 100)?;
    require_complete_name([
        input.title.as_deref().unwrap_or(""),
        &input.first_name,
        &input.last_name,
        input.title_en.as_deref().unwrap_or(""),
        input.first_name_en.as_deref().unwrap_or(""),
        input.last_name_en.as_deref().unwrap_or(""),
    ])
}

/// Called only after locking the parent staff-user row. Unchanged historical partial names survive unrelated edits.
pub(super) async fn normalize_updated_names(
    tx: &mut Transaction<'_, Postgres>,
    staff_id: Uuid,
    input: &mut UpdateStaffRequest,
) -> Result<(), AppError> {
    let row=sqlx::query("SELECT title,first_name,last_name,title_en,first_name_en,last_name_en FROM users WHERE id=$1 AND user_type='staff'")
        .bind(staff_id).fetch_one(&mut **tx).await?;
    let before: Vec<Option<String>> = (0..6)
        .map(|index| row.try_get(index))
        .collect::<Result<_, _>>()?;
    let updates = [
        &mut input.title,
        &mut input.first_name,
        &mut input.last_name,
        &mut input.title_en,
        &mut input.first_name_en,
        &mut input.last_name_en,
    ];
    let mut after = before.clone();
    let mut changed = false;
    for (index, part) in updates.into_iter().enumerate() {
        if let Some(value) = part {
            *value = normalize_name_part(value, if index == 0 || index == 3 { 50 } else { 100 })?;
            changed |= value.as_str() != before[index].as_deref().unwrap_or("").trim();
            after[index] = Some(value.clone());
        }
    }
    if changed {
        require_complete_name(std::array::from_fn(|index| {
            after[index].as_deref().unwrap_or("")
        }))?;
    }
    Ok(())
}
