-- ============================================
-- activity_slot_classrooms: junction table บันทึกว่า "ห้องไหน" เข้าร่วม slot ไหน
-- ============================================
-- Background: slot ถูก share ระหว่าง plan ตาม (catalog, semester) แต่ต้องรู้ว่า
-- ห้องไหนจริงๆ ที่เข้าร่วม เพื่อให้ Course Planning ลบ/จัดการรายห้องได้
--
-- Behavior:
--   - ลบ junction row สุดท้ายของ slot → trigger ลบ slot ต่อ (cascade ทุกอย่าง)
--   - ลบ slot → junction cascade ออกเอง (ON DELETE CASCADE)
--   - ลบ classroom → junction cascade (ห้องหาย = ไม่อยู่ใน slot)
-- ============================================

CREATE TABLE IF NOT EXISTS activity_slot_classrooms (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    slot_id UUID NOT NULL REFERENCES activity_slots(id) ON DELETE CASCADE,
    classroom_id UUID NOT NULL REFERENCES class_rooms(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_slot_classroom_participation UNIQUE (slot_id, classroom_id)
);

CREATE INDEX IF NOT EXISTS idx_asc_slot ON activity_slot_classrooms(slot_id);
CREATE INDEX IF NOT EXISTS idx_asc_classroom ON activity_slot_classrooms(classroom_id);

COMMENT ON TABLE activity_slot_classrooms IS
    'Junction: ห้องไหนเข้าร่วม slot ไหน — ลบ row สุดท้ายของ slot → ลบ slot เอง';

-- ============================================
-- Trigger: ลบ junction row สุดท้าย → ลบ slot ต่อ (cascade ทุกอย่าง)
-- ============================================
CREATE OR REPLACE FUNCTION trg_asc_cleanup_empty_slot()
RETURNS TRIGGER AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM activity_slot_classrooms WHERE slot_id = OLD.slot_id
    ) THEN
        DELETE FROM activity_slots WHERE id = OLD.slot_id;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS asc_cleanup_empty_slot ON activity_slot_classrooms;
CREATE TRIGGER asc_cleanup_empty_slot
AFTER DELETE ON activity_slot_classrooms
FOR EACH ROW EXECUTE FUNCTION trg_asc_cleanup_empty_slot();

COMMENT ON FUNCTION trg_asc_cleanup_empty_slot IS
    'ลบ junction row สุดท้ายของ slot → ลบ slot (cascade ลบ groups, members, timetable entries, etc.)';

-- ============================================
-- Backfill: สำหรับ slot ที่มีอยู่ สร้าง junction rows จาก classroom ที่เข้าเกณฑ์
-- เกณฑ์: classroom.grade_level_id อยู่ใน catalog.grade_level_ids
--        + classroom.academic_year_id = semester.academic_year_id
-- ถ้า catalog.grade_level_ids = NULL → ทุกห้องในปีนั้น
-- ============================================
INSERT INTO activity_slot_classrooms (slot_id, classroom_id)
SELECT DISTINCT s.id, cr.id
FROM activity_slots s
JOIN activity_catalog ac ON ac.id = s.activity_catalog_id
JOIN academic_semesters sem ON sem.id = s.semester_id
JOIN class_rooms cr ON cr.academic_year_id = sem.academic_year_id
WHERE cr.study_plan_version_id IS NOT NULL
  AND (
    ac.grade_level_ids IS NULL
    OR ac.grade_level_ids ? cr.grade_level_id::text
  )
ON CONFLICT (slot_id, classroom_id) DO NOTHING;
