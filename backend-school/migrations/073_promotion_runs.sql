CREATE TABLE academic_promotion_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    target_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    policy_id UUID NOT NULL REFERENCES academic_promotion_policy_versions(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft','calculated','reviewed','approved','executing','completed','failed')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    request_id UUID NOT NULL UNIQUE,
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    reviewed_at TIMESTAMPTZ,
    approved_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    approved_at TIMESTAMPTZ,
    executed_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    executed_at TIMESTAMPTZ,
    CHECK (source_year_id <> target_year_id),
    CHECK ((reviewed_by IS NULL) = (reviewed_at IS NULL)),
    CHECK ((approved_by IS NULL) = (approved_at IS NULL)),
    CHECK ((executed_by IS NULL) = (executed_at IS NULL)),
    UNIQUE (id, source_year_id, target_year_id)
);
CREATE INDEX academic_promotion_runs_years_idx
    ON academic_promotion_runs(source_year_id,target_year_id,created_at DESC,id);

CREATE FUNCTION academic_protect_promotion_run_identity() RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'PROMOTION_RUN_HISTORY_RETAINED' USING ERRCODE = '23514';
    END IF;
    IF (NEW.id,NEW.source_year_id,NEW.target_year_id,NEW.policy_id,
        NEW.request_id,NEW.request_checksum,NEW.created_by,NEW.created_at)
       IS DISTINCT FROM
       (OLD.id,OLD.source_year_id,OLD.target_year_id,OLD.policy_id,
        OLD.request_id,OLD.request_checksum,OLD.created_by,OLD.created_at) THEN
        RAISE EXCEPTION 'PROMOTION_RUN_IDENTITY_IMMUTABLE' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER academic_promotion_runs_identity
BEFORE UPDATE OR DELETE ON academic_promotion_runs
FOR EACH ROW EXECUTE FUNCTION academic_protect_promotion_run_identity();
