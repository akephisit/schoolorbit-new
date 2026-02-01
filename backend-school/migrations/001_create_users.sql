-- Create users table for school tenants
-- This will store students, teachers, and staff
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    national_id TEXT, -- Encrypted (Base64 AES-GCM)
    national_id_hash TEXT UNIQUE, -- Blind Index (SHA256 Hex)
    email VARCHAR(255) UNIQUE,
    username VARCHAR(100) UNIQUE, -- Added username field
    password_hash VARCHAR(255) NOT NULL,
    title VARCHAR(50),
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    nickname VARCHAR(50),
    user_type VARCHAR(50) NOT NULL DEFAULT 'student', -- 'student', 'staff', 'parent'
    phone VARCHAR(20),
    emergency_contact VARCHAR(20),
    line_id VARCHAR(100),
    date_of_birth DATE,
    gender VARCHAR(20),
    address TEXT,
    hired_date DATE,
    resigned_date DATE,
    status VARCHAR(50) NOT NULL DEFAULT 'active', -- 'active', 'inactive', 'suspended', 'resigned'
    profile_image_url TEXT,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_national_id_hash ON users(national_id_hash);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_user_type ON users(user_type);
CREATE INDEX IF NOT EXISTS idx_users_status ON users(status);

COMMENT ON COLUMN users.user_type IS 'ประเภทผู้ใช้: student, staff, parent';
