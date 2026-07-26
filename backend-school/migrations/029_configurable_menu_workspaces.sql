CREATE TABLE menu_workspaces (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    code character varying(50) NOT NULL UNIQUE,
    name character varying(100) NOT NULL,
    name_en character varying(100),
    description text,
    icon character varying(50),
    display_order integer NOT NULL DEFAULT 0,
    is_active boolean NOT NULL DEFAULT true,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now()
);

COMMENT ON TABLE menu_workspaces IS
    'Configurable top-level navigation workspaces. They organize services but never grant permissions.';

COMMENT ON COLUMN menu_groups.workspace_code IS
    'Navigation workspace code for this department/section. Placement never grants access.';

INSERT INTO menu_workspaces (code, name, name_en, icon, display_order)
VALUES
    ('home', 'หน้าหลักของฉัน', 'My Home', 'house', 10),
    ('academic', 'กลุ่มบริหารวิชาการ', 'Academic Affairs', 'graduation-cap', 20),
    ('student_affairs', 'กลุ่มบริหารกิจการนักเรียน', 'Student Affairs', 'user-round-check', 30),
    ('personnel', 'กลุ่มบริหารงานบุคคล', 'Personnel Affairs', 'users', 40),
    ('budget', 'กลุ่มบริหารงบประมาณ', 'Budget Administration', 'wallet', 50),
    ('operations', 'กลุ่มบริหารทั่วไป', 'General Administration', 'building-2', 60),
    ('settings', 'ตั้งค่าระบบ', 'System Settings', 'settings', 1000)
ON CONFLICT (code) DO NOTHING;

UPDATE menu_groups
SET workspace_code = 'budget'
WHERE code = 'budget'
  AND workspace_code = 'operations';

INSERT INTO menu_workspaces (code, name, name_en, icon, display_order)
SELECT DISTINCT
    menu_group.workspace_code,
    menu_group.workspace_code,
    menu_group.workspace_code,
    'panel-left',
    900
FROM menu_groups AS menu_group
WHERE NOT EXISTS (
    SELECT 1
    FROM menu_workspaces AS workspace
    WHERE workspace.code = menu_group.workspace_code
);

INSERT INTO menu_groups (
    code,
    name,
    name_en,
    description,
    icon,
    display_order,
    is_active,
    workspace_code
)
VALUES
    ('home_main', 'งานประจำของฉัน', 'My Daily Work', 'เมนูประจำวันของผู้ใช้', 'inbox', 10, true, 'home'),
    ('academic_foundation', 'โครงสร้างวิชาการ', 'Academic Foundation', NULL, 'framer', 10, true, 'academic'),
    ('academic_curriculum', 'หลักสูตรและรายวิชา', 'Curriculum and Subjects', NULL, 'book-open', 20, true, 'academic'),
    ('academic_students', 'ทะเบียนและนักเรียน', 'Registry and Students', NULL, 'users', 30, true, 'academic'),
    ('academic_admission', 'รับสมัครนักเรียน', 'Student Admission', NULL, 'clipboard-list', 40, true, 'academic'),
    ('academic_timetable', 'ตารางสอนและตารางสอบ', 'Timetables and Exams', NULL, 'calendar-days', 50, true, 'academic'),
    ('academic_quality', 'วัดผลและพัฒนาคุณภาพ', 'Assessment and Quality', NULL, 'badge-check', 60, true, 'academic'),
    ('personnel_management', 'งานบุคลากร', 'Personnel Services', NULL, 'users', 10, true, 'personnel'),
    ('personnel_organization', 'โครงสร้างองค์กร', 'Organization Structure', NULL, 'network', 20, true, 'personnel'),
    ('operations_facility', 'อาคารสถานที่', 'Facilities', NULL, 'school', 10, true, 'operations'),
    ('settings_system', 'ระบบและสิทธิ์', 'System and Access', NULL, 'settings', 10, true, 'settings'),
    ('other', 'อื่น ๆ', 'Other', 'กลุ่มสำรองสำหรับเมนูที่ยังไม่ได้จัดหมวด', 'circle-ellipsis', 9999, true, 'operations')
ON CONFLICT (code) DO NOTHING;

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'home_main')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'main');

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'academic_foundation')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'academic')
  AND path IN (
      '/staff/academic/structure',
      '/staff/academic/periods',
      '/staff/academic/classrooms'
  );

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'academic_curriculum')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'academic')
  AND path IN (
      '/staff/academic/subject-groups',
      '/staff/academic/subjects',
      '/staff/academic/study-plans',
      '/staff/academic/planning',
      '/staff/academic/activities'
  );

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'academic_students')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'academic')
  AND path IN (
      '/staff/students',
      '/staff/academic/enrollments'
  );

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'academic_admission')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'academic')
  AND path = '/staff/academic/admission';

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'academic_timetable')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'academic')
  AND path IN (
      '/staff/academic/timetable/today',
      '/staff/academic/timetable',
      '/staff/academic/exam-schedules'
  );

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'academic_quality')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'academic')
  AND path IN (
      '/staff/academic/assessments',
      '/staff/academic/question-bank',
      '/staff/academic/supervision'
  );

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'personnel_management')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'personnel')
  AND path IN (
      '/staff/manage',
      '/staff/achievements'
  );

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'personnel_organization')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'personnel')
  AND path = '/staff/organization';

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'operations_facility')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'general_admin')
  AND path = '/staff/facility/buildings';

UPDATE menu_items
SET group_id = (SELECT id FROM menu_groups WHERE code = 'settings_system')
WHERE group_id = (SELECT id FROM menu_groups WHERE code = 'settings');

ALTER TABLE menu_groups
ADD CONSTRAINT menu_groups_workspace_code_fkey
FOREIGN KEY (workspace_code)
REFERENCES menu_workspaces(code)
ON UPDATE CASCADE
ON DELETE RESTRICT;

CREATE INDEX idx_menu_workspaces_active_order
ON menu_workspaces (is_active, display_order);

CREATE INDEX idx_menu_groups_workspace_order
ON menu_groups (workspace_code, display_order);

CREATE TRIGGER update_menu_workspaces_updated_at
    BEFORE UPDATE ON menu_workspaces
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
