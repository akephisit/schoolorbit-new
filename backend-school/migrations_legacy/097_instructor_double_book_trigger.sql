-- ============================================
-- Prevent instructor double-booking at DB level
-- ============================================
-- App validation (timetable.rs) เช็คก่อน insert อยู่แล้ว แต่:
--   - หลาย admin ทำงานพร้อมกัน → race condition (เช็คพร้อมกันก่อน insert ทั้งคู่)
--   - direct SQL / import script bypass validation
--
-- เพิ่ม trigger 2 ตัว:
--   1. BEFORE INSERT/UPDATE on timetable_entry_instructors
--      → เช็คว่าครูคนนี้ว่างในคาบ (day+period ของ entry)
--   2. BEFORE UPDATE OF day_of_week, period_id on academic_timetable_entries
--      → ถ้าย้าย entry ไปคาบใหม่ เช็คว่าทุกครูใน entry ว่างในคาบใหม่
--
-- หมายเหตุ: ใช้ AS DEFERRABLE INITIALLY IMMEDIATE ไม่ได้ (PG ไม่รองรับสำหรับ
-- trigger-based check) → pre-existing force flow ต้อง DELETE ก่อน INSERT (ทำอยู่แล้ว)
-- ============================================

-- Trigger 1: ห้ามเพิ่มครูเข้า entry ถ้าครูมีคาบเดียวกันใน entry อื่นแล้ว
CREATE OR REPLACE FUNCTION check_instructor_no_double_book()
RETURNS TRIGGER AS $$
DECLARE
    e_day VARCHAR(10);
    e_period UUID;
    e_active BOOLEAN;
    conflict_name TEXT;
BEGIN
    -- ดึง day/period ของ entry ที่จะแนบ
    SELECT day_of_week, period_id, is_active
      INTO e_day, e_period, e_active
    FROM academic_timetable_entries
    WHERE id = NEW.entry_id;

    -- ถ้า entry ไม่ active → ไม่ต้องเช็ค (FK/logic อื่นจัดการ)
    IF e_day IS NULL OR NOT e_active THEN
        RETURN NEW;
    END IF;

    -- หา entry อื่นที่ครูคนนี้สอนอยู่ในคาบเดียวกัน
    SELECT concat(u.first_name, ' ', u.last_name)
      INTO conflict_name
    FROM timetable_entry_instructors tei
    JOIN academic_timetable_entries te ON te.id = tei.entry_id
    JOIN users u ON u.id = tei.instructor_id
    WHERE tei.instructor_id = NEW.instructor_id
      AND te.day_of_week = e_day
      AND te.period_id = e_period
      AND te.id <> NEW.entry_id
      AND te.is_active = true
    LIMIT 1;

    IF conflict_name IS NOT NULL THEN
        RAISE EXCEPTION 'ครู % มีสอนในคาบนี้อยู่แล้ว (instructor double-book)', conflict_name
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS tei_prevent_double_book ON timetable_entry_instructors;
CREATE TRIGGER tei_prevent_double_book
BEFORE INSERT OR UPDATE ON timetable_entry_instructors
FOR EACH ROW EXECUTE FUNCTION check_instructor_no_double_book();


-- Trigger 2: ห้ามย้าย entry ไปคาบที่ครูคนใดคนหนึ่งใน team ติด (ผ่าน junction)
CREATE OR REPLACE FUNCTION check_entry_move_no_instructor_conflict()
RETURNS TRIGGER AS $$
DECLARE
    conflict_name TEXT;
BEGIN
    -- ถ้าไม่ได้ย้าย day/period และ is_active ไม่เปลี่ยนจาก false→true → ข้าม
    IF OLD.day_of_week = NEW.day_of_week
       AND OLD.period_id = NEW.period_id
       AND (OLD.is_active IS NOT DISTINCT FROM NEW.is_active OR NOT NEW.is_active)
    THEN
        RETURN NEW;
    END IF;

    IF NOT NEW.is_active THEN
        RETURN NEW;
    END IF;

    -- หาครูใน team ของ entry นี้ที่ติดคาบใหม่
    SELECT concat(u.first_name, ' ', u.last_name)
      INTO conflict_name
    FROM timetable_entry_instructors tei_self
    JOIN users u ON u.id = tei_self.instructor_id
    WHERE tei_self.entry_id = NEW.id
      AND EXISTS (
          SELECT 1 FROM timetable_entry_instructors tei_other
          JOIN academic_timetable_entries te_other ON te_other.id = tei_other.entry_id
          WHERE tei_other.instructor_id = tei_self.instructor_id
            AND te_other.day_of_week = NEW.day_of_week
            AND te_other.period_id = NEW.period_id
            AND te_other.id <> NEW.id
            AND te_other.is_active = true
      )
    LIMIT 1;

    IF conflict_name IS NOT NULL THEN
        RAISE EXCEPTION 'ไม่สามารถย้าย — ครู % จะติดคาบ', conflict_name
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS ate_prevent_move_conflict ON academic_timetable_entries;
CREATE TRIGGER ate_prevent_move_conflict
BEFORE UPDATE OF day_of_week, period_id, is_active ON academic_timetable_entries
FOR EACH ROW EXECUTE FUNCTION check_entry_move_no_instructor_conflict();

COMMENT ON FUNCTION check_instructor_no_double_book IS
    'BEFORE INSERT/UPDATE บน timetable_entry_instructors: ป้องกัน race condition จาก multi-admin';
COMMENT ON FUNCTION check_entry_move_no_instructor_conflict IS
    'BEFORE UPDATE day/period/is_active บน timetable entries: ถ้าย้ายไปคาบที่ทีมติด → reject';
