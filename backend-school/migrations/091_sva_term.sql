-- ============================================
-- Plan pins the term of an activity when added (mirror sps.term behavior)
-- เดิม sva ไม่มี term → อ่านจาก catalog live → แก้ catalog แล้ว plan ที่ assign ไว้เปลี่ยนตาม
-- ใหม่: sva.term เก็บ snapshot ตอน add → catalog แก้ได้ไม่กระทบ plan เก่า
-- ============================================

ALTER TABLE study_plan_version_activities
    ADD COLUMN IF NOT EXISTS term VARCHAR(20);

-- Backfill: snapshot catalog.term ที่มีอยู่เข้า sva (รวม NULL สำหรับ "ทุกเทอม")
UPDATE study_plan_version_activities sva
SET term = ac.term
FROM activity_catalog ac
WHERE sva.activity_catalog_id = ac.id;

COMMENT ON COLUMN study_plan_version_activities.term IS
    'Snapshot จาก activity_catalog.term ตอน add — null = ทุกเทอม, "1"/"2"/"SUMMER" = ระบุเทอม';
