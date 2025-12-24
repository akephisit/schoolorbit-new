-- Quick script to grant settings permissions to admin role
-- Run this to ensure admins can access the settings menu

-- Find or create admin role with settings permissions
DO $$
DECLARE
    admin_role_id UUID;
BEGIN
    -- Get the admin role (adjust name if different)
    SELECT id INTO admin_role_id 
    FROM roles 
    WHERE name = 'ผู้ดูแลระบบ' 
    LIMIT 1;
    
    -- If admin role exists, ensure it has settings permissions
    IF admin_role_id IS NOT NULL THEN
        -- Update role permissions to include settings.manage.all
        UPDATE roles
        SET permissions = array_append(
            COALESCE(permissions, ARRAY[]::varchar[]), 
            'settings.manage.all'
        )
        WHERE id = admin_role_id
        AND NOT ('settings.manage.all' = ANY(COALESCE(permissions, ARRAY[]::varchar[])));
        
        RAISE NOTICE 'Added settings.manage.all to admin role';
    ELSE
        RAISE NOTICE 'Admin role not found. Please create it first or adjust the role name in this script.';
    END IF;
END $$;

-- Alternative: Grant individual settings permissions instead of .manage.all
-- Uncomment below if you want more granular control
/*
DO $$
DECLARE
    admin_role_id UUID;
    perm_code VARCHAR;
BEGIN
    SELECT id INTO admin_role_id FROM roles WHERE name = 'ผู้ดูแลระบบ' LIMIT 1;
    
    IF admin_role_id IS NOT NULL THEN
        FOREACH perm_code IN ARRAY ARRAY[
            'settings.features.read',
            'settings.features.update',
            'settings.menu.read',
            'settings.menu.create',
            'settings.menu.update',
            'settings.menu.delete'
        ]
        LOOP
            UPDATE roles
            SET permissions = array_append(permissions, perm_code)
            WHERE id = admin_role_id
            AND NOT (perm_code = ANY(permissions));
        END LOOP;
        
        RAISE NOTICE 'Added individual settings permissions to admin role';
    END IF;
END $$;
*/
