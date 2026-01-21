-- Add start_academic_year_id to subjects table
ALTER TABLE subjects 
ADD COLUMN start_academic_year_id UUID REFERENCES academic_years(id);

-- Create index for performance
CREATE INDEX idx_subjects_start_year ON subjects(start_academic_year_id);
