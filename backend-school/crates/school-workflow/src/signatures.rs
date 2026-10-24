//! Source-neutral signature validation; source owners retain consent and evidence atomically.
use crate::{
    process::{invalid, stale},
    process_models::{SignatureInput, SignatureMethod},
};
use school_database::DbSession;
use school_errors::AppError;

pub fn validate_content_bound_signature(
    input: &SignatureInput,
    consent: bool,
    revision: i64,
    hash: &str,
    image_present: bool,
) -> Result<(), AppError> {
    if !consent {
        return Err(invalid("กรุณายินยอมใช้ลายมือชื่อของตนเองสำหรับรายการนี้"));
    }
    if input.document_revision != revision || input.content_hash != hash {
        return Err(stale());
    }
    if input.page != 1
        || ![input.x, input.y, input.width, input.height]
            .iter()
            .all(|n| n.is_finite())
        || input.x < 0.0
        || input.y < 0.0
        || input.width <= 0.0
        || input.height <= 0.0
        || input.x + input.width > 1.0
        || input.y + input.height > 1.0
        || input.asset_id.is_some()
    {
        return Err(invalid("ตำแหน่งลายมือชื่อไม่ถูกต้อง"));
    }
    match input.method {
        SignatureMethod::Drawn => {
            if image_present || input.strokes.is_empty() || input.strokes.len() > 500 {
                return Err(invalid("กรุณาวาดลายมือชื่อ"));
            }
            let mut total = 0;
            for s in &input.strokes {
                total += s.points.len();
                if s.points.len() < 2
                    || !s.width.is_finite()
                    || !(0.0001..=0.1).contains(&s.width)
                    || !matches!(s.color.as_str(), "#000000" | "#000" | "#111827" | "#1e293b")
                    || s.points.iter().any(|p| {
                        !p.x.is_finite()
                            || !p.y.is_finite()
                            || !(0.0..=1.0).contains(&p.x)
                            || !(0.0..=1.0).contains(&p.y)
                    })
                {
                    return Err(invalid("เส้นลายมือชื่อไม่ถูกต้อง"));
                }
            }
            if total > 50000 {
                return Err(invalid("ลายมือชื่อมีรายละเอียดมากเกินไป"));
            }
        }
        SignatureMethod::Image => {
            if !image_present || !input.strokes.is_empty() {
                return Err(invalid("กรุณาเลือกไฟล์ลายมือชื่อของตนเอง"));
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_models::{InkPoint, InkStroke};
    #[test]
    fn signature_requires_consent_and_exact_revision_and_hash() {
        let s = SignatureInput {
            method: SignatureMethod::Drawn,
            asset_id: None,
            strokes: vec![InkStroke {
                points: vec![InkPoint { x: 0.1, y: 0.1 }, InkPoint { x: 0.2, y: 0.2 }],
                width: 0.005,
                color: "#000000".into(),
            }],
            page: 1,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            document_revision: 2,
            content_hash: "abc".into(),
        };
        assert!(validate_content_bound_signature(&s, true, 2, "abc", false).is_ok());
        assert!(validate_content_bound_signature(&s, false, 2, "abc", false).is_err());
        assert!(validate_content_bound_signature(&s, true, 3, "abc", false).is_err());
        assert!(validate_content_bound_signature(&s, true, 2, "def", false).is_err());
    }
}

/// Immutable private image identity for an explicitly consenting acting signer.
/// The source owner retains its relationship and promotes retention in the same transaction.
pub async fn private_image_snapshot_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor: uuid::Uuid,
    file: uuid::Uuid,
    purpose: school_file_platform::platform_types::FilePurpose,
) -> Result<(uuid::Uuid, uuid::Uuid, String), AppError> {
    DbSession::new(&mut **tx).query_as("workflow.src.signatures.private_image_snapshot_in_transaction", "SELECT f.id,v.id,v.checksum FROM files f JOIN file_versions v ON v.id=f.current_version_id AND v.file_id=f.id WHERE f.id=$1 AND f.purpose_code=$3 AND f.visibility='private' AND f.lifecycle_status='ready' AND f.deleted_at IS NULL AND f.owner_user_id=$2 AND f.created_by=$2 AND v.scan_status='clean' AND v.storage_status='stored' AND v.detected_mime_type IN ('image/png','image/jpeg','image/webp') AND (f.retention_class='standard' OR (f.retention_class='temporary' AND f.expires_at>now())) FOR UPDATE OF f").bind(file).bind(actor).bind(purpose.code()).fetch_optional().await?.ok_or_else(||invalid("กรุณาเลือกไฟล์ลายมือชื่อส่วนตัวที่พร้อมใช้งาน"))
}
