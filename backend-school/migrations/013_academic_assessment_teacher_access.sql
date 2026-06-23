INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    (
        'academic_assessment.read.assigned',
        'ดูโครงสร้างคะแนนรายวิชาที่รับผิดชอบ',
        'academic_assessment',
        'read',
        'assigned',
        'ดูโครงสร้างคะแนนของรายวิชาที่ตนเองเป็นครูผู้สอน'
    ),
    (
        'academic_assessment.manage.assigned',
        'จัดโครงสร้างคะแนนรายวิชาที่รับผิดชอบ',
        'academic_assessment',
        'manage',
        'assigned',
        'เพิ่ม/แก้ไข/ส่งโครงสร้างคะแนนของรายวิชาที่ตนเองเป็นครูผู้สอน'
    ),
    (
        'academic_assessment.read.school',
        'ดูภาพรวมโครงสร้างคะแนนทั้งโรงเรียน',
        'academic_assessment',
        'read',
        'school',
        'ดูภาพรวมโครงสร้างคะแนนและรูปแบบการสอบของรายวิชาที่เปิดสอนทั้งโรงเรียน'
    ),
    (
        'academic_assessment.manage.school',
        'จัดการโครงสร้างคะแนนทั้งโรงเรียน',
        'academic_assessment',
        'manage',
        'school',
        'จัดการโครงสร้างคะแนนและสถานะการส่งของรายวิชาที่เปิดสอนทั้งโรงเรียน'
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
    'academic_assessment.read.assigned',
    'academic_assessment.manage.assigned'
)
WHERE r.code = 'TEACHER'
ON CONFLICT (role_id, permission_id) DO NOTHING;

INSERT INTO feature_toggles (
    code,
    name,
    name_en,
    description,
    module,
    is_enabled,
    metadata
)
VALUES (
    'academic_assessment_teacher_access',
    'เปิดให้ครูกรอกโครงสร้างคะแนน',
    'Academic Assessment Teacher Access',
    'ควบคุมว่าครูผู้สอนสามารถเข้าและบันทึกโครงสร้างคะแนนรายวิชาที่รับผิดชอบได้หรือไม่',
    'academic_assessment',
    TRUE,
    '{"scope": "assigned_teachers"}'::jsonb
)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    name_en = EXCLUDED.name_en,
    description = EXCLUDED.description,
    module = EXCLUDED.module,
    metadata = EXCLUDED.metadata,
    updated_at = NOW();
