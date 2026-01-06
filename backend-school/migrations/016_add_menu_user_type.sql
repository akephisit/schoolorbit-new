-- Migration: Add user_type column to menu_items table
-- This allows filtering menus by user type (staff, student, parent)

-- Add user_type column
ALTER TABLE menu_items
ADD COLUMN user_type VARCHAR(20) NOT NULL DEFAULT 'staff';

-- Add check constraint to ensure valid values
ALTER TABLE menu_items
ADD CONSTRAINT menu_items_user_type_check 
CHECK (user_type IN ('staff', 'student', 'parent'));

-- Update existing menus based on their path
-- Student menus
UPDATE menu_items 
SET user_type = 'student' 
WHERE path LIKE '/student%';

-- Parent menus (if any exist)
UPDATE menu_items 
SET user_type = 'parent' 
WHERE path LIKE '/parent%';

-- Staff menus (all others)
UPDATE menu_items 
SET user_type = 'staff' 
WHERE path NOT LIKE '/student%' AND path NOT LIKE '/parent%';

-- Add index for performance
CREATE INDEX idx_menu_items_user_type ON menu_items(user_type);

-- Comments
COMMENT ON COLUMN menu_items.user_type IS 'Type of user who can see this menu: staff, student, or parent';
