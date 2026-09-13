CREATE TABLE academic_promotion_run_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL,
    source_year_id UUID NOT NULL,
    target_year_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    student_id UUID NOT NULL,
    source_grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
    source_study_program_id UUID NOT NULL REFERENCES study_programs(id) ON DELETE RESTRICT,
    source_row_version BIGINT NOT NULL CHECK (source_row_version > 0),
    annual_revision_id UUID,
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    recommendation JSONB NOT NULL CHECK (jsonb_typeof(recommendation)='object'),
    existing_target_student_year_id UUID,
    decision JSONB CHECK (jsonb_typeof(decision)='object'),
    reviewed_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    reviewed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'calculated' CHECK (status IN ('calculated','reviewed','executed','failed')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(run_id,student_academic_year_id),
    UNIQUE(id,run_id,source_year_id,target_year_id,student_id),
    FOREIGN KEY(run_id,source_year_id,target_year_id)
        REFERENCES academic_promotion_runs(id,source_year_id,target_year_id) ON DELETE RESTRICT,
    FOREIGN KEY(student_academic_year_id,source_year_id,student_id)
        REFERENCES student_academic_years(id,academic_year_id,student_id) ON DELETE RESTRICT,
    FOREIGN KEY(annual_revision_id,source_year_id,student_academic_year_id)
        REFERENCES academic_annual_result_revisions(id,academic_year_id,student_academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY(existing_target_student_year_id,target_year_id,student_id)
        REFERENCES student_academic_years(id,academic_year_id,student_id) ON DELETE RESTRICT,
    CHECK ((reviewed_by IS NULL) = (reviewed_at IS NULL)),
    CHECK (status='calculated' OR (decision IS NOT NULL AND reviewed_by IS NOT NULL))
);
CREATE INDEX academic_promotion_run_items_source_idx
    ON academic_promotion_run_items(source_year_id,student_academic_year_id);

CREATE FUNCTION academic_protect_promotion_item_identity() RETURNS trigger AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'PROMOTION_ITEM_HISTORY_RETAINED' USING ERRCODE='23514';
    END IF;
    IF OLD.status='executed' OR
       (NEW.id,NEW.run_id,NEW.source_year_id,NEW.target_year_id,
        NEW.student_academic_year_id,NEW.student_id,NEW.created_at)
       IS DISTINCT FROM
       (OLD.id,OLD.run_id,OLD.source_year_id,OLD.target_year_id,
        OLD.student_academic_year_id,OLD.student_id,OLD.created_at) THEN
        RAISE EXCEPTION 'PROMOTION_ITEM_IDENTITY_IMMUTABLE' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER academic_promotion_run_items_identity
BEFORE UPDATE OR DELETE ON academic_promotion_run_items
FOR EACH ROW EXECUTE FUNCTION academic_protect_promotion_item_identity();

CREATE TABLE academic_promotion_run_commands (
    request_id UUID PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES academic_promotion_runs(id) ON DELETE RESTRICT,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    action TEXT NOT NULL CHECK (action IN ('calculate','approve','execute')),
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    outcome JSONB NOT NULL CHECK (jsonb_typeof(outcome)='object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER academic_promotion_run_commands_immutable
BEFORE UPDATE OR DELETE ON academic_promotion_run_commands
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
