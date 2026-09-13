CREATE TABLE academic_promotion_policy_versions (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 200),
    rules JSONB NOT NULL CHECK (jsonb_typeof(rules) = 'array' AND jsonb_array_length(rules) BETWEEN 1 AND 500),
    progression_row_version BIGINT NOT NULL CHECK (progression_row_version > 0),
    reviewed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    reviewed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX academic_promotion_policy_versions_reviewed_idx
    ON academic_promotion_policy_versions (reviewed_at DESC, id);
CREATE TRIGGER academic_promotion_policy_versions_immutable
BEFORE UPDATE OR DELETE ON academic_promotion_policy_versions
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();

INSERT INTO permissions (code,name,module,action,scope,description,is_active) VALUES
('academic_promotion.read.school','ดูการเลื่อนชั้นและเกณฑ์ของโรงเรียน','academic_promotion','read','school','ดูเกณฑ์และรายการเตรียมเลื่อนชั้นโดยไม่เปลี่ยนข้อมูลนักเรียน',true),
('academic_promotion.manage.school','จัดเตรียมการเลื่อนชั้น','academic_promotion','manage','school','จัดทำเกณฑ์และเตรียมรายการเลื่อนชั้น โดยไม่รวมการอนุมัติหรือดำเนินการจริง',true),
('academic_promotion.approve.school','ตรวจและอนุมัติการเลื่อนชั้น','academic_promotion','approve','school','ยืนยันเกณฑ์และตรวจอนุมัติรายการเลื่อนชั้นก่อนดำเนินการ',true),
('academic_promotion.execute.school','ดำเนินการเลื่อนชั้นที่อนุมัติแล้ว','academic_promotion','execute','school','สร้างข้อมูลปีถัดไปหรือเปลี่ยนสถานะตามรายการเลื่อนชั้นที่ผ่านการอนุมัติ',true),
('academic_promotion.correct.school','ปรับผลการเลื่อนชั้นหลังดำเนินการ','academic_promotion','correct','school','แก้ไขผลกระทบหลังเลื่อนชั้นโดยระบุเหตุผลและเก็บหลักฐานการปรับ',true);

INSERT INTO role_permissions (role_id,permission_id)
SELECT role.id,permission.id FROM roles role CROSS JOIN permissions permission
WHERE role.is_system AND role.code='ADMIN' AND permission.module='academic_promotion'
ON CONFLICT DO NOTHING;
