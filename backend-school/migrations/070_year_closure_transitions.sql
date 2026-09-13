CREATE TABLE academic_year_transition_receipts (
    request_id UUID PRIMARY KEY,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    action TEXT NOT NULL CHECK (action IN ('mark_ready','begin_closing','cancel_closing','close','reopen','activate')),
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    accepted_readiness JSONB NOT NULL CHECK (jsonb_typeof(accepted_readiness)='object'),
    outcome JSONB NOT NULL CHECK (jsonb_typeof(outcome)='object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX academic_year_transition_receipts_year_idx
    ON academic_year_transition_receipts(academic_year_id,created_at);
CREATE TRIGGER academic_year_transition_receipts_immutable
BEFORE UPDATE OR DELETE ON academic_year_transition_receipts
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();

-- Closing retains ownership of the running year, just as it does for a term.
CREATE UNIQUE INDEX academic_years_one_running_context
    ON academic_years ((true)) WHERE status IN ('active','closing');
