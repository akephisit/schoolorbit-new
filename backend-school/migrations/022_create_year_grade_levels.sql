CREATE TABLE academic_year_grade_levels (
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (academic_year_id, grade_level_id)
);

CREATE INDEX idx_aygl_year ON academic_year_grade_levels(academic_year_id);
CREATE INDEX idx_aygl_level ON academic_year_grade_levels(grade_level_id);
