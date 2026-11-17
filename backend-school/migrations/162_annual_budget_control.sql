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
