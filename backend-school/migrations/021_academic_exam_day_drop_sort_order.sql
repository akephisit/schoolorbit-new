ALTER TABLE academic_exam_days
  DROP CONSTRAINT IF EXISTS academic_exam_days_exam_round_id_sort_order_key;

ALTER TABLE academic_exam_days
  DROP COLUMN IF EXISTS sort_order;
