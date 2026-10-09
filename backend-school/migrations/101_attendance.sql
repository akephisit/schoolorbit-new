-- Attendance facts are independent of the academic catalog and preserve dated rosters.
CREATE TABLE attendance_settings (
 academic_term_id UUID PRIMARY KEY REFERENCES academic_terms(id) ON DELETE RESTRICT,
 configuration JSONB NOT NULL,
 row_version BIGINT NOT NULL DEFAULT 1 CHECK(row_version>0),
 updated_by UUID NOT NULL REFERENCES users(id), updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE attendance_days (
 academic_term_id UUID NOT NULL REFERENCES academic_terms(id), date DATE NOT NULL,
 counted BOOLEAN NOT NULL, note TEXT NOT NULL DEFAULT '' CHECK(length(note)<=500),
 PRIMARY KEY(academic_term_id,date)
);
CREATE TABLE attendance_audience_groups (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), academic_term_id UUID NOT NULL REFERENCES academic_terms(id),
 name TEXT NOT NULL CHECK(length(btrim(name)) BETWEEN 1 AND 200),
 student_ids UUID[] NOT NULL, row_version BIGINT NOT NULL DEFAULT 1
);
CREATE TABLE attendance_special_templates (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), academic_term_id UUID NOT NULL REFERENCES academic_terms(id),
 definition JSONB NOT NULL, row_version BIGINT NOT NULL DEFAULT 1, created_by UUID NOT NULL REFERENCES users(id)
);
CREATE TABLE attendance_special_rounds (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), template_id UUID NOT NULL REFERENCES attendance_special_templates(id),
 date DATE NOT NULL, UNIQUE(template_id,date)
);
CREATE TABLE attendance_sessions (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), academic_term_id UUID NOT NULL REFERENCES academic_terms(id),
 date DATE NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('arrival','flag','lesson','special')),
 source_key TEXT NOT NULL, title TEXT NOT NULL, teacher_ids UUID[] NOT NULL,
 homeroom_id UUID REFERENCES homerooms(id), learning_group_id UUID REFERENCES learning_groups(id),
 offering_id UUID REFERENCES learning_offerings(id), special_round_id UUID REFERENCES attendance_special_rounds(id),
 start_time TIME NOT NULL, end_time TIME NOT NULL,
 count_override BOOLEAN, cancelled BOOLEAN NOT NULL DEFAULT false, cancellation_reason TEXT,
 saved_at TIMESTAMPTZ, saved_by UUID REFERENCES users(id), row_version BIGINT NOT NULL DEFAULT 1,
 UNIQUE(academic_term_id,date,kind,source_key), UNIQUE(id,special_round_id),
 CHECK((kind='special')=(special_round_id IS NOT NULL))
);
CREATE INDEX attendance_sessions_date_idx ON attendance_sessions(academic_term_id,date,kind);
CREATE INDEX attendance_sessions_teacher_idx ON attendance_sessions USING gin(teacher_ids);
CREATE TABLE attendance_calendar_snapshots (
 academic_term_id UUID NOT NULL REFERENCES academic_terms(id), date DATE NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY(academic_term_id,date)
);
CREATE TABLE attendance_records (
 session_id UUID NOT NULL REFERENCES attendance_sessions(id) ON DELETE CASCADE,
 student_id UUID NOT NULL REFERENCES users(id), student_academic_year_id UUID NOT NULL REFERENCES student_academic_years(id),
 special_round_id UUID, class_number INTEGER,
 result TEXT NOT NULL DEFAULT 'unchecked' CHECK(result IN ('unchecked','present','late','absent','leave','activity')),
 origin TEXT NOT NULL DEFAULT 'none' CHECK(origin IN ('none','teacher','inferred','scan')),
 note TEXT NOT NULL DEFAULT '' CHECK(length(note)<=1000), observed_at TIMESTAMPTZ,
 updated_by UUID REFERENCES users(id), row_version BIGINT NOT NULL DEFAULT 1,
 PRIMARY KEY(session_id,student_id),
 FOREIGN KEY(session_id,special_round_id) REFERENCES attendance_sessions(id,special_round_id),
 UNIQUE(special_round_id,student_id)
);
CREATE INDEX attendance_records_student_idx ON attendance_records(student_id,session_id);
CREATE TABLE attendance_devices (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), name TEXT NOT NULL CHECK(length(btrim(name)) BETWEEN 1 AND 100),
 enabled BOOLEAN NOT NULL DEFAULT true, paired_by UUID NOT NULL REFERENCES users(id),
 operator_id UUID NOT NULL REFERENCES users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE attendance_face_enrollments (
 student_id UUID PRIMARY KEY REFERENCES users(id), model TEXT NOT NULL,
 encrypted_descriptors TEXT NOT NULL, consent_at TIMESTAMPTZ NOT NULL,
 enrolled_by UUID NOT NULL REFERENCES users(id), enrolled_at TIMESTAMPTZ NOT NULL DEFAULT now(), row_version BIGINT NOT NULL DEFAULT 1
);
CREATE TABLE attendance_scan_events (
 id UUID PRIMARY KEY, session_id UUID NOT NULL, student_id UUID NOT NULL, device_id UUID NOT NULL REFERENCES attendance_devices(id),
 evidence_file_id UUID NOT NULL REFERENCES files(id), captured_at TIMESTAMPTZ NOT NULL, accepted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 FOREIGN KEY(session_id,student_id) REFERENCES attendance_records(session_id,student_id),
 UNIQUE(session_id,student_id), UNIQUE(evidence_file_id)
);
CREATE TABLE attendance_notifications (
 id UUID PRIMARY KEY DEFAULT gen_random_uuid(), academic_term_id UUID NOT NULL REFERENCES academic_terms(id),
 recipient_id UUID NOT NULL REFERENCES users(id), student_id UUID REFERENCES users(id),
 dedup_key TEXT NOT NULL, title TEXT NOT NULL, message TEXT NOT NULL, link TEXT NOT NULL,
 notification_id UUID NOT NULL DEFAULT gen_random_uuid(), created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 stored_at TIMESTAMPTZ, published_at TIMESTAMPTZ, next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 attempts INTEGER NOT NULL DEFAULT 0, UNIQUE(recipient_id,dedup_key)
);
CREATE INDEX attendance_notification_due_idx ON attendance_notifications(next_attempt_at) WHERE published_at IS NULL;
CREATE TABLE attendance_term_archives (
 academic_term_id UUID PRIMARY KEY REFERENCES academic_terms(id),
 archived_at TIMESTAMPTZ NOT NULL DEFAULT now(), archived_by UUID NOT NULL REFERENCES users(id),
 record_count BIGINT NOT NULL, counted_record_count BIGINT NOT NULL, settings_snapshot JSONB NOT NULL
);
CREATE TABLE attendance_term_summaries (
 academic_term_id UUID NOT NULL REFERENCES academic_terms(id), student_id UUID NOT NULL REFERENCES users(id),
 category TEXT NOT NULL, scope_key TEXT NOT NULL, scope_label TEXT NOT NULL,
 present BIGINT NOT NULL, late BIGINT NOT NULL, absent BIGINT NOT NULL, leave BIGINT NOT NULL, activity BIGINT NOT NULL,
 unchecked BIGINT NOT NULL, expected BIGINT NOT NULL,
 PRIMARY KEY(academic_term_id,student_id,category,scope_key),
 CHECK(present+late+absent+leave+activity+unchecked=expected)
);

