-- Durable fail-closed barrier for runtime directory status/credential mutations.
CREATE TABLE tenant_directory_mutations (
    school_id UUID PRIMARY KEY REFERENCES schools(id),
    token UUID NOT NULL UNIQUE,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
