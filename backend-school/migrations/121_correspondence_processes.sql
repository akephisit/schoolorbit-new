-- Additive phase-one process/correspondence owner; existing work/windows are preserved.
CREATE TABLE process_templates (
 id uuid PRIMARY KEY DEFAULT uuidv7(), code text NOT NULL UNIQUE,
 version bigint NOT NULL CHECK(version>0), definition jsonb NOT NULL,
 created_by uuid REFERENCES users(id), created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE process_template_versions (
 template_id uuid NOT NULL REFERENCES process_templates(id), version bigint NOT NULL,
 definition jsonb NOT NULL, created_by uuid REFERENCES users(id), created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY(template_id,version)
);
CREATE TABLE process_instances (
 id uuid PRIMARY KEY DEFAULT uuidv7(), source_module text NOT NULL, source_type text NOT NULL,
 source_id uuid NOT NULL, template_id uuid NOT NULL, template_version bigint NOT NULL,
 revision bigint NOT NULL CHECK(revision>0), definition jsonb NOT NULL,
 created_by uuid NOT NULL REFERENCES users(id), created_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(source_module,source_type,source_id),
 FOREIGN KEY(template_id,template_version) REFERENCES process_template_versions(template_id,version)
);
CREATE TABLE process_participants (
 instance_id uuid NOT NULL REFERENCES process_instances(id), user_id uuid NOT NULL REFERENCES users(id),
 PRIMARY KEY(instance_id,user_id)
);
CREATE TABLE correspondence_records (
 id uuid PRIMARY KEY DEFAULT uuidv7(), revision bigint NOT NULL CHECK(revision>0),
 created_by uuid NOT NULL REFERENCES users(id), organization_unit_id uuid REFERENCES organization_units(id),
 summary jsonb NOT NULL, source_links jsonb NOT NULL DEFAULT '[]',
 current_document_revision bigint NOT NULL, instance_id uuid UNIQUE REFERENCES process_instances(id),
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE correspondence_document_revisions (
 correspondence_id uuid NOT NULL REFERENCES correspondence_records(id), revision bigint NOT NULL,
 document jsonb NOT NULL, created_by uuid NOT NULL REFERENCES users(id), created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY(correspondence_id,revision)
);
CREATE TABLE correspondence_file_links (
 correspondence_id uuid NOT NULL REFERENCES correspondence_records(id),
 file_id uuid NOT NULL REFERENCES files(id), document_revision bigint NOT NULL,
 PRIMARY KEY(correspondence_id,file_id,document_revision),
 FOREIGN KEY(correspondence_id,document_revision) REFERENCES correspondence_document_revisions(correspondence_id,revision)
);
CREATE TABLE correspondence_signature_assets (
 id uuid PRIMARY KEY DEFAULT uuidv7(), owner_id uuid NOT NULL REFERENCES users(id),
 label text NOT NULL, file_id uuid NOT NULL UNIQUE REFERENCES files(id), created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE process_action_history (
 id uuid PRIMARY KEY DEFAULT uuidv7(), instance_id uuid REFERENCES process_instances(id),
 correspondence_id uuid REFERENCES correspondence_records(id), actor_id uuid NOT NULL REFERENCES users(id),
 entry jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 CHECK(instance_id IS NOT NULL OR correspondence_id IS NOT NULL)
);
CREATE TABLE correspondence_signing_evidence (
 id uuid PRIMARY KEY DEFAULT uuidv7(), correspondence_id uuid NOT NULL REFERENCES correspondence_records(id),
 document_revision bigint NOT NULL, signer_id uuid NOT NULL REFERENCES users(id),
 asset_file_id uuid REFERENCES files(id), action_id uuid NOT NULL REFERENCES process_action_history(id),
 evidence jsonb NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 FOREIGN KEY(correspondence_id,document_revision) REFERENCES correspondence_document_revisions(correspondence_id,revision)
);
CREATE TABLE workflow_mutation_receipts (
 actor_id uuid NOT NULL REFERENCES users(id), request_id uuid NOT NULL, operation text NOT NULL,
 payload_hash text NOT NULL, resource_id uuid NOT NULL, created_at timestamptz NOT NULL DEFAULT now(),
 PRIMARY KEY(actor_id,request_id)
);
CREATE INDEX process_participants_user_idx ON process_participants(user_id,instance_id);
CREATE INDEX correspondence_creator_idx ON correspondence_records(created_by,id);
CREATE INDEX correspondence_org_idx ON correspondence_records(organization_unit_id,id);
CREATE INDEX process_history_instance_idx ON process_action_history(instance_id,created_at,id);
CREATE INDEX process_history_correspondence_idx ON process_action_history(correspondence_id,created_at,id);
CREATE FUNCTION preserve_workflow_evidence() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'WORKFLOW_EVIDENCE_IMMUTABLE'; END $$;
CREATE TRIGGER immutable_process_versions BEFORE UPDATE OR DELETE ON process_template_versions FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TRIGGER immutable_document_revisions BEFORE UPDATE OR DELETE ON correspondence_document_revisions FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TRIGGER immutable_action_history BEFORE UPDATE OR DELETE ON process_action_history FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TRIGGER immutable_signing_evidence BEFORE UPDATE OR DELETE ON correspondence_signing_evidence FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
CREATE TRIGGER immutable_correspondence_files BEFORE UPDATE OR DELETE ON correspondence_file_links FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();
-- Prevent lifecycle deletion from racing attachment or discarding immutable revision evidence.
CREATE FUNCTION protect_correspondence_files() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF (NEW.deleted_at IS NOT NULL OR NEW.lifecycle_status IN ('delete_requested','deleted') OR NEW.current_version_id IS DISTINCT FROM OLD.current_version_id) AND
  (EXISTS(SELECT 1 FROM correspondence_file_links WHERE file_id=OLD.id)
   OR EXISTS(SELECT 1 FROM correspondence_signature_assets WHERE file_id=OLD.id)
   OR EXISTS(SELECT 1 FROM correspondence_signing_evidence WHERE asset_file_id=OLD.id)) THEN
  RAISE EXCEPTION 'CORRESPONDENCE_FILE_ATTACHED';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER protect_correspondence_file_lifecycle BEFORE UPDATE ON files FOR EACH ROW EXECUTE FUNCTION protect_correspondence_files();

INSERT INTO process_templates(id,code,version,definition) VALUES
('0199ebd0-0000-7000-8000-000000000001','correspondence_circulation',1,
'{"id":"0199ebd0-0000-7000-8000-000000000001","code":"correspondence_circulation","title":"เวียนหนังสือเพื่อทราบ","version":1,"steps":[{"title":"รับทราบหนังสือ","kind":"acknowledge","completionPolicy":"all","recipients":[],"dueAfterDays":3,"requiresSignature":false,"requiresComment":false,"requiresEvidence":false,"requiresDelegatedAuthority":false}],"active":true,"createdAt":"2026-10-18T00:00:00Z"}'),
('0199ebd0-0000-7000-8000-000000000002','correspondence_directed',1,
'{"id":"0199ebd0-0000-7000-8000-000000000002","code":"correspondence_directed","title":"เสนอ พิจารณา และดำเนินการ","version":1,"steps":[{"title":"พิจารณาหนังสือ","kind":"review","completionPolicy":"all","recipients":[],"dueAfterDays":3,"requiresSignature":false,"requiresComment":true,"requiresEvidence":false,"requiresDelegatedAuthority":false},{"title":"อนุมัติ","kind":"approve","completionPolicy":"any","recipients":[],"dueAfterDays":3,"requiresSignature":false,"requiresComment":false,"requiresEvidence":false,"requiresDelegatedAuthority":false},{"title":"ดำเนินการและรายงาน","kind":"execute","completionPolicy":"all","recipients":[],"dueAfterDays":7,"requiresSignature":false,"requiresComment":true,"requiresEvidence":false,"requiresDelegatedAuthority":false}],"active":true,"createdAt":"2026-10-18T00:00:00Z"}');
INSERT INTO process_template_versions(template_id,version,definition) SELECT id,version,definition FROM process_templates;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.read.own','ดูหนังสือของตนเอง','correspondence','read','own','ดูหนังสือของตนเอง') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.read.assigned','ดูหนังสือที่มีส่วนร่วม','correspondence','read','assigned','ดูหนังสือที่มีส่วนร่วม') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.read.organization_unit','ดูหนังสือหน่วยงานที่ได้รับสิทธิ์','correspondence','read','organization_unit','ดูหนังสือหน่วยงานที่ได้รับสิทธิ์') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.read.school','ดูหนังสือทั้งโรงเรียน','correspondence','read','school','ดูหนังสือทั้งโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.create.own','สร้างและส่งหนังสือของตนเอง','correspondence','create','own','สร้างและส่งหนังสือของตนเอง') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.manage.organization_unit','จัดการหนังสือหน่วยงานที่ได้รับสิทธิ์','correspondence','manage','organization_unit','จัดการหนังสือหน่วยงานที่ได้รับสิทธิ์') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.manage.school','จัดการหนังสือทั้งโรงเรียน','correspondence','manage','school','จัดการหนังสือทั้งโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.update.assigned','ดำเนินการหนังสือที่ได้รับมอบหมาย','correspondence','update','assigned','ดำเนินการหนังสือที่ได้รับมอบหมาย') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.sign.own','ลงนามหนังสือด้วยตนเอง','correspondence','sign','own','ลงนามหนังสือด้วยตนเอง') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('correspondence.download.assigned','ส่งออกหนังสือที่เข้าถึงได้','correspondence','download','assigned','ส่งออกหนังสือที่เข้าถึงได้') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('process.manage.school','จัดการแม่แบบกระบวนงาน','process','manage','school','จัดการแม่แบบกระบวนงาน') ON CONFLICT(code) DO NOTHING;
INSERT INTO role_permissions(role_id,permission_id) SELECT r.id,p.id FROM roles r CROSS JOIN permissions p WHERE r.code IN ('TEACHER','OFFICER','HEAD','MEMBER','DIRECTOR') AND p.code IN ('correspondence.read.own','correspondence.read.assigned','correspondence.create.own','correspondence.update.assigned','correspondence.sign.own','correspondence.download.assigned') ON CONFLICT DO NOTHING;

