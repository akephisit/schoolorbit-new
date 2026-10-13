-- Behavior owns score effects. Attendance facts are linked by immutable generation/key,
-- never cascading foreign keys, so independent attendance retention preserves scores.
CREATE TABLE behavior_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK(singleton),
    revision BIGINT NOT NULL DEFAULT 1,
    activation_date DATE NOT NULL DEFAULT (now() AT TIME ZONE 'Asia/Bangkok')::date
);
INSERT INTO behavior_state(singleton) VALUES(true);
CREATE TABLE behavior_settings (
    singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK(singleton),
    enabled BOOLEAN NOT NULL DEFAULT false,
    count_absent_lessons BOOLEAN NOT NULL DEFAULT false,
    count_special BOOLEAN NOT NULL DEFAULT true,
    daily_merit_cap INTEGER CHECK(daily_merit_cap IS NULL OR daily_merit_cap>=0),
    daily_deduction_cap INTEGER CHECK(daily_deduction_cap IS NULL OR daily_deduction_cap>=0),
    merit_reset TEXT NOT NULL DEFAULT 'manual' CHECK(merit_reset IN('manual','monthly','term','year')),
    deduction_reset TEXT NOT NULL DEFAULT 'manual' CHECK(deduction_reset IN('manual','monthly','term','year')),
    row_version BIGINT NOT NULL DEFAULT 1
);
INSERT INTO behavior_settings(singleton) VALUES(true);
CREATE TABLE behavior_items (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    name VARCHAR(200) NOT NULL CHECK(length(btrim(name))>0),
    side TEXT NOT NULL CHECK(side IN('merit','deduction')),
    enabled BOOLEAN NOT NULL DEFAULT true,
    row_version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE behavior_item_versions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    item_id UUID NOT NULL REFERENCES behavior_items(id) ON DELETE RESTRICT,
    effective_from DATE NOT NULL,
    points INTEGER NOT NULL CHECK(points BETWEEN 1 AND 100000),
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(item_id,effective_from)
);
CREATE TABLE behavior_attendance_rules (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    kind TEXT NOT NULL CHECK(kind IN('arrival','lesson','special')),
    status TEXT NOT NULL CHECK(status IN('present','absent','late','truant','sick_leave','personal_leave','activity','unchecked')),
    grade_level_id UUID REFERENCES grade_levels(id) ON DELETE RESTRICT,
    effective_from DATE NOT NULL,
    action TEXT NOT NULL CHECK(action IN('none','merit','deduction')),
    points INTEGER NOT NULL CHECK(points BETWEEN 0 AND 100000),
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK((action='none' AND points=0) OR (action<>'none' AND points>0)),
    UNIQUE NULLS NOT DISTINCT(kind,status,grade_level_id,effective_from)
);
CREATE TABLE behavior_cycles (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    side TEXT NOT NULL CHECK(side IN('merit','deduction')),
    starts_on DATE NOT NULL,
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ,
    ends_on DATE,
    state TEXT NOT NULL CHECK(state IN('current','history')),
    label VARCHAR(200) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK(ends_on IS NULL OR ends_on>=starts_on)
);
CREATE UNIQUE INDEX behavior_current_cycle_idx ON behavior_cycles(side) WHERE state='current';
INSERT INTO behavior_cycles(side,starts_on,starts_at,state,label)
SELECT side,activation_date,activation_date::timestamp AT TIME ZONE 'Asia/Bangkok','current','รอบปัจจุบัน' FROM behavior_state CROSS JOIN(VALUES('merit'),('deduction')) s(side);
CREATE TABLE behavior_batches (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    academic_term_id UUID NOT NULL REFERENCES academic_terms(id) ON DELETE RESTRICT,
    item_version_id UUID NOT NULL REFERENCES behavior_item_versions(id) ON DELETE RESTRICT,
    occurred_at TIMESTAMPTZ NOT NULL,
    date DATE NOT NULL,
    detail TEXT CHECK(detail IS NULL OR length(detail)<=5000),
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE behavior_daily_totals (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    date DATE NOT NULL,
    academic_term_id UUID NOT NULL REFERENCES academic_terms(id) ON DELETE RESTRICT,
    grade_level_id UUID NOT NULL REFERENCES grade_levels(id) ON DELETE RESTRICT,
    homeroom_id UUID REFERENCES homerooms(id) ON DELETE RESTRICT,
    side TEXT NOT NULL CHECK(side IN('merit','deduction')),
    source TEXT NOT NULL CHECK(source IN('manual','arrival','lesson','special')),
    cycle_id UUID NOT NULL REFERENCES behavior_cycles(id) ON DELETE RESTRICT,
    points BIGINT NOT NULL CHECK(points>=0),
    details_purged BOOLEAN NOT NULL DEFAULT false,
    row_version BIGINT NOT NULL DEFAULT 1,
    UNIQUE NULLS NOT DISTINCT(student_id,date,academic_term_id,grade_level_id,homeroom_id,side,source,cycle_id)
);
CREATE INDEX behavior_daily_scope_idx ON behavior_daily_totals(date,side,source,cycle_id);
CREATE INDEX behavior_daily_student_idx ON behavior_daily_totals(student_id,date);
CREATE TABLE behavior_entries (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    total_id UUID NOT NULL REFERENCES behavior_daily_totals(id) ON DELETE RESTRICT,
    batch_id UUID REFERENCES behavior_batches(id) ON DELETE RESTRICT,
    source_key TEXT,
    source_session_id UUID,
    source_generation UUID,
    source_revision BIGINT,
    source_status TEXT,
    raw_points INTEGER,
    rule_version_id UUID REFERENCES behavior_attendance_rules(id) ON DELETE RESTRICT,
    item_name VARCHAR(200) NOT NULL,
    points INTEGER NOT NULL CHECK(points>0),
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    cancelled_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    cancelled_at TIMESTAMPTZ,
    cancellation_reason VARCHAR(1000),
    CHECK((batch_id IS NOT NULL AND source_key IS NULL) OR(batch_id IS NULL AND source_key IS NOT NULL AND source_generation IS NOT NULL AND source_revision>0 AND source_status IS NOT NULL AND raw_points>=points)),
    UNIQUE(batch_id,total_id),
    CHECK((cancelled_at IS NULL AND cancelled_by IS NULL AND cancellation_reason IS NULL)
        OR(cancelled_at IS NOT NULL AND (cancelled_by IS NOT NULL OR (source_key IS NOT NULL AND source_generation IS NOT NULL)) AND length(btrim(cancellation_reason))>0))
);
CREATE INDEX behavior_entries_source_idx ON behavior_entries(source_generation,source_key) WHERE source_generation IS NOT NULL;
CREATE INDEX behavior_entries_creator_idx ON behavior_entries(created_by,created_at DESC,id DESC);
CREATE TABLE behavior_receipts (
    actor_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    request_id UUID NOT NULL,
    operation TEXT NOT NULL,
    digest TEXT NOT NULL,
    outcome JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(actor_id,request_id)
);
CREATE TABLE behavior_suppressions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    academic_term_id UUID REFERENCES academic_terms(id) ON DELETE RESTRICT,
    academic_year_id UUID REFERENCES academic_years(id) ON DELETE RESTRICT,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL CHECK(end_date>=start_date),
    side TEXT NOT NULL CHECK(side IN('merit','deduction')),
    source TEXT NOT NULL CHECK(source IN('manual','arrival','lesson','special')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX behavior_suppressions_range_idx ON behavior_suppressions(side,source,start_date,end_date);
CREATE TABLE behavior_source_days (
    generation UUID NOT NULL,
    academic_term_id UUID NOT NULL REFERENCES academic_terms(id) ON DELETE RESTRICT,
    date DATE NOT NULL,
    revision BIGINT NOT NULL CHECK(revision>0),
    policy JSONB NOT NULL,
    PRIMARY KEY(generation,academic_term_id,date)
);
CREATE TABLE behavior_cleanup_previews (
    token UUID PRIMARY KEY DEFAULT uuidv7(),
    actor_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    revision BIGINT NOT NULL,
    request JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE behavior_cleanup_audits (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    actor_id UUID REFERENCES users(id) ON DELETE RESTRICT,
    scope JSONB NOT NULL,
    mode TEXT NOT NULL CHECK(mode IN('details_only','archive','delete_effects','reset')),
    reason VARCHAR(1000) NOT NULL,
    students BIGINT NOT NULL,
    entries BIGINT NOT NULL,
    merit BIGINT NOT NULL,
    deduction BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE behavior_history_adjustments (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    total_id UUID NOT NULL REFERENCES behavior_daily_totals(id) ON DELETE RESTRICT,
    delta BIGINT NOT NULL CHECK(delta<>0),
    reason VARCHAR(1000) NOT NULL,
    actor_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO permissions(code,name,module,action,scope,description) VALUES
('behavior.read.own','ดูพฤติกรรมของตนเอง','behavior','read','own','ดูรายการที่บันทึกเองและคะแนนของตนเองหรือบุตร'),
('behavior.read.assigned','ดูพฤติกรรมที่รับผิดชอบ','behavior','read','assigned','ดูนักเรียนในห้องที่ปรึกษาตามวันที่'),
('behavior.read.school','ดูพฤติกรรมทั้งโรงเรียน','behavior','read','school','ดูรายงานและคะแนนทั้งโรงเรียน'),
('behavior.create.assigned','บันทึกพฤติกรรมที่รับผิดชอบ','behavior','create','assigned','บันทึกพฤติกรรมห้องที่ปรึกษา'),
('behavior.create.school','บันทึกพฤติกรรมทั้งโรงเรียน','behavior','create','school','บันทึกนักเรียนผสมข้ามห้อง'),
('behavior.delete.own','ยกเลิกรายการของตนเอง','behavior','delete','own','ยกเลิกรายการพฤติกรรมที่ตนเองบันทึก'),
('behavior.delete.school','ยกเลิกรายการทั้งโรงเรียน','behavior','delete','school','ยกเลิกรายการของครูคนอื่น'),
('behavior.manage.school','ตั้งเกณฑ์พฤติกรรม','behavior','manage','school','ตั้งรายการ เกณฑ์ และรอบคะแนน'),
('behavior.correct.school','ปรับประวัติคะแนน','behavior','correct','school','ปรับยอดประวัติพร้อมเหตุผล'),
('behavior_cleanup.execute.school','รีเซ็ตและล้างคะแนน','behavior_cleanup','execute','school','ล้างรายละเอียดจริงและรีเซ็ตคะแนน')
ON CONFLICT(code) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description,is_active=true;
INSERT INTO role_permissions(role_id,permission_id)
SELECT r.id,p.id FROM roles r CROSS JOIN permissions p
WHERE r.code IN('ADMIN','SUPER_ADMIN') AND(p.module='behavior' OR p.module='behavior_cleanup')
ON CONFLICT DO NOTHING;
INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code)
SELECT u.id,p.id,NULL FROM organization_units u CROSS JOIN permissions p
WHERE u.is_active AND p.code IN('behavior.read.own','behavior.read.assigned','behavior.create.assigned','behavior.delete.own')
ON CONFLICT DO NOTHING;
INSERT INTO role_permissions(role_id,permission_id)
SELECT r.id,p.id FROM roles r CROSS JOIN permissions p
WHERE r.user_type IN('student','parent') AND p.code='behavior.read.own'
ON CONFLICT DO NOTHING;

CREATE INDEX behavior_entries_total_idx ON behavior_entries(total_id);
CREATE TABLE behavior_cleanup_jobs (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    actor_id UUID REFERENCES users(id) ON DELETE RESTRICT,
    scope JSONB NOT NULL,
    mode TEXT NOT NULL CHECK(mode IN('details_only','archive','delete_effects')),
    reset BOOLEAN NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN('pending','completed')),
    reason VARCHAR(1000) NOT NULL,
    students BIGINT NOT NULL,
    entries_expected BIGINT NOT NULL,
    entries_deleted BIGINT NOT NULL DEFAULT 0,
    merit BIGINT NOT NULL,
    deduction BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX behavior_one_pending_cleanup ON behavior_cleanup_jobs(status) WHERE status='pending';
