-- ============================================
-- Activity slots: live-link → activity_catalog (share slot across plans)
-- ============================================
-- Changes:
--   1. Slot เชื่อมตรงไปที่ activity_catalog (snapshot version ตอน generate)
--   2. Slot unique: (activity_catalog_id, semester_id) ← slot ถูก share ระหว่าง plan
--   3. ลบ denormalized columns: name, description, activity_type, periods_per_week,
--      scheduling_mode, allowed_grade_level_ids, source_plan_activity_id
--   4. ลบ sva.allowed_grade_level_ids → grade scope อ่านจาก catalog
-- ============================================

-- 1. เพิ่ม activity_catalog_id FK
ALTER TABLE activity_slots
    ADD COLUMN IF NOT EXISTS activity_catalog_id UUID
        REFERENCES activity_catalog(id) ON DELETE RESTRICT;

-- Backfill จาก source_plan_activity_id → sva.activity_catalog_id
UPDATE activity_slots slot
SET activity_catalog_id = sva.activity_catalog_id
FROM study_plan_version_activities sva
WHERE slot.source_plan_activity_id = sva.id
  AND slot.activity_catalog_id IS NULL;

-- สำหรับ slot ที่ไม่มี source_plan_activity_id (standalone legacy):
-- match ด้วย name เข้ากับ catalog (unique name ก่อน migration 089 จัด version)
UPDATE activity_slots slot
SET activity_catalog_id = ac.id
FROM activity_catalog ac
WHERE slot.activity_catalog_id IS NULL
  AND slot.name = ac.name;

-- ถ้ายังมี slot ที่ไม่จับคู่ → สร้าง catalog row ใหม่สำหรับเก็บประวัติ
INSERT INTO activity_catalog (name, activity_type, description, periods_per_week, scheduling_mode, start_academic_year_id)
SELECT DISTINCT ON (slot.name)
    slot.name, slot.activity_type, slot.description, slot.periods_per_week, slot.scheduling_mode,
    COALESCE(
        (SELECT id FROM academic_years WHERE is_active = true ORDER BY year DESC LIMIT 1),
        (SELECT id FROM academic_years ORDER BY year ASC LIMIT 1)
    )
FROM activity_slots slot
WHERE slot.activity_catalog_id IS NULL
ON CONFLICT (name, start_academic_year_id) DO NOTHING;

UPDATE activity_slots slot
SET activity_catalog_id = ac.id
FROM activity_catalog ac
WHERE slot.activity_catalog_id IS NULL
  AND slot.name = ac.name;

-- บังคับ NOT NULL
ALTER TABLE activity_slots
    ALTER COLUMN activity_catalog_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_activity_slots_catalog ON activity_slots(activity_catalog_id);

-- 2. ลบ unique เดิม (ถ้ามี) แล้วสร้างใหม่: slot 1 ตัว/(catalog × semester)
-- ไล่ duplicate ก่อน: ถ้ามี slot ซ้ำ (catalog + semester) เดียวกัน → เก็บ row แรก ลบที่เหลือ
DELETE FROM activity_slots
WHERE id IN (
    SELECT id FROM (
        SELECT id, ROW_NUMBER() OVER (
            PARTITION BY activity_catalog_id, semester_id ORDER BY created_at ASC
        ) AS rn
        FROM activity_slots
    ) t
    WHERE t.rn > 1
);

-- ลบ unique เดิม (source_plan_activity_id-based ถ้ามี) แล้ว add ใหม่
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE table_name = 'activity_slots' AND constraint_type = 'UNIQUE'
          AND constraint_name LIKE '%source_plan%'
    ) THEN
        EXECUTE 'ALTER TABLE activity_slots DROP CONSTRAINT ' ||
            (SELECT constraint_name FROM information_schema.table_constraints
             WHERE table_name = 'activity_slots' AND constraint_type = 'UNIQUE'
               AND constraint_name LIKE '%source_plan%' LIMIT 1);
    END IF;
END$$;

ALTER TABLE activity_slots
    ADD CONSTRAINT unique_slot_per_catalog_semester UNIQUE (activity_catalog_id, semester_id);

-- 3. ลบ columns ที่ซ้ำกับ catalog + source link
ALTER TABLE activity_slots
    DROP COLUMN IF EXISTS name,
    DROP COLUMN IF EXISTS description,
    DROP COLUMN IF EXISTS activity_type,
    DROP COLUMN IF EXISTS periods_per_week,
    DROP COLUMN IF EXISTS scheduling_mode,
    DROP COLUMN IF EXISTS allowed_grade_level_ids,
    DROP COLUMN IF EXISTS source_plan_activity_id;

-- 4. ลบ sva.allowed_grade_level_ids — grade scope มาจาก catalog.grade_level_ids
ALTER TABLE study_plan_version_activities
    DROP COLUMN IF EXISTS allowed_grade_level_ids;

COMMENT ON COLUMN activity_slots.activity_catalog_id IS 'Snapshot ของ version catalog ตอน generate — ย้อนดูประวัติเทอมเก่าได้ (catalog มี version แยกปี)';
