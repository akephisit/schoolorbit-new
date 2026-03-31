-- ============================================
-- ย้าย activity schedule จาก timetable_entries ไป activity_groups
--
-- เหตุผล: ตารางสอนต้องจัดก่อนเปิดเทอม (จองคาบ ACTIVITY ไว้)
-- แต่ครูสร้างชุมนุมตอนเปิดเทอม จึงไม่ควรผูก activity_group_id ใน timetable
-- ชุมนุมควรเก็บ day/period ในตัวเอง
-- ============================================

-- 1. เพิ่ม day + period_ids ใน activity_groups
ALTER TABLE activity_groups
    ADD COLUMN IF NOT EXISTS day_of_week VARCHAR(3),
    ADD COLUMN IF NOT EXISTS period_ids JSONB;

COMMENT ON COLUMN activity_groups.day_of_week IS 'วันที่สอน: MON, TUE, WED, THU, FRI';
COMMENT ON COLUMN activity_groups.period_ids IS 'JSONB array ของ period UUID ที่สอน เช่น ["uuid1","uuid2"]';

-- 2. ย้ายข้อมูลจาก timetable_entries → activity_groups
UPDATE activity_groups ag SET
    day_of_week = sub.day_of_week,
    period_ids = sub.period_ids
FROM (
    SELECT
        te.activity_group_id,
        te.day_of_week,
        jsonb_agg(te.period_id::text ORDER BY ap.order_index) AS period_ids
    FROM academic_timetable_entries te
    JOIN academic_periods ap ON ap.id = te.period_id
    WHERE te.activity_group_id IS NOT NULL AND te.is_active = true
    GROUP BY te.activity_group_id, te.day_of_week
) sub
WHERE ag.id = sub.activity_group_id;

-- 3. ลบ timetable_entries ที่ผูก activity_group_id (ย้ายไป activity_groups แล้ว)
DELETE FROM academic_timetable_entries WHERE activity_group_id IS NOT NULL;

-- 4. ลบ entries ที่ classroom_id เป็น null (เตรียมคืน NOT NULL)
DELETE FROM academic_timetable_entries WHERE classroom_id IS NULL;

-- 5. Drop activity_group_id จาก timetable_entries
DROP INDEX IF EXISTS idx_timetable_activity_group;
DROP INDEX IF EXISTS unique_activity_group_slot;
ALTER TABLE academic_timetable_entries DROP CONSTRAINT IF EXISTS timetable_source_check;
ALTER TABLE academic_timetable_entries DROP COLUMN IF EXISTS activity_group_id;

-- 6. classroom_id กลับเป็น NOT NULL
ALTER TABLE academic_timetable_entries ALTER COLUMN classroom_id SET NOT NULL;

-- 7. Index สำหรับ query ชุมนุมตามวัน
CREATE INDEX IF NOT EXISTS idx_activity_groups_day
    ON activity_groups(day_of_week) WHERE day_of_week IS NOT NULL;
