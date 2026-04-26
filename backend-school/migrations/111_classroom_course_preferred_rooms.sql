-- Migration 111: ห้องที่ classroom_course ใช้สอน (Phase D)
-- Hierarchy: cc preferred rooms → instructor preferred rooms → no preference

CREATE TABLE IF NOT EXISTS classroom_course_preferred_rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    classroom_course_id UUID NOT NULL
        REFERENCES classroom_courses(id) ON DELETE CASCADE,
    room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    rank INTEGER NOT NULL DEFAULT 1,
    is_required BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (classroom_course_id, room_id)
);

CREATE INDEX IF NOT EXISTS idx_ccpr_cc_rank
    ON classroom_course_preferred_rooms(classroom_course_id, rank);

COMMENT ON TABLE classroom_course_preferred_rooms IS
    'ห้องที่ classroom_course นี้ใช้สอน — ranked. ใช้คู่กับ instructor_room_assignments เป็น fallback';
COMMENT ON COLUMN classroom_course_preferred_rooms.rank IS 'ลำดับการลอง (1 = ลองก่อน)';
COMMENT ON COLUMN classroom_course_preferred_rooms.is_required IS
    'true → ห้ามใช้ห้องอื่น scheduler จะ fail ถ้าทุกห้อง required เต็ม';
