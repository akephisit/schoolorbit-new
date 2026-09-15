//! Persistence of temporary file relationships owned by the certificate domain.

use school_errors::AppError;
use school_file_platform::platform_types::FilePurpose;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn record_template_upload(
    pool: &PgPool,
    file_id: Uuid,
    template_id: Uuid,
    purpose: FilePurpose,
    uploaded_by: Uuid,
) -> Result<(), AppError> {
    if !matches!(
        purpose,
        FilePurpose::CertificateTemplateBackground | FilePurpose::CertificateTemplateImage
    ) {
        return Err(AppError::BadRequest(
            "purpose ไม่ใช่ไฟล์ของแม่แบบเกียรติบัตร".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    lock_active_campaign(&mut transaction, template_id).await?;
    sqlx::query(
        "INSERT INTO certificate_template_file_uploads
            (file_id, template_id, purpose_code, uploaded_by)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(purpose.code())
    .bind(uploaded_by)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn record_school_font_upload(
    pool: &PgPool,
    file_id: Uuid,
    template_id: Uuid,
    uploaded_by: Uuid,
) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    lock_active_campaign(&mut transaction, template_id).await?;
    sqlx::query(
        "INSERT INTO certificate_school_font_file_uploads
            (file_id, purpose_code, template_id, uploaded_by)
         VALUES ($1, 'school_font', $2, $3)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(uploaded_by)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

async fn lock_active_campaign(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    template_id: Uuid,
) -> Result<(), AppError> {
    let campaign_status = sqlx::query_scalar::<_, String>(
        "SELECT campaign.status
         FROM certificate_templates AS template
         JOIN certificate_campaigns AS campaign ON campaign.id = template.campaign_id
         WHERE template.id = $1
         FOR UPDATE OF campaign",
    )
    .bind(template_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบเกียรติบัตร".to_string()))?;
    if campaign_status == "purging" {
        return Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ));
    }
    Ok(())
}
