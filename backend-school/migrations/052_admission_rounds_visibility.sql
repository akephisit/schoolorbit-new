-- เพิ่ม is_visible flag ให้ admission_rounds
-- ควบคุมว่าจะแสดงรอบนี้บน portal ผู้สมัครหรือไม่ (แยกจาก status)
ALTER TABLE admission_rounds
    ADD COLUMN is_visible BOOLEAN NOT NULL DEFAULT FALSE;
