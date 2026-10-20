-- Additive Budget owner. Existing tenants and all previous owner identities are preserved.
CREATE TABLE budget_settings (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),revision bigint NOT NULL CHECK(revision>0),
 routing jsonb NOT NULL,plan_template_id uuid REFERENCES process_templates(id),
 request_template_id uuid REFERENCES process_templates(id),adjustment_template_id uuid REFERENCES process_templates(id)
);
INSERT INTO budget_settings(singleton,revision,routing) VALUES(true,1,'{"planSteps":[],"requestSteps":[],"adjustmentSteps":[]}');
CREATE TABLE budget_fiscal_years(id uuid PRIMARY KEY,name text NOT NULL CHECK(length(btrim(name)) BETWEEN 1 AND 120),start_date date NOT NULL,end_date date NOT NULL,active boolean NOT NULL,CHECK(end_date>=start_date));
CREATE TABLE budget_funds(id uuid PRIMARY KEY,name text NOT NULL CHECK(length(btrim(name)) BETWEEN 1 AND 120),active boolean NOT NULL);
CREATE TABLE budget_expense_categories(id uuid PRIMARY KEY,name text NOT NULL CHECK(length(btrim(name)) BETWEEN 1 AND 120),active boolean NOT NULL);
CREATE TABLE budget_projects(
 id uuid PRIMARY KEY,owner_id uuid NOT NULL REFERENCES users(id),fiscal_year_id uuid NOT NULL REFERENCES budget_fiscal_years(id),
 organization_unit_id uuid NOT NULL REFERENCES organization_units(id),revision bigint NOT NULL CHECK(revision>0),
 status text NOT NULL CHECK(status IN('draft','submitted','returned','approved','rejected')),
 attempt_id uuid NOT NULL UNIQUE,accepted_version bigint,accepted_snapshot_id uuid,
 document jsonb NOT NULL,instance_id uuid REFERENCES process_instances(id),created_at timestamptz NOT NULL DEFAULT now(),
 CHECK((accepted_version IS NULL)=(accepted_snapshot_id IS NULL))
);
CREATE INDEX budget_projects_scope_idx ON budget_projects(fiscal_year_id,organization_unit_id,created_at DESC,id DESC);
CREATE INDEX budget_projects_owner_idx ON budget_projects(owner_id,created_at DESC,id DESC);
CREATE TABLE budget_activities(id uuid PRIMARY KEY,project_id uuid NOT NULL REFERENCES budget_projects(id),UNIQUE(id,project_id));
CREATE TABLE budget_plan_lines(id uuid PRIMARY KEY,project_id uuid NOT NULL REFERENCES budget_projects(id),activity_id uuid NOT NULL,UNIQUE(id,project_id,activity_id),FOREIGN KEY(activity_id,project_id) REFERENCES budget_activities(id,project_id));
CREATE TABLE budget_plan_snapshots(id uuid PRIMARY KEY,project_id uuid NOT NULL REFERENCES budget_projects(id),version bigint NOT NULL CHECK(version>0),document jsonb NOT NULL,accepted_by uuid NOT NULL REFERENCES users(id),accepted_at timestamptz NOT NULL DEFAULT now(),UNIQUE(project_id,version),UNIQUE(id,project_id));
ALTER TABLE budget_projects ADD CONSTRAINT budget_project_snapshot_fk FOREIGN KEY(accepted_snapshot_id,id) REFERENCES budget_plan_snapshots(id,project_id);
CREATE TABLE budget_project_revisions(project_id uuid NOT NULL REFERENCES budget_projects(id),revision bigint NOT NULL,document jsonb NOT NULL,PRIMARY KEY(project_id,revision));
CREATE TABLE budget_allocations(
 project_id uuid NOT NULL,activity_id uuid NOT NULL,fund_id uuid NOT NULL REFERENCES budget_funds(id),expense_category_id uuid NOT NULL REFERENCES budget_expense_categories(id),
 planned bigint NOT NULL DEFAULT 0 CHECK(planned BETWEEN 0 AND 999999999999),adjusted bigint NOT NULL DEFAULT 0 CHECK(adjusted BETWEEN -999999999999 AND 999999999999),
 reserved bigint NOT NULL DEFAULT 0 CHECK(reserved BETWEEN 0 AND 999999999999),paid bigint NOT NULL DEFAULT 0 CHECK(paid BETWEEN 0 AND 999999999999),
 PRIMARY KEY(project_id,activity_id,fund_id,expense_category_id),FOREIGN KEY(activity_id,project_id) REFERENCES budget_activities(id,project_id),
 CHECK(planned+adjusted BETWEEN 0 AND 999999999999),CHECK(planned+adjusted>=reserved+paid)
);
CREATE TABLE budget_requests(
 id uuid PRIMARY KEY,project_id uuid NOT NULL REFERENCES budget_projects(id),activity_id uuid NOT NULL,owner_id uuid NOT NULL REFERENCES users(id),
 revision bigint NOT NULL CHECK(revision>0),status text NOT NULL CHECK(status IN('draft','submitted','returned','approved','rejected','closed')),
 document jsonb NOT NULL,instance_id uuid REFERENCES process_instances(id),created_at timestamptz NOT NULL DEFAULT now(),
 FOREIGN KEY(activity_id,project_id) REFERENCES budget_activities(id,project_id),UNIQUE(id,project_id,activity_id)
);
CREATE INDEX budget_requests_project_idx ON budget_requests(project_id,created_at DESC,id DESC);
CREATE TABLE budget_request_lines(
 request_id uuid NOT NULL,project_id uuid NOT NULL,activity_id uuid NOT NULL,plan_line_id uuid NOT NULL,
 fund_id uuid NOT NULL REFERENCES budget_funds(id),expense_category_id uuid NOT NULL REFERENCES budget_expense_categories(id),
 description text NOT NULL,unit text NOT NULL,fund_name text NOT NULL,expense_category_name text NOT NULL,
 amount bigint NOT NULL CHECK(amount BETWEEN 1 AND 999999999999),reserved bigint NOT NULL DEFAULT 0 CHECK(reserved>=0),paid bigint NOT NULL DEFAULT 0 CHECK(paid>=0),
 PRIMARY KEY(request_id,plan_line_id),FOREIGN KEY(request_id,project_id,activity_id) REFERENCES budget_requests(id,project_id,activity_id),
 FOREIGN KEY(plan_line_id,project_id,activity_id) REFERENCES budget_plan_lines(id,project_id,activity_id),CHECK(reserved+paid<=amount)
);
CREATE TABLE budget_request_revisions(request_id uuid NOT NULL REFERENCES budget_requests(id),revision bigint NOT NULL,document jsonb NOT NULL,PRIMARY KEY(request_id,revision));
CREATE TABLE budget_payments(id uuid PRIMARY KEY,request_id uuid NOT NULL REFERENCES budget_requests(id),document jsonb NOT NULL,created_at timestamptz NOT NULL DEFAULT now(),UNIQUE(id,request_id));
CREATE TABLE budget_payment_lines(payment_id uuid NOT NULL,request_id uuid NOT NULL,plan_line_id uuid NOT NULL,amount bigint NOT NULL CHECK(amount BETWEEN 1 AND 999999999999),PRIMARY KEY(payment_id,plan_line_id),FOREIGN KEY(payment_id,request_id) REFERENCES budget_payments(id,request_id),FOREIGN KEY(request_id,plan_line_id) REFERENCES budget_request_lines(request_id,plan_line_id));
CREATE TABLE budget_refunds(id uuid PRIMARY KEY,payment_id uuid NOT NULL REFERENCES budget_payments(id),request_id uuid NOT NULL REFERENCES budget_requests(id),document jsonb NOT NULL,created_at timestamptz NOT NULL DEFAULT now(),FOREIGN KEY(payment_id,request_id) REFERENCES budget_payments(id,request_id));
CREATE TABLE budget_refund_lines(refund_id uuid NOT NULL REFERENCES budget_refunds(id),payment_id uuid NOT NULL,plan_line_id uuid NOT NULL,amount bigint NOT NULL CHECK(amount BETWEEN 1 AND 999999999999),PRIMARY KEY(refund_id,plan_line_id),FOREIGN KEY(payment_id,plan_line_id) REFERENCES budget_payment_lines(payment_id,plan_line_id));
CREATE TABLE budget_adjustments(
 id uuid PRIMARY KEY,owner_id uuid NOT NULL REFERENCES users(id),fiscal_year_id uuid NOT NULL REFERENCES budget_fiscal_years(id),
 from_project_id uuid REFERENCES budget_projects(id),to_project_id uuid REFERENCES budget_projects(id),
 revision bigint NOT NULL CHECK(revision>0),status text NOT NULL CHECK(status IN('draft','submitted','returned','approved','rejected')),
 document jsonb NOT NULL,instance_id uuid REFERENCES process_instances(id),created_at timestamptz NOT NULL DEFAULT now(),CHECK(from_project_id IS NOT NULL OR to_project_id IS NOT NULL)
);
CREATE INDEX budget_adjustments_from_idx ON budget_adjustments(from_project_id,created_at DESC,id DESC);
CREATE INDEX budget_adjustments_to_idx ON budget_adjustments(to_project_id,created_at DESC,id DESC);
CREATE TABLE budget_loans(id uuid PRIMARY KEY REFERENCES budget_adjustments(id),amount bigint NOT NULL CHECK(amount BETWEEN 1 AND 999999999999),repaid bigint NOT NULL DEFAULT 0 CHECK(repaid>=0),document jsonb NOT NULL,CHECK(repaid<=amount));
CREATE TABLE budget_repayments(adjustment_id uuid PRIMARY KEY REFERENCES budget_adjustments(id),loan_id uuid NOT NULL REFERENCES budget_loans(id),amount bigint NOT NULL CHECK(amount BETWEEN 1 AND 999999999999));
CREATE TABLE budget_ledger(
 id uuid PRIMARY KEY,project_id uuid NOT NULL,activity_id uuid NOT NULL,fund_id uuid NOT NULL,expense_category_id uuid NOT NULL,
 planned_delta bigint NOT NULL DEFAULT 0,adjusted_delta bigint NOT NULL DEFAULT 0,reserved_delta bigint NOT NULL DEFAULT 0,paid_delta bigint NOT NULL DEFAULT 0,
 source_kind text NOT NULL,source_id uuid NOT NULL,actor_id uuid NOT NULL REFERENCES users(id),created_at timestamptz NOT NULL DEFAULT now(),
 FOREIGN KEY(project_id,activity_id,fund_id,expense_category_id) REFERENCES budget_allocations(project_id,activity_id,fund_id,expense_category_id),
 CHECK(planned_delta<>0 OR adjusted_delta<>0 OR reserved_delta<>0 OR paid_delta<>0)
);
CREATE INDEX budget_ledger_project_idx ON budget_ledger(project_id,created_at,id);
CREATE TABLE budget_history(id uuid PRIMARY KEY,resource_kind text NOT NULL CHECK(resource_kind IN('project','request','adjustment','settings')),resource_id uuid NOT NULL,revision bigint NOT NULL,actor_id uuid NOT NULL REFERENCES users(id),document jsonb NOT NULL,created_at timestamptz NOT NULL DEFAULT now());
CREATE INDEX budget_history_resource_idx ON budget_history(resource_kind,resource_id,revision,id);
CREATE TABLE budget_receipts(actor_id uuid NOT NULL REFERENCES users(id),request_id uuid NOT NULL,operation text NOT NULL,payload_hash text NOT NULL,resource_id uuid NOT NULL,PRIMARY KEY(actor_id,request_id));
CREATE TABLE budget_file_uploads(file_id uuid PRIMARY KEY REFERENCES files(id),request_id uuid NOT NULL REFERENCES budget_requests(id),uploaded_by uuid NOT NULL REFERENCES users(id));
CREATE TABLE budget_file_links(request_id uuid NOT NULL REFERENCES budget_requests(id),revision bigint NOT NULL,file_id uuid NOT NULL REFERENCES files(id),version_id uuid NOT NULL REFERENCES file_versions(id),checksum text NOT NULL,PRIMARY KEY(request_id,revision,file_id),FOREIGN KEY(request_id,revision) REFERENCES budget_request_revisions(request_id,revision));
CREATE FUNCTION preserve_budget_evidence() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'BUDGET_IMMUTABLE_EVIDENCE'; END $$;
CREATE TRIGGER immutable_budget_snapshot BEFORE UPDATE OR DELETE ON budget_plan_snapshots FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_project_revision BEFORE UPDATE OR DELETE ON budget_project_revisions FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_request_revision BEFORE UPDATE OR DELETE ON budget_request_revisions FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_payment BEFORE UPDATE OR DELETE ON budget_payments FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_payment_line BEFORE UPDATE OR DELETE ON budget_payment_lines FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_refund BEFORE UPDATE OR DELETE ON budget_refunds FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_refund_line BEFORE UPDATE OR DELETE ON budget_refund_lines FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_ledger BEFORE UPDATE OR DELETE ON budget_ledger FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_history BEFORE UPDATE OR DELETE ON budget_history FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_receipt BEFORE UPDATE OR DELETE ON budget_receipts FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_file_link BEFORE UPDATE OR DELETE ON budget_file_links FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_repayment BEFORE UPDATE OR DELETE ON budget_repayments FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE FUNCTION protect_budget_files() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
 IF (NEW.deleted_at IS NOT NULL OR NEW.lifecycle_status IN('delete_requested','deleted') OR NEW.current_version_id IS DISTINCT FROM OLD.current_version_id) AND EXISTS(SELECT 1 FROM budget_file_links WHERE file_id=OLD.id) THEN RAISE EXCEPTION 'BUDGET_FILE_ATTACHED'; END IF; RETURN NEW; END $$;
