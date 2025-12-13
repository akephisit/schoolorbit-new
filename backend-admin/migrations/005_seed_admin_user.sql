-- Seed default admin user for testing/development
-- Password: test123 (hashed with bcrypt cost 12)
-- National ID: 1234567890123
-- 
-- SECURITY: Change these credentials immediately in production!

INSERT INTO admin_users (national_id, password_hash, name, role)
VALUES (
    '1234567890123',
    '$2b$12$5I9rmOFkuiP4zWVloq3MmeETnmMvr9t1DSHmLlSl1a1/lRyhd2c6C',
    'System Administrator',
    'admin'
)
ON CONFLICT (national_id) DO NOTHING;

-- Note: The password hash above corresponds to 'test123'
-- This is for testing/development purposes only
-- In production, you should:
--   1. Change the password immediately after first login
--   2. Or create a new admin user with secure credentials
--   3. Or remove this migration and create admins manually
