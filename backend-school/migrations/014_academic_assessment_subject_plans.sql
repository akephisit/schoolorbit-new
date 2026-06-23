ALTER TABLE academic_assessment_plans
    ADD COLUMN IF NOT EXISTS academic_semester_id UUID,
    ADD COLUMN IF NOT EXISTS subject_id UUID;

UPDATE academic_assessment_plans ap
SET
    academic_semester_id = cc.academic_semester_id,
    subject_id = cc.subject_id
FROM classroom_courses cc
WHERE ap.classroom_course_id = cc.id
  AND (ap.academic_semester_id IS NULL OR ap.subject_id IS NULL);

WITH ranked_plans AS (
    SELECT
        ap.id,
        ROW_NUMBER() OVER (
            PARTITION BY ap.academic_semester_id, ap.subject_id
            ORDER BY
                CASE ap.status
                    WHEN 'locked' THEN 3
                    WHEN 'submitted' THEN 2
                    ELSE 1
                END DESC,
                category_counts.category_count DESC,
                ap.updated_at DESC,
                ap.created_at DESC,
                ap.id ASC
        ) AS row_number
    FROM academic_assessment_plans ap
    LEFT JOIN LATERAL (
        SELECT COUNT(*) AS category_count
        FROM academic_assessment_categories c
        WHERE c.plan_id = ap.id
    ) category_counts ON TRUE
    WHERE ap.academic_semester_id IS NOT NULL
      AND ap.subject_id IS NOT NULL
)
DELETE FROM academic_assessment_plans ap
USING ranked_plans ranked
WHERE ap.id = ranked.id
  AND ranked.row_number > 1;

ALTER TABLE academic_assessment_plans
    DROP CONSTRAINT IF EXISTS academic_assessment_plans_classroom_course_id_key,
    DROP CONSTRAINT IF EXISTS academic_assessment_plans_classroom_course_id_fkey,
    ALTER COLUMN classroom_course_id DROP NOT NULL,
    ALTER COLUMN academic_semester_id SET NOT NULL,
    ALTER COLUMN subject_id SET NOT NULL,
    ADD CONSTRAINT academic_assessment_plans_classroom_course_id_fkey
        FOREIGN KEY (classroom_course_id) REFERENCES classroom_courses(id) ON DELETE SET NULL,
    ADD CONSTRAINT academic_assessment_plans_academic_semester_id_fkey
        FOREIGN KEY (academic_semester_id) REFERENCES academic_semesters(id) ON DELETE CASCADE,
    ADD CONSTRAINT academic_assessment_plans_subject_id_fkey
        FOREIGN KEY (subject_id) REFERENCES subjects(id) ON DELETE CASCADE,
    ADD CONSTRAINT academic_assessment_plans_academic_semester_id_subject_id_key
        UNIQUE (academic_semester_id, subject_id);

ALTER TABLE academic_assessment_categories
    ADD COLUMN IF NOT EXISTS exam_duration_minutes INTEGER;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'academic_assessment_categories_exam_duration_check'
    ) THEN
        ALTER TABLE academic_assessment_categories
            ADD CONSTRAINT academic_assessment_categories_exam_duration_check
                CHECK (exam_duration_minutes IS NULL OR exam_duration_minutes > 0);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_academic_assessment_plans_semester_subject
    ON academic_assessment_plans(academic_semester_id, subject_id);
