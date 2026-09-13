-- Immutable receipts for explicit, selected-module future-term preparation.
-- Module-owned configuration is created as drafts; this table is evidence only.

CREATE TABLE academic_term_preparation_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL UNIQUE,
    source_academic_term_id UUID NOT NULL,
    source_academic_year_id UUID NOT NULL,
    target_academic_term_id UUID NOT NULL,
    target_academic_year_id UUID NOT NULL,
    selected_modules TEXT[] NOT NULL,
    mappings JSONB NOT NULL DEFAULT '{"entities":[],"dates":[]}'::jsonb,
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    outcome JSONB NOT NULL CHECK (jsonb_typeof(outcome) = 'array'),
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_term_preparation_runs_source_context_fkey
        FOREIGN KEY (source_academic_term_id, source_academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT academic_term_preparation_runs_target_context_fkey
        FOREIGN KEY (target_academic_term_id, target_academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT academic_term_preparation_runs_distinct_terms_check
        CHECK (source_academic_term_id <> target_academic_term_id),
    CONSTRAINT academic_term_preparation_runs_modules_check CHECK (
        cardinality(selected_modules) BETWEEN 1 AND 5
        AND selected_modules <@ ARRAY[
            'delivery', 'assessments', 'timetable', 'exams', 'supervision'
        ]::text[]
    )
);

CREATE INDEX academic_term_preparation_runs_target_idx
    ON academic_term_preparation_runs(target_academic_term_id, created_at DESC, id);

CREATE TRIGGER academic_term_preparation_runs_immutable
BEFORE UPDATE OR DELETE ON academic_term_preparation_runs
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
