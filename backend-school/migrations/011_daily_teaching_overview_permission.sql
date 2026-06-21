-- Read-only daily teaching overview permission.

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES (
    'academic_timetable_today.read.school',
    'ดูตารางสอนวันนี้ทั้งโรงเรียน',
    'academic_timetable_today',
    'read',
    'school',
    'ดูภาพรวมตารางสอนรายวันของครูทั้งโรงเรียนแบบอ่านอย่างเดียว'
)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description;
