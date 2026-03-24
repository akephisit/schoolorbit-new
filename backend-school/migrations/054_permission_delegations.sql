-- Migration 054: Permission Delegations
-- Allows department heads to temporarily delegate specific permissions to members

CREATE TABLE IF NOT EXISTS permission_delegations (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    from_user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    to_user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission_id  UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    department_id  UUID REFERENCES departments(id) ON DELETE CASCADE,  -- delegation context
    reason         TEXT,
    started_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at     TIMESTAMPTZ,          -- NULL = no expiry
    revoked_at     TIMESTAMPTZ,          -- set when head revokes early
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT delegation_not_self CHECK (from_user_id <> to_user_id)
);

CREATE INDEX IF NOT EXISTS idx_delegations_to_user   ON permission_delegations(to_user_id);
CREATE INDEX IF NOT EXISTS idx_delegations_from_user ON permission_delegations(from_user_id);
CREATE INDEX IF NOT EXISTS idx_delegations_active    ON permission_delegations(to_user_id)
    WHERE revoked_at IS NULL;

COMMENT ON TABLE permission_delegations IS
    'หัวหน้ากลุ่ม/ฝ่าย มอบหมายสิทธิ์ชั่วคราวให้สมาชิก';
