-- Canonical year settings preserve stable catalog identities and immutable documents.
ALTER TABLE budget_fiscal_years
 ADD COLUMN revision bigint NOT NULL DEFAULT 1 CHECK(revision>0),
 ADD COLUMN is_closed boolean NOT NULL DEFAULT false,
 ADD COLUMN closed_at timestamptz,
 ADD COLUMN funding_configured boolean NOT NULL DEFAULT false,
 ADD COLUMN routing jsonb NOT NULL DEFAULT '{"planSteps":[],"requestSteps":[],"adjustmentSteps":[]}',
 ADD COLUMN plan_template_id uuid REFERENCES process_templates(id),
 ADD COLUMN request_template_id uuid REFERENCES process_templates(id),
 ADD COLUMN adjustment_template_id uuid REFERENCES process_templates(id),
 ADD CONSTRAINT budget_year_close_timestamp CHECK(is_closed OR closed_at IS NULL);
-- Legacy inactive blocked new selection only and never proved financial closure.
-- All existing years remain open until explicitly closed under obligation guards.
UPDATE budget_fiscal_years y SET revision=s.revision,
 routing=s.routing,plan_template_id=s.plan_template_id,request_template_id=s.request_template_id,
 adjustment_template_id=s.adjustment_template_id FROM budget_settings s WHERE s.singleton;
CREATE TABLE budget_year_funds(
 fiscal_year_id uuid NOT NULL REFERENCES budget_fiscal_years(id),
 fund_id uuid NOT NULL REFERENCES budget_funds(id),
 name text NOT NULL CHECK(length(btrim(name)) BETWEEN 1 AND 120),active boolean NOT NULL,
 carryover bigint CHECK(carryover BETWEEN 0 AND 999999999999),
 annual_funding bigint CHECK(annual_funding BETWEEN 0 AND 999999999999),
 reserve bigint CHECK(reserve BETWEEN 0 AND 999999999999),
 received bigint CHECK(received BETWEEN 0 AND 999999999999),
 PRIMARY KEY(fiscal_year_id,fund_id),
 CHECK((carryover IS NULL)=(annual_funding IS NULL) AND (annual_funding IS NULL)=(reserve IS NULL)),
 CHECK(carryover+annual_funding BETWEEN 0 AND 999999999999),
 CHECK(reserve<=carryover+annual_funding)
);
CREATE TABLE budget_year_expense_categories(
 fiscal_year_id uuid NOT NULL REFERENCES budget_fiscal_years(id),
 expense_category_id uuid NOT NULL REFERENCES budget_expense_categories(id),
 name text NOT NULL CHECK(length(btrim(name)) BETWEEN 1 AND 120),active boolean NOT NULL,
 PRIMARY KEY(fiscal_year_id,expense_category_id)
);
INSERT INTO budget_year_funds(fiscal_year_id,fund_id,name,active)
 SELECT y.id,f.id,f.name,f.active FROM budget_fiscal_years y CROSS JOIN budget_funds f;
INSERT INTO budget_year_expense_categories(fiscal_year_id,expense_category_id,name,active)
 SELECT y.id,c.id,c.name,c.active FROM budget_fiscal_years y CROSS JOIN budget_expense_categories c;
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM budget_fiscal_years y CROSS JOIN budget_funds f
   LEFT JOIN budget_year_funds overlay ON overlay.fiscal_year_id=y.id AND overlay.fund_id=f.id
   WHERE overlay.fund_id IS NULL OR overlay.name IS DISTINCT FROM f.name OR overlay.active IS DISTINCT FROM f.active)
 OR EXISTS(SELECT 1 FROM budget_fiscal_years y CROSS JOIN budget_expense_categories c
   LEFT JOIN budget_year_expense_categories overlay ON overlay.fiscal_year_id=y.id AND overlay.expense_category_id=c.id
   WHERE overlay.expense_category_id IS NULL OR overlay.name IS DISTINCT FROM c.name OR overlay.active IS DISTINCT FROM c.active)
 THEN RAISE EXCEPTION 'BUDGET_YEAR_OVERLAY_RECONCILIATION_FAILED'; END IF;
END $$;
-- Reconcile every current and immutable plan reference before the canonical switch.
DO $$ BEGIN
 IF EXISTS(
   WITH documents AS (
     SELECT document FROM budget_projects UNION ALL SELECT document FROM budget_project_revisions
     UNION ALL SELECT document FROM budget_plan_snapshots
   )
   SELECT 1 FROM documents d
   WHERE jsonb_typeof(d.document->'project') IS DISTINCT FROM 'object'
      OR NOT EXISTS(SELECT 1 FROM budget_fiscal_years y WHERE y.id::text=d.document->'project'->>'fiscalYearId')
      OR jsonb_typeof(d.document->'activities') IS DISTINCT FROM 'array'
      OR EXISTS(SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof(d.document->'activities')='array' THEN d.document->'activities' ELSE '[]'::jsonb END) a
         WHERE jsonb_typeof(a->'lines') IS DISTINCT FROM 'array'
          OR (a->>'defaultFundId' IS NOT NULL AND NOT EXISTS(SELECT 1 FROM budget_year_funds f WHERE f.fiscal_year_id::text=d.document->'project'->>'fiscalYearId' AND f.fund_id::text=a->>'defaultFundId'))
          OR (a->>'defaultExpenseCategoryId' IS NOT NULL AND NOT EXISTS(SELECT 1 FROM budget_year_expense_categories c WHERE c.fiscal_year_id::text=d.document->'project'->>'fiscalYearId' AND c.expense_category_id::text=a->>'defaultExpenseCategoryId'))
          OR EXISTS(SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof(a->'lines')='array' THEN a->'lines' ELSE '[]'::jsonb END) l
            WHERE NOT EXISTS(SELECT 1 FROM budget_year_funds f WHERE f.fiscal_year_id::text=d.document->'project'->>'fiscalYearId' AND f.fund_id::text=l->>'fundId')
               OR NOT EXISTS(SELECT 1 FROM budget_year_expense_categories c WHERE c.fiscal_year_id::text=d.document->'project'->>'fiscalYearId' AND c.expense_category_id::text=l->>'expenseCategoryId')))
 ) THEN RAISE EXCEPTION 'BUDGET_YEAR_REFERENCE_UNMAPPABLE'; END IF;
