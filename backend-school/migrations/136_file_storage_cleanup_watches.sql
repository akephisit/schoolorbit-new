-- Physical cleanup identities must survive application metadata and Certificate purge.
-- NULL namespace means historical storage ownership is unverified, never current-config inferred.
CREATE TABLE file_storage_cleanup_watches (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    file_id uuid NOT NULL,
    target_kind text NOT NULL CHECK (target_kind IN ('version', 'derivative')),
    target_id uuid NOT NULL,
    provider_code varchar(32) NOT NULL CHECK (provider_code ~ '^[a-z0-9_]+$'),
    storage_class varchar(16) NOT NULL CHECK (storage_class IN ('public', 'private')),
    object_key text NOT NULL CHECK (octet_length(object_key) BETWEEN 1 AND 1024
        AND object_key ~ '^[A-Za-z0-9/.-]+$' AND object_key NOT LIKE '%/../%'),
    content_type varchar(100) NOT NULL CHECK (btrim(content_type) <> ''),
    namespace_fingerprint varchar(64) CHECK (namespace_fingerprint ~ '^[0-9a-f]{64}$'),
    namespace_bound_at timestamptz,
    binding_evidence_digest varchar(64) CHECK (binding_evidence_digest ~ '^[0-9a-f]{64}$'),
    may_write_since timestamptz NOT NULL DEFAULT statement_timestamp(),
    write_generation bigint NOT NULL DEFAULT 1 CHECK (write_generation > 0),
    row_revision bigint NOT NULL DEFAULT 0 CHECK (row_revision >= 0),
    revoked_at timestamptz,
    next_probe_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    lease_owner varchar(100),
    lease_generation bigint NOT NULL DEFAULT 0 CHECK (lease_generation >= 0),
    leased_at timestamptz,
    lease_expires_at timestamptz,
    last_probe_at timestamptz,
    last_absent_at timestamptz,
    last_error_code varchar(64) CHECK (last_error_code ~ '^[a-z0-9_]{1,64}$'),
    created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    UNIQUE (provider_code, storage_class, object_key),
    UNIQUE (target_kind, target_id),
    CHECK ((namespace_fingerprint IS NULL) = (namespace_bound_at IS NULL)),
    CHECK ((lease_owner IS NULL AND leased_at IS NULL AND lease_expires_at IS NULL)
        OR (lease_owner IS NOT NULL AND length(btrim(lease_owner)) > 0
            AND leased_at IS NOT NULL AND lease_expires_at > leased_at))
);
-- Intentionally NO foreign keys to file/domain/operation rows: 039 deletes those rows.
CREATE INDEX file_storage_cleanup_watches_due
    ON file_storage_cleanup_watches(next_probe_at, id)
    WHERE revoked_at IS NOT NULL AND namespace_fingerprint IS NOT NULL;
CREATE INDEX file_storage_cleanup_watches_file ON file_storage_cleanup_watches(file_id);

CREATE FUNCTION file_storage_watch_preserve_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'file_storage_watch_history_required' USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF ROW(NEW.id, NEW.file_id, NEW.target_kind, NEW.target_id, NEW.provider_code,
           NEW.storage_class, NEW.object_key, NEW.content_type, NEW.may_write_since, NEW.created_at)
       IS DISTINCT FROM
       ROW(OLD.id, OLD.file_id, OLD.target_kind, OLD.target_id, OLD.provider_code,
           OLD.storage_class, OLD.object_key, OLD.content_type, OLD.may_write_since, OLD.created_at)
       OR NEW.write_generation < OLD.write_generation
       OR (OLD.revoked_at IS NOT NULL AND NEW.revoked_at IS DISTINCT FROM OLD.revoked_at)
       OR (OLD.namespace_fingerprint IS NOT NULL AND
           ROW(NEW.namespace_fingerprint, NEW.namespace_bound_at, NEW.binding_evidence_digest)
           IS DISTINCT FROM ROW(OLD.namespace_fingerprint, OLD.namespace_bound_at, OLD.binding_evidence_digest)) THEN
        RAISE EXCEPTION 'file_storage_watch_identity_immutable' USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF OLD.namespace_fingerprint IS NULL AND NEW.namespace_fingerprint IS NOT NULL THEN
        -- Custom GUCs are writable by application roles and are evidence only.
        -- Actual table-owner authority is independently required for legacy binding.
        IF NOT pg_has_role(current_user, (SELECT relowner FROM pg_class WHERE oid=TG_RELID), 'USAGE') THEN
            RAISE EXCEPTION 'file_storage_namespace_owner_required' USING ERRCODE = 'insufficient_privilege';
        END IF;
        IF NEW.binding_evidence_digest IS NULL OR
            current_setting('schoolorbit.storage_binding_attestation', true)
                IS DISTINCT FROM NEW.binding_evidence_digest THEN
            RAISE EXCEPTION 'file_storage_namespace_attestation_required' USING ERRCODE = 'integrity_constraint_violation';
        END IF;
    END IF;
    NEW.row_revision := OLD.row_revision + 1;
    NEW.updated_at := statement_timestamp();
    RETURN NEW;
END $$;
CREATE TRIGGER file_storage_watch_identity BEFORE UPDATE OR DELETE ON file_storage_cleanup_watches
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_preserve_identity();

