-- Add new permissions for menu access
-- Migration: Add dashboard, subjects, classes, calendar, settings permissions
-- Date: 2025-12-22
-- Note: This runs BEFORE migration 010 (still JSONB), so use JSONB functions

-- Insert new permissions
INSERT INTO permissions (code, name, module, action, description) VALUES
  ('dashboard.view', 'View Dashboard', 'dashboard', 'view', 'Access dashboard page'),
  ('subjects.view', 'View Subjects', 'subjects', 'view', 'View and manage subjects'),
  ('classes.view', 'View Classes', 'classes', 'view', 'View and manage classes'),
  ('calendar.view', 'View Calendar', 'calendar', 'view', 'Access calendar and events'),
  ('settings.view', 'View Settings', 'settings', 'view', 'Access system settings')
ON CONFLICT (code) DO NOTHING;

-- Helper function to add permissions to JSONB array (if not exists)
CREATE OR REPLACE FUNCTION add_permissions_to_role_jsonb(
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
    
    -- Skip if role not found
    IF current_perms IS NULL THEN
        RETURN;
    END IF;
    
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

-- Update roles with new permissions
SELECT add_permissions_to_role_jsonb('DIRECTOR', ARRAY[
    'dashboard.view',
    'subjects.view', 
    'classes.view',
    'calendar.view',
    'settings.view'
]);

SELECT add_permissions_to_role_jsonb('DEPT_HEAD', ARRAY[
    'dashboard.view',
    'subjects.view',
    'classes.view', 
    'calendar.view'
]);

SELECT add_permissions_to_role_jsonb('TEACHER', ARRAY[
    'dashboard.view',
    'subjects.view',
    'classes.view',
    'calendar.view'
]);

SELECT add_permissions_to_role_jsonb('SECRETARY', ARRAY[
    'dashboard.view',
    'calendar.view',
    'settings.view'
]);

-- Drop helper function
DROP FUNCTION IF EXISTS add_permissions_to_role_jsonb(VARCHAR, TEXT[]);

-- Verify the changes
SELECT 
  r.code,
  r.name,
  jsonb_array_length(r.permissions) as permission_count,
  r.permissions
FROM roles r
WHERE r.is_active = true
ORDER BY r.level DESC;
