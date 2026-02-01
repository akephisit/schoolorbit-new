-- Create department_menu_access table
CREATE TABLE IF NOT EXISTS department_menu_access (
    department_id UUID NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
    menu_item_id UUID NOT NULL REFERENCES system_menu_items(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID REFERENCES users(id),
    PRIMARY KEY (department_id, menu_item_id)
);

-- Index for fast lookup
CREATE INDEX IF NOT EXISTS idx_department_menu_access_dept_id ON department_menu_access(department_id);

-- Comment
COMMENT ON TABLE department_menu_access IS 'Mapping table defining which menus are accessible by which department';
