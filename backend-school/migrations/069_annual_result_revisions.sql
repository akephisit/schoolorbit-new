-- Annual evidence references immutable term revisions; no legacy grade backfill.
CREATE TABLE academic_annual_result_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_academic_year_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    request_id UUID NOT NULL UNIQUE,
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    snapshot JSONB NOT NULL CHECK (jsonb_typeof(snapshot)='object'),
    official_gpa NUMERIC(3,2) CHECK (official_gpa BETWEEN 0 AND 4),
    hold_reason TEXT CHECK (length(btrim(hold_reason)) BETWEEN 1 AND 1000),
    locked_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    locked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(student_academic_year_id,academic_year_id,revision),
    UNIQUE(id,academic_year_id,student_academic_year_id),
    FOREIGN KEY(student_academic_year_id,academic_year_id)
        REFERENCES student_academic_years(id,academic_year_id) ON DELETE RESTRICT,
    CHECK(hold_reason IS NULL OR official_gpa IS NULL)
);
CREATE TRIGGER academic_annual_result_revisions_immutable
BEFORE UPDATE OR DELETE ON academic_annual_result_revisions
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();

CREATE TABLE academic_annual_result_term_sources (
    annual_revision_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    term_aggregate_revision_id UUID NOT NULL,
    PRIMARY KEY(annual_revision_id,academic_term_id),
    FOREIGN KEY(annual_revision_id,academic_year_id,student_academic_year_id)
        REFERENCES academic_annual_result_revisions(id,academic_year_id,student_academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY(term_aggregate_revision_id,academic_year_id,academic_term_id,student_academic_year_id)
        REFERENCES academic_term_aggregate_revisions(id,academic_year_id,academic_term_id,student_academic_year_id) ON DELETE RESTRICT
);
CREATE INDEX academic_annual_result_term_sources_context_idx
    ON academic_annual_result_term_sources(academic_year_id,academic_term_id);
CREATE TRIGGER academic_annual_result_term_sources_immutable
BEFORE UPDATE OR DELETE ON academic_annual_result_term_sources
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
