-- Seed default admin user
-- Password: admin123 (hashed with bcrypt cost 10)
-- National ID: 1234567890123

INSERT INTO admin_users (national_id, password_hash, name, role)
VALUES (
    '1234567890123',
    '$2b$10$YzvLJ8qNqH3p8yOXZ/7o7.K9VzJN6JxJ5aH2QXv9F8vZR4kW5mFHm',
    'System Administrator',
    'admin'
)
ON CONFLICT (national_id) DO NOTHING;

-- Note: The password hash above corresponds to 'admin123'
-- Change this password immediately after first login in production!
