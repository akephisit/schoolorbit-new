CREATE TABLE IF NOT EXISTS academic_assessment_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    classroom_course_id UUID NOT NULL UNIQUE REFERENCES classroom_courses(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'draft',
    submitted_at TIMESTAMPTZ,
    submitted_by UUID REFERENCES users(id) ON DELETE SET NULL,
    locked_at TIMESTAMPTZ,
    locked_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT academic_assessment_plans_status_check
        CHECK (status IN ('draft', 'submitted', 'locked'))
);

CREATE TABLE IF NOT EXISTS academic_assessment_categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id UUID NOT NULL REFERENCES academic_assessment_plans(id) ON DELETE CASCADE,
    code TEXT,
    name TEXT NOT NULL,
    max_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    exam_mode TEXT NOT NULL DEFAULT 'none',
    display_order INTEGER NOT NULL DEFAULT 0,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT academic_assessment_categories_name_check
        CHECK (BTRIM(name) <> ''),
    CONSTRAINT academic_assessment_categories_max_score_check
        CHECK (max_score >= 0),
    CONSTRAINT academic_assessment_categories_code_check
        CHECK (code IS NULL OR code IN ('before_midterm', 'midterm', 'after_midterm', 'final', 'custom')),
    CONSTRAINT academic_assessment_categories_exam_mode_check
        CHECK (exam_mode IN ('none', 'in_timetable', 'outside_timetable', 'practical'))
);

CREATE TABLE IF NOT EXISTS academic_assessment_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    category_id UUID NOT NULL REFERENCES academic_assessment_categories(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    max_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    display_order INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT academic_assessment_items_name_check
        CHECK (BTRIM(name) <> ''),
    CONSTRAINT academic_assessment_items_max_score_check
        CHECK (max_score >= 0)
);

CREATE INDEX IF NOT EXISTS idx_academic_assessment_plans_course
    ON academic_assessment_plans(classroom_course_id);

CREATE INDEX IF NOT EXISTS idx_academic_assessment_plans_status
    ON academic_assessment_plans(status);

CREATE INDEX IF NOT EXISTS idx_academic_assessment_categories_plan
    ON academic_assessment_categories(plan_id, display_order);

CREATE INDEX IF NOT EXISTS idx_academic_assessment_categories_exam_mode
    ON academic_assessment_categories(exam_mode);

CREATE INDEX IF NOT EXISTS idx_academic_assessment_items_category
    ON academic_assessment_items(category_id, display_order);
