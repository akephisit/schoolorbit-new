use crate::error::AppError;
use serde::de::DeserializeOwned;
use sqlx::{types::Json, Postgres, Transaction};
use uuid::Uuid;

/// Call under the tenant transition lock. Check identity before decoding:
/// different year commands intentionally have different outcome contracts.
pub(crate) async fn replay_year_command<T: DeserializeOwned + Send + Unpin + 'static>(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    actor: Uuid,
    action: &str,
    checksum: &str,
) -> Result<Option<T>, AppError> {
    let receipt: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT actor_user_id,action,request_checksum FROM academic_year_transition_receipts WHERE request_id=$1",
    ).bind(request).fetch_optional(&mut **tx).await?;
    let Some((owner, original_action, original_checksum)) = receipt else {
        return Ok(None);
    };
    verify_identity(
        owner,
        &original_action,
        &original_checksum,
        actor,
        action,
        checksum,
    )?;
    let outcome: Json<T> = sqlx::query_scalar(
        "SELECT outcome FROM academic_year_transition_receipts WHERE request_id=$1",
    )
    .bind(request)
    .fetch_one(&mut **tx)
    .await?;
    Ok(Some(outcome.0))
}

fn verify_identity(
    owner: Uuid,
    original_action: &str,
    original_checksum: &str,
    actor: Uuid,
    action: &str,
    checksum: &str,
) -> Result<(), AppError> {
    if owner != actor || original_action != action || original_checksum != checksum {
        return Err(AppError::Conflict(
            "คำขอนี้เคยใช้กับข้อมูล การดำเนินการ หรือผู้ดำเนินการอื่นแล้ว".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_lifecycle_replay_identity_requires_actor_action_and_checksum() {
        let actor = Uuid::new_v4();
        assert!(verify_identity(actor, "reopen", "a", actor, "reopen", "a").is_ok());
        for (other, action, checksum) in [
            (Uuid::new_v4(), "reopen", "a"),
            (actor, "close", "a"),
            (actor, "reopen", "b"),
        ] {
            assert!(matches!(
                verify_identity(actor, "reopen", "a", other, action, checksum),
                Err(AppError::Conflict(_))
            ));
        }
    }
}
