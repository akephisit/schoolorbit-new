CREATE TABLE academic_promotion_run_approvals (
    id UUID PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES academic_promotion_runs(id) ON DELETE RESTRICT,
    approved_run_version BIGINT NOT NULL CHECK (approved_run_version > 0),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    intent JSONB NOT NULL CHECK (jsonb_typeof(intent)='object'),
    approved_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    approved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (id,run_id),
    UNIQUE (run_id,approved_run_version)
);
CREATE TRIGGER academic_promotion_run_approvals_immutable
BEFORE UPDATE OR DELETE ON academic_promotion_run_approvals
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();

ALTER TABLE academic_promotion_runs
    ADD COLUMN approval_id UUID,
    ADD CONSTRAINT academic_promotion_run_approval_owner
        FOREIGN KEY (approval_id,id) REFERENCES academic_promotion_run_approvals(id,run_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_promotion_run_approval_actor
        CHECK ((approval_id IS NULL) = (approved_by IS NULL)),
    ADD CONSTRAINT academic_promotion_run_execution_requires_approval
        CHECK (status NOT IN ('approved','executing','completed') OR approval_id IS NOT NULL);
