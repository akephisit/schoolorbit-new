ALTER TABLE menu_items
    ADD COLUMN recommended_workspace_code character varying(50),
    ADD COLUMN recommended_group_code character varying(50),
    ADD COLUMN recommended_display_order integer;

COMMENT ON COLUMN menu_items.recommended_workspace_code IS
    'Frontend-owned recommended workspace. Actual persisted placement remains school-owned.';

COMMENT ON COLUMN menu_items.recommended_group_code IS
    'Frontend-owned recommended work section. Actual group_id remains school-owned.';

COMMENT ON COLUMN menu_items.recommended_display_order IS
    'Frontend-owned recommended order. Actual display_order remains school-owned.';

INSERT INTO menu_groups
    (code, name, name_en, icon, display_order, is_active, workspace_code)
VALUES
    ('academic_curriculum', 'งานหลักสูตรและกลุ่มสาระ', 'Curriculum and Learning Areas', 'book-open', 10, true, 'academic'),
    ('academic_delivery', 'งานจัดการเรียนการสอน', 'Teaching and Learning Delivery', 'calendar-days', 20, true, 'academic'),
    ('academic_registry', 'งานทะเบียนนักเรียน', 'Student Registry', 'users', 30, true, 'academic'),
    ('academic_assessment', 'งานวัดผลและประเมินผล', 'Measurement and Evaluation', 'badge-check', 40, true, 'academic'),
    ('academic_activities', 'งานกิจกรรมพัฒนาผู้เรียน', 'Learner Development Activities', 'sparkles', 50, true, 'academic'),
    ('academic_supervision', 'งานนิเทศและพัฒนาการสอน', 'Instructional Supervision', 'clipboard-check', 60, true, 'academic'),
    ('academic_admission', 'งานรับนักเรียน', 'Student Admission', 'clipboard-list', 70, true, 'academic')
ON CONFLICT (code) DO NOTHING;
