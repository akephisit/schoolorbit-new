-- Keep question-bank entries attached to an exact subject catalog record.
-- Existing legacy rows without a subject remain readable so staff can repair them;
-- the NOT VALID check still rejects new or updated rows without a subject.

ALTER TABLE academic_question_bank_questions
    DROP CONSTRAINT IF EXISTS academic_question_bank_questions_subject_id_fkey;

ALTER TABLE academic_question_bank_questions
    ADD CONSTRAINT academic_question_bank_questions_subject_id_fkey
    FOREIGN KEY (subject_id) REFERENCES subjects(id) ON DELETE RESTRICT NOT VALID;

ALTER TABLE academic_question_bank_questions
    VALIDATE CONSTRAINT academic_question_bank_questions_subject_id_fkey;

ALTER TABLE academic_question_bank_questions
    ADD CONSTRAINT academic_question_bank_questions_subject_required
    CHECK (subject_id IS NOT NULL) NOT VALID;

COMMENT ON COLUMN academic_question_bank_questions.subject_id IS
    'Exact subjects.id catalog record selected by the question owner; required for new and updated questions.';

COMMENT ON COLUMN academic_question_bank_questions.grade_level_id IS
    'Deprecated for question-bank workflows. Grade levels are derived from the selected subject catalog record.';

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX idx_question_bank_questions_updated_at
    ON academic_question_bank_questions (updated_at DESC, created_at DESC, id DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_question_bank_questions_stem_trgm
    ON academic_question_bank_questions
    USING GIN ((stem_content::text) gin_trgm_ops)
    WHERE deleted_at IS NULL;

UPDATE files
SET expires_at = COALESCE(expires_at, created_at + INTERVAL '24 hours')
WHERE is_temporary = true
  AND deleted_at IS NULL;
