-- เพิ่ม room_assignment_track_id สำหรับ override สายจัดห้อง โดยไม่แตะข้อมูลที่นักเรียนสมัคร
ALTER TABLE admission_applications
    ADD COLUMN IF NOT EXISTS room_assignment_track_id UUID
        REFERENCES admission_tracks(id) ON DELETE SET NULL;

-- เพิ่ม selection_settings สำหรับจำการตั้งค่าหน้า selections (แชร์ระหว่าง staff)
ALTER TABLE admission_rounds
    ADD COLUMN IF NOT EXISTS selection_settings JSONB;
