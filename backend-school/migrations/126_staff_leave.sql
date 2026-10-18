-- Additive Staff-owned leave and planned school-exit capability.
CREATE TABLE staff_leave_types(id uuid PRIMARY KEY DEFAULT uuidv7(),code text NOT NULL UNIQUE,kind text NOT NULL CHECK(kind IN ('leave','exit')),revision bigint NOT NULL CHECK(revision>0),document jsonb NOT NULL);
CREATE TABLE staff_leave_policies(id uuid PRIMARY KEY,type_id uuid NOT NULL REFERENCES staff_leave_types(id),version bigint NOT NULL,effective_from date NOT NULL,document jsonb NOT NULL,created_by uuid NOT NULL REFERENCES users(id),UNIQUE(type_id,version));
CREATE TABLE staff_leave_policy_categories(policy_id uuid NOT NULL REFERENCES staff_leave_policies(id),type_id uuid NOT NULL REFERENCES staff_leave_types(id),personnel_type text NOT NULL,effective_from date NOT NULL,PRIMARY KEY(policy_id,personnel_type));
CREATE TABLE staff_leave_requests(id uuid PRIMARY KEY,owner_id uuid NOT NULL REFERENCES users(id),type_id uuid NOT NULL REFERENCES staff_leave_types(id),status text NOT NULL,revision bigint NOT NULL,document jsonb NOT NULL,policy_id uuid REFERENCES staff_leave_policies(id),instance_id uuid REFERENCES process_instances(id),cancellation_id uuid,quota_year_start date,amount bigint NOT NULL DEFAULT 0,content_hash text NOT NULL,evidence_file_ids uuid[] NOT NULL DEFAULT '{}',created_at timestamptz NOT NULL DEFAULT now());
CREATE INDEX staff_leave_owner_idx ON staff_leave_requests(owner_id,created_at,id);
CREATE INDEX staff_leave_quota_idx ON staff_leave_requests(owner_id,type_id,quota_year_start,status);
CREATE TABLE staff_leave_revisions(request_id uuid NOT NULL REFERENCES staff_leave_requests(id),revision bigint NOT NULL,document jsonb NOT NULL,content_hash text NOT NULL,PRIMARY KEY(request_id,revision));
CREATE TABLE staff_leave_history(id uuid PRIMARY KEY,request_id uuid NOT NULL REFERENCES staff_leave_requests(id),actor_id uuid NOT NULL REFERENCES users(id),entry jsonb NOT NULL);
CREATE TABLE staff_leave_receipts(actor_id uuid NOT NULL REFERENCES users(id),request_id uuid NOT NULL,operation text NOT NULL,payload_hash text NOT NULL,resource_id uuid NOT NULL,PRIMARY KEY(actor_id,request_id));
CREATE TABLE staff_leave_file_uploads(file_id uuid PRIMARY KEY REFERENCES files(id),request_id uuid NOT NULL REFERENCES staff_leave_requests(id),uploaded_by uuid NOT NULL REFERENCES users(id));
CREATE TABLE staff_leave_file_links(request_id uuid NOT NULL REFERENCES staff_leave_requests(id),revision bigint NOT NULL,file_id uuid NOT NULL REFERENCES files(id),version_id uuid NOT NULL REFERENCES file_versions(id),checksum text NOT NULL,PRIMARY KEY(request_id,revision,file_id),FOREIGN KEY(request_id,revision) REFERENCES staff_leave_revisions(request_id,revision));
CREATE TRIGGER immutable_staff_leave_policy BEFORE UPDATE OR DELETE ON staff_leave_policies FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TRIGGER immutable_staff_leave_categories BEFORE UPDATE OR DELETE ON staff_leave_policy_categories FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TRIGGER immutable_staff_leave_revision BEFORE UPDATE OR DELETE ON staff_leave_revisions FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TRIGGER immutable_staff_leave_history BEFORE UPDATE OR DELETE ON staff_leave_history FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TRIGGER immutable_staff_leave_files BEFORE UPDATE OR DELETE ON staff_leave_file_links FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE FUNCTION protect_staff_leave_files() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF (NEW.deleted_at IS NOT NULL OR NEW.lifecycle_status IN ('delete_requested','deleted') OR NEW.current_version_id IS DISTINCT FROM OLD.current_version_id) AND EXISTS(SELECT 1 FROM staff_leave_file_links WHERE file_id=OLD.id) THEN RAISE EXCEPTION 'STAFF_LEAVE_FILE_ATTACHED'; END IF; RETURN NEW; END $$;
CREATE TRIGGER protect_staff_leave_file_lifecycle BEFORE UPDATE ON files FOR EACH ROW EXECUTE FUNCTION protect_staff_leave_files();
INSERT INTO staff_leave_types(id,code,kind,revision,document) SELECT uuidv7(),code,kind,1,jsonb_build_object('id',null,'code',code,'name',name,'kind',kind,'archived',false,'revision',1) FROM (VALUES('sick','ลาป่วย','leave'),('personal','ลากิจ','leave'),('school_exit','ขออนุญาตออกนอกสถานศึกษา','exit')) AS t(code,name,kind);
UPDATE staff_leave_types SET document=jsonb_set(document,'{id}',to_jsonb(id));

ALTER TABLE process_templates ADD COLUMN managed_source_module text;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('staff_leave.read.own','ดูคำขอลาและขอออกนอกสถานศึกษาของตนเอง','staff_leave','read','own','ดูคำขอลาและขอออกนอกสถานศึกษาของตนเอง') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('staff_leave.read.assigned','ดูคำขอลาที่ได้รับมอบหมาย','staff_leave','read','assigned','ดูคำขอลาที่ได้รับมอบหมาย') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('staff_leave.read.school','ดูคำขอลาทั้งโรงเรียน','staff_leave','read','school','ดูคำขอลาทั้งโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('staff_leave.create.own','สร้างและส่งคำขอลาของตนเอง','staff_leave','create','own','สร้างและส่งคำขอลาของตนเอง') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('staff_leave.update.assigned','พิจารณาคำขอลาที่ได้รับมอบหมาย','staff_leave','update','assigned','พิจารณาคำขอลาที่ได้รับมอบหมาย') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('staff_leave.manage.school','จัดการประเภทและเกณฑ์การลาของโรงเรียน','staff_leave','manage','school','จัดการประเภทและเกณฑ์การลาของโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('staff_leave.download.own','ดูและส่งออกรายงานการลาของตนเอง','staff_leave','download','own','ดูและส่งออกรายงานการลาของตนเอง') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('staff_leave.download.school','ดูและส่งออกรายงานการลาทั้งโรงเรียน','staff_leave','download','school','ดูและส่งออกรายงานการลาทั้งโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO role_permissions(role_id,permission_id) SELECT r.id,p.id FROM roles r CROSS JOIN permissions p WHERE (r.code IN ('TEACHER','OFFICER','HEAD','MEMBER','DIRECTOR') AND p.code IN ('staff_leave.read.own','staff_leave.read.assigned','staff_leave.create.own','staff_leave.update.assigned','staff_leave.download.own')) OR (r.code='DIRECTOR' AND p.module='staff_leave') ON CONFLICT DO NOTHING;
CREATE INDEX staff_leave_category_resolution_idx ON staff_leave_policy_categories(type_id,personnel_type,effective_from DESC,policy_id);
CREATE INDEX staff_leave_history_request_idx ON staff_leave_history(request_id,id);
