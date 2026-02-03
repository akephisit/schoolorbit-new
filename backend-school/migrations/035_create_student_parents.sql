-- ===================================================================
-- Migration 035: Support Multiple Parents per Student
-- ===================================================================

-- 1. Create student_parents junction table
CREATE TABLE IF NOT EXISTS student_parents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Specific relationship for this student-parent pair
    relationship VARCHAR(50) NOT NULL DEFAULT 'guardian',
    
    -- Flags
    is_primary BOOLEAN DEFAULT false,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Prevent duplicate pairs
    UNIQUE(student_user_id, parent_user_id)
);

CREATE INDEX IF NOT EXISTS idx_student_parents_student ON student_parents(student_user_id);
CREATE INDEX IF NOT EXISTS idx_student_parents_parent ON student_parents(parent_user_id);

-- 2. Migrate existing data from student_info
-- Note: We default relationship to 'guardian' or fetch from parent_info if possible, 
-- but parent_info.relationship is single.
INSERT INTO student_parents (student_user_id, parent_user_id, relationship, is_primary)
SELECT 
    si.user_id, 
    si.parent_id, 
    COALESCE(pi.relationship, 'guardian'), 
    true
FROM student_info si
LEFT JOIN parent_info pi ON si.parent_id = pi.user_id
WHERE si.parent_id IS NOT NULL;

-- 3. Remove parent_id from student_info
ALTER TABLE student_info DROP COLUMN IF EXISTS parent_id;

-- 4. Add trigger for updated_at
DROP TRIGGER IF EXISTS update_student_parents_updated_at ON student_parents;
CREATE TRIGGER update_student_parents_updated_at
    BEFORE UPDATE ON student_parents
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
