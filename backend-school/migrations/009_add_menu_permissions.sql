-- Add new permissions for menu access (CRUD Format)
-- Migration: Add dashboard, subjects, classes, calendar, settings, roles permissions
-- Date: 2025-12-23

-- Insert new permissions using CRUD format
INSERT INTO permissions (code, name, module, action, description) VALUES
  ('dashboard.read', 'View Dashboard', 'dashboard', 'read', 'Access dashboard page'),
  ('subjects.read', 'View Subjects', 'subjects', 'read', 'View subjects'),
  ('subjects.create', 'Create Subjects', 'subjects', 'create', 'Create new subjects'),
  ('subjects.update', 'Update Subjects', 'subjects', 'update', 'Edit subjects'),
  ('subjects.delete', 'Delete Subjects', 'subjects', 'delete', 'Delete subjects'),
  ('classes.read', 'View Classes', 'classes', 'read', 'View classes'),
  ('classes.create', 'Create Classes', 'classes', 'create', 'Create new classes'),
  ('classes.update', 'Update Classes', 'classes', 'update', 'Edit classes'),
  ('classes.delete', 'Delete Classes', 'classes', 'delete', 'Delete classes'),
  ('calendar.read', 'View Calendar', 'calendar', 'read', 'Access calendar and events'),
  ('calendar.create', 'Create Events', 'calendar', 'create', 'Create calendar events'),
  ('calendar.update', 'Update Events', 'calendar', 'update', 'Update calendar events'),
  ('calendar.delete', 'Delete Events', 'calendar', 'delete', 'Delete calendar events'),
  ('settings.read', 'View Settings', 'settings', 'read', 'Access system settings'),
  ('settings.update', 'Update Settings', 'settings', 'update', 'Modify system settings'),
  ('roles.read', 'View Roles', 'roles', 'read', 'View roles and permissions'),
  ('roles.create', 'Create Roles', 'roles', 'create', 'Create new roles'),
  ('roles.update', 'Update Roles', 'roles', 'update', 'Edit roles'),
  ('roles.delete', 'Delete Roles', 'roles', 'delete', 'Delete roles')
ON CONFLICT (code) DO NOTHING;

-- Update roles with new permissions (using text[] operators)
-- Add permissions only if they don't exist

-- Update DIRECTOR role - full access except some admin functions
UPDATE roles 
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'dashboard.read',
    'subjects.read',
    'subjects.create',
    'subjects.update',
    'subjects.delete',
    'classes.read',
    'classes.create', 
    'classes.update',
    'classes.delete',
    'calendar.read',
    'calendar.create',
    'calendar.update',
    'calendar.delete',
    'settings.read',
    'settings.update',
    'roles.read'
  ])
)
WHERE code = 'DIRECTOR';

-- Update DEPT_HEAD role - department level access
UPDATE roles 
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'dashboard.read',
    'subjects.read',
    'classes.read',
    'calendar.read'
  ])
)
WHERE code = 'DEPT_HEAD';

-- Update TEACHER role - classroom level access  
UPDATE roles
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'dashboard.read',
    'subjects.read',
    'classes.read',
    'calendar.read'
  ])
)
WHERE code = 'TEACHER';

-- Update SECRETARY role - administrative support
UPDATE roles
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'dashboard.read',
    'calendar.read',
    'calendar.create',
    'settings.read'
  ])
)
WHERE code = 'SECRETARY';

-- Update ADMIN role - add roles management
UPDATE roles
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'roles.read',
    'roles.create',
    'roles.update',
    'roles.delete'
  ])
)
WHERE code = 'ADMIN'
  AND NOT ('roles.read' = ANY(permissions));

-- Verify the changes
SELECT 
  r.code,
  r.name,
  array_length(r.permissions, 1) as permission_count,
  r.permissions
FROM roles r
WHERE r.is_active = true
ORDER BY r.level DESC;
