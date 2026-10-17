-- Central delivery owns transport state; domain ledgers retain business ownership.
-- This expansion is structurally additive, but strict migration-history readiness
-- still requires a binary compiled for the complete applied timeline.
CREATE TABLE notification_dispatch_jobs (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    notification_id UUID NOT NULL UNIQUE,
    recipient_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('direct', 'attendance')),
    source_id UUID,
    dedup_key TEXT,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('info', 'success', 'warning', 'error')),
    link TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    stored_at TIMESTAMPTZ,
    published_at TIMESTAMPTZ,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    lease_token UUID,
    CHECK ((source_kind = 'direct' AND source_id IS NULL)
        OR (source_kind = 'attendance' AND source_id IS NOT NULL)),
    UNIQUE (source_kind, recipient_id, dedup_key)
);

CREATE UNIQUE INDEX notification_dispatch_source_idx
    ON notification_dispatch_jobs(source_kind, source_id) WHERE source_id IS NOT NULL;
CREATE INDEX notification_dispatch_due_idx
    ON notification_dispatch_jobs(next_attempt_at, id) WHERE published_at IS NULL;
