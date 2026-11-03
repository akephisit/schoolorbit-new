-- Retention is opt-in. Deployment never queues or removes existing detail.
CREATE TABLE academic_retention_policy (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    enabled boolean NOT NULL DEFAULT false,
    primary_boundary text NOT NULL DEFAULT 'year' CHECK(primary_boundary IN('term','year')),
    secondary_boundary text NOT NULL DEFAULT 'term' CHECK(secondary_boundary IN('term','year')),
    grace_days integer NOT NULL DEFAULT 90 CHECK(grace_days BETWEEN 1 AND 3650),
    effective_from timestamptz,
    row_version bigint NOT NULL DEFAULT 1 CHECK(row_version > 0),
    updated_by uuid REFERENCES users(id) ON DELETE RESTRICT,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK(NOT enabled OR (effective_from IS NOT NULL AND updated_by IS NOT NULL))
);
INSERT INTO academic_retention_policy(singleton) VALUES(true);

CREATE TABLE academic_retention_scan_state (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    after_closed_at timestamptz,
    after_request_id uuid,
    CHECK((after_closed_at IS NULL)=(after_request_id IS NULL))
);
INSERT INTO academic_retention_scan_state(singleton) VALUES(true);
CREATE INDEX academic_term_close_retention_scan
    ON academic_term_transition_receipts(created_at,request_id) WHERE action='close';

CREATE TABLE academic_retention_jobs (
    id uuid PRIMARY KEY,
    academic_term_id uuid NOT NULL REFERENCES academic_terms(id) ON DELETE RESTRICT,
    academic_year_id uuid NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    close_request_id uuid NOT NULL REFERENCES academic_term_transition_receipts(request_id) ON DELETE RESTRICT,
    policy_revision bigint NOT NULL CHECK(policy_revision > 0),
    policy_snapshot jsonb NOT NULL CHECK(jsonb_typeof(policy_snapshot)='object'),
    authorized_by uuid NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    eligible_at timestamptz NOT NULL,
    status text NOT NULL DEFAULT 'awaiting_attendance'
        CHECK(status IN('awaiting_attendance','awaiting_behavior','awaiting_files','completed','cancelled')),
    evidence_file_ids uuid[] NOT NULL DEFAULT '{}',
    behavior_job_id uuid,
    pending_files bigint NOT NULL DEFAULT 0 CHECK(pending_files >= 0),
    failed_files bigint NOT NULL DEFAULT 0 CHECK(failed_files >= 0),
    started_at timestamptz,
    completed_at timestamptz,
    attempt_count bigint NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
    last_error_code text CHECK(last_error_code ~ '^[a-z0-9_]{1,64}$'),
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    lease_token uuid,
    leased_until timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(academic_term_id,close_request_id),
    FOREIGN KEY(academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT,
    CHECK((lease_token IS NULL)=(leased_until IS NULL))
);
CREATE INDEX academic_retention_jobs_due ON academic_retention_jobs(next_attempt_at,created_at)
    WHERE status NOT IN('completed','cancelled');

-- Behavior owns replay of its accepted details-only provider operation.
CREATE TABLE behavior_retention_receipts (
    request_id uuid PRIMARY KEY,
    academic_term_id uuid NOT NULL REFERENCES academic_terms(id) ON DELETE RESTRICT,
    actor_id uuid NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    outcome jsonb NOT NULL CHECK(jsonb_typeof(outcome)='object'),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER behavior_retention_receipts_immutable
BEFORE UPDATE OR DELETE ON behavior_retention_receipts
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
