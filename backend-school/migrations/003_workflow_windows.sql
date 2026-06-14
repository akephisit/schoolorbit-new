CREATE TABLE workflow_windows (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    module_code character varying(100) NOT NULL,
    workflow_code character varying(100) NOT NULL,
    title character varying(200) NOT NULL,
    description text,
    organization_unit_id uuid,
    managed_by_permission character varying(100) NOT NULL,
    opens_at timestamp with time zone,
    due_at timestamp with time zone,
    closes_at timestamp with time zone,
    status character varying(20) DEFAULT 'draft'::character varying NOT NULL,
    metadata jsonb DEFAULT '{"tags":[]}'::jsonb NOT NULL,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT workflow_windows_status_check CHECK (
        status IN ('draft', 'open', 'closed', 'archived')
    )
);

ALTER TABLE ONLY workflow_windows
ADD CONSTRAINT workflow_windows_pkey PRIMARY KEY (id);

ALTER TABLE ONLY workflow_windows
ADD CONSTRAINT workflow_windows_organization_unit_id_fkey
FOREIGN KEY (organization_unit_id) REFERENCES organization_units(id) ON DELETE SET NULL;

ALTER TABLE ONLY workflow_windows
ADD CONSTRAINT workflow_windows_created_by_fkey
FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX idx_workflow_windows_module_code ON workflow_windows (module_code);
CREATE INDEX idx_workflow_windows_workflow_code ON workflow_windows (workflow_code);
CREATE INDEX idx_workflow_windows_status ON workflow_windows (status);
CREATE INDEX idx_workflow_windows_organization_unit ON workflow_windows (organization_unit_id);
CREATE INDEX idx_workflow_windows_schedule ON workflow_windows (opens_at, due_at, closes_at);

CREATE TRIGGER update_workflow_windows_updated_at
BEFORE UPDATE ON workflow_windows
FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

COMMENT ON TABLE workflow_windows IS 'Time-bounded school workflow windows such as score submission, club registration, lesson-plan submission, and document collection.';
COMMENT ON COLUMN workflow_windows.managed_by_permission IS 'Permission code required to manage this workflow window.';
COMMENT ON COLUMN workflow_windows.metadata IS 'Flexible non-sensitive workflow metadata. Do not store PII or plaintext document identifiers.';
