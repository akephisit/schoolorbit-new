-- ============================================
-- academic_periods: ทำ unique_period_per_year ให้ DEFERRABLE
-- ============================================
-- เหตุผล: ตอน drag-and-drop reorder ต้อง UPDATE หลายแถวสลับ order_index
-- ถ้า constraint เป็น IMMEDIATE จะชนกลางคันเพราะค่าซ้ำชั่วคราว
-- INITIALLY IMMEDIATE = ปกติยังเช็คทันที (ป้องกัน insert/update ผิด)
-- ใน reorder transaction ค่อย SET CONSTRAINTS DEFERRED ก่อน UPDATE batch
-- ============================================

ALTER TABLE academic_periods
    DROP CONSTRAINT unique_period_per_year;

ALTER TABLE academic_periods
    ADD CONSTRAINT unique_period_per_year
    UNIQUE (academic_year_id, order_index)
    DEFERRABLE INITIALLY IMMEDIATE;
