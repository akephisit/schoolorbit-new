-- Migration: Add user_type column to menus table
-- This allows filtering menus by user type (staff, student, parent, or all)

-- Add user_type column
ALTER TABLE menus
ADD COLUMN user_type VARCHAR(20) NOT NULL DEFAULT 'staff';

-- Add check constraint to ensure valid values
ALTER TABLE menus
ADD CONSTRAINT menus_user_type_check 
CHECK (user_type IN ('staff', 'student', 'parent', 'all'));

-- Update existing menus based on their path
-- Student menus
UPDATE menus 
SET user_type = 'student' 
WHERE path LIKE '/student%';

-- Parent menus (if any exist)
UPDATE menus 
SET user_type = 'parent' 
WHERE path LIKE '/parent%';

-- Staff menus (all others)
UPDATE menus 
SET user_type = 'staff' 
WHERE path NOT LIKE '/student%' AND path NOT LIKE '/parent%';

-- Add index for performance
CREATE INDEX idx_menus_user_type ON menus(user_type);

-- Comments
COMMENT ON COLUMN menus.user_type IS 'Type of user who can see this menu: staff, student, parent, or all';
