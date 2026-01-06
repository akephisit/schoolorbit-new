-- Migration: Add user_type column to menus table
-- This allows filtering menus by user type (staff, student, all)

-- Add user_type column
ALTER TABLE menus
ADD COLUMN user_type VARCHAR(20) NOT NULL DEFAULT 'staff';

-- Update existing menus based on their path
-- All existing menus are under /staff except student profile
UPDATE menus 
SET user_type = 'student' 
WHERE path LIKE '/student%';

-- Menus that start with /staff or other paths are for staff
UPDATE menus 
SET user_type = 'staff' 
WHERE path NOT LIKE '/student%';

-- Add index for performance
CREATE INDEX idx_menus_user_type ON menus(user_type);

-- Comments
COMMENT ON COLUMN menus.user_type IS 'Type of user who can see this menu: staff, student, or all';
