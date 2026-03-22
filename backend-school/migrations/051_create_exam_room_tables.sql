-- Migration 051: ระบบจัดห้องสอบ (Pre-exam seat assignment)
-- ห้องสอบที่ใช้ต่อรอบ + ที่นั่งสอบของผู้สมัคร + config การจัดที่นั่ง

-- ห้องสอบที่ใช้ในรอบนี้ (ดึงจาก rooms table หรือ custom)
CREATE TABLE admission_exam_rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_round_id UUID NOT NULL REFERENCES admission_rounds(id) ON DELETE CASCADE,
    room_id UUID REFERENCES rooms(id) ON DELETE SET NULL,  -- NULL = custom room
    custom_name VARCHAR(100),           -- ชื่อห้องกรณีไม่ดึงจาก rooms table
    capacity_override INT,              -- ตั้งได้เอง (NULL = ใช้จาก rooms.capacity หรือ 40)
    display_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- capacity จริง = COALESCE(capacity_override, rooms.capacity, 40)

CREATE INDEX idx_admission_exam_rooms_round ON admission_exam_rooms(admission_round_id);

-- ที่นั่งสอบของผู้สมัครแต่ละคน
CREATE TABLE admission_exam_seat_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    application_id UUID NOT NULL REFERENCES admission_applications(id) ON DELETE CASCADE,
    exam_room_id UUID NOT NULL REFERENCES admission_exam_rooms(id) ON DELETE CASCADE,
    seat_number INT NOT NULL,           -- เลขที่นั่งในห้อง (1, 2, 3, …)
    exam_id VARCHAR(50),                -- เลขประจำตัวสอบ (รูปแบบตาม exam_config)
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    assigned_by UUID REFERENCES users(id),
    UNIQUE(application_id),
    UNIQUE(exam_room_id, seat_number)
);

CREATE INDEX idx_exam_seat_assignments_room ON admission_exam_seat_assignments(exam_room_id);
CREATE INDEX idx_exam_seat_assignments_app ON admission_exam_seat_assignments(application_id);

-- Config การจัดที่นั่งต่อรอบ (เก็บใน admission_rounds)
-- exam_config JSONB: {
--   "exam_id_type": "application_number" | "sequential" | "custom_prefix",
--   "exam_id_prefix": "6801",
--   "sort_order": "by_application" | "by_track" | "random"
-- }
ALTER TABLE admission_rounds
    ADD COLUMN IF NOT EXISTS exam_config JSONB NOT NULL DEFAULT '{}';
