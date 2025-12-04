-- Create schools table
CREATE TABLE IF NOT EXISTS schools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    subdomain VARCHAR(100) NOT NULL UNIQUE,
    db_name VARCHAR(100) NOT NULL,
    db_connection_string TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    config JSONB DEFAULT '{}',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Create index on subdomain for faster lookups
CREATE INDEX idx_schools_subdomain ON schools(subdomain);
CREATE INDEX idx_schools_status ON schools(status);
