//! Source-rendering validation required before accepting a document mutation.
use crate::process_models::{CorrespondenceDocumentRevision, DocumentAnnotation, SignatureInput};
use chrono::{Datelike, Timelike};
use school_errors::AppError;

/// Required source-rendering port; HTTP composition binds the private File Platform implementation.
#[async_trait::async_trait]
pub trait DocumentValidationPort: Send + Sync {
    async fn validate(
        &self,
        document: &CorrespondenceDocumentRevision,
        annotations: &[DocumentAnnotation],
        signature: Option<&SignatureInput>,
        signer_label: &str,
    ) -> Result<(), AppError>;
}

/// Canonical signer identity and acceptance time shared by preflight and rendering.
pub fn signing_label(
    name: &str,
    role: Option<&str>,
    signed_at: chrono::DateTime<chrono::Utc>,
) -> String {
    let local = signed_at + chrono::Duration::hours(7);
    format!(
        "{} {} {:02}/{:02}/{} {:02}:{:02} น. (UTC+07:00)",
        name,
        role.unwrap_or(""),
        local.day(),
        local.month(),
        local.year() + 543,
        local.hour(),
        local.minute()
    )
}
