-- Finalize the provider-neutral File Platform contract. This migration is a
-- one-way binary compatibility boundary: backend releases that still query
-- the removed path-based columns cannot run after it is applied.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM files
        WHERE filename IS NULL
           OR btrim(filename) = ''
           OR purpose_code IS NULL
           OR btrim(purpose_code) = ''
    ) THEN
        RAISE EXCEPTION
            'file platform contract cutover blocked: invalid logical file metadata'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM files AS f
        WHERE NOT EXISTS (
            SELECT 1
            FROM file_versions AS v
            WHERE v.file_id = f.id
        )
    ) THEN
        RAISE EXCEPTION
            'file platform contract cutover blocked: file version missing'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM files AS f
        WHERE f.lifecycle_status = 'ready'
          AND NOT EXISTS (
              SELECT 1
              FROM file_versions AS v
              WHERE v.id = f.current_version_id
                AND v.file_id = f.id
          )
    ) THEN
        RAISE EXCEPTION
            'file platform contract cutover blocked: ready file version invalid'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM users
        WHERE NULLIF(btrim(profile_image_url), '') IS NOT NULL
          AND profile_image_file_id IS NULL
    ) THEN
        RAISE EXCEPTION
            'file platform contract cutover blocked: legacy profile reference remains'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM staff_achievements
        WHERE NULLIF(btrim(image_path), '') IS NOT NULL
          AND image_file_id IS NULL
    ) THEN
        RAISE EXCEPTION
            'file platform contract cutover blocked: legacy achievement reference remains'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;

DROP VIEW active_files;
DROP FUNCTION generate_storage_path(VARCHAR, VARCHAR, UUID, VARCHAR);

ALTER TABLE files
    DROP CONSTRAINT files_user_id_fkey;

ALTER TABLE files
    RENAME COLUMN user_id TO owner_user_id;

ALTER INDEX idx_files_user_id
    RENAME TO idx_files_owner_user_id;

ALTER TABLE files
    ADD CONSTRAINT files_owner_user_id_fkey
    FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE files
    RENAME COLUMN filename TO display_filename;

ALTER TABLE files
    RENAME COLUMN uploaded_by TO created_by;

ALTER TABLE files
    RENAME CONSTRAINT files_uploaded_by_fkey TO files_created_by_fkey;

ALTER TABLE files
    ALTER COLUMN purpose_code SET NOT NULL,
    ADD CONSTRAINT files_display_filename_nonblank_check
        CHECK (btrim(display_filename) <> '');

DROP INDEX idx_files_temp_expires;

CREATE INDEX idx_files_temporary_expires
    ON files (expires_at)
    WHERE retention_class = 'temporary' AND deleted_at IS NULL;

ALTER TABLE users
    DROP COLUMN profile_image_url;

ALTER TABLE staff_achievements
    DROP COLUMN image_path;

ALTER TABLE files
    DROP COLUMN school_id,
    DROP COLUMN original_filename,
    DROP COLUMN file_size,
    DROP COLUMN mime_type,
    DROP COLUMN storage_path,
    DROP COLUMN file_type,
    DROP COLUMN width,
    DROP COLUMN height,
    DROP COLUMN has_thumbnail,
    DROP COLUMN thumbnail_path,
    DROP COLUMN is_temporary,
    DROP COLUMN is_public,
    DROP COLUMN checksum;

COMMENT ON TABLE files IS
    'Provider-neutral logical file identities. Physical object metadata belongs to immutable versions and derivatives.';

COMMENT ON COLUMN files.owner_user_id IS
    'Optional owner used by purpose policies; user deletion preserves retained file metadata.';

COMMENT ON COLUMN files.display_filename IS
    'Sanitized display filename; never an object key or provider locator.';
