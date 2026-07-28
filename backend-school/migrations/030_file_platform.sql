-- File Platform foundation. Legacy files columns remain during the compatibility
-- window so application rollback can continue using the existing logical IDs.

ALTER TABLE files
    ADD COLUMN purpose_code VARCHAR(100),
    ADD COLUMN visibility VARCHAR(16) NOT NULL DEFAULT 'private',
    ADD COLUMN lifecycle_status VARCHAR(32) NOT NULL DEFAULT 'pending',
    ADD COLUMN current_version_id UUID,
    ADD COLUMN retention_class VARCHAR(32) NOT NULL DEFAULT 'standard',
    ADD COLUMN delete_requested_at TIMESTAMPTZ,
    ADD CONSTRAINT files_visibility_check
        CHECK (visibility IN ('public', 'private')),
    ADD CONSTRAINT files_lifecycle_status_check
        CHECK (lifecycle_status IN (
            'pending',
            'processing',
            'ready',
            'delete_requested',
            'deleted',
            'failed',
            'quarantined'
        )),
    ADD CONSTRAINT files_retention_class_check
        CHECK (retention_class IN ('standard', 'temporary', 'legal_hold'));

CREATE TABLE file_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL REFERENCES files(id) ON DELETE RESTRICT,
    version_number INTEGER NOT NULL,
    provider_code VARCHAR(32) NOT NULL,
    storage_class VARCHAR(16) NOT NULL,
    object_key TEXT NOT NULL,
    detected_mime_type VARCHAR(100) NOT NULL,
    canonical_extension VARCHAR(20) NOT NULL,
    byte_size BIGINT NOT NULL,
    checksum CHAR(64) NOT NULL,
    scan_status VARCHAR(32) NOT NULL DEFAULT 'pending',
    scanner_result_code VARCHAR(64),
    scanned_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT file_versions_version_number_check CHECK (version_number > 0),
    CONSTRAINT file_versions_storage_class_check
        CHECK (storage_class IN ('public', 'private')),
    CONSTRAINT file_versions_detected_mime_type_check
        CHECK (btrim(detected_mime_type) <> ''),
    CONSTRAINT file_versions_canonical_extension_check
        CHECK (canonical_extension ~ '^[a-z0-9]{1,20}$'),
    CONSTRAINT file_versions_byte_size_check CHECK (byte_size > 0),
    CONSTRAINT file_versions_checksum_check
        CHECK (checksum ~ '^[0-9a-f]{64}$'),
    CONSTRAINT file_versions_scan_status_check
        CHECK (scan_status IN ('pending', 'clean', 'infected', 'failed', 'skipped')),
    CONSTRAINT file_versions_scanner_result_code_check
        CHECK (scanner_result_code IS NULL OR scanner_result_code ~ '^[a-z0-9_]{1,64}$'),
    CONSTRAINT file_versions_file_version_number_key UNIQUE (file_id, version_number),
    CONSTRAINT file_versions_provider_storage_object_key_key
        UNIQUE (provider_code, storage_class, object_key)
);

ALTER TABLE files
    ADD CONSTRAINT files_current_version_id_fkey
    FOREIGN KEY (current_version_id) REFERENCES file_versions(id) ON DELETE SET NULL;

CREATE TABLE file_derivatives (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_version_id UUID NOT NULL REFERENCES file_versions(id) ON DELETE RESTRICT,
    derivative_kind VARCHAR(64) NOT NULL,
    provider_code VARCHAR(32) NOT NULL,
    storage_class VARCHAR(16) NOT NULL,
    object_key TEXT NOT NULL,
    detected_mime_type VARCHAR(100) NOT NULL,
    canonical_extension VARCHAR(20) NOT NULL,
    byte_size BIGINT NOT NULL,
    checksum CHAR(64) NOT NULL,
    lifecycle_status VARCHAR(32) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT file_derivatives_kind_check
        CHECK (derivative_kind ~ '^[a-z0-9][a-z0-9-]{0,63}$'),
    CONSTRAINT file_derivatives_storage_class_check
        CHECK (storage_class IN ('public', 'private')),
    CONSTRAINT file_derivatives_detected_mime_type_check
        CHECK (btrim(detected_mime_type) <> ''),
    CONSTRAINT file_derivatives_canonical_extension_check
        CHECK (canonical_extension ~ '^[a-z0-9]{1,20}$'),
    CONSTRAINT file_derivatives_byte_size_check CHECK (byte_size > 0),
    CONSTRAINT file_derivatives_checksum_check
        CHECK (checksum ~ '^[0-9a-f]{64}$'),
    CONSTRAINT file_derivatives_lifecycle_status_check
        CHECK (lifecycle_status IN ('pending', 'processing', 'ready', 'failed', 'deleted')),
    CONSTRAINT file_derivatives_source_version_kind_key
        UNIQUE (source_version_id, derivative_kind),
    CONSTRAINT file_derivatives_provider_storage_object_key_key
        UNIQUE (provider_code, storage_class, object_key)
);

