-- ============================================
-- Activity Catalog (คลังกิจกรรมพัฒนาผู้เรียน — pattern เดียวกับ subjects)
-- ============================================

CREATE TABLE IF NOT EXISTS activity_catalog (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(200) NOT NULL,
    activity_type VARCHAR(20) NOT NULL CHECK (activity_type IN ('scout', 'club', 'guidance', 'social', 'other')),
    description TEXT,
    periods_per_week INTEGER NOT NULL DEFAULT 1,
    scheduling_mode VARCHAR(20) NOT NULL DEFAULT 'synchronized'
        CHECK (scheduling_mode IN ('synchronized', 'independent')),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_catalog_name UNIQUE (name)
);

CREATE INDEX IF NOT EXISTS idx_activity_catalog_type ON activity_catalog(activity_type);

COMMENT ON TABLE activity_catalog IS 'คลังกิจกรรมพัฒนาผู้เรียน (ลูกเสือ, ชุมนุม, แนะแนว) — ใช้อ้างอิงใน study_plan_version_activities';

-- ============================================
-- Refactor study_plan_version_activities:
-- เดิมเก็บข้อมูลเต็ม → เปลี่ยนเป็นอ้าง activity_catalog_id
-- ============================================

-- Migrate existing data: extract unique (name, activity_type, ...) into catalog
INSERT INTO activity_catalog (name, activity_type, description, periods_per_week, scheduling_mode)
SELECT DISTINCT ON (name)
    name, activity_type, description, periods_per_week, scheduling_mode
FROM study_plan_version_activities
ON CONFLICT (name) DO NOTHING;

-- Add FK column
ALTER TABLE study_plan_version_activities
    ADD COLUMN IF NOT EXISTS activity_catalog_id UUID
        REFERENCES activity_catalog(id) ON DELETE RESTRICT;

-- Populate FK based on name match
UPDATE study_plan_version_activities sva
SET activity_catalog_id = ac.id
FROM activity_catalog ac
WHERE sva.name = ac.name AND sva.activity_catalog_id IS NULL;

-- Make FK required
ALTER TABLE study_plan_version_activities
    ALTER COLUMN activity_catalog_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_sva_catalog ON study_plan_version_activities(activity_catalog_id);

-- Drop duplicated columns (data now lives in catalog)
ALTER TABLE study_plan_version_activities
    DROP COLUMN IF EXISTS name,
    DROP COLUMN IF EXISTS activity_type,
    DROP COLUMN IF EXISTS description,
    DROP COLUMN IF EXISTS periods_per_week,
    DROP COLUMN IF EXISTS scheduling_mode;

COMMENT ON COLUMN study_plan_version_activities.activity_catalog_id IS 'อ้างอิง activity_catalog — ข้อมูลกิจกรรมมาจากคลัง';
