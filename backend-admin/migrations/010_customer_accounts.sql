-- Explicitly authorized replacement of central accounts only. Never touch tenants.
CREATE TEMP TABLE admin_portal_preservation ON COMMIT DROP AS
SELECT
    (SELECT COALESCE(jsonb_agg(to_jsonb(s) ORDER BY id), '[]'::jsonb) FROM schools s) AS schools,
    (SELECT COALESCE(jsonb_agg(to_jsonb(d) ORDER BY id), '[]'::jsonb) FROM deployment_history d) AS deployments;

CREATE TABLE platform_accounts (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    email TEXT NOT NULL UNIQUE CHECK (email = lower(btrim(email))),
    password_hash TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    role TEXT NOT NULL DEFAULT 'customer' CHECK (role IN ('operator', 'customer')),
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE platform_sessions (
    token_hash TEXT PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES platform_accounts(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX platform_sessions_account ON platform_sessions(account_id);
CREATE INDEX platform_sessions_expiry ON platform_sessions(expires_at);
CREATE TABLE platform_email_tokens (
    token_hash TEXT PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES platform_accounts(id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN ('verify', 'reset')),
    expires_at TIMESTAMPTZ NOT NULL,
    UNIQUE (account_id, purpose)
);
CREATE TABLE platform_auth_buckets (
    bucket TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    attempts INTEGER NOT NULL,
    PRIMARY KEY (bucket, window_start)
);
CREATE INDEX platform_auth_buckets_expiry ON platform_auth_buckets(window_start);
CREATE TABLE school_memberships (
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE RESTRICT,
    account_id UUID NOT NULL REFERENCES platform_accounts(id) ON DELETE RESTRICT,
    granted_by UUID NOT NULL REFERENCES platform_accounts(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (school_id, account_id)
);
CREATE INDEX school_memberships_account ON school_memberships(account_id);
CREATE TABLE school_service_periods (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE RESTRICT,
    cadence TEXT NOT NULL CHECK (cadence IN ('monthly', 'yearly')),
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL CHECK (ends_at > starts_at),
    granted_by UUID NOT NULL REFERENCES platform_accounts(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX school_service_periods_school ON school_service_periods(school_id, ends_at DESC);
CREATE TABLE school_service_requests (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES platform_accounts(id) ON DELETE RESTRICT,
    school_id UUID REFERENCES schools(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('open_school', 'renew_school')),
    school_name TEXT NOT NULL,
    subdomain TEXT NOT NULL,
    cadence TEXT NOT NULL CHECK (cadence IN ('monthly', 'yearly')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    decided_by UUID REFERENCES platform_accounts(id) ON DELETE RESTRICT,
    decision_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ,
    CHECK ((kind = 'renew_school' AND school_id IS NOT NULL) OR kind = 'open_school')
);
CREATE INDEX school_service_requests_account ON school_service_requests(account_id, created_at DESC);
CREATE INDEX school_service_requests_pending ON school_service_requests(created_at) WHERE status = 'pending';

-- No CASCADE: an unexpected legacy dependency must stop the migration.
DROP TABLE admin_users;
DO $$ BEGIN
    IF (SELECT schools FROM admin_portal_preservation) IS DISTINCT FROM
       (SELECT COALESCE(jsonb_agg(to_jsonb(s) ORDER BY id), '[]'::jsonb) FROM schools s)
    OR (SELECT deployments FROM admin_portal_preservation) IS DISTINCT FROM
       (SELECT COALESCE(jsonb_agg(to_jsonb(d) ORDER BY id), '[]'::jsonb) FROM deployment_history d)
    THEN RAISE EXCEPTION 'ADMIN_PORTAL_SCHOOL_PRESERVATION_FAILED';
    END IF;
END $$;
