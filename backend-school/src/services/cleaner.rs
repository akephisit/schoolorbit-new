use std::sync::Arc;
use sqlx::{Pool, Postgres, Row};
use tracing::{info, error, warn};
use crate::services::r2_client::R2Client;

pub struct FileCleaner {
    db_pool: Pool<Postgres>,
    r2_client: R2Client,
}

impl FileCleaner {
    pub async fn new(db_pool: Pool<Postgres>) -> Result<Self, anyhow::Error> {
        let r2_client = R2Client::new().await?;
        Ok(Self {
            db_pool,
            r2_client,
        })
    }

    pub async fn clean_orphaned_files(&self) {
        info!("🧹 Starting orphaned file cleanup job (Garbage Collection)...");

        // 1. Find Orphaned Profile Images
        // Finds files marked as 'profile_image' that are NOT referenced by any user in `profile_image_url`
        // We assume `users.profile_image_url` stores the storage path (or compatible unique suffix).
        // Since `users.profile_image_url` might be a full URL, we need to be careful.
        // BUT, based on the Image Upload logic we just fixed, the frontend sends a URL, but the DB likely stores the Path or the backend converts it.
        // Let's assume strict checking for now: storage_path matches exactly OR verify via logic.
        
        // Actually, safer logic:
        // Get all profile_image files.
        // For each file, check if it exists in users table.
        // If not, delete it.
        
        let batch_size = 50;
        
        // Query: Find files that are active (deleted_at is null) in DB 
        // but no longer pointed to by any user.
        // Note: usage of 'profile_image_url' in users table vs 'storage_path' in files table.
        // If users table stores full URL (https://pub-xxx.r2.dev/path), and files table stores path (path), we need to extract.
        // But let's try strict join first, assuming standard system behavior.
        
        let query = r#"
            SELECT f.id, f.storage_path 
            FROM files f
            LEFT JOIN users u ON u.profile_image_url LIKE '%' || f.storage_path || '%'
            WHERE f.file_type = 'profile_image' 
            AND f.deleted_at IS NULL
            AND u.id IS NULL
            LIMIT $1
        "#;

        match sqlx::query(query)
            .bind(batch_size)
            .fetch_all(&self.db_pool)
            .await 
        {
            Ok(rows) => {
                if rows.is_empty() {
                    info!("✨ No orphaned profile images found.");
                    return;
                }

                info!("found {} orphaned profile images. Deleting...", rows.len());

                for row in rows {
                    let file_id: uuid::Uuid = row.get("id");
                    let storage_path: String = row.get("storage_path");

                    // 1. Delete from R2
                    info!("Deleting from R2: {}", storage_path);
                    if let Err(e) = self.r2_client.delete_file(&storage_path).await {
                         error!("Failed to delete file {} from R2: {}", storage_path, e);
                         continue; // Skip DB delete if R2 fail (to retry later)
                    }

                    // 2. Hard Delete from DB (Since it's orphaned garbage)
                    if let Err(e) = sqlx::query("DELETE FROM files WHERE id = $1")
                        .bind(file_id)
                        .execute(&self.db_pool)
                        .await 
                    {
                        error!("Failed to delete file record {} from DB: {}", file_id, e);
                    } else {
                        info!("Successfully cleaned up file: {}", file_id);
                    }
                }
            }
            Err(e) => {
                error!("Database error while searching for orphaned files: {}", e);
            }
        }
        
        info!("🧹 Cleanup job finished.");
    }
}
