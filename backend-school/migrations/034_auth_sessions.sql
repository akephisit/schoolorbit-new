CREATE TABLE auth_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    current_token_hash BYTEA NOT NULL CHECK (octet_length(current_token_hash) = 32),
    previous_token_hash BYTEA,
    previous_token_valid_until TIMESTAMPTZ,
    remember_me BOOLEAN NOT NULL,
    device_label TEXT NOT NULL CHECK (btrim(device_label) <> ''),
    created_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL,
    idle_expires_at TIMESTAMPTZ NOT NULL,
    absolute_expires_at TIMESTAMPTZ NOT NULL,
    rotated_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revocation_reason TEXT,
    CHECK (idle_expires_at > created_at),
    CHECK (absolute_expires_at > created_at),
    CHECK (last_seen_at >= created_at),
    CHECK (rotated_at >= created_at),
    CHECK (idle_expires_at <= absolute_expires_at),
    CHECK ((previous_token_hash IS NULL) = (previous_token_valid_until IS NULL)),
    CHECK (previous_token_hash IS NULL OR octet_length(previous_token_hash) = 32),
    CHECK (previous_token_valid_until IS NULL OR previous_token_valid_until > rotated_at),
    CHECK (revocation_reason IS NULL OR revoked_at IS NOT NULL)
);

CREATE UNIQUE INDEX auth_sessions_current_token_hash_key
    ON auth_sessions (current_token_hash);

CREATE UNIQUE INDEX auth_sessions_previous_token_hash_key
    ON auth_sessions (previous_token_hash)
    WHERE previous_token_hash IS NOT NULL;

CREATE INDEX auth_sessions_active_user_expiry_idx
    ON auth_sessions (user_id, absolute_expires_at, id)
    WHERE revoked_at IS NULL;

CREATE INDEX auth_sessions_cleanup_idx
    ON auth_sessions (
        (COALESCE(revoked_at, LEAST(idle_expires_at, absolute_expires_at))),
        id
    );

CREATE TABLE auth_login_throttles (
    bucket_kind TEXT NOT NULL CHECK (bucket_kind IN ('identifier', 'source')),
    bucket_hash BYTEA NOT NULL CHECK (octet_length(bucket_hash) = 32),
    failure_count INTEGER NOT NULL CHECK (failure_count >= 0),
    window_started_at TIMESTAMPTZ NOT NULL,
    blocked_until TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (bucket_kind, bucket_hash),
    CHECK (updated_at >= window_started_at),
    CHECK (blocked_until IS NULL OR blocked_until >= window_started_at)
);

CREATE INDEX auth_login_throttles_cleanup_idx
    ON auth_login_throttles (updated_at, bucket_kind, bucket_hash);
