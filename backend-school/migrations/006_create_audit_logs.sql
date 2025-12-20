-- Audit Logging System
-- Track all changes to important data

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- Who made the change
    user_id UUID REFERENCES users(id),
    user_email VARCHAR(255),
    user_name VARCHAR(255),
    
    -- What was changed
    action VARCHAR(50) NOT NULL, -- 'create', 'update', 'delete', 'login', 'logout'
    entity_type VARCHAR(100) NOT NULL, -- 'user', 'role', 'department', 'grade', etc.
    entity_id UUID,
    entity_name VARCHAR(255),
    
    -- Change details
    old_values JSONB, -- Previous state
    new_values JSONB, -- New state
    changes JSONB, -- Specific fields changed
    
    -- Request context  
    ip_address INET,
    user_agent TEXT,
    request_path VARCHAR(500),
    request_method VARCHAR(10),
    
    -- Additional info
    description TEXT,
    metadata JSONB,
    
    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_entity_type ON audit_logs(entity_type);
CREATE INDEX idx_audit_logs_entity_id ON audit_logs(entity_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at DESC);
CREATE INDEX idx_audit_logs_user_email ON audit_logs(user_email);

-- Composite indexes for common queries
CREATE INDEX idx_audit_logs_entity ON audit_logs(entity_type, entity_id, created_at DESC);
CREATE INDEX idx_audit_logs_user_action ON audit_logs(user_id, action, created_at DESC);

COMMENT ON TABLE audit_logs IS 'Audit trail for all important changes in the system';
COMMENT ON COLUMN audit_logs.action IS 'Type of action: create, update, delete, login, logout';
COMMENT ON COLUMN audit_logs.entity_type IS 'Type of entity affected: user, role, department, grade, etc.';
COMMENT ON COLUMN audit_logs.old_values IS 'Previous state of the entity (JSON)';
COMMENT ON COLUMN audit_logs.new_values IS 'New state of the entity (JSON)';
COMMENT ON COLUMN audit_logs.changes IS 'Specific fields that changed (JSON)';
