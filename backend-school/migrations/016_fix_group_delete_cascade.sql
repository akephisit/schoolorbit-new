-- Change menu_items.group_id foreign key from CASCADE to SET NULL
-- This allows manual handling of group deletion in application code

-- Drop existing constraint
ALTER TABLE menu_items 
DROP CONSTRAINT IF EXISTS menu_items_group_id_fkey;

-- Add new constraint with SET NULL instead of CASCADE
ALTER TABLE menu_items
ADD CONSTRAINT menu_items_group_id_fkey 
FOREIGN KEY (group_id) 
REFERENCES menu_groups(id) 
ON DELETE SET NULL;

-- NOTE: Application code in delete_menu_group handler 
-- will manually move items to 'other' group before deletion
