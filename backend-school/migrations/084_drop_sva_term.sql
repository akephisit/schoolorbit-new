-- ลบ column term ที่ค้างอยู่ใน study_plan_version_activities
-- เคยเพิ่มใน 082 (sva_term) แต่ revert → ไม่มีโค้ดใช้แล้ว
-- (term ของ activity ตอนนี้อยู่ใน activity_catalog.term แทน)

ALTER TABLE study_plan_version_activities
    DROP COLUMN IF EXISTS term;
