-- ============================================
-- activity_groups: allowed_grade_level_ids → allowed_classroom_ids
-- ============================================
-- เปลี่ยน filter "รับชั้นไหน" (grade-level) → "รับห้องไหน" (classroom-level)
-- เพื่อให้ผู้ใช้เลือกรายห้องได้ (เช่น รับ ม.1/1 ไม่เอา ม.1/2)
--
-- Backfill strategy (snapshot):
--   - group ที่มี allowed_grade_level_ids = [ม.1] + slot รวม [ม.1/1, ม.1/2, ม.2/1]
--     → allowed_classroom_ids = [ม.1/1, ม.1/2]  (filter ตาม grade)
--   - group ที่มี allowed_grade_level_ids = NULL → คง NULL (inherit ทุกห้องใน slot)
-- ============================================

ALTER TABLE activity_groups
    ADD COLUMN IF NOT EXISTS allowed_classroom_ids jsonb;

-- Backfill: expand grade → ห้องที่อยู่ใน slot เดียวกันและ match grade
WITH expanded AS (
    SELECT ag.id AS group_id,
           jsonb_agg(DISTINCT asc_row.classroom_id::text) AS classroom_ids
    FROM activity_groups ag
    JOIN activity_slot_classrooms asc_row ON asc_row.slot_id = ag.activity_slot_id
    JOIN class_rooms cr ON cr.id = asc_row.classroom_id
    WHERE ag.allowed_grade_level_ids IS NOT NULL
      AND jsonb_array_length(ag.allowed_grade_level_ids) > 0
      AND ag.allowed_grade_level_ids ? cr.grade_level_id::text
    GROUP BY ag.id
)
UPDATE activity_groups ag
SET allowed_classroom_ids = e.classroom_ids
FROM expanded e
WHERE ag.id = e.group_id;

-- Drop column เก่า
ALTER TABLE activity_groups DROP COLUMN IF EXISTS allowed_grade_level_ids;

COMMENT ON COLUMN activity_groups.allowed_classroom_ids IS
    'ห้องที่รับ (override slot), NULL = รับทุกห้องที่ slot รับ';
