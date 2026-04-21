-- ============================================
-- academic_timetable_entries.activity_slot_id: SET NULL → CASCADE
-- ============================================
-- เดิม (migration 073) ตั้ง ON DELETE SET NULL → เมื่อ slot ถูกลบ
-- (เช่นถูก trigger asc_cleanup_empty_slot เมื่อห้องสุดท้ายออก)
-- entry ที่มี activity_slot_id จะกลายเป็น orphan (slot_id=NULL + entry_type='ACTIVITY')
-- → แสดงเป็น "กิจกรรมไร้ slot" ในตาราง
--
-- ใหม่: CASCADE → slot ถูกลบ → entry ก็หายด้วย (สอดคล้องกับ contract ใหม่ที่
-- ทุก activity entry ต้องผูกกับ slot ที่ยังคงอยู่)
-- ============================================

ALTER TABLE academic_timetable_entries
    DROP CONSTRAINT IF EXISTS academic_timetable_entries_activity_slot_id_fkey;

ALTER TABLE academic_timetable_entries
    ADD CONSTRAINT academic_timetable_entries_activity_slot_id_fkey
    FOREIGN KEY (activity_slot_id)
    REFERENCES activity_slots(id)
    ON DELETE CASCADE;
