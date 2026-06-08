-- ============================================
-- แก้ instructor double-book trigger: ข้าม conflict ภายใน batch/slot เดียวกัน
-- ============================================
-- บั๊ก: trigger จาก migration 097 reject self-conflict ภายใน batch insert
--   - "พัฒนาผู้เรียน" batch สร้าง entry หลายห้อง × วัน × คาบ พร้อมกัน
--   - ครูคนเดียวกัน attach ทุก entry (semantic: ครูคุมกิจกรรมพร้อมกันหลายห้อง)
--   - trigger ตรวจ entry #2 → เห็น entry #1 (เพิ่งใส่) → block
--
-- แก้: skip ถ้า "conflict entry" share activity_slot_id หรือ batch_id กับ NEW
-- (entry สร้างใน batch เดียวกัน → user เจตนาให้ครูข้าม instance ในกลุ่มได้)
-- COURSE ↔ COURSE หรือ ACTIVITY ต่างชุด ยัง block อยู่ (real double-book)
-- ============================================

CREATE OR REPLACE FUNCTION check_instructor_no_double_book()
RETURNS TRIGGER AS $$
DECLARE
    e_day VARCHAR(10);
    e_period UUID;
    e_active BOOLEAN;
    e_slot UUID;
    e_batch UUID;
    conflict_name TEXT;
BEGIN
    SELECT day_of_week, period_id, is_active, activity_slot_id, batch_id
      INTO e_day, e_period, e_active, e_slot, e_batch
    FROM academic_timetable_entries
    WHERE id = NEW.entry_id;

    IF e_day IS NULL OR NOT e_active THEN
        RETURN NEW;
    END IF;

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
      -- ไม่ใช่ conflict ถ้า entry ตัวที่ชน อยู่ใน slot/batch เดียวกับ NEW
      -- (กลุ่มเดียวกัน — ครูคุมกิจกรรมพร้อมกันหลายห้องได้)
      AND NOT (e_slot IS NOT NULL AND te.activity_slot_id = e_slot)
      AND NOT (e_batch IS NOT NULL AND te.batch_id = e_batch)
    LIMIT 1;

    IF conflict_name IS NOT NULL THEN
        RAISE EXCEPTION 'ครู % มีสอนในคาบนี้อยู่แล้ว (instructor double-book)', conflict_name
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
