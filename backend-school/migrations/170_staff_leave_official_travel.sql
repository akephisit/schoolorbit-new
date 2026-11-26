ALTER TABLE staff_leave_types DROP CONSTRAINT staff_leave_types_kind_check;
ALTER TABLE staff_leave_types ADD CONSTRAINT staff_leave_types_kind_check
    CHECK (kind IN ('leave', 'exit', 'official_travel'));

CREATE INDEX staff_leave_companions_idx ON staff_leave_requests
    USING gin ((document->'companions') jsonb_path_ops)
    WHERE document ? 'companions';

CREATE TABLE staff_leave_companion_confirmations (
    request_id uuid NOT NULL,
    submission_revision bigint NOT NULL,
    user_id uuid NOT NULL REFERENCES users(id),
    confirmed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (request_id, submission_revision, user_id),
    FOREIGN KEY (request_id, submission_revision) REFERENCES staff_leave_revisions(request_id, revision)
);
CREATE TRIGGER preserve_staff_leave_companion_confirmations
    BEFORE UPDATE OR DELETE ON staff_leave_companion_confirmations
    FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
