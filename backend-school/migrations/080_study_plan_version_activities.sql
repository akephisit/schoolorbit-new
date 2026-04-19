-- ============================================
-- Study Plan Version Activities (template สำหรับกิจกรรมพัฒนาผู้เรียนในหลักสูตร)
-- ============================================

CREATE TABLE IF NOT EXISTS study_plan_version_activities (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    study_plan_version_id UUID NOT NULL REFERENCES study_plan_versions(id) ON DELETE CASCADE,

    activity_type VARCHAR(20) NOT NULL CHECK (activity_type IN ('scout', 'club', 'guidance', 'social', 'other')),
    name VARCHAR(200) NOT NULL,
    description TEXT,

    periods_per_week INTEGER NOT NULL DEFAULT 1,
    scheduling_mode VARCHAR(20) NOT NULL DEFAULT 'synchronized'
        CHECK (scheduling_mode IN ('synchronized', 'independent')),

    allowed_grade_level_ids JSONB,

    is_required BOOLEAN NOT NULL DEFAULT true,
    display_order INTEGER NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_spv_activities_version ON study_plan_version_activities(study_plan_version_id);

COMMENT ON TABLE study_plan_version_activities IS 'แม่แบบกิจกรรมพัฒนาผู้เรียนในหลักสูตร (scout, club, guidance) — generate เป็น activity_slots ต่อเทอม';

ALTER TABLE activity_slots
    ADD COLUMN IF NOT EXISTS source_plan_activity_id UUID
        REFERENCES study_plan_version_activities(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_activity_slots_source_plan ON activity_slots(source_plan_activity_id)
    WHERE source_plan_activity_id IS NOT NULL;

COMMENT ON COLUMN activity_slots.source_plan_activity_id IS 'ถ้า generate จาก study plan template — อ้างมาที่นี่ (SET NULL เมื่อ template ถูกลบ)';
