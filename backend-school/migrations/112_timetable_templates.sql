-- Migration 112: Timetable templates (Phase F)
-- บันทึก fixed slots (พัก/โฮมรูม/sync activity) เป็น template → reapply ได้

CREATE TABLE IF NOT EXISTS timetable_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS timetable_template_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL REFERENCES timetable_templates(id) ON DELETE CASCADE,
    day_of_week TEXT NOT NULL,
    period_id UUID NOT NULL REFERENCES academic_periods(id) ON DELETE CASCADE,
    entry_type TEXT NOT NULL,        -- BREAK / HOMEROOM / ACTIVITY / ACADEMIC
    title TEXT,                       -- TEXT batch — title
    activity_slot_id UUID REFERENCES activity_slots(id) ON DELETE SET NULL,
    -- Scope ของห้อง: ใส่หลายแบบ
    grade_level_ids JSONB NOT NULL DEFAULT '[]'::jsonb,  -- ["uuid", "uuid"] ← apply ทุกห้องใน grade
    classroom_ids JSONB NOT NULL DEFAULT '[]'::jsonb,    -- specific classrooms (ถ้ามี)
    instructor_ids JSONB NOT NULL DEFAULT '[]'::jsonb,   -- ครูที่ tag ใน entries
    room_id UUID REFERENCES rooms(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_template_entries_template
    ON timetable_template_entries(template_id);

COMMENT ON TABLE timetable_templates IS
    'Template ของตารางพื้นฐาน (Phase 1 fixed slots) — สามารถ apply เข้า semester ใดก็ได้';
COMMENT ON COLUMN timetable_template_entries.grade_level_ids IS
    'Resolve เป็นห้องจริง ๆ ตอน apply (จาก class_rooms.grade_level_id)';