END $$;
CREATE TABLE budget_unit_allocations(
 fiscal_year_id uuid NOT NULL,
 organization_unit_id uuid NOT NULL REFERENCES organization_units(id),
 fund_id uuid NOT NULL,
 allocated bigint NOT NULL CHECK(allocated BETWEEN 0 AND 999999999999),
 PRIMARY KEY(fiscal_year_id,organization_unit_id,fund_id),
 FOREIGN KEY(fiscal_year_id,fund_id) REFERENCES budget_year_funds(fiscal_year_id,fund_id)
);
ALTER TABLE budget_fiscal_years DROP COLUMN active;
ALTER TABLE budget_funds DROP COLUMN name,DROP COLUMN active;
ALTER TABLE budget_expense_categories DROP COLUMN name,DROP COLUMN active;
DROP TABLE budget_settings;

INSERT INTO permissions(code,name,module,action,scope,description)
 SELECT 'budget.delete.'||scope,'ลบโครงการที่ยังไม่เคยอนุมัติถาวร','budget','delete',scope,
 'ลบโครงการที่ยังไม่เคยอนุมัติพร้อมประวัติและกระบวนงานทั้งหมด'
 FROM unnest(ARRAY['own','organization_unit','organization_tree','school']) scope ON CONFLICT(code) DO NOTHING;
INSERT INTO role_permissions(role_id,permission_id)
 SELECT r.id,p.id FROM roles r CROSS JOIN permissions p WHERE p.module='budget' AND p.action='delete'
 AND (r.code IN('ADMIN','SUPER_ADMIN') OR (r.code IN('TEACHER','OFFICER','HEAD','MEMBER','DIRECTOR') AND p.scope='own') OR (r.code='DIRECTOR' AND p.scope='school')) ON CONFLICT DO NOTHING;

-- Only templates minted by annual saves acquire exclusive year ownership.
-- Legacy shared global templates have no guessed annual owner.
CREATE TABLE budget_year_template_ownership(
 fiscal_year_id uuid NOT NULL REFERENCES budget_fiscal_years(id),
 template_id uuid PRIMARY KEY REFERENCES process_templates(id)
);
CREATE TRIGGER immutable_budget_year_template_ownership BEFORE UPDATE OR DELETE ON budget_year_template_ownership
FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE FUNCTION budget_register_year_template(selected_year uuid,selected_template uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
DECLARE current_xid numeric:=mod(pg_current_xact_id()::text::numeric,4294967296);
BEGIN
 PERFORM 1 FROM budget_fiscal_years WHERE id=selected_year AND NOT is_closed FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'BUDGET_YEAR_TEMPLATE_REGISTRATION_REFUSED'; END IF;
 IF NOT EXISTS(SELECT 1 FROM process_templates t JOIN process_template_versions v ON v.template_id=t.id AND v.version=1
   WHERE t.id=selected_template AND t.managed_source_module='budget' AND t.version=1 AND t.definition=v.definition
    AND t.xmin::text::numeric=current_xid AND v.xmin::text::numeric=current_xid
    AND (SELECT count(*) FROM process_template_versions all_versions WHERE all_versions.template_id=t.id)=1)
 OR EXISTS(SELECT 1 FROM process_instances WHERE template_id=selected_template)
 OR EXISTS(SELECT 1 FROM process_instance_revisions WHERE definition->>'templateId'=selected_template::text)
 OR EXISTS(SELECT 1 FROM budget_fiscal_years WHERE plan_template_id=selected_template OR request_template_id=selected_template OR adjustment_template_id=selected_template)
 THEN RAISE EXCEPTION 'BUDGET_YEAR_TEMPLATE_REGISTRATION_REFUSED'; END IF;
 INSERT INTO budget_year_template_ownership VALUES(selected_year,selected_template);
END $$;
DO $$ DECLARE owned_schema text:=current_schema(); runtime_role text; BEGIN
 EXECUTE format('ALTER FUNCTION budget_register_year_template(uuid,uuid) SET search_path TO pg_catalog,%I,pg_temp',owned_schema);
 IF current_user ~ '^so_[0-9a-fA-F]{32}_owner$' THEN
   runtime_role:=regexp_replace(current_user,'_owner$','_runtime');
   IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname=runtime_role) THEN
     EXECUTE format('REVOKE INSERT,UPDATE,DELETE ON budget_year_template_ownership FROM %I',runtime_role);
   END IF;
 END IF;
END $$;
