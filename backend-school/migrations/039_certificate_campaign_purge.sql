-- Durable, forward-only deletion for a complete certificate campaign.
--
-- A campaign is hidden while its storage objects are deleted by File Platform.
-- Only the guarded finalizer below may remove immutable certificate history and
-- the frozen File Platform metadata after every object is confirmed deleted.

ALTER TABLE certificate_campaigns
    DROP CONSTRAINT certificate_campaigns_status_check,
    ADD CONSTRAINT certificate_campaigns_status_check
        CHECK (status IN ('draft', 'active', 'closed', 'archived', 'purging'));

CREATE TABLE certificate_campaign_purge_jobs (
    campaign_id UUID PRIMARY KEY
        REFERENCES certificate_campaigns(id) ON DELETE CASCADE,
    status TEXT NOT NULL
        CHECK (status IN ('deleting_files', 'failed', 'finalizing')),
    requested_by UUID REFERENCES users(id) ON DELETE SET NULL,
    template_count BIGINT NOT NULL CHECK (template_count >= 0),
    candidate_count BIGINT NOT NULL CHECK (candidate_count >= 0),
    request_count BIGINT NOT NULL CHECK (request_count >= 0),
    open_request_count BIGINT NOT NULL CHECK (open_request_count >= 0),
    issued_certificate_count BIGINT NOT NULL CHECK (issued_certificate_count >= 0),
    revoked_certificate_count BIGINT NOT NULL CHECK (revoked_certificate_count >= 0),
    file_count BIGINT NOT NULL CHECK (file_count >= 0),
    total_file_bytes BIGINT NOT NULL CHECK (total_file_bytes >= 0),
    last_error_code VARCHAR(64) CHECK (
        last_error_code IS NULL OR last_error_code ~ '^[a-z0-9_]{1,64}$'
    ),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE certificate_campaign_purge_files (
    campaign_id UUID NOT NULL
        REFERENCES certificate_campaign_purge_jobs(campaign_id) ON DELETE CASCADE,
    file_id UUID NOT NULL UNIQUE REFERENCES files(id) ON DELETE CASCADE,
    object_count INTEGER NOT NULL CHECK (object_count >= 0),
    byte_size BIGINT NOT NULL CHECK (byte_size >= 0),
    PRIMARY KEY (campaign_id, file_id)
);

CREATE INDEX certificate_campaign_purge_jobs_status_updated_idx
    ON certificate_campaign_purge_jobs (status, updated_at, campaign_id);

CREATE TRIGGER update_certificate_campaign_purge_jobs_updated_at
    BEFORE UPDATE ON certificate_campaign_purge_jobs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

UPDATE permissions
SET name = CASE code
        WHEN 'certificate.delete.organization_unit'
            THEN 'ลบกิจกรรมเกียรติบัตรถาวรในหน่วยงาน'
        WHEN 'certificate.delete.school'
            THEN 'ลบกิจกรรมเกียรติบัตรถาวรทั้งโรงเรียน'
    END,
    description = CASE code
        WHEN 'certificate.delete.organization_unit'
            THEN 'ลบกิจกรรม แบบ รายชื่อ คำขอ เกียรติบัตร และไฟล์ของหน่วยงานที่ได้รับสิทธิ์โดยตรงแบบถาวร'
        WHEN 'certificate.delete.school'
            THEN 'ลบกิจกรรม แบบ รายชื่อ คำขอ เกียรติบัตร และไฟล์ในขอบเขตทั้งโรงเรียนแบบถาวร'
    END,
    updated_at = NOW()
WHERE code IN (
    'certificate.delete.organization_unit',
    'certificate.delete.school'
);

CREATE FUNCTION certificate_campaign_purge_guard_allows(p_campaign_id UUID)
RETURNS BOOLEAN
LANGUAGE sql
STABLE
AS $$
    SELECT p_campaign_id IS NOT NULL
        AND COALESCE(
            current_setting('schoolorbit.certificate_purge_campaign_id', TRUE),
            ''
        ) = p_campaign_id::TEXT
        AND EXISTS (
            SELECT 1
            FROM certificate_campaigns AS campaign
            JOIN certificate_campaign_purge_jobs AS job
              ON job.campaign_id = campaign.id
            WHERE campaign.id = p_campaign_id
              AND campaign.status = 'purging'
              AND job.status = 'finalizing'
        );
$$;

CREATE FUNCTION certificate_file_purge_guard_allows(p_file_id UUID)
RETURNS BOOLEAN
LANGUAGE sql
STABLE
AS $$
    SELECT p_file_id IS NOT NULL
        AND EXISTS (
            SELECT 1
            FROM certificate_campaign_purge_files AS inventory
            JOIN certificate_campaign_purge_jobs AS job
              ON job.campaign_id = inventory.campaign_id
            JOIN certificate_campaigns AS campaign
              ON campaign.id = inventory.campaign_id
            WHERE inventory.file_id = p_file_id
              AND campaign.status = 'purging'
              AND job.status = 'finalizing'
              AND COALESCE(
                    current_setting(
                        'schoolorbit.certificate_purge_campaign_id',
                        TRUE
                    ),
                    ''
                  ) = inventory.campaign_id::TEXT
        );
$$;

-- Keep destructive ownership discovery in one place so the pre-provider check
-- and the guarded finalizer cannot drift across File Platform consumers.
CREATE FUNCTION certificate_campaign_purge_has_external_file_consumer(
    p_campaign_id UUID,
    p_file_ids UUID[]
)
RETURNS BOOLEAN
LANGUAGE sql
STABLE
AS $$
    SELECT EXISTS (
        SELECT 1
        FROM certificate_templates AS template
        WHERE template.background_file_id = ANY(p_file_ids)
          AND template.campaign_id <> p_campaign_id
        UNION ALL
        SELECT 1
        FROM certificate_template_assets AS asset
        JOIN certificate_templates AS template ON template.id = asset.template_id
        WHERE asset.file_id = ANY(p_file_ids)
          AND template.campaign_id <> p_campaign_id
        UNION ALL
        SELECT 1
        FROM certificate_template_file_uploads AS upload
        JOIN certificate_templates AS template ON template.id = upload.template_id
        WHERE upload.file_id = ANY(p_file_ids)
          AND template.campaign_id <> p_campaign_id
        UNION ALL
        SELECT 1
        FROM users
        WHERE profile_image_file_id = ANY(p_file_ids)
        UNION ALL
        SELECT 1
        FROM staff_achievements
        WHERE image_file_id = ANY(p_file_ids)
        UNION ALL
        SELECT 1
        FROM admission_application_documents
        WHERE file_id = ANY(p_file_ids)
        UNION ALL
        SELECT 1
        FROM school_settings
        WHERE logo_file_id = ANY(p_file_ids)
        UNION ALL
        SELECT 1
        FROM (
            SELECT question.stem_content AS content
            FROM academic_question_bank_questions AS question
            UNION ALL
            SELECT question.explanation_content
            FROM academic_question_bank_questions AS question
            UNION ALL
            SELECT question.rubric_content
            FROM academic_question_bank_questions AS question
            UNION ALL
            SELECT choice.content
            FROM academic_question_bank_choices AS choice
        ) AS question_document
        CROSS JOIN LATERAL jsonb_array_elements(
            COALESCE(
                question_document.content -> 'document' -> 'content',
                '[]'::JSONB
            )
        ) AS block
        WHERE block ->> 'type' = 'image'
          AND EXISTS (
              SELECT 1
              FROM unnest(p_file_ids) AS candidate(file_id)
              WHERE candidate.file_id::TEXT = block -> 'attrs' ->> 'fileId'
          )
    );
$$;

CREATE FUNCTION prevent_uncontrolled_certificate_campaign_delete()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF certificate_campaign_purge_guard_allows(OLD.id) THEN
        RETURN OLD;
    END IF;

    RAISE EXCEPTION 'certificate campaigns require the guarded purge finalizer'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER prevent_certificate_campaign_delete
    BEFORE DELETE ON certificate_campaigns
    FOR EACH ROW
    EXECUTE FUNCTION prevent_uncontrolled_certificate_campaign_delete();

CREATE FUNCTION prevent_uncontrolled_certificate_campaign_purge_job_delete()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    -- During ON DELETE CASCADE the parent campaign is no longer visible to a
    -- query issued by this child trigger. The transaction-local finalizer guard
    -- and the locked job's finalizing state therefore form the narrow check.
    IF OLD.status = 'finalizing'
       AND COALESCE(
            current_setting('schoolorbit.certificate_purge_campaign_id', TRUE),
            ''
       ) = OLD.campaign_id::TEXT THEN
        RETURN OLD;
    END IF;

    RAISE EXCEPTION 'certificate campaign purge jobs cannot be deleted directly'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER prevent_certificate_campaign_purge_job_delete
    BEFORE DELETE ON certificate_campaign_purge_jobs
    FOR EACH ROW
    EXECUTE FUNCTION prevent_uncontrolled_certificate_campaign_purge_job_delete();

CREATE FUNCTION preserve_certificate_campaign_purge_inventory()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE'
       AND certificate_campaign_purge_guard_allows(OLD.campaign_id) THEN
        RETURN OLD;
    END IF;

    RAISE EXCEPTION 'certificate campaign purge inventory is immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER preserve_certificate_campaign_purge_inventory
    BEFORE UPDATE OR DELETE ON certificate_campaign_purge_files
    FOR EACH ROW
    EXECUTE FUNCTION preserve_certificate_campaign_purge_inventory();

CREATE FUNCTION prevent_uncontrolled_certificate_file_delete()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF certificate_file_purge_guard_allows(OLD.id) THEN
        RETURN OLD;
    END IF;

    RAISE EXCEPTION 'files require a guarded domain finalizer'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER prevent_certificate_file_delete
    BEFORE DELETE ON files
    FOR EACH ROW
    EXECUTE FUNCTION prevent_uncontrolled_certificate_file_delete();

CREATE OR REPLACE FUNCTION file_platform_prevent_version_deletion()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF certificate_file_purge_guard_allows(OLD.file_id) THEN
        RETURN OLD;
    END IF;

    RAISE EXCEPTION 'file versions must be soft-deleted';
END;
$$;

COMMENT ON TRIGGER file_versions_prevent_deletion ON file_versions IS
    'Prevents hard deletion except inside the guarded certificate campaign finalizer.';

CREATE OR REPLACE FUNCTION file_platform_prevent_derivative_deletion()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF certificate_file_purge_guard_allows(OLD.file_id) THEN
        RETURN OLD;
    END IF;

    RAISE EXCEPTION 'file derivatives must be soft-deleted';
END;
$$;

COMMENT ON TRIGGER file_derivatives_prevent_deletion ON file_derivatives IS
    'Prevents hard deletion except inside the guarded certificate campaign finalizer.';

CREATE OR REPLACE FUNCTION prevent_certificate_issue_request_item_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE'
       AND certificate_campaign_purge_guard_allows(OLD.campaign_id) THEN
        RETURN OLD;
    END IF;

    RAISE EXCEPTION 'certificate issue request items are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE OR REPLACE FUNCTION prevent_certificate_issue_run_problem_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    problem_campaign_id UUID;
BEGIN
    IF TG_OP = 'DELETE' THEN
        SELECT candidate.campaign_id
        INTO problem_campaign_id
        FROM certificate_candidates AS candidate
        WHERE candidate.id = OLD.candidate_id;

        IF certificate_campaign_purge_guard_allows(problem_campaign_id) THEN
            RETURN OLD;
        END IF;
    END IF;

    RAISE EXCEPTION 'certificate issue run problems are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE OR REPLACE FUNCTION enforce_certificate_snapshot_immutability()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF certificate_campaign_purge_guard_allows(OLD.campaign_id)
       AND NEW.replacement_for_certificate_id IS NULL
       AND NEW.replaced_by_certificate_id IS NULL
       AND (
            to_jsonb(NEW)
                - 'replacement_for_certificate_id'
                - 'replaced_by_certificate_id'
                - 'updated_at'
       ) = (
            to_jsonb(OLD)
                - 'replacement_for_certificate_id'
                - 'replaced_by_certificate_id'
                - 'updated_at'
       ) THEN
        RETURN NEW;
    END IF;

    IF ROW(
        NEW.id,
        NEW.campaign_id,
        NEW.template_id,
        NEW.candidate_id,
        NEW.issue_run_id,
        NEW.academic_year_id,
        NEW.academic_year_value,
        NEW.activity_sequence,
        NEW.certificate_sequence,
        NEW.check_digit,
        NEW.certificate_number,
        NEW.recipient_type,
        NEW.user_id,
        NEW.title_snapshot,
        NEW.first_name_snapshot,
        NEW.last_name_snapshot,
        NEW.template_name_snapshot,
        NEW.activity_item_snapshot,
        NEW.award_or_role_snapshot,
        NEW.custom_values_snapshot,
        NEW.school_name_snapshot,
        NEW.owner_organization_unit_name_snapshot,
        NEW.issue_date,
        NEW.qr_proof_encrypted,
        NEW.qr_proof_hash,
        NEW.replacement_for_certificate_id,
        NEW.created_at
    ) IS DISTINCT FROM ROW(
        OLD.id,
        OLD.campaign_id,
        OLD.template_id,
        OLD.candidate_id,
        OLD.issue_run_id,
        OLD.academic_year_id,
        OLD.academic_year_value,
        OLD.activity_sequence,
        OLD.certificate_sequence,
        OLD.check_digit,
        OLD.certificate_number,
        OLD.recipient_type,
        OLD.user_id,
        OLD.title_snapshot,
        OLD.first_name_snapshot,
        OLD.last_name_snapshot,
        OLD.template_name_snapshot,
        OLD.activity_item_snapshot,
        OLD.award_or_role_snapshot,
        OLD.custom_values_snapshot,
        OLD.school_name_snapshot,
        OLD.owner_organization_unit_name_snapshot,
        OLD.issue_date,
        OLD.qr_proof_encrypted,
        OLD.qr_proof_hash,
        OLD.replacement_for_certificate_id,
        OLD.created_at
    ) THEN
        RAISE EXCEPTION 'issued certificate snapshots are immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    IF OLD.status = 'revoked'
       AND ROW(
           NEW.status,
           NEW.revoked_by,
           NEW.revoked_at,
           NEW.revocation_reason
       ) IS DISTINCT FROM ROW(
           OLD.status,
           OLD.revoked_by,
           OLD.revoked_at,
           OLD.revocation_reason
       ) THEN
        RAISE EXCEPTION 'certificate revocation is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    IF OLD.replaced_by_certificate_id IS NOT NULL
       AND NEW.replaced_by_certificate_id IS DISTINCT FROM OLD.replaced_by_certificate_id THEN
        RAISE EXCEPTION 'certificate replacement link is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION prevent_certificate_delete()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF certificate_campaign_purge_guard_allows(OLD.campaign_id) THEN
        RETURN OLD;
    END IF;

    RAISE EXCEPTION 'issued certificates cannot be deleted'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE FUNCTION finalize_certificate_campaign_purge(p_campaign_id UUID)
RETURNS BOOLEAN
LANGUAGE plpgsql
AS $$
DECLARE
    campaign_status TEXT;
    purge_status TEXT;
    expected_file_count BIGINT;
    inventory_file_count BIGINT;
BEGIN
    SELECT campaign.status
    INTO campaign_status
    FROM certificate_campaigns AS campaign
    WHERE campaign.id = p_campaign_id
    FOR UPDATE;

    IF NOT FOUND THEN
        RETURN FALSE;
    END IF;

    SELECT job.status, job.file_count
    INTO purge_status, expected_file_count
    FROM certificate_campaign_purge_jobs AS job
    WHERE job.campaign_id = p_campaign_id
    FOR UPDATE;

    IF NOT FOUND THEN
        RETURN FALSE;
    END IF;

    IF campaign_status <> 'purging' OR purge_status <> 'finalizing' THEN
        RAISE EXCEPTION 'certificate campaign purge is not ready to finalize'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    PERFORM file.id
    FROM certificate_campaign_purge_files AS inventory
    JOIN files AS file ON file.id = inventory.file_id
    WHERE inventory.campaign_id = p_campaign_id
    ORDER BY file.id
    FOR UPDATE OF inventory, file;

    SELECT COUNT(*)
    INTO inventory_file_count
    FROM certificate_campaign_purge_files
    WHERE campaign_id = p_campaign_id;

    IF inventory_file_count <> expected_file_count THEN
        RAISE EXCEPTION 'certificate campaign purge inventory count changed'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    -- The file rows above are locked before the shared-consumer recheck. This
    -- serializes relational foreign-key attachments and prevents a finalizer
    -- from detaching or dangling any known File Platform consumer.
    IF certificate_campaign_purge_has_external_file_consumer(
        p_campaign_id,
        ARRAY(
            SELECT inventory.file_id
            FROM certificate_campaign_purge_files AS inventory
            WHERE inventory.campaign_id = p_campaign_id
            ORDER BY inventory.file_id
        )
    ) THEN
        RAISE EXCEPTION 'certificate_purge_file_shared'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM certificate_campaign_purge_files AS inventory
        JOIN files AS file ON file.id = inventory.file_id
        WHERE inventory.campaign_id = p_campaign_id
          AND (
              file.lifecycle_status <> 'deleted'
              OR file.deleted_at IS NULL
          )
    ) OR EXISTS (
        SELECT 1
        FROM certificate_campaign_purge_files AS inventory
        JOIN file_versions AS version ON version.file_id = inventory.file_id
        WHERE inventory.campaign_id = p_campaign_id
          AND version.storage_status <> 'deleted'
    ) OR EXISTS (
        SELECT 1
        FROM certificate_campaign_purge_files AS inventory
        JOIN file_derivatives AS derivative ON derivative.file_id = inventory.file_id
        WHERE inventory.campaign_id = p_campaign_id
          AND (
              derivative.storage_status <> 'deleted'
              OR derivative.lifecycle_status <> 'deleted'
          )
    ) THEN
        RAISE EXCEPTION 'certificate campaign storage deletion is incomplete'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    PERFORM set_config(
        'schoolorbit.certificate_purge_campaign_id',
        p_campaign_id::TEXT,
        TRUE
    );

    DELETE FROM audit_logs AS audit
    WHERE (
        audit.entity_type = 'certificate_campaign'
        AND audit.entity_id = p_campaign_id
    ) OR (
        audit.entity_type = 'certificate_template'
        AND audit.entity_id IN (
            SELECT template.id
            FROM certificate_templates AS template
            WHERE template.campaign_id = p_campaign_id
        )
    ) OR (
        audit.entity_type = 'certificate_candidate'
        AND (
            audit.entity_id = p_campaign_id
            OR audit.entity_id IN (
                SELECT candidate.id
                FROM certificate_candidates AS candidate
                WHERE candidate.campaign_id = p_campaign_id
            )
        )
    ) OR (
        audit.entity_type = 'certificate_issue_request'
        AND audit.entity_id IN (
            SELECT request.id
            FROM certificate_issue_requests AS request
            WHERE request.campaign_id = p_campaign_id
        )
    ) OR (
        audit.entity_type = 'certificate'
        AND audit.entity_id IN (
            SELECT certificate.id
            FROM certificates AS certificate
            WHERE certificate.campaign_id = p_campaign_id
        )
    ) OR (
        audit.entity_type LIKE 'certificate%'
        AND audit.metadata ->> 'campaignId' = p_campaign_id::TEXT
    );

    UPDATE certificate_candidates
    SET replacement_for_certificate_id = NULL,
        issued_certificate_id = NULL
    WHERE campaign_id = p_campaign_id;

    UPDATE certificates
    SET replacement_for_certificate_id = NULL,
        replaced_by_certificate_id = NULL
    WHERE campaign_id = p_campaign_id;

    DELETE FROM certificate_issue_run_problems AS problem
    USING certificate_candidates AS candidate
    WHERE problem.candidate_id = candidate.id
      AND candidate.campaign_id = p_campaign_id;

    DELETE FROM certificate_candidate_issue_locks AS candidate_lock
    USING certificate_candidates AS candidate
    WHERE candidate_lock.candidate_id = candidate.id
      AND candidate.campaign_id = p_campaign_id;

    DELETE FROM certificate_issue_request_items
    WHERE campaign_id = p_campaign_id;

    DELETE FROM certificates
    WHERE campaign_id = p_campaign_id;

    DELETE FROM certificate_issue_runs AS issue_run
    USING certificate_issue_requests AS request
    WHERE issue_run.request_id = request.id
      AND request.campaign_id = p_campaign_id;

    DELETE FROM certificate_issue_requests
    WHERE campaign_id = p_campaign_id;

    DELETE FROM certificate_candidates
    WHERE campaign_id = p_campaign_id;

    DELETE FROM certificate_import_batches
    WHERE campaign_id = p_campaign_id;

    DELETE FROM certificate_template_assets AS asset
    USING certificate_templates AS template
    WHERE asset.template_id = template.id
      AND template.campaign_id = p_campaign_id;

    DELETE FROM certificate_template_file_uploads AS upload
    USING certificate_templates AS template
    WHERE upload.template_id = template.id
      AND template.campaign_id = p_campaign_id;

    DELETE FROM certificate_templates
    WHERE campaign_id = p_campaign_id;

    DELETE FROM file_operations AS operation
    USING certificate_campaign_purge_files AS inventory
    WHERE operation.file_id = inventory.file_id
      AND inventory.campaign_id = p_campaign_id;

    UPDATE files AS file
    SET current_version_id = NULL
    FROM certificate_campaign_purge_files AS inventory
    WHERE file.id = inventory.file_id
      AND inventory.campaign_id = p_campaign_id;

    DELETE FROM file_derivatives AS derivative
    USING certificate_campaign_purge_files AS inventory
    WHERE derivative.file_id = inventory.file_id
      AND inventory.campaign_id = p_campaign_id;

    DELETE FROM file_versions AS version
    USING certificate_campaign_purge_files AS inventory
    WHERE version.file_id = inventory.file_id
      AND inventory.campaign_id = p_campaign_id;

    DELETE FROM files AS file
    USING certificate_campaign_purge_files AS inventory
    WHERE file.id = inventory.file_id
      AND inventory.campaign_id = p_campaign_id;

    DELETE FROM certificate_campaigns
    WHERE id = p_campaign_id;

    RETURN TRUE;
END;
$$;

COMMENT ON FUNCTION finalize_certificate_campaign_purge(UUID) IS
    'Atomically removes one purging certificate campaign after File Platform confirms every object deleted.';
