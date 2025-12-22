-- Add new permissions for menu access
-- Migration: Add dashboard, subjects, classes, calendar, settings permissions
-- Date: 2025-12-22

-- Insert new permissions
INSERT INTO permissions (code, name, module, action, description) VALUES
  ('dashboard.view', 'View Dashboard', 'dashboard', 'view', 'Access dashboard page'),
  ('subjects.view', 'View Subjects', 'subjects', 'view', 'View and manage subjects'),
  ('classes.view', 'View Classes', 'classes', 'view', 'View and manage classes'),
  ('calendar.view', 'View Calendar', 'calendar', 'view', 'Access calendar and events'),
  ('settings.view', 'View Settings', 'settings', 'view', 'Access system settings')
ON CONFLICT (code) DO NOTHING;

-- Update ADMIN role to include all new permissions (already has * wildcard, so skip)
-- Admin role permissions should already contain "*"

-- Helper function to add permissions to JSONB array (if not exists)
CREATE OR REPLACE FUNCTION add_permissions_to_role(
    role_code VARCHAR,
    new_perms TEXT[]
) RETURNS VOID AS $$
DECLARE
    current_perms JSONB;
    perm TEXT;
    perms_array TEXT[];
BEGIN
    -- Get current permissions
    SELECT permissions INTO current_perms
    FROM roles WHERE code = role_code;
    
    -- Convert JSONB to text array
    SELECT ARRAY(SELECT jsonb_array_elements_text(current_perms))
    INTO perms_array;
    
    -- Add new permissions if not exists
    FOREACH perm IN ARRAY new_perms LOOP
        IF perm != ALL(perms_array) THEN
            perms_array := perms_array || perm;
        END IF;
    END LOOP;
    
    -- Update role with merged permissions
    UPDATE roles 
    SET permissions = to_jsonb(perms_array)
    WHERE code = role_code;
END;
$$ LANGUAGE plpgsql;

-- Update DIRECTOR role - full access except some admin functions
SELECT add_permissions_to_role('DIRECTOR', ARRAY[
    'dashboard.view',
    'subjects.view', 
    'classes.view',
    'calendar.view',
    'settings.view'
]);

-- Update DEPT_HEAD role - department level access
SELECT add_permissions_to_role('DEPT_HEAD', ARRAY[
    'dashboard.view',
    'subjects.view',
    'classes.view', 
    'calendar.view'
]);

-- Update TEACHER role - classroom level access  
SELECT add_permissions_to_role('TEACHER', ARRAY[
    'dashboard.view',
    'subjects.view',
    'classes.view',
    'calendar.view'
]);

-- Update SECRETARY role - administrative support
SELECT add_permissions_to_role('SECRETARY', ARRAY[
    'dashboard.view',
    'calendar.view',
    'settings.view'
]);

-- Drop helper function
DROP FUNCTION IF EXISTS add_permissions_to_role(VARCHAR, TEXT[]);

-- Verify the changes
SELECT 
  r.code,
  r.name,
  jsonb_array_length(r.permissions) as permission_count,
  r.permissions
FROM roles r
WHERE r.is_active = true
ORDER BY r.level DESC;
