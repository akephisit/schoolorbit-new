-- Migration 044: Add flexible scheduling constraints
-- Add more granular control over allowed periods, days, and room assignments

-- ============================================
-- Part 1: Subject Constraints - Allow specific periods and days
-- ============================================

-- Add columns for specific period/day selection
ALTER TABLE subjects 
ADD COLUMN IF NOT EXISTS allowed_period_ids JSONB DEFAULT NULL,
ADD COLUMN IF NOT EXISTS allowed_days JSONB DEFAULT NULL;

-- Comments
COMMENT ON COLUMN subjects.allowed_period_ids IS 'Array of period UUIDs that this subject can be scheduled in. NULL = all periods allowed. Example: ["uuid1", "uuid2", "uuid3"]';
COMMENT ON COLUMN subjects.allowed_days IS 'Array of days (MON,TUE,WED,THU,FRI,SAT,SUN) that this subject can be scheduled. NULL = all days allowed. Example: ["MON", "TUE", "WED"]';

-- ============================================
-- Part 2: Instructor Room Assignments - Per Subject
-- ============================================

-- Add subject_id to make room assignments subject-specific
ALTER TABLE instructor_room_assignments 
ADD COLUMN IF NOT EXISTS subject_id UUID REFERENCES subjects(id) ON DELETE CASCADE;

COMMENT ON COLUMN instructor_room_assignments.subject_id IS 'Specific subject for this room assignment. NULL = default room for all subjects not explicitly assigned.';

-- Drop old unique constraint if exists
ALTER TABLE instructor_room_assignments 
DROP CONSTRAINT IF EXISTS instructor_room_assignments_instructor_id_academic_year_id_key;

ALTER TABLE instructor_room_assignments 
DROP CONSTRAINT IF EXISTS unique_instructor_room_year;

-- Create new unique index that includes subject_id
-- Using COALESCE to treat NULL as a special UUID for the unique constraint
CREATE UNIQUE INDEX IF NOT EXISTS idx_instructor_subject_room_unique 
ON instructor_room_assignments(
    instructor_id, 
    COALESCE(subject_id, '00000000-0000-0000-0000-000000000000'::uuid), 
    academic_year_id, 
    room_id
);

-- Keep the for_subjects column for backward compatibility during transition
-- But mark it as deprecated in comments
COMMENT ON COLUMN instructor_room_assignments.for_subjects IS 'DEPRECATED: Use subject_id instead. This will be removed in future migration.';

-- ============================================
-- Part 3: Validation Function (Optional but helpful)
-- ============================================

-- Function to validate allowed_days format
CREATE OR REPLACE FUNCTION validate_allowed_days(days JSONB) 
RETURNS BOOLEAN AS $$
DECLARE
    day TEXT;
    valid_days TEXT[] := ARRAY['MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN'];
BEGIN
    IF days IS NULL THEN
        RETURN TRUE;
    END IF;
    
    FOR day IN SELECT jsonb_array_elements_text(days)
    LOOP
        IF NOT (day = ANY(valid_days)) THEN
            RETURN FALSE;
        END IF;
    END LOOP;
    
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Add check constraint for allowed_days
ALTER TABLE subjects 
ADD CONSTRAINT valid_allowed_days 
CHECK (validate_allowed_days(allowed_days));

-- ============================================
-- Part 4: Example Data (for testing)
-- ============================================

-- Example: PE subject should only be scheduled in afternoon (periods 5-8) on MON, WED, FRI
-- This is just an example - adjust based on your actual period IDs
/*
UPDATE subjects 
SET allowed_days = '["MON", "WED", "FRI"]'::jsonb
WHERE type = 'PE';
*/

-- Example: Assign specific room for Math teacher in Lab 1
-- This is just an example structure
/*
UPDATE instructor_room_assignments 
SET subject_id = (SELECT id FROM subjects WHERE code = 'MATH' LIMIT 1)
WHERE instructor_id = 'some-instructor-uuid'
  AND room_id = 'lab-1-uuid';
*/
