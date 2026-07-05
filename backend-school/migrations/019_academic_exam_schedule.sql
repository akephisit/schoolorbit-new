-- Academic exam scheduling

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
  (
    'academic_exam_schedule.read.school',
    'ดูตารางสอบวิชาการทั้งโรงเรียน',
    'academic_exam_schedule',
    'read',
    'school',
    'Read academic exam schedules for the school'
  ),
  (
    'academic_exam_schedule.manage.school',
    'จัดการตารางสอบวิชาการ',
    'academic_exam_schedule',
    'manage',
    'school',
    'Create and manage academic exam schedules for the school'
  ),
  (
    'academic_exam_schedule.publish.school',
    'ประกาศตารางสอบวิชาการ',
    'academic_exam_schedule',
    'publish',
    'school',
    'Publish academic exam schedules for the school'
  )
ON CONFLICT (code) DO UPDATE SET
  name = EXCLUDED.name,
  module = EXCLUDED.module,
  action = EXCLUDED.action,
  scope = EXCLUDED.scope,
  description = EXCLUDED.description,
  updated_at = NOW();

WITH inserted_permissions AS (
  SELECT id, code
  FROM permissions
  WHERE code IN (
    'academic_exam_schedule.read.school',
    'academic_exam_schedule.manage.school',
    'academic_exam_schedule.publish.school'
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
INSERT INTO role_permissions (role_id, permission_id)
SELECT admin_roles.id, inserted_permissions.id
FROM admin_roles
CROSS JOIN inserted_permissions
ON CONFLICT DO NOTHING;

ALTER TABLE academic_assessment_categories
  ADD CONSTRAINT academic_assessment_categories_id_plan_id_key
  UNIQUE (id, plan_id);

ALTER TABLE academic_assessment_plans
  ADD CONSTRAINT academic_assessment_plans_id_semester_subject_key
  UNIQUE (id, academic_semester_id, subject_id);

ALTER TABLE classroom_courses
  ADD CONSTRAINT classroom_courses_id_classroom_subject_semester_key
  UNIQUE (id, classroom_id, subject_id, academic_semester_id);

ALTER TABLE class_rooms
  ADD CONSTRAINT class_rooms_id_grade_level_id_key
  UNIQUE (id, grade_level_id);

CREATE TABLE academic_exam_rounds (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  academic_semester_id UUID NOT NULL REFERENCES academic_semesters(id) ON DELETE RESTRICT,
  name TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published')),
  published_at TIMESTAMPTZ,
  published_by UUID REFERENCES users(id) ON DELETE SET NULL,
  created_by UUID REFERENCES users(id) ON DELETE SET NULL,
  updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_rounds_name_not_blank CHECK (btrim(name) <> ''),
  CONSTRAINT academic_exam_rounds_published_fields CHECK (
    (status = 'draft' AND published_at IS NULL)
    OR (status = 'published' AND published_at IS NOT NULL)
  ),
  UNIQUE (academic_semester_id, name),
  UNIQUE (id, academic_semester_id)
);

CREATE TABLE academic_exam_days (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_round_id UUID NOT NULL REFERENCES academic_exam_rounds(id) ON DELETE CASCADE,
  exam_date DATE NOT NULL,
  label TEXT,
  start_time TIME NOT NULL,
  end_time TIME NOT NULL,
  sort_order INTEGER NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_days_time_order CHECK (start_time < end_time),
  UNIQUE (exam_round_id, exam_date),
  UNIQUE (exam_round_id, sort_order),
  UNIQUE (id, exam_round_id)
);

CREATE TABLE academic_exam_day_grade_levels (
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
  PRIMARY KEY (exam_day_id, grade_level_id)
);

CREATE TABLE academic_exam_day_blocked_windows (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  label TEXT NOT NULL,
  start_time TIME NOT NULL,
  end_time TIME NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_day_blocked_windows_label_not_blank CHECK (btrim(label) <> ''),
  CONSTRAINT academic_exam_day_blocked_windows_time_order CHECK (start_time < end_time)
);

CREATE TABLE academic_exam_day_room_assignments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  classroom_id UUID NOT NULL REFERENCES class_rooms(id) ON DELETE RESTRICT,
  room_id UUID NOT NULL REFERENCES rooms(id) ON DELETE RESTRICT,
  capacity_override INTEGER CHECK (capacity_override IS NULL OR capacity_override > 0),
  created_by UUID REFERENCES users(id) ON DELETE SET NULL,
  updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (exam_day_id, classroom_id),
  UNIQUE (exam_day_id, room_id),
  UNIQUE (id, exam_day_id)
);

CREATE TABLE academic_exam_day_invigilators (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_day_id UUID NOT NULL REFERENCES academic_exam_days(id) ON DELETE CASCADE,
  day_room_assignment_id UUID NOT NULL,
  staff_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  role_label TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_day_invigilators_assignment_day_fkey
    FOREIGN KEY (day_room_assignment_id, exam_day_id)
    REFERENCES academic_exam_day_room_assignments(id, exam_day_id)
    ON DELETE CASCADE,
  UNIQUE (day_room_assignment_id, staff_id),
  UNIQUE (exam_day_id, staff_id)
);

CREATE TABLE academic_exam_schedule_items (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_round_id UUID NOT NULL,
  academic_semester_id UUID NOT NULL,
  assessment_category_id UUID NOT NULL,
  assessment_plan_id UUID NOT NULL,
  classroom_course_id UUID NOT NULL,
  classroom_id UUID NOT NULL,
  subject_id UUID NOT NULL,
  grade_level_id UUID NOT NULL,
  duration_minutes INTEGER NOT NULL CHECK (duration_minutes > 0),
  imported_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_schedule_items_round_semester_fkey
    FOREIGN KEY (exam_round_id, academic_semester_id)
    REFERENCES academic_exam_rounds(id, academic_semester_id)
    ON DELETE CASCADE,
  CONSTRAINT academic_exam_schedule_items_category_plan_fkey
    FOREIGN KEY (assessment_category_id, assessment_plan_id)
    REFERENCES academic_assessment_categories(id, plan_id)
    ON DELETE RESTRICT,
  CONSTRAINT academic_exam_schedule_items_plan_semester_subject_fkey
    FOREIGN KEY (assessment_plan_id, academic_semester_id, subject_id)
    REFERENCES academic_assessment_plans(id, academic_semester_id, subject_id)
    ON DELETE RESTRICT,
  CONSTRAINT academic_exam_schedule_items_course_classroom_subject_semester_fkey
    FOREIGN KEY (classroom_course_id, classroom_id, subject_id, academic_semester_id)
    REFERENCES classroom_courses(id, classroom_id, subject_id, academic_semester_id)
    ON DELETE RESTRICT,
  CONSTRAINT academic_exam_schedule_items_classroom_grade_fkey
    FOREIGN KEY (classroom_id, grade_level_id)
    REFERENCES class_rooms(id, grade_level_id)
    ON DELETE RESTRICT,
  UNIQUE (exam_round_id, assessment_category_id, classroom_id),
  UNIQUE (id, exam_round_id)
);

CREATE TABLE academic_exam_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  exam_schedule_item_id UUID NOT NULL,
  exam_round_id UUID NOT NULL,
  exam_day_id UUID NOT NULL,
  starts_at TIME NOT NULL,
  ends_at TIME NOT NULL,
  created_by UUID REFERENCES users(id) ON DELETE SET NULL,
  updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_sessions_time_order CHECK (starts_at < ends_at),
  CONSTRAINT academic_exam_sessions_schedule_item_round_fkey
    FOREIGN KEY (exam_schedule_item_id, exam_round_id)
    REFERENCES academic_exam_schedule_items(id, exam_round_id)
    ON DELETE CASCADE,
  CONSTRAINT academic_exam_sessions_day_round_fkey
    FOREIGN KEY (exam_day_id, exam_round_id)
    REFERENCES academic_exam_days(id, exam_round_id)
    ON DELETE CASCADE,
  UNIQUE (exam_schedule_item_id)
);

