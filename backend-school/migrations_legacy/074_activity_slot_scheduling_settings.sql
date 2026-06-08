-- ============================================
-- Activity Slot Scheduling Settings
-- เพิ่มจำนวนคาบต่อสัปดาห์ + รูปแบบการจัดตาราง
-- ============================================

ALTER TABLE activity_slots
  ADD COLUMN periods_per_week INTEGER NOT NULL DEFAULT 1,
  ADD COLUMN scheduling_mode VARCHAR(20) NOT NULL DEFAULT 'synchronized'
    CHECK (scheduling_mode IN ('synchronized', 'independent'));

COMMENT ON COLUMN activity_slots.periods_per_week IS 'จำนวนคาบต่อสัปดาห์ที่ต้องจัดตาราง';
COMMENT ON COLUMN activity_slots.scheduling_mode IS 'synchronized = ทุกห้องจัดคาบเดียวกัน, independent = แต่ละห้องจัดเอง';
