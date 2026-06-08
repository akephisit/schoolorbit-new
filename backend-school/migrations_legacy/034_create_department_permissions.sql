-- Migration 034: Change from Menu-based to Permission-based Department Access

-- 1. Drop old table (menu-based)
DROP TABLE IF EXISTS department_menu_access;

-- 2. Create new table (permission-based)
CREATE TABLE IF NOT EXISTS department_permissions (
    department_id UUID NOT NULL REFERENCES departments(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    created_by UUID REFERENCES users(id),
    
    PRIMARY KEY (department_id, permission_id)
);

-- Index
CREATE INDEX IF NOT EXISTS idx_department_permissions_dept_id ON department_permissions(department_id);

-- Comment
COMMENT ON TABLE department_permissions IS 'Mapping table defining which permissions are granted to members of a department. Members of the department effectively gain these permissions.';
