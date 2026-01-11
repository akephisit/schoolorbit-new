-- ===================================================================
-- Migration 019: Create Staff Achievements Table
-- Description: Stores achievements, awards, and certifications for staff members
-- Date: 2026-01-11
-- ===================================================================

CREATE TABLE IF NOT EXISTS staff_achievements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    achievement_date DATE NOT NULL,
    image_path TEXT,
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Index for faster lookup by user (for displaying list of a specific staff)
CREATE INDEX IF NOT EXISTS idx_staff_achievements_user_id ON staff_achievements(user_id);
-- Index for faster lookup by date (optional, but good for sorting/filtering)
CREATE INDEX IF NOT EXISTS idx_staff_achievements_date ON staff_achievements(achievement_date);

COMMENT ON TABLE staff_achievements IS 'Stores individual achievements for staff members';
COMMENT ON COLUMN staff_achievements.image_path IS 'Relative path to the stored image file';
