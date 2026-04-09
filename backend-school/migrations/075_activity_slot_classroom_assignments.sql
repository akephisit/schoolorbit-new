-- ============================================
-- Activity Slot Classroom Assignments
-- กำหนดครูผู้สอนต่อห้องสำหรับ slot แบบ independent (เช่น แนะแนว)
-- ============================================

CREATE TABLE IF NOT EXISTS activity_slot_classroom_assignments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    slot_id UUID NOT NULL REFERENCES activity_slots(id) ON DELETE CASCADE,
    classroom_id UUID NOT NULL REFERENCES class_rooms(id) ON DELETE CASCADE,
    instructor_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_slot_classroom UNIQUE (slot_id, classroom_id)
);

CREATE INDEX idx_asca_slot ON activity_slot_classroom_assignments(slot_id);
CREATE INDEX idx_asca_classroom ON activity_slot_classroom_assignments(classroom_id);
CREATE INDEX idx_asca_instructor ON activity_slot_classroom_assignments(instructor_id);

COMMENT ON TABLE activity_slot_classroom_assignments IS 'กำหนดครูต่อห้องสำหรับ activity slot แบบ independent เช่น แนะแนว';
