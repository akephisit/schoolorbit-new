ALTER TABLE academic_exam_rounds
  ADD COLUMN exam_kind TEXT NOT NULL DEFAULT 'midterm',
  ADD CONSTRAINT academic_exam_rounds_exam_kind_check
  CHECK (exam_kind IN ('midterm', 'final'));
