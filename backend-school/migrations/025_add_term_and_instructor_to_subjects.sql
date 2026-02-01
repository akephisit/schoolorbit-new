-- Add term preference and default instructor to subjects
ALTER TABLE subjects
ADD COLUMN term VARCHAR(20) DEFAULT NULL,
ADD COLUMN default_instructor_id UUID REFERENCES users(id) ON DELETE SET NULL;

COMMENT ON COLUMN subjects.term IS 'Recommended semester: 1, 2, Summer, or NULL for Any';
