ALTER TABLE menu_items
    ALTER COLUMN required_permission TYPE TEXT;

COMMENT ON COLUMN menu_items.required_permission IS
    'One permission/module gate or a pipe-delimited set of alternative generated permission gates.';
