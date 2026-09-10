-- Policies are explicitly approved versions; there is no inferred default policy.
CREATE TABLE academic_aggregate_policy_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 160),
    passing_grade NUMERIC(3,2) NOT NULL CHECK (
        passing_grade > 0 AND passing_grade <= 4 AND mod(passing_grade * 2, 1) = 0
    ),
    minimum_learner_level SMALLINT NOT NULL CHECK (minimum_learner_level BETWEEN 0 AND 3),
    allow_reviewed_holds BOOLEAN NOT NULL,
    approved_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    approved_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER academic_aggregate_policy_versions_immutable
BEFORE UPDATE OR DELETE ON academic_aggregate_policy_versions
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();

CREATE TABLE academic_term_aggregate_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_academic_year_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    policy_id UUID NOT NULL REFERENCES academic_aggregate_policy_versions(id) ON DELETE RESTRICT,
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    request_id UUID NOT NULL UNIQUE,
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    snapshot JSONB NOT NULL CHECK (jsonb_typeof(snapshot) = 'object'),
    official_gpa NUMERIC(3,2) CHECK (official_gpa BETWEEN 0 AND 4),
    hold_reason TEXT CHECK (length(btrim(hold_reason)) BETWEEN 1 AND 1000),
    locked_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    locked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (student_academic_year_id, academic_term_id, revision),
    UNIQUE (id, academic_year_id, academic_term_id, student_academic_year_id),
    FOREIGN KEY (student_academic_year_id, academic_year_id)
        REFERENCES student_academic_years(id, academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    CHECK (hold_reason IS NULL OR official_gpa IS NULL)
);
CREATE TRIGGER academic_term_aggregate_revisions_immutable
BEFORE UPDATE OR DELETE ON academic_term_aggregate_revisions
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
