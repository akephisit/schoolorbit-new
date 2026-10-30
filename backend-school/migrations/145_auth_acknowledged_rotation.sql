ALTER TABLE auth_sessions
    ADD COLUMN pending_token_hash BYTEA,
    ADD COLUMN pending_created_at TIMESTAMPTZ,
    ADD CONSTRAINT auth_sessions_pending_pair_check
        CHECK ((pending_token_hash IS NULL) = (pending_created_at IS NULL)),
    ADD CONSTRAINT auth_sessions_pending_hash_check
        CHECK (pending_token_hash IS NULL OR octet_length(pending_token_hash) = 32),
    ADD CONSTRAINT auth_sessions_pending_time_check
        CHECK (pending_created_at IS NULL OR
            (pending_created_at >= rotated_at AND pending_created_at < absolute_expires_at)),
    ADD CONSTRAINT auth_sessions_pending_revocation_check
        CHECK (revoked_at IS NULL OR pending_token_hash IS NULL);

CREATE UNIQUE INDEX auth_sessions_pending_token_hash_key
    ON auth_sessions (pending_token_hash)
    WHERE pending_token_hash IS NOT NULL;
