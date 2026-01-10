-- ===================================================================
-- Migration 018: File Storage System (Cloudflare R2/S3 Compatible)
-- Description: Comprehensive file management system for user files,
--              documents, and images using path-based storage
-- Author: SchoolOrbit Team
-- Date: 2026-01-09
-- ===================================================================

-- ===================================================================
-- 1. Create files table for file metadata
-- ===================================================================
CREATE TABLE IF NOT EXISTS files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Ownership
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    school_id VARCHAR(100),  -- For multi-tenant tracking
    
    -- File Information
    filename VARCHAR(255) NOT NULL,
    original_filename VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    
    -- Storage (PATH-BASED for flexibility)
    storage_path TEXT NOT NULL UNIQUE,
    -- Example: "school-abc/users/profiles/550e8400-e29b-41d4-a716-446655440000.jpg"
    
    -- File Classification
    file_type VARCHAR(50) NOT NULL,
    -- Types: 'profile_image', 'document', 'transcript', 'certificate', 
    --        'course_material', 'assignment', 'school_logo', etc.
    
    -- Image Metadata (if applicable)
    width INTEGER,
    height INTEGER,
    has_thumbnail BOOLEAN DEFAULT false,
    thumbnail_path TEXT,
    
    -- Lifecycle Management
    is_temporary BOOLEAN DEFAULT false,
    is_public BOOLEAN DEFAULT false,
    expires_at TIMESTAMPTZ,
    
    -- Security
    checksum VARCHAR(64),  -- SHA-256 checksum for integrity
    
    -- Audit
    uploaded_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ  -- Soft delete support
);

-- ===================================================================
-- 2. Create Indexes for Performance
-- ===================================================================
CREATE INDEX IF NOT EXISTS idx_files_user_id ON files(user_id);
CREATE INDEX IF NOT EXISTS idx_files_school_id ON files(school_id);
CREATE INDEX IF NOT EXISTS idx_files_file_type ON files(file_type);
CREATE INDEX IF NOT EXISTS idx_files_storage_path ON files(storage_path);
CREATE INDEX IF NOT EXISTS idx_files_created_at ON files(created_at);

-- Index for cleanup jobs (temporary files)
CREATE INDEX IF NOT EXISTS idx_files_temp_expires 
    ON files(expires_at) 
    WHERE is_temporary = true AND deleted_at IS NULL;

-- Index for soft deletes
CREATE INDEX IF NOT EXISTS idx_files_deleted 
    ON files(deleted_at) 
    WHERE deleted_at IS NOT NULL;

-- ===================================================================
-- 3. Add Check Constraints
-- ===================================================================
ALTER TABLE files
    ADD CONSTRAINT chk_file_size_positive 
    CHECK (file_size > 0);

ALTER TABLE files
    ADD CONSTRAINT chk_file_type_valid
    CHECK (file_type IN (
        'profile_image', 'document', 'transcript', 'certificate',
        'course_material', 'assignment', 'school_logo', 'school_banner',
        'id_card', 'other'
    ));

-- ===================================================================
-- 4. Update users table: Rename profile_image_url to profile_image_path
-- ===================================================================
-- Note: We keep the column for backward compatibility but change semantics
-- The field now stores PATH instead of URL
COMMENT ON COLUMN users.profile_image_url IS 
    'Storage path for profile image (e.g., school-xxx/users/profiles/uuid.jpg). 
     Will be converted to URL at runtime using configured base URL.';

-- Optional: If you want to rename the column (uncomment if needed)
-- ALTER TABLE users RENAME COLUMN profile_image_url TO profile_image_path;

-- ===================================================================
-- 5. Create updated_at trigger for files table
-- ===================================================================
DROP TRIGGER IF EXISTS update_files_updated_at ON files;
CREATE TRIGGER update_files_updated_at
    BEFORE UPDATE ON files
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ===================================================================
-- 6. Create helper function to generate storage path
-- ===================================================================
CREATE OR REPLACE FUNCTION generate_storage_path(
    p_school_id VARCHAR,
    p_file_type VARCHAR,
    p_user_id UUID,
    p_extension VARCHAR
) RETURNS TEXT AS $$
DECLARE
    v_path TEXT;
    v_uuid TEXT;
BEGIN
    v_uuid := gen_random_uuid()::TEXT;
    
    -- Generate path based on file type
    CASE p_file_type
        WHEN 'profile_image' THEN
            v_path := format('school-%s/users/profiles/%s%s', 
                p_school_id, v_uuid, p_extension);
        WHEN 'document', 'transcript', 'certificate' THEN
            v_path := format('school-%s/users/documents/%s/%s%s', 
                p_school_id, p_user_id, v_uuid, p_extension);
        WHEN 'course_material', 'assignment' THEN
            v_path := format('school-%s/courses/%s%s', 
                p_school_id, v_uuid, p_extension);
        WHEN 'school_logo', 'school_banner' THEN
            v_path := format('school-%s/branding/%s%s', 
                p_school_id, v_uuid, p_extension);
        ELSE
            v_path := format('school-%s/files/%s%s', 
                p_school_id, v_uuid, p_extension);
    END CASE;
    
    RETURN v_path;
END;
$$ LANGUAGE plpgsql;

-- ===================================================================
-- 7. Create view for active files (excluding soft-deleted)
-- ===================================================================
CREATE OR REPLACE VIEW active_files AS
SELECT 
    f.*,
    u.first_name || ' ' || u.last_name AS uploaded_by_name
FROM files f
LEFT JOIN users u ON f.uploaded_by = u.id
WHERE f.deleted_at IS NULL;

-- ===================================================================
-- 8. Comments for documentation
-- ===================================================================
COMMENT ON TABLE files IS 'File metadata for all uploaded files (images, documents, etc.). Uses path-based storage for flexibility with R2/S3.';
COMMENT ON COLUMN files.storage_path IS 'Relative storage path in R2 bucket (e.g., school-abc/users/profiles/uuid.jpg)';
COMMENT ON COLUMN files.file_type IS 'Classification of file for organizing storage structure';
COMMENT ON COLUMN files.is_temporary IS 'Temporary files are automatically cleaned up after expiration';
COMMENT ON COLUMN files.is_public IS 'Public files can be accessed without authentication';
COMMENT ON COLUMN files.checksum IS 'SHA-256 checksum for file integrity verification';
COMMENT ON COLUMN files.deleted_at IS 'Soft delete timestamp - files marked for deletion but not yet removed from storage';

-- ===================================================================
-- 9. Grant permissions (adjust as needed)
-- ===================================================================
-- This will vary based on your database user setup
-- Uncomment and adjust if needed:
-- GRANT SELECT, INSERT, UPDATE, DELETE ON files TO school_app_user;
-- GRANT USAGE, SELECT ON SEQUENCE files_id_seq TO school_app_user;
