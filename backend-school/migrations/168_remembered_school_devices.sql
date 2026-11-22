-- Additive cutover: existing remember_me sessions and subscriptions remain finite.
-- Once durable subscriptions are written, use forward recovery, never an old binary.
CREATE TABLE auth_remembered_devices (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    current_token_hash BYTEA NOT NULL CHECK (octet_length(current_token_hash) = 32),
    pending_token_hash BYTEA CHECK (pending_token_hash IS NULL OR octet_length(pending_token_hash) = 32),
    pending_created_at TIMESTAMPTZ,
    pending_session_id UUID,
    device_label TEXT NOT NULL CHECK (btrim(device_label) <> ''),
    created_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revocation_reason TEXT,
    UNIQUE (id, user_id),
    CHECK (last_used_at >= created_at),
    CHECK ((pending_token_hash IS NULL) = (pending_created_at IS NULL)),
    CHECK ((pending_token_hash IS NULL) = (pending_session_id IS NULL)),
    CHECK (pending_created_at IS NULL OR pending_created_at >= created_at),
    CHECK (revocation_reason IS NULL OR revoked_at IS NOT NULL),
    CHECK (revoked_at IS NULL OR pending_token_hash IS NULL)
);
CREATE UNIQUE INDEX auth_remembered_devices_current_hash_key ON auth_remembered_devices(current_token_hash);
CREATE UNIQUE INDEX auth_remembered_devices_pending_hash_key ON auth_remembered_devices(pending_token_hash)
    WHERE pending_token_hash IS NOT NULL;
CREATE INDEX auth_remembered_devices_active_user_idx ON auth_remembered_devices(user_id,id)
    WHERE revoked_at IS NULL;
ALTER TABLE auth_sessions ADD COLUMN remembered_device_id UUID,
    ADD CONSTRAINT auth_sessions_remembered_device_owner_fk
    FOREIGN KEY (remembered_device_id,user_id) REFERENCES auth_remembered_devices(id,user_id) ON DELETE CASCADE;
CREATE INDEX auth_sessions_remembered_device_idx ON auth_sessions(remembered_device_id,id)
    WHERE remembered_device_id IS NOT NULL;
-- pending_session_id is generation metadata, deliberately retained after finite-session cleanup.
-- It carries no bearer secret and lets the next restore replace an expired/deleted generation.
ALTER TABLE push_subscriptions ADD COLUMN remembered_device_id UUID,
    ALTER COLUMN expires_at DROP NOT NULL,
    ADD CONSTRAINT push_subscriptions_remembered_device_owner_fk
    FOREIGN KEY (remembered_device_id,user_id) REFERENCES auth_remembered_devices(id,user_id) ON DELETE CASCADE,
    ADD CONSTRAINT push_subscriptions_remembered_owner_check CHECK (
        remembered_device_id IS NULL OR (session_id IS NULL AND expires_at IS NULL)
    );
CREATE INDEX push_subscriptions_remembered_device_idx ON push_subscriptions(remembered_device_id,id)
    WHERE remembered_device_id IS NOT NULL;
CREATE VIEW notification_eligible_push_subscriptions WITH (security_invoker=true) AS
SELECT p.* FROM push_subscriptions p
JOIN users u ON u.id=p.user_id AND u.status='active'
WHERE (
    p.remembered_device_id IS NOT NULL AND p.session_id IS NULL AND p.expires_at IS NULL
    AND EXISTS (SELECT 1 FROM auth_remembered_devices g
        WHERE g.id=p.remembered_device_id AND g.user_id=p.user_id AND g.revoked_at IS NULL)
) OR (
    p.remembered_device_id IS NULL AND p.session_id IS NOT NULL AND p.expires_at>now()
    AND EXISTS (SELECT 1 FROM auth_sessions s WHERE s.id=p.session_id AND s.user_id=p.user_id
        AND s.revoked_at IS NULL AND s.idle_expires_at>now() AND s.absolute_expires_at>now())
);
-- Covers every account owner, including future status writers. The user row is already locked.
CREATE FUNCTION revoke_remembered_devices_on_account_deactivation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status='active' AND NEW.status IS DISTINCT FROM 'active' THEN
        UPDATE auth_remembered_devices SET revoked_at=now(),revocation_reason='account_deactivated',
            pending_token_hash=NULL,pending_created_at=NULL,pending_session_id=NULL
        WHERE user_id=NEW.id AND revoked_at IS NULL;
        UPDATE auth_sessions SET revoked_at=now(),revocation_reason='account_deactivated',
            pending_token_hash=NULL,pending_created_at=NULL
        WHERE user_id=NEW.id AND remembered_device_id IS NOT NULL AND revoked_at IS NULL;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER users_revoke_remembered_devices_after_deactivation
    AFTER UPDATE OF status ON users FOR EACH ROW
    EXECUTE FUNCTION revoke_remembered_devices_on_account_deactivation();
