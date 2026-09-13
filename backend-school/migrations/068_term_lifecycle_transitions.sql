-- Transition receipts are immutable evidence, not a second academic state owner.
CREATE TABLE academic_term_transition_receipts (
    request_id UUID PRIMARY KEY,
    academic_year_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    action TEXT NOT NULL CHECK (action IN ('mark_ready','begin_closing','cancel_closing','close','reopen','cancel','activate')),
    request_checksum TEXT NOT NULL CHECK (request_checksum ~ '^[0-9a-f]{64}$'),
    accepted_readiness JSONB NOT NULL CHECK (jsonb_typeof(accepted_readiness)='object'),
    outcome JSONB NOT NULL CHECK (jsonb_typeof(outcome)='object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (academic_term_id,academic_year_id)
        REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT
);
CREATE INDEX academic_term_transition_receipts_term_idx
    ON academic_term_transition_receipts(academic_year_id,academic_term_id,created_at);
CREATE TRIGGER academic_term_transition_receipts_immutable
BEFORE UPDATE OR DELETE ON academic_term_transition_receipts
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();

-- Closing still owns the running term; a successor cannot become active alongside it.
CREATE UNIQUE INDEX academic_terms_one_running_context
    ON academic_terms ((true)) WHERE status IN ('active','closing');

INSERT INTO permissions (code,name,module,action,scope,description,is_active)
VALUES
 ('academic_lifecycle.read.school','ดูความพร้อมปิดและเปลี่ยนภาคเรียน','academic_lifecycle','read','school','ดูสถานะและความพร้อมด้านวิชาการของภาคเรียนทั้งโรงเรียน',true),
 ('academic_lifecycle.manage.school','เตรียมและจัดการสถานะภาคเรียน','academic_lifecycle','manage','school','เตรียมภาคเรียนและเริ่มหรือยกเลิกขั้นตอนกำลังปิด โดยไม่ใช่สิทธิ์ปิดภาคเรียน',true),
 ('academic_lifecycle.close.school','ปิดภาคเรียน','academic_lifecycle','close','school','ยืนยันปิดภาคเรียนหลังผลสรุปและความพร้อมผ่านการตรวจสอบ',true),
 ('academic_lifecycle.reopen.school','เปิดภาคเรียนที่ปิดแล้วกลับมาตรวจสอบ','academic_lifecycle','reopen','school','เปิดกลับสู่สถานะกำลังปิดโดยระบุเหตุผลและไม่มีงานต่อเนื่องที่ห้ามเปิดกลับ',true),
 ('academic_lifecycle.activate.school','เริ่มใช้ภาคเรียน','academic_lifecycle','activate','school','เริ่มใช้ภาคเรียนที่พร้อมอย่างชัดเจนหลังตรวจบริบทและภาคเรียนก่อนหน้า',true);

-- No permission lineage from teacher/subject-group grants into school transitions.
INSERT INTO role_permissions (role_id,permission_id)
SELECT role.id,permission.id FROM roles role CROSS JOIN permissions permission
WHERE role.is_system AND role.code='ADMIN' AND permission.module='academic_lifecycle'
ON CONFLICT DO NOTHING;
