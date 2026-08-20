-- Preserve the exact per-candidate revalidation result returned by an idempotent
-- certificate issue command. Issue runs and their problem rows are immutable history.

CREATE TABLE certificate_issue_run_problems (
    issue_run_id UUID NOT NULL
        REFERENCES certificate_issue_runs(id) ON DELETE RESTRICT,
    candidate_id UUID NOT NULL
        REFERENCES certificate_candidates(id) ON DELETE RESTRICT,
    issue_codes TEXT[] NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (issue_run_id, candidate_id),
    CONSTRAINT certificate_issue_run_problems_codes_check CHECK (
        cardinality(issue_codes) BETWEEN 1 AND 64
        AND array_position(issue_codes, NULL) IS NULL
    )
);

CREATE INDEX certificate_issue_run_problems_candidate_idx
    ON certificate_issue_run_problems (candidate_id, issue_run_id);

CREATE FUNCTION prevent_certificate_issue_run_problem_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'certificate issue run problems are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER prevent_certificate_issue_run_problem_update
    BEFORE UPDATE ON certificate_issue_run_problems
    FOR EACH ROW
    EXECUTE FUNCTION prevent_certificate_issue_run_problem_mutation();

CREATE TRIGGER prevent_certificate_issue_run_problem_delete
    BEFORE DELETE ON certificate_issue_run_problems
    FOR EACH ROW
    EXECUTE FUNCTION prevent_certificate_issue_run_problem_mutation();