CREATE TRIGGER protect_budget_file_lifecycle BEFORE UPDATE ON files FOR EACH ROW EXECUTE FUNCTION protect_budget_files();
INSERT INTO menu_groups(id,code,name,name_en,icon,display_order,is_active,workspace_code) VALUES
 (uuidv7(),'budget_planning','แผนงานและโครงการ','Budget Planning','notebook-pen',20,true,'budget'),
 (uuidv7(),'budget_finance','การเงินและงบประมาณ','Budget Finance','wallet',30,true,'budget') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.read.own','ดูโครงการและงบประมาณ','budget','read','own','ดูโครงการและงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.read.assigned','ดูโครงการและงบประมาณ','budget','read','assigned','ดูโครงการและงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.read.organization_unit','ดูโครงการและงบประมาณ','budget','read','organization_unit','ดูโครงการและงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.read.organization_tree','ดูโครงการและงบประมาณ','budget','read','organization_tree','ดูโครงการและงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.read.school','ดูโครงการและงบประมาณ','budget','read','school','ดูโครงการและงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.create.own','สร้างโครงการและคำขอใช้งบประมาณ','budget','create','own','สร้างโครงการและคำขอใช้งบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.update.own','แก้ไขโครงการและคำขอใช้งบประมาณ','budget','update','own','แก้ไขโครงการและคำขอใช้งบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.update.organization_unit','แก้ไขโครงการและคำขอใช้งบประมาณ','budget','update','organization_unit','แก้ไขโครงการและคำขอใช้งบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.update.organization_tree','แก้ไขโครงการและคำขอใช้งบประมาณ','budget','update','organization_tree','แก้ไขโครงการและคำขอใช้งบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.update.school','แก้ไขโครงการและคำขอใช้งบประมาณ','budget','update','school','แก้ไขโครงการและคำขอใช้งบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.approve.assigned','พิจารณางบประมาณที่ได้รับมอบหมาย','budget','approve','assigned','พิจารณางบประมาณที่ได้รับมอบหมาย') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget_payment.execute.school','บันทึกจ่ายและคืนเงินงบประมาณ','budget_payment','execute','school','บันทึกจ่ายและคืนเงินงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget_adjustment.create.school','จัดทำรายการปรับและยืมงบประมาณ','budget_adjustment','create','school','จัดทำรายการปรับและยืมงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.manage.school','ตั้งค่างบประมาณโรงเรียน','budget','manage','school','ตั้งค่างบประมาณโรงเรียน') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.download.own','ส่งออกรายงานงบประมาณ','budget','download','own','ส่งออกรายงานงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.download.organization_unit','ส่งออกรายงานงบประมาณ','budget','download','organization_unit','ส่งออกรายงานงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.download.organization_tree','ส่งออกรายงานงบประมาณ','budget','download','organization_tree','ส่งออกรายงานงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO permissions(code,name,module,action,scope,description) VALUES('budget.download.school','ส่งออกรายงานงบประมาณ','budget','download','school','ส่งออกรายงานงบประมาณ') ON CONFLICT(code) DO NOTHING;
INSERT INTO role_permissions(role_id,permission_id) SELECT r.id,p.id FROM roles r CROSS JOIN permissions p WHERE (r.code IN('ADMIN','SUPER_ADMIN') AND p.module IN('budget','budget_payment','budget_adjustment')) OR(r.code IN('TEACHER','OFFICER','HEAD','MEMBER','DIRECTOR') AND p.code IN('budget.read.own','budget.read.assigned','budget.create.own','budget.update.own','budget.approve.assigned','budget.download.own')) OR(r.code='DIRECTOR' AND p.module='budget' AND p.action IN('read','update','download','manage')) ON CONFLICT DO NOTHING;
