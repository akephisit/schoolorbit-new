-- ============================================
-- classroom_advisors: junction สำหรับครูที่ปรึกษาหลายคนต่อห้อง
-- pattern เดียวกับ subject_default_instructors + activity_catalog_default_instructors
-- ============================================
-- เดิม class_rooms มี advisor_id + co_advisor_id (fixed 2 slot)
-- ใหม่: N คนต่อห้อง, role primary (1 คน max) + secondary (N คน)
-- ============================================

CREATE TABLE IF NOT EXISTS classroom_advisors (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    classroom_id UUID NOT NULL REFERENCES class_rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('primary', 'secondary')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_classroom_advisor UNIQUE (classroom_id, user_id)
);

-- ต้องมี primary ได้ไม่เกิน 1 คนต่อห้อง (partial unique)
CREATE UNIQUE INDEX IF NOT EXISTS idx_classroom_advisors_one_primary
    ON classroom_advisors(classroom_id) WHERE role = 'primary';

CREATE INDEX IF NOT EXISTS idx_classroom_advisors_classroom ON classroom_advisors(classroom_id);
CREATE INDEX IF NOT EXISTS idx_classroom_advisors_user ON classroom_advisors(user_id);

COMMENT ON TABLE classroom_advisors IS
    'ครูที่ปรึกษาต่อห้อง — 1 primary + N secondary (pattern เดียวกับ subject_default_instructors)';

-- ============================================
-- Backfill จาก class_rooms.advisor_id / co_advisor_id
-- ============================================
INSERT INTO classroom_advisors (classroom_id, user_id, role)
SELECT id, advisor_id, 'primary'
FROM class_rooms
WHERE advisor_id IS NOT NULL
ON CONFLICT (classroom_id, user_id) DO NOTHING;

INSERT INTO classroom_advisors (classroom_id, user_id, role)
SELECT id, co_advisor_id, 'secondary'
FROM class_rooms
WHERE co_advisor_id IS NOT NULL
  AND co_advisor_id <> COALESCE(advisor_id, '00000000-0000-0000-0000-000000000000'::uuid)
ON CONFLICT (classroom_id, user_id) DO NOTHING;

-- ============================================
-- Drop columns เก่า
-- ============================================
ALTER TABLE class_rooms DROP CONSTRAINT IF EXISTS class_rooms_advisor_id_fkey;
ALTER TABLE class_rooms DROP CONSTRAINT IF EXISTS class_rooms_co_advisor_id_fkey;
ALTER TABLE class_rooms DROP COLUMN IF EXISTS advisor_id;
ALTER TABLE class_rooms DROP COLUMN IF EXISTS co_advisor_id;