CREATE TABLE process_notification_sources (id uuid PRIMARY KEY,instance_id uuid NOT NULL REFERENCES process_instances(id),recipient_id uuid NOT NULL REFERENCES users(id));
ALTER TABLE notification_dispatch_jobs DROP CONSTRAINT notification_dispatch_jobs_source_kind_check;
ALTER TABLE notification_dispatch_jobs DROP CONSTRAINT notification_dispatch_jobs_check;
ALTER TABLE notification_dispatch_jobs ADD CONSTRAINT notification_dispatch_jobs_source_kind_check CHECK(source_kind IN ('direct','attendance','process'));
ALTER TABLE notification_dispatch_jobs ADD CONSTRAINT notification_dispatch_jobs_check CHECK((source_kind='direct' AND source_id IS NULL) OR (source_kind IN ('attendance','process') AND source_id IS NOT NULL));
CREATE TABLE correspondence_register_counters(register text NOT NULL,year integer NOT NULL,last_number bigint NOT NULL,PRIMARY KEY(register,year));
ALTER TABLE correspondence_records ADD COLUMN register_number bigint NOT NULL,ADD COLUMN register_year integer NOT NULL,ADD COLUMN register_kind text NOT NULL;
ALTER TABLE correspondence_records ADD CONSTRAINT correspondence_register_number_unique UNIQUE(register_kind,register_year,register_number);

CREATE TABLE correspondence_file_uploads(file_id uuid PRIMARY KEY REFERENCES files(id),correspondence_id uuid NOT NULL REFERENCES correspondence_records(id),uploaded_by uuid NOT NULL REFERENCES users(id));

CREATE TABLE process_instance_revisions(instance_id uuid NOT NULL REFERENCES process_instances(id),revision bigint NOT NULL,definition jsonb NOT NULL,created_at timestamptz NOT NULL DEFAULT now(),PRIMARY KEY(instance_id,revision));
CREATE TRIGGER immutable_process_snapshots BEFORE UPDATE OR DELETE ON process_instance_revisions FOR EACH ROW EXECUTE FUNCTION preserve_workflow_evidence();

INSERT INTO menu_groups(id,code,name,name_en,icon,display_order,is_active,workspace_code) VALUES(uuidv7(),'correspondence','งานสารบรรณ','Correspondence','mail',15,true,'operations') ON CONFLICT(code) DO NOTHING;
