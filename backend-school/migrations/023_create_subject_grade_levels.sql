CREATE TABLE subject_grade_levels (
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (subject_id, grade_level_id)
);

CREATE INDEX idx_sgl_subject ON subject_grade_levels(subject_id);
CREATE INDEX idx_sgl_level ON subject_grade_levels(grade_level_id);
