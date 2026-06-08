-- Migration 110: Scheduler config ระดับ classroom_courses (Phase B)
-- pattern + same_day_unique + unavailable เก็บที่ subject × classroom

ALTER TABLE classroom_courses
    ADD COLUMN IF NOT EXISTS consecutive_pattern JSONB DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS same_day_unique BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS hard_unavailable_slots JSONB NOT NULL DEFAULT '[]'::jsonb;

COMMENT ON COLUMN classroom_courses.consecutive_pattern IS
    'รูปแบบการจัดคาบ — array เช่น [1,1,1], [2,1], [3]. NULL → fallback [1]*periods_per_week. Sum ต้องเท่ากับ subjects.periods_per_week';
COMMENT ON COLUMN classroom_courses.same_day_unique IS
    'true → วันเดียวกันห้ามมีรหัสวิชาซ้ำ (เช่น คณิตห้ามสอน 2 ครั้งวันเดียว ยกเว้นเป็น chunk เดียวกัน)';
COMMENT ON COLUMN classroom_courses.hard_unavailable_slots IS
    'คาบที่ไม่จัดสอนวิชานี้ในห้องนี้ — format [{"day": "MON", "period_id": "uuid"}, ...] effective = ค่านี้ ∪ ครูทุกคนใน team';
