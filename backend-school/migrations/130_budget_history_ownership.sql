-- Preserve every plan-attempt identity and exact known adjustment revisions.
-- Older binaries must not run after the compiled migration timeline advances.
CREATE TABLE budget_project_attempts (
 id uuid PRIMARY KEY,project_id uuid NOT NULL REFERENCES budget_projects(id),UNIQUE(id,project_id)
);
DO $$ BEGIN
 IF EXISTS(
  SELECT id FROM (
   SELECT attempt_id AS id,id AS project_id FROM budget_projects
   UNION SELECT id,project_id FROM budget_plan_snapshots
  ) candidates GROUP BY id HAVING count(DISTINCT project_id)>1
 ) THEN RAISE EXCEPTION 'BUDGET_ATTEMPT_HISTORY_AMBIGUOUS'; END IF;
END $$;
INSERT INTO budget_project_attempts(id,project_id)
 SELECT attempt_id,id FROM budget_projects UNION SELECT id,project_id FROM budget_plan_snapshots;
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM process_instances p WHERE p.source_module='budget' AND p.source_type='budget_project_plan'
           AND NOT EXISTS(SELECT 1 FROM budget_project_attempts a WHERE a.id=p.source_id))
 THEN RAISE EXCEPTION 'BUDGET_ATTEMPT_HISTORY_UNMAPPABLE'; END IF;
END $$;
ALTER TABLE budget_projects ADD CONSTRAINT budget_project_attempt_owner_fk
 FOREIGN KEY(attempt_id,id) REFERENCES budget_project_attempts(id,project_id) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE budget_plan_snapshots ADD CONSTRAINT budget_snapshot_attempt_owner_fk
 FOREIGN KEY(id,project_id) REFERENCES budget_project_attempts(id,project_id);
CREATE TABLE budget_adjustment_revisions(
 adjustment_id uuid NOT NULL REFERENCES budget_adjustments(id),revision bigint NOT NULL CHECK(revision>0),
 status text NOT NULL CHECK(status IN('draft','submitted','returned','approved','rejected')),document jsonb NOT NULL,
 PRIMARY KEY(adjustment_id,revision)
);
-- Only the known current document is recoverable from 129. Do not invent past drafts.
INSERT INTO budget_adjustment_revisions(adjustment_id,revision,status,document)
 SELECT id,revision,status,document FROM budget_adjustments;
CREATE TRIGGER immutable_budget_attempt BEFORE UPDATE OR DELETE ON budget_project_attempts
 FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
CREATE TRIGGER immutable_budget_adjustment_revision BEFORE UPDATE OR DELETE ON budget_adjustment_revisions
 FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