CREATE FUNCTION file_storage_watch_capture_target() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    target record;
    kind text;
    watched file_storage_cleanup_watches%ROWTYPE;
    revoked timestamptz;
BEGIN
    IF TG_OP = 'DELETE' THEN target := OLD; ELSE target := NEW; END IF;
    kind := CASE WHEN TG_TABLE_NAME = 'file_versions' THEN 'version' ELSE 'derivative' END;
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.id, NEW.file_id, NEW.provider_code, NEW.storage_class, NEW.object_key, NEW.detected_mime_type)
           IS DISTINCT FROM ROW(OLD.id, OLD.file_id, OLD.provider_code, OLD.storage_class, OLD.object_key, OLD.detected_mime_type) THEN
            RAISE EXCEPTION 'file_storage_target_identity_immutable' USING ERRCODE = 'integrity_constraint_violation';
        END IF;
        RETURN NEW;
    END IF;
    SELECT CASE WHEN lifecycle_status IN ('delete_requested', 'deleted') THEN
        COALESCE(delete_requested_at, deleted_at, statement_timestamp()) END
        INTO revoked FROM files WHERE id = target.file_id;
    IF TG_OP = 'DELETE' THEN revoked := COALESCE(revoked, statement_timestamp()); END IF;
    INSERT INTO file_storage_cleanup_watches(file_id,target_kind,target_id,provider_code,
        storage_class,object_key,content_type,revoked_at)
    VALUES(target.file_id,kind,target.id,target.provider_code,target.storage_class,
        target.object_key,target.detected_mime_type,revoked)
    ON CONFLICT (provider_code,storage_class,object_key) DO NOTHING;
    SELECT * INTO STRICT watched FROM file_storage_cleanup_watches
        WHERE provider_code=target.provider_code AND storage_class=target.storage_class
            AND object_key=target.object_key FOR UPDATE;
    IF ROW(watched.file_id,watched.target_kind,watched.target_id,watched.content_type)
       IS DISTINCT FROM ROW(target.file_id,kind,target.id,target.detected_mime_type)
       OR (TG_OP='INSERT' AND watched.revoked_at IS NOT NULL) THEN
        RAISE EXCEPTION 'file_storage_target_reuse_forbidden' USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF revoked IS NOT NULL THEN
        UPDATE file_storage_cleanup_watches SET revoked_at=COALESCE(revoked_at,revoked),
            next_probe_at=LEAST(next_probe_at,statement_timestamp()) WHERE id=watched.id;
    END IF;
    IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END $$;

CREATE TRIGGER file_versions_watch_insert AFTER INSERT ON file_versions
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_capture_target();
CREATE TRIGGER file_versions_watch_delete BEFORE DELETE ON file_versions
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_capture_target();
CREATE TRIGGER file_versions_watch_identity BEFORE UPDATE OF id,file_id,provider_code,storage_class,object_key,detected_mime_type ON file_versions
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_capture_target();
CREATE TRIGGER file_derivatives_watch_insert AFTER INSERT ON file_derivatives
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_capture_target();
CREATE TRIGGER file_derivatives_watch_delete BEFORE DELETE ON file_derivatives
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_capture_target();
CREATE TRIGGER file_derivatives_watch_identity BEFORE UPDATE OF id,file_id,provider_code,storage_class,object_key,detected_mime_type ON file_derivatives
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_capture_target();

CREATE FUNCTION file_storage_watch_revoke_file() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        UPDATE file_storage_cleanup_watches SET revoked_at=COALESCE(revoked_at,statement_timestamp()),
            next_probe_at=LEAST(next_probe_at,statement_timestamp()) WHERE file_id=OLD.id;
        RETURN OLD;
    ELSIF NEW.lifecycle_status IN ('delete_requested','deleted') THEN
        UPDATE file_storage_cleanup_watches SET revoked_at=COALESCE(revoked_at,statement_timestamp()),
            next_probe_at=LEAST(next_probe_at,statement_timestamp()) WHERE file_id=OLD.id;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER files_watch_revoke AFTER UPDATE OF lifecycle_status ON files
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_revoke_file();
CREATE TRIGGER files_watch_delete BEFORE DELETE ON files
    FOR EACH ROW EXECUTE FUNCTION file_storage_watch_revoke_file();

-- Preserve every surviving locator, including deleted targets. Current configuration
-- cannot prove its historical namespace, so all backfilled bindings remain NULL.
INSERT INTO file_storage_cleanup_watches(file_id,target_kind,target_id,provider_code,
    storage_class,object_key,content_type,revoked_at)
SELECT v.file_id,'version',v.id,v.provider_code,v.storage_class,v.object_key,v.detected_mime_type,
    CASE WHEN f.lifecycle_status IN ('delete_requested','deleted') OR v.storage_status='deleted'
        THEN COALESCE(f.delete_requested_at,v.deleted_at,f.deleted_at,statement_timestamp()) END
FROM file_versions v JOIN files f ON f.id=v.file_id
UNION ALL
SELECT d.file_id,'derivative',d.id,d.provider_code,d.storage_class,d.object_key,d.detected_mime_type,
    CASE WHEN f.lifecycle_status IN ('delete_requested','deleted') OR d.storage_status='deleted'
        THEN COALESCE(f.delete_requested_at,d.deleted_at,f.deleted_at,statement_timestamp()) END
FROM file_derivatives d JOIN files f ON f.id=d.file_id;