CREATE TABLE academic_exam_seat_assignments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  day_room_assignment_id UUID NOT NULL REFERENCES academic_exam_day_room_assignments(id) ON DELETE CASCADE,
  student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  seat_number TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT academic_exam_seat_assignments_seat_not_blank CHECK (btrim(seat_number) <> ''),
  UNIQUE (day_room_assignment_id, student_id),
  UNIQUE (day_room_assignment_id, seat_number)
);

CREATE TRIGGER update_academic_exam_rounds_updated_at
  BEFORE UPDATE ON academic_exam_rounds
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_academic_exam_days_updated_at
  BEFORE UPDATE ON academic_exam_days
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_academic_exam_day_room_assignments_updated_at
  BEFORE UPDATE ON academic_exam_day_room_assignments
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_academic_exam_sessions_updated_at
  BEFORE UPDATE ON academic_exam_sessions
  FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE INDEX idx_academic_exam_rounds_semester_status
  ON academic_exam_rounds (academic_semester_id, status);
CREATE INDEX idx_academic_exam_schedule_items_round_classroom
  ON academic_exam_schedule_items (exam_round_id, classroom_id);
CREATE INDEX idx_academic_exam_sessions_day_time
  ON academic_exam_sessions (exam_day_id, starts_at, ends_at);
CREATE INDEX idx_academic_exam_seat_assignments_student
  ON academic_exam_seat_assignments (student_id);
