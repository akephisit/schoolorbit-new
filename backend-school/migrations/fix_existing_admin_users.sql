-- Fix existing admin users that don't have role assignments
-- Run this on each school database that was provisioned before the fix

-- This script:
-- 1. Finds admin users (users created by provision usually have specific pattern)
-- 2. Assigns ADMIN role to them if not already assigned

-- Step 1: Check current state
SELECT 
    u.id,
    u.national_id,
    u.first_name,
    u.last_name,
    COUNT(ur.id) as role_count
FROM users u
LEFT JOIN user_roles ur ON ur.user_id = u.id AND ur.ended_at IS NULL
WHERE u.user_type = 'staff'
GROUP BY u.id, u.national_id, u.first_name, u.last_name
ORDER BY u.created_at
LIMIT 10;

-- Step 2: Assign ADMIN role to admin user (change the national_id to match yours)
-- Replace '1234567890123' with your actual admin national_id

DO $$
DECLARE
    v_user_id UUID;
    v_admin_role_id UUID;
BEGIN
    -- Get admin user ID (adjust WHERE clause to match your admin user)
    SELECT id INTO v_user_id
    FROM users
    WHERE national_id = '1234567890123'  -- CHANGE THIS!
    LIMIT 1;

    -- Get ADMIN role ID
    SELECT id INTO v_admin_role_id
    FROM roles
    WHERE code = 'ADMIN';

    -- Assign if both exist
    IF v_user_id IS NOT NULL AND v_admin_role_id IS NOT NULL THEN
        INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
        VALUES (v_user_id, v_admin_role_id, true, CURRENT_DATE)
        ON CONFLICT (user_id, role_id, started_at) DO NOTHING;
        
        RAISE NOTICE 'Admin role assigned to user: %', v_user_id;
    ELSE
        RAISE NOTICE 'User or role not found!';
    END IF;
END $$;

-- Step 3: Verify
SELECT 
    u.id,
    u.national_id,
    u.first_name,
    u.last_name,
    r.code as role_code,
    r.name as role_name,
    array_length(r.permissions, 1) as permission_count,
    r.permissions
FROM users u
JOIN user_roles ur ON ur.user_id = u.id
JOIN roles r ON r.id = ur.role_id
WHERE ur.ended_at IS NULL
ORDER BY u.created_at;
