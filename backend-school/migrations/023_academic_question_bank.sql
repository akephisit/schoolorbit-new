-- Academic question bank: rich-content questions, choices, and scoped permissions.

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    (
        'academic_question_bank.read.assigned',
        'ดูคลังข้อสอบที่รับผิดชอบ',
        'academic_question_bank',
        'read',
        'assigned',
        'ดูข้อสอบที่ตนเองสร้างหรือเกี่ยวข้องกับรายวิชาที่รับผิดชอบ'
    ),
    (
        'academic_question_bank.read.organization_unit',
        'ดูคลังข้อสอบในกลุ่มสาระ',
        'academic_question_bank',
        'read',
        'organization_unit',
        'ดูข้อสอบของรายวิชาในกลุ่มสาระเดียวกัน'
    ),
    (
        'academic_question_bank.read.school',
        'ดูคลังข้อสอบทั้งโรงเรียน',
        'academic_question_bank',
        'read',
        'school',
        'ดูข้อสอบในคลังข้อสอบทั้งโรงเรียน'
    ),
    (
        'academic_question_bank.manage.assigned',
        'จัดการคลังข้อสอบที่รับผิดชอบ',
        'academic_question_bank',
        'manage',
        'assigned',
        'สร้างและแก้ไขข้อสอบของตนเอง'
    ),
    (
        'academic_question_bank.manage.organization_unit',
        'จัดการคลังข้อสอบในกลุ่มสาระ',
        'academic_question_bank',
        'manage',
        'organization_unit',
        'สร้างและแก้ไขข้อสอบของรายวิชาในกลุ่มสาระเดียวกัน'
    ),
    (
        'academic_question_bank.manage.school',
        'จัดการคลังข้อสอบทั้งโรงเรียน',
        'academic_question_bank',
        'manage',
        'school',
        'สร้าง แก้ไข และลบข้อสอบในคลังข้อสอบทั้งโรงเรียน'
    )
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT r.id, p.id, NOW()
FROM roles r
JOIN permissions p ON p.code IN (
    'academic_question_bank.read.assigned',
    'academic_question_bank.manage.assigned'
)
WHERE r.code = 'TEACHER'
ON CONFLICT (role_id, permission_id) DO NOTHING;

WITH inserted_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'academic_question_bank.read.school',
        'academic_question_bank.manage.school'
    )
),
admin_roles AS (
    SELECT id
    FROM roles
    WHERE user_type = 'staff'
      AND (
        upper(code) IN ('ADMIN', 'SUPER_ADMIN', 'SCHOOL_ADMIN')
        OR lower(name) IN ('admin', 'administrator', 'super admin', 'school admin')
        OR lower(COALESCE(name_en, '')) IN (
            'admin',
            'administrator',
            'system admin',
            'super admin',
            'school admin'
        )
      )
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT admin_roles.id, inserted_permissions.id, NOW()
FROM admin_roles
CROSS JOIN inserted_permissions
ON CONFLICT (role_id, permission_id) DO NOTHING;

CREATE TABLE academic_question_bank_questions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL,
    grade_level_id UUID REFERENCES grade_levels(id) ON DELETE SET NULL,
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    question_type TEXT NOT NULL DEFAULT 'single_choice',
    difficulty TEXT NOT NULL DEFAULT 'medium',
    points DOUBLE PRECISION NOT NULL DEFAULT 1,
    stem_content JSONB NOT NULL DEFAULT '{"blocks":[]}'::jsonb,
    explanation_content JSONB,
    rubric_content JSONB,
    tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    status TEXT NOT NULL DEFAULT 'draft',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT academic_question_bank_questions_type_check
        CHECK (question_type IN ('single_choice', 'multiple_choice', 'short_answer', 'essay')),
    CONSTRAINT academic_question_bank_questions_difficulty_check
        CHECK (difficulty IN ('easy', 'medium', 'hard')),
    CONSTRAINT academic_question_bank_questions_status_check
        CHECK (status IN ('draft', 'ready', 'archived')),
    CONSTRAINT academic_question_bank_questions_points_check
        CHECK (points >= 0 AND points < 10000),
    CONSTRAINT academic_question_bank_questions_stem_content_check
        CHECK (jsonb_typeof(stem_content) = 'object' AND stem_content ? 'blocks')
);

CREATE TABLE academic_question_bank_choices (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    question_id UUID NOT NULL REFERENCES academic_question_bank_questions(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    content JSONB NOT NULL DEFAULT '{"blocks":[]}'::jsonb,
    is_correct BOOLEAN NOT NULL DEFAULT false,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT academic_question_bank_choices_label_check
        CHECK (length(trim(label)) > 0),
    CONSTRAINT academic_question_bank_choices_content_check
        CHECK (jsonb_typeof(content) = 'object' AND content ? 'blocks')
);

CREATE UNIQUE INDEX idx_question_bank_choices_question_label
    ON academic_question_bank_choices (question_id, lower(label));

CREATE INDEX idx_question_bank_questions_subject
    ON academic_question_bank_questions (subject_id)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_question_bank_questions_grade_level
    ON academic_question_bank_questions (grade_level_id)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_question_bank_questions_owner
    ON academic_question_bank_questions (owner_user_id)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_question_bank_questions_type_status
    ON academic_question_bank_questions (question_type, status)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_question_bank_questions_tags
    ON academic_question_bank_questions USING GIN (tags)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_question_bank_questions_stem_content
    ON academic_question_bank_questions USING GIN (stem_content jsonb_path_ops)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_question_bank_choices_question_sort
    ON academic_question_bank_choices (question_id, sort_order, label);

CREATE TRIGGER update_academic_question_bank_questions_updated_at
    BEFORE UPDATE ON academic_question_bank_questions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_academic_question_bank_choices_updated_at
    BEFORE UPDATE ON academic_question_bank_choices
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
