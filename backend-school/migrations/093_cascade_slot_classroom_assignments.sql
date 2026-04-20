-- ============================================
-- เมื่อลบห้องออกจาก activity_slot_classrooms (participation)
-- → ต้องลบ activity_slot_classroom_assignments ของห้องนั้นใน slot นั้นด้วย
-- (independent mode: ครูต่อห้อง — ห้องไม่เข้าร่วมแล้วจะไม่มีครูค้าง orphan)
-- ============================================

CREATE OR REPLACE FUNCTION trg_asc_cascade_assignments()
RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM activity_slot_classroom_assignments
    WHERE slot_id = OLD.slot_id AND classroom_id = OLD.classroom_id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS asc_cascade_assignments ON activity_slot_classrooms;
-- BEFORE DELETE — ให้ลบ assignments ก่อน แล้วค่อยให้ trg_asc_cleanup_empty_slot ทำงานต่อ
CREATE TRIGGER asc_cascade_assignments
BEFORE DELETE ON activity_slot_classrooms
FOR EACH ROW EXECUTE FUNCTION trg_asc_cascade_assignments();

COMMENT ON FUNCTION trg_asc_cascade_assignments IS
    'ลบห้องจาก participation junction → ลบครูที่ผูกกับห้องนั้น (independent mode)';
