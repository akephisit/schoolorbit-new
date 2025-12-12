-- Create deployment_history table
CREATE TABLE IF NOT EXISTS deployment_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL, -- 'pending', 'in_progress', 'success', 'failed'
    message TEXT,
    github_run_id VARCHAR(255),
    github_run_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    
    CONSTRAINT valid_status CHECK (status IN ('pending', 'in_progress', 'success', 'failed'))
);

-- Create index for faster lookups
CREATE INDEX idx_deployment_history_school_id ON deployment_history(school_id);
CREATE INDEX idx_deployment_history_created_at ON deployment_history(created_at DESC);
