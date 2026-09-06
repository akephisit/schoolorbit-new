-- A durable header distinguishes an initialized empty configuration from first access.
CREATE TABLE subject_term_evaluation_configurations (
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    domain TEXT NOT NULL CHECK (domain IN ('desirable_characteristic', 'reading_thinking_writing')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (subject_id, academic_term_id, domain),
    FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT
);

-- Preserve configurations that may already have been created against schema 060.
INSERT INTO subject_term_evaluation_configurations
    (subject_id, academic_term_id, academic_year_id, domain, row_version)
SELECT subject_id, academic_term_id, academic_year_id, domain, max(row_version)
FROM (
    SELECT subject_id, academic_term_id, academic_year_id, domain, row_version
    FROM subject_term_evaluation_criteria
    UNION ALL
    SELECT subject_id, academic_term_id, academic_year_id, domain, row_version
    FROM learning_group_student_evaluations
    UNION ALL
    SELECT subject_id, academic_term_id, academic_year_id, domain, row_version
    FROM subject_term_evaluation_locks
) existing
GROUP BY subject_id, academic_term_id, academic_year_id, domain;
