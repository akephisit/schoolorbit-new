ALTER TABLE academic_promotion_run_items
    ADD CONSTRAINT academic_promotion_run_items_id_run_key UNIQUE (id, run_id);

CREATE TABLE academic_promotion_impact_resolutions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL UNIQUE,
    run_id UUID NOT NULL REFERENCES academic_promotion_runs(id) ON DELETE RESTRICT,
    item_id UUID NOT NULL,
    correction_id UUID NOT NULL REFERENCES academic_result_corrections(id) ON DELETE RESTRICT,
    impact_id UUID NOT NULL UNIQUE,
    resolution_kind TEXT NOT NULL
        CHECK (resolution_kind IN ('keep_existing', 'replace_decision')),
    replacement_decision JSONB
        CHECK (replacement_decision IS NULL OR jsonb_typeof(replacement_decision) = 'object'),
    reason TEXT NOT NULL CHECK (length(btrim(reason)) BETWEEN 1 AND 1000),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    outcome JSONB NOT NULL CHECK (jsonb_typeof(outcome) = 'object'),
    resolved_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    resolved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (item_id, run_id)
        REFERENCES academic_promotion_run_items(id, run_id) ON DELETE RESTRICT,
    UNIQUE (item_id, correction_id),
    CHECK (
        (resolution_kind = 'keep_existing' AND replacement_decision IS NULL)
        OR (resolution_kind = 'replace_decision' AND replacement_decision IS NOT NULL)
    )
);

CREATE INDEX academic_promotion_impact_resolutions_run_idx
    ON academic_promotion_impact_resolutions(run_id, item_id, resolved_at);

CREATE TRIGGER academic_promotion_impact_resolutions_immutable
BEFORE UPDATE OR DELETE ON academic_promotion_impact_resolutions
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
