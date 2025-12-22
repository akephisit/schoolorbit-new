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

-- Update ADMIN role to include all new permissions (already has * wildcard, but for clarity)
-- Admin role already has *, so no update needed

-- Update DIRECTOR role - full access except some admin functions
UPDATE roles 
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'dashboard.view',
    'subjects.view', 
    'classes.view',
    'calendar.view',
    'settings.view'
  ])
)
WHERE code = 'DIRECTOR';

-- Update DEPT_HEAD role - department level access
UPDATE roles 
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'dashboard.view',
    'subjects.view',
    'classes.view', 
    'calendar.view'
  ])
)
WHERE code = 'DEPT_HEAD';

-- Update TEACHER role - classroom level access  
UPDATE roles
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'dashboard.view',
    'subjects.view',
    'classes.view',
    'calendar.view'
  ])
)
WHERE code = 'TEACHER';

-- Update SECRETARY role - administrative support
UPDATE roles
SET permissions = ARRAY(
  SELECT DISTINCT unnest(permissions || ARRAY[
    'dashboard.view',
    'calendar.view',
    'settings.view'
  ])
)
WHERE code = 'SECRETARY';

-- Verify the changes
SELECT 
  r.code,
  r.name,
  array_length(r.permissions, 1) as permission_count,
  r.permissions
FROM roles r
WHERE r.is_active = true
ORDER BY r.level DESC;
