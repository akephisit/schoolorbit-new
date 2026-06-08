-- Create classes/courses table
CREATE TABLE IF NOT EXISTS classes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) NOT NULL UNIQUE, -- e.g., 'MATH101', 'ENG201'
    name VARCHAR(255) NOT NULL,
    description TEXT,
    teacher_id UUID REFERENCES users(id) ON DELETE SET NULL,
    grade_level VARCHAR(20), -- e.g., '1', '2', '10', '11'
    subject VARCHAR(100), -- e.g., 'Mathematics', 'English'
    room VARCHAR(50),
    schedule JSONB DEFAULT '{}', -- Store schedule as JSON
    max_students INTEGER,
    status VARCHAR(50) NOT NULL DEFAULT 'active', -- 'active', 'archived', 'draft'
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_classes_code ON classes(code);
CREATE INDEX IF NOT EXISTS idx_classes_teacher_id ON classes(teacher_id);
CREATE INDEX IF NOT EXISTS idx_classes_status ON classes(status);
CREATE INDEX IF NOT EXISTS idx_classes_grade_level ON classes(grade_level);
