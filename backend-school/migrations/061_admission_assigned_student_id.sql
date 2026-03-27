-- Add pre-assigned student ID to admission applications
-- Allows staff to assign student IDs before enrollment (batch assignment step)
ALTER TABLE admission_applications
    ADD COLUMN IF NOT EXISTS assigned_student_id VARCHAR(50);