CREATE TABLE file_operations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL REFERENCES files(id) ON DELETE RESTRICT,
    file_version_id UUID REFERENCES file_versions(id) ON DELETE RESTRICT,
    file_derivative_id UUID REFERENCES file_derivatives(id) ON DELETE RESTRICT,
    operation_type VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_retry_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner VARCHAR(100),
    leased_at TIMESTAMPTZ,
    lease_expires_at TIMESTAMPTZ,
    last_error_code VARCHAR(64),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT file_operations_operation_type_check
        CHECK (operation_type IN ('scan', 'generate_derivative', 'delete_object', 'reconcile')),
    CONSTRAINT file_operations_status_check
        CHECK (status IN ('pending', 'leased', 'succeeded', 'retryable_failure', 'failed', 'cancelled')),
    CONSTRAINT file_operations_attempt_count_check
        CHECK (attempt_count BETWEEN 0 AND 100),
    CONSTRAINT file_operations_lease_check CHECK (
        (lease_owner IS NULL AND leased_at IS NULL AND lease_expires_at IS NULL)
        OR (
            lease_owner IS NOT NULL
            AND leased_at IS NOT NULL
            AND lease_expires_at IS NOT NULL
            AND lease_expires_at > leased_at
        )
    ),
    CONSTRAINT file_operations_last_error_code_check
        CHECK (last_error_code IS NULL OR last_error_code ~ '^[a-z0-9_]{1,64}$')
);

CREATE FUNCTION file_platform_preserve_version_identity()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.file_id IS DISTINCT FROM OLD.file_id
        OR NEW.version_number IS DISTINCT FROM OLD.version_number
        OR NEW.provider_code IS DISTINCT FROM OLD.provider_code
        OR NEW.storage_class IS DISTINCT FROM OLD.storage_class
        OR NEW.object_key IS DISTINCT FROM OLD.object_key
        OR NEW.detected_mime_type IS DISTINCT FROM OLD.detected_mime_type
        OR NEW.canonical_extension IS DISTINCT FROM OLD.canonical_extension
        OR NEW.byte_size IS DISTINCT FROM OLD.byte_size
        OR NEW.checksum IS DISTINCT FROM OLD.checksum
    THEN
        RAISE EXCEPTION 'file version identity is immutable';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER file_versions_preserve_identity
BEFORE UPDATE ON file_versions
FOR EACH ROW EXECUTE FUNCTION file_platform_preserve_version_identity();

CREATE FUNCTION file_platform_preserve_derivative_identity()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.source_version_id IS DISTINCT FROM OLD.source_version_id
        OR NEW.derivative_kind IS DISTINCT FROM OLD.derivative_kind
        OR NEW.provider_code IS DISTINCT FROM OLD.provider_code
        OR NEW.storage_class IS DISTINCT FROM OLD.storage_class
        OR NEW.object_key IS DISTINCT FROM OLD.object_key
        OR NEW.detected_mime_type IS DISTINCT FROM OLD.detected_mime_type
        OR NEW.canonical_extension IS DISTINCT FROM OLD.canonical_extension
        OR NEW.byte_size IS DISTINCT FROM OLD.byte_size
        OR NEW.checksum IS DISTINCT FROM OLD.checksum
    THEN
        RAISE EXCEPTION 'file derivative identity is immutable';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER file_derivatives_preserve_identity
BEFORE UPDATE ON file_derivatives
FOR EACH ROW EXECUTE FUNCTION file_platform_preserve_derivative_identity();

CREATE INDEX idx_files_lifecycle_status
    ON files (lifecycle_status, delete_requested_at)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_files_purpose_visibility
    ON files (purpose_code, visibility)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_file_versions_file_created
    ON file_versions (file_id, created_at DESC);

CREATE INDEX idx_file_versions_scan_status
    ON file_versions (scan_status)
    WHERE scan_status IN ('pending', 'failed');

CREATE INDEX idx_file_derivatives_source_status
    ON file_derivatives (source_version_id, lifecycle_status);

CREATE INDEX idx_file_operations_due
    ON file_operations (status, next_retry_at)
    WHERE status IN ('pending', 'retryable_failure');

CREATE INDEX idx_file_operations_lease_expiry
    ON file_operations (lease_expires_at)
    WHERE status = 'leased';

CREATE INDEX idx_file_operations_file_created
    ON file_operations (file_id, created_at DESC);

COMMENT ON TABLE file_versions IS
    'Immutable content identities for logical files. Object locator fields cannot be changed after creation.';

COMMENT ON TABLE file_derivatives IS
    'Immutable derivative content identities. Object locator fields cannot be changed after creation.';

COMMENT ON TABLE file_operations IS
    'Durable scan, derivative, deletion, and reconciliation work. Error fields contain only bounded safe codes.';
