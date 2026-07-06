ALTER TABLE academic_exam_day_invigilators
  DROP CONSTRAINT IF EXISTS academic_exam_day_invigilators_exam_day_id_staff_id_key;

CREATE INDEX IF NOT EXISTS idx_academic_exam_day_invigilators_exam_day_staff
  ON academic_exam_day_invigilators (exam_day_id, staff_id);