ALTER TABLE files ADD CONSTRAINT files_attendance_private_check CHECK(purpose_code<>'attendance_evidence' OR visibility='private');
CREATE TABLE attendance_term_teacher_summaries (
 academic_term_id UUID NOT NULL REFERENCES academic_terms(id), teacher_id UUID NOT NULL REFERENCES users(id),
 student_id UUID NOT NULL REFERENCES users(id), category TEXT NOT NULL, scope_key TEXT NOT NULL, scope_label TEXT NOT NULL,
 present BIGINT NOT NULL, late BIGINT NOT NULL, absent BIGINT NOT NULL, leave BIGINT NOT NULL, activity BIGINT NOT NULL, unchecked BIGINT NOT NULL, expected BIGINT NOT NULL,
 PRIMARY KEY(academic_term_id,teacher_id,student_id,category,scope_key),
 CHECK(present>=0 AND late>=0 AND absent>=0 AND leave>=0 AND activity>=0 AND unchecked>=0 AND expected=present+late+absent+leave+activity+unchecked)
);

INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.read.assigned','ดูการเช็คชื่อที่รับผิดชอบ','attendance','read','assigned','ดูการเช็คชื่อที่รับผิดชอบ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.read.school','ดูการเช็คชื่อทั้งโรงเรียน','attendance','read','school','ดูการเช็คชื่อทั้งโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.read.own','ดูการเช็คชื่อของตนเองและบุตรหลาน','attendance','read','own','ดูการเช็คชื่อของตนเองและบุตรหลาน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.update.assigned','เช็คชื่อห้องและกลุ่มที่รับผิดชอบ','attendance','update','assigned','เช็คชื่อห้องและกลุ่มที่รับผิดชอบ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.update.school','เช็คชื่อทั้งโรงเรียน','attendance','update','school','เช็คชื่อทั้งโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.manage.school','ตั้งค่าระบบเช็คชื่อและรอบพิเศษ','attendance','manage','school','ตั้งค่าระบบเช็คชื่อและรอบพิเศษ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.enroll.assigned','ลงทะเบียนใบหน้านักเรียนที่รับผิดชอบ','attendance','enroll','assigned','ลงทะเบียนใบหน้านักเรียนที่รับผิดชอบ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.enroll.school','ลงทะเบียนใบหน้านักเรียนทั้งโรงเรียน','attendance','enroll','school','ลงทะเบียนใบหน้านักเรียนทั้งโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.verify.assigned','ใช้เครื่องสแกนที่ได้รับมอบหมาย','attendance','verify','assigned','ใช้เครื่องสแกนที่ได้รับมอบหมาย') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('attendance.delete.school','ล้างรายละเอียดเช็คชื่อภาคเรียนที่ปิดแล้ว','attendance','delete','school','ล้างรายละเอียดเช็คชื่อภาคเรียนที่ปิดแล้ว') ON CONFLICT(code) DO NOTHING;
INSERT INTO role_permissions(role_id,permission_id)
 SELECT r.id,p.id FROM roles r CROSS JOIN permissions p
 WHERE (r.user_type='staff' AND p.code IN ('attendance.read.assigned','attendance.update.assigned','attendance.enroll.assigned','attendance.verify.assigned'))
 OR (r.user_type IN ('student','parent') AND p.code='attendance.read.own') ON CONFLICT DO NOTHING;
INSERT INTO organization_permission_grants(organization_unit_id,permission_id,created_at,created_by,position_code)
 SELECT u.id,p.id,now(),NULL,NULL FROM organization_units u CROSS JOIN permissions p
 WHERE u.is_active AND p.code IN ('attendance.read.assigned','attendance.update.assigned','attendance.enroll.assigned','attendance.verify.assigned') ON CONFLICT DO NOTHING;
