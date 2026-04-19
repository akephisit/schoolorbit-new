-- ============================================
-- Activity Catalog versioning (pattern เดียวกับ subjects)
-- แก้ UX: แก้คลังกิจกรรม → อดีตเทอมเก่าไม่กระทบ (สร้าง version ใหม่)
-- ============================================

ALTER TABLE activity_catalog
    ADD COLUMN IF NOT EXISTS start_academic_year_id UUID
        REFERENCES academic_years(id) ON DELETE RESTRICT;

-- Backfill: ใช้ปีที่ current ถ้ามี ไม่เช่นนั้นใช้ปีแรกสุดในระบบ
UPDATE activity_catalog
SET start_academic_year_id = COALESCE(
    (SELECT id FROM academic_years WHERE is_active = true ORDER BY year DESC LIMIT 1),
    (SELECT id FROM academic_years ORDER BY year ASC LIMIT 1)
)
WHERE start_academic_year_id IS NULL;

ALTER TABLE activity_catalog
    ALTER COLUMN start_academic_year_id SET NOT NULL;

-- เปลี่ยน unique constraint: name เดียวมีหลาย version ได้ แยกตามปีที่เริ่มใช้
ALTER TABLE activity_catalog
    DROP CONSTRAINT IF EXISTS unique_catalog_name;

ALTER TABLE activity_catalog
    ADD CONSTRAINT unique_catalog_name_per_year UNIQUE (name, start_academic_year_id);

CREATE INDEX IF NOT EXISTS idx_activity_catalog_year ON activity_catalog(start_academic_year_id);

COMMENT ON COLUMN activity_catalog.start_academic_year_id IS 'ปีที่ version นี้เริ่มใช้งาน — name เดียวมีหลาย version ได้';
