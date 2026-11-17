-- Eligible never-approved projects may be removed with their exact owned evidence.
-- These sealed scopes cannot be populated by runtime DML or session settings.
CREATE TABLE budget_draft_removal_scope (
    transaction_id xid8 NOT NULL, backend_pid integer NOT NULL,
    project_id uuid NOT NULL, relation_name text NOT NULL, before_hash text NOT NULL,
    PRIMARY KEY(transaction_id,backend_pid,project_id,relation_name,before_hash)
);
CREATE TABLE workflow_source_removal_scope (
    transaction_id xid8 NOT NULL, backend_pid integer NOT NULL,
    source_module text NOT NULL, source_type text NOT NULL, source_id uuid NOT NULL,
    PRIMARY KEY(transaction_id,backend_pid,source_module,source_type,source_id)
);
CREATE TABLE workflow_evidence_removal_scope (
    transaction_id xid8 NOT NULL, backend_pid integer NOT NULL,
    relation_name text NOT NULL, before_hash text NOT NULL,
    PRIMARY KEY(transaction_id,backend_pid,relation_name,before_hash)
);
-- Opaque technical denial keys prevent delayed CREATE retries from resurrecting erased work.
-- No actor, resource, document, operation or business timestamp is retained.
CREATE TABLE budget_erased_intents(intent_hash bytea PRIMARY KEY CHECK(octet_length(intent_hash)=32));
CREATE FUNCTION budget_erased_intent_key(actor uuid,request uuid) RETURNS bytea
LANGUAGE sql IMMUTABLE SET search_path=pg_catalog AS $$
 SELECT sha256(convert_to(actor::text||':'||request::text,'UTF8'));
$$;
CREATE TRIGGER immutable_budget_erased_intents BEFORE UPDATE OR DELETE ON budget_erased_intents
FOR EACH ROW EXECUTE FUNCTION preserve_budget_evidence();
REVOKE ALL ON budget_draft_removal_scope,workflow_source_removal_scope,workflow_evidence_removal_scope FROM PUBLIC;
DO $$ DECLARE runtime_role text; BEGIN
    IF current_user ~ '^so_[0-9a-fA-F]{32}_owner$' THEN
        runtime_role:=regexp_replace(current_user,'_owner$','_runtime');
        IF EXISTS(SELECT 1 FROM pg_roles WHERE rolname=runtime_role) THEN
            EXECUTE format('REVOKE ALL ON budget_draft_removal_scope,workflow_source_removal_scope,workflow_evidence_removal_scope FROM %I',runtime_role);
            EXECUTE format('REVOKE INSERT,UPDATE,DELETE ON budget_erased_intents FROM %I',runtime_role);
        END IF;
    END IF;
END $$;

CREATE FUNCTION budget_draft_project_deletable(selected_project uuid) RETURNS boolean
LANGUAGE sql STABLE SET search_path FROM CURRENT AS $$
    SELECT EXISTS(SELECT 1 FROM budget_projects p JOIN budget_fiscal_years y ON y.id=p.fiscal_year_id WHERE p.id=selected_project
        AND NOT y.is_closed
        AND p.accepted_version IS NULL AND p.accepted_snapshot_id IS NULL
        AND NOT EXISTS(SELECT 1 FROM budget_plan_snapshots s WHERE s.project_id=p.id)
        AND NOT EXISTS(SELECT 1 FROM budget_allocations a WHERE a.project_id=p.id)
        AND NOT EXISTS(SELECT 1 FROM budget_ledger l WHERE l.project_id=p.id)
        AND NOT EXISTS(SELECT 1 FROM budget_requests r WHERE r.project_id=p.id)
        AND NOT EXISTS(SELECT 1 FROM budget_adjustments a WHERE a.from_project_id=p.id OR a.to_project_id=p.id)
        AND NOT EXISTS(SELECT 1 FROM budget_project_attempts a JOIN process_instances i
            ON i.source_module='budget' AND i.source_type='budget_project_plan' AND i.source_id=a.id
            WHERE a.project_id=p.id AND i.definition->>'status'='completed'));
$$;

CREATE FUNCTION budget_year_templates_referenced(selected_year uuid) RETURNS boolean
LANGUAGE sql STABLE SET search_path FROM CURRENT AS $$
 SELECT EXISTS(SELECT 1 FROM budget_year_template_ownership t WHERE t.fiscal_year_id=selected_year AND (
   EXISTS(SELECT 1 FROM budget_fiscal_years y WHERE y.id<>selected_year AND (y.plan_template_id=t.template_id OR y.request_template_id=t.template_id OR y.adjustment_template_id=t.template_id))
   OR EXISTS(SELECT 1 FROM process_instances i WHERE i.template_id=t.template_id AND NOT EXISTS(
      SELECT 1 FROM budget_project_attempts a JOIN budget_projects p ON p.id=a.project_id WHERE a.id=i.source_id AND i.source_module='budget' AND i.source_type='budget_project_plan' AND p.fiscal_year_id=selected_year AND budget_draft_project_deletable(p.id)))
   OR EXISTS(SELECT 1 FROM process_instance_revisions r JOIN process_instances i ON i.id=r.instance_id WHERE r.definition->>'templateId'=t.template_id::text AND NOT EXISTS(
      SELECT 1 FROM budget_project_attempts a JOIN budget_projects p ON p.id=a.project_id WHERE a.id=i.source_id AND i.source_module='budget' AND i.source_type='budget_project_plan' AND p.fiscal_year_id=selected_year AND budget_draft_project_deletable(p.id)))
 ));
$$;

CREATE FUNCTION budget_guard_draft_removal() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        DELETE FROM budget_draft_removal_scope s
        WHERE s.transaction_id=pg_current_xact_id() AND s.backend_pid=pg_backend_pid()
          AND s.relation_name=TG_TABLE_NAME
          AND s.before_hash=encode(sha256(convert_to(to_jsonb(OLD)::text,'UTF8')),'hex');
        IF FOUND THEN RETURN OLD; END IF;
    END IF;
    RAISE EXCEPTION 'BUDGET_IMMUTABLE_EVIDENCE' USING ERRCODE='check_violation';
END $$;

CREATE FUNCTION workflow_guard_source_removal() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        DELETE FROM workflow_evidence_removal_scope s
        WHERE s.transaction_id=pg_current_xact_id() AND s.backend_pid=pg_backend_pid()
          AND s.relation_name=TG_TABLE_NAME
          AND s.before_hash=encode(sha256(convert_to(to_jsonb(OLD)::text,'UTF8')),'hex');
        IF FOUND THEN RETURN OLD; END IF;
    END IF;
    RAISE EXCEPTION 'WORKFLOW_EVIDENCE_IMMUTABLE' USING ERRCODE='check_violation';
END $$;

DROP TRIGGER immutable_budget_project_revision ON budget_project_revisions;
CREATE TRIGGER immutable_budget_project_revision BEFORE UPDATE OR DELETE ON budget_project_revisions
FOR EACH ROW EXECUTE FUNCTION budget_guard_draft_removal();
DROP TRIGGER immutable_budget_attempt ON budget_project_attempts;
CREATE TRIGGER immutable_budget_attempt BEFORE UPDATE OR DELETE ON budget_project_attempts
FOR EACH ROW EXECUTE FUNCTION budget_guard_draft_removal();
DROP TRIGGER immutable_budget_history ON budget_history;
CREATE TRIGGER immutable_budget_history BEFORE UPDATE OR DELETE ON budget_history
FOR EACH ROW EXECUTE FUNCTION budget_guard_draft_removal();
DROP TRIGGER immutable_budget_receipt ON budget_receipts;
CREATE TRIGGER immutable_budget_receipt BEFORE UPDATE OR DELETE ON budget_receipts
FOR EACH ROW EXECUTE FUNCTION budget_guard_draft_removal();
CREATE TRIGGER guarded_budget_project_removal BEFORE DELETE ON budget_projects
FOR EACH ROW EXECUTE FUNCTION budget_guard_draft_removal();
CREATE TRIGGER guarded_budget_year_removal BEFORE DELETE ON budget_fiscal_years
FOR EACH ROW EXECUTE FUNCTION budget_guard_draft_removal();

DROP TRIGGER immutable_budget_year_template_ownership ON budget_year_template_ownership;
CREATE TRIGGER immutable_budget_year_template_ownership BEFORE UPDATE OR DELETE ON budget_year_template_ownership
FOR EACH ROW EXECUTE FUNCTION budget_guard_draft_removal();
DROP TRIGGER immutable_process_versions ON process_template_versions;
CREATE TRIGGER immutable_process_versions BEFORE UPDATE OR DELETE ON process_template_versions
FOR EACH ROW EXECUTE FUNCTION workflow_guard_source_removal();

DROP TRIGGER immutable_action_history ON process_action_history;
CREATE TRIGGER immutable_action_history BEFORE UPDATE OR DELETE ON process_action_history
FOR EACH ROW EXECUTE FUNCTION workflow_guard_source_removal();
DROP TRIGGER immutable_process_snapshots ON process_instance_revisions;
CREATE TRIGGER immutable_process_snapshots BEFORE UPDATE OR DELETE ON process_instance_revisions
FOR EACH ROW EXECUTE FUNCTION workflow_guard_source_removal();
DROP TRIGGER immutable_process_action_events ON process_action_events;
CREATE TRIGGER immutable_process_action_events BEFORE UPDATE OR DELETE ON process_action_events
FOR EACH ROW EXECUTE FUNCTION workflow_guard_source_removal();

CREATE FUNCTION budget_prepare_draft_removal(selected_project uuid,expected_revision bigint) RETURNS SETOF uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
DECLARE project_row budget_projects%ROWTYPE; owned_table text; owned_row record; selected_year uuid; closed boolean;
BEGIN
    SELECT fiscal_year_id INTO selected_year FROM budget_projects WHERE id=selected_project;
    SELECT is_closed INTO closed FROM budget_fiscal_years WHERE id=selected_year FOR UPDATE;
    IF closed IS DISTINCT FROM false THEN RAISE EXCEPTION 'BUDGET_YEAR_CLOSED'; END IF;
    SELECT * INTO project_row FROM budget_projects WHERE id=selected_project FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'BUDGET_PROJECT_NOT_FOUND'; END IF;
    IF project_row.fiscal_year_id IS DISTINCT FROM selected_year THEN RAISE EXCEPTION 'BUDGET_STALE_REVISION'; END IF;
    IF project_row.revision IS DISTINCT FROM expected_revision THEN RAISE EXCEPTION 'BUDGET_STALE_REVISION'; END IF;
    IF NOT budget_draft_project_deletable(selected_project) THEN RAISE EXCEPTION 'BUDGET_PROJECT_NOT_DELETABLE'; END IF;
    -- Release the immediate aggregate-to-workflow FK only inside this authorized transaction.
    -- Seal the exact post-detachment root; a later refusal rolls the pointer back with all history.
    UPDATE budget_projects SET instance_id=NULL WHERE id=selected_project RETURNING * INTO project_row;
    PERFORM 1 FROM budget_project_attempts WHERE project_id=selected_project ORDER BY id FOR UPDATE;
    INSERT INTO workflow_source_removal_scope
        SELECT pg_current_xact_id(),pg_backend_pid(),'budget','budget_project_plan',id
        FROM budget_project_attempts WHERE project_id=selected_project ON CONFLICT DO NOTHING;
    FOREACH owned_table IN ARRAY ARRAY['budget_project_revisions','budget_project_attempts'] LOOP
        FOR owned_row IN EXECUTE format('SELECT to_jsonb(r) row_data FROM %I r WHERE project_id=$1',owned_table) USING selected_project LOOP
            INSERT INTO budget_draft_removal_scope VALUES(pg_current_xact_id(),pg_backend_pid(),selected_project,
                owned_table,encode(sha256(convert_to(owned_row.row_data::text,'UTF8')),'hex')) ON CONFLICT DO NOTHING;
        END LOOP;
    END LOOP;
    INSERT INTO budget_draft_removal_scope
        SELECT pg_current_xact_id(),pg_backend_pid(),selected_project,'budget_history',encode(sha256(convert_to(to_jsonb(h)::text,'UTF8')),'hex')
        FROM budget_history h WHERE h.resource_kind='project' AND h.resource_id=selected_project ON CONFLICT DO NOTHING;
    INSERT INTO budget_draft_removal_scope
        SELECT pg_current_xact_id(),pg_backend_pid(),selected_project,'budget_receipts',encode(sha256(convert_to(to_jsonb(r)::text,'UTF8')),'hex')
        FROM budget_receipts r WHERE r.resource_id=selected_project ON CONFLICT DO NOTHING;
    INSERT INTO budget_draft_removal_scope VALUES(pg_current_xact_id(),pg_backend_pid(),selected_project,
        'budget_projects',encode(sha256(convert_to(to_jsonb(project_row)::text,'UTF8')),'hex')) ON CONFLICT DO NOTHING;
    RETURN QUERY SELECT id FROM budget_project_attempts WHERE project_id=selected_project ORDER BY id;
END $$;

-- Called only after all eligible current children have been purged in this transaction.
-- Historical references held by retained projects still prevent annual identity removal.
CREATE FUNCTION budget_finalize_draft_year_removal(selected_year uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
DECLARE year_row budget_fiscal_years%ROWTYPE;
BEGIN
    SELECT * INTO year_row FROM budget_fiscal_years WHERE id=selected_year FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'BUDGET_YEAR_NOT_FOUND'; END IF;
    IF year_row.is_closed THEN RAISE EXCEPTION 'BUDGET_YEAR_CLOSED'; END IF;
    IF EXISTS(SELECT 1 FROM budget_projects WHERE fiscal_year_id=selected_year)
        OR EXISTS(SELECT 1 FROM budget_project_revisions WHERE document->'project'->>'fiscalYearId'=selected_year::text)
        OR EXISTS(SELECT 1 FROM budget_plan_snapshots WHERE document->'project'->>'fiscalYearId'=selected_year::text)
        OR EXISTS(SELECT 1 FROM budget_adjustments WHERE fiscal_year_id=selected_year)
        OR EXISTS(SELECT 1 FROM budget_adjustment_revisions WHERE document->>'fiscalYearId'=selected_year::text)
        OR EXISTS(SELECT 1 FROM budget_year_funds WHERE fiscal_year_id=selected_year AND received>0)
       OR EXISTS(SELECT 1 FROM budget_history h CROSS JOIN LATERAL jsonb_array_elements(
         CASE WHEN jsonb_typeof(h.document->'settings'->'funds')='array' THEN h.document->'settings'->'funds' ELSE '[]'::jsonb END) f
         WHERE h.resource_kind='settings' AND h.resource_id=selected_year AND h.document->>'action'='funding_received_saved'
           AND f->>'receivedSatang' ~ '^[0-9]+$' AND (f->>'receivedSatang')::numeric>0)
    THEN RAISE EXCEPTION 'BUDGET_YEAR_NOT_DELETABLE'; END IF;
    INSERT INTO budget_draft_removal_scope
        SELECT pg_current_xact_id(),pg_backend_pid(),selected_year,'budget_history',encode(sha256(convert_to(to_jsonb(h)::text,'UTF8')),'hex')
        FROM budget_history h WHERE h.resource_kind='settings' AND h.resource_id=selected_year ON CONFLICT DO NOTHING;
    INSERT INTO budget_draft_removal_scope
        SELECT pg_current_xact_id(),pg_backend_pid(),selected_year,'budget_receipts',encode(sha256(convert_to(to_jsonb(r)::text,'UTF8')),'hex')
        FROM budget_receipts r WHERE r.resource_id=selected_year ON CONFLICT DO NOTHING;
    INSERT INTO budget_draft_removal_scope VALUES(pg_current_xact_id(),pg_backend_pid(),selected_year,
        'budget_fiscal_years',encode(sha256(convert_to(to_jsonb(year_row)::text,'UTF8')),'hex')) ON CONFLICT DO NOTHING;
    DELETE FROM budget_history WHERE resource_kind='settings' AND resource_id=selected_year;
    INSERT INTO budget_erased_intents SELECT budget_erased_intent_key(actor_id,request_id) FROM budget_receipts WHERE resource_id=selected_year ON CONFLICT DO NOTHING;
    DELETE FROM budget_receipts WHERE resource_id=selected_year;
    DELETE FROM budget_unit_allocations WHERE fiscal_year_id=selected_year;
    DELETE FROM budget_year_expense_categories WHERE fiscal_year_id=selected_year;
    DELETE FROM budget_year_funds WHERE fiscal_year_id=selected_year;
    DELETE FROM budget_fiscal_years WHERE id=selected_year;
    DELETE FROM budget_draft_removal_scope WHERE transaction_id=pg_current_xact_id() AND backend_pid=pg_backend_pid() AND project_id=selected_year;
END $$;

CREATE FUNCTION budget_prepare_year_template_removal(selected_year uuid) RETURNS SETOF uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
DECLARE owned_row record;
BEGIN
 PERFORM 1 FROM budget_fiscal_years WHERE id=selected_year AND NOT is_closed FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'BUDGET_YEAR_NOT_DELETABLE'; END IF;
    IF EXISTS(SELECT 1 FROM budget_projects WHERE fiscal_year_id=selected_year)
        OR EXISTS(SELECT 1 FROM budget_project_revisions WHERE document->'project'->>'fiscalYearId'=selected_year::text)
        OR EXISTS(SELECT 1 FROM budget_plan_snapshots WHERE document->'project'->>'fiscalYearId'=selected_year::text)
        OR EXISTS(SELECT 1 FROM budget_adjustments WHERE fiscal_year_id=selected_year)
        OR EXISTS(SELECT 1 FROM budget_adjustment_revisions WHERE document->>'fiscalYearId'=selected_year::text)
        OR EXISTS(SELECT 1 FROM budget_year_funds WHERE fiscal_year_id=selected_year AND received>0)
       OR EXISTS(SELECT 1 FROM budget_history h CROSS JOIN LATERAL jsonb_array_elements(
         CASE WHEN jsonb_typeof(h.document->'settings'->'funds')='array' THEN h.document->'settings'->'funds' ELSE '[]'::jsonb END) f
         WHERE h.resource_kind='settings' AND h.resource_id=selected_year AND h.document->>'action'='funding_received_saved'
           AND f->>'receivedSatang' ~ '^[0-9]+$' AND (f->>'receivedSatang')::numeric>0)
    THEN RAISE EXCEPTION 'BUDGET_YEAR_NOT_DELETABLE'; END IF;

 UPDATE budget_fiscal_years SET plan_template_id=NULL,request_template_id=NULL,adjustment_template_id=NULL WHERE id=selected_year;
 FOR owned_row IN SELECT t.* FROM budget_year_template_ownership t WHERE fiscal_year_id=selected_year ORDER BY template_id FOR UPDATE LOOP
   INSERT INTO workflow_source_removal_scope VALUES(pg_current_xact_id(),pg_backend_pid(),'budget','budget_year_template',owned_row.template_id) ON CONFLICT DO NOTHING;
   INSERT INTO budget_draft_removal_scope VALUES(pg_current_xact_id(),pg_backend_pid(),selected_year,'budget_year_template_ownership',encode(sha256(convert_to(to_jsonb(owned_row)::text,'UTF8')),'hex')) ON CONFLICT DO NOTHING;
   RETURN NEXT owned_row.template_id;
 END LOOP;
 DELETE FROM budget_year_template_ownership WHERE fiscal_year_id=selected_year;
END $$;

CREATE FUNCTION workflow_finalize_year_template_removal(selected_template uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
DECLARE owned_row record;
BEGIN
 IF NOT EXISTS(SELECT 1 FROM workflow_source_removal_scope WHERE transaction_id=pg_current_xact_id() AND backend_pid=pg_backend_pid()
   AND source_module='budget' AND source_type='budget_year_template' AND source_id=selected_template)
 THEN RAISE EXCEPTION 'WORKFLOW_TEMPLATE_REMOVAL_NOT_AUTHORIZED'; END IF;
 PERFORM 1 FROM process_templates WHERE id=selected_template AND managed_source_module='budget' FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'WORKFLOW_TEMPLATE_REMOVAL_NOT_AUTHORIZED'; END IF;
 IF EXISTS(SELECT 1 FROM process_instances WHERE template_id=selected_template)
 OR EXISTS(SELECT 1 FROM process_instance_revisions WHERE definition->>'templateId'=selected_template::text)
 OR EXISTS(SELECT 1 FROM budget_fiscal_years WHERE plan_template_id=selected_template OR request_template_id=selected_template OR adjustment_template_id=selected_template)
 OR EXISTS(SELECT 1 FROM budget_year_template_ownership WHERE template_id=selected_template)
 THEN RAISE EXCEPTION 'WORKFLOW_TEMPLATE_REMOVAL_REFERENCED'; END IF;
 FOR owned_row IN SELECT v.* FROM process_template_versions v WHERE template_id=selected_template FOR UPDATE LOOP
   INSERT INTO workflow_evidence_removal_scope VALUES(pg_current_xact_id(),pg_backend_pid(),'process_template_versions',encode(sha256(convert_to(to_jsonb(owned_row)::text,'UTF8')),'hex')) ON CONFLICT DO NOTHING;
 END LOOP;
 DELETE FROM process_template_versions WHERE template_id=selected_template;
 DELETE FROM process_templates WHERE id=selected_template;
 DELETE FROM workflow_source_removal_scope WHERE transaction_id=pg_current_xact_id() AND backend_pid=pg_backend_pid()
   AND source_module='budget' AND source_type='budget_year_template' AND source_id=selected_template;
END $$;

-- The source owner creates the sealed identity before this provider locks work.
CREATE FUNCTION workflow_lock_source_removal(selected_module text,selected_type text,selected_source uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
BEGIN
    IF NOT EXISTS(SELECT 1 FROM workflow_source_removal_scope s
        WHERE s.transaction_id=pg_current_xact_id() AND s.backend_pid=pg_backend_pid()
          AND s.source_module=selected_module AND s.source_type=selected_type AND s.source_id=selected_source)
    THEN RAISE EXCEPTION 'WORKFLOW_SOURCE_REMOVAL_NOT_AUTHORIZED'; END IF;
    PERFORM 1 FROM process_instances WHERE source_module=selected_module AND source_type=selected_type
        AND source_id=selected_source ORDER BY id FOR UPDATE;
    PERFORM 1 FROM process_notification_sources WHERE instance_id IN(SELECT id FROM process_instances
        WHERE source_module=selected_module AND source_type=selected_type AND source_id=selected_source) ORDER BY id FOR UPDATE;
    PERFORM 1 FROM work_items WHERE module_code=selected_module AND source_resource_type=selected_type
        AND source_resource_id=selected_source ORDER BY id FOR UPDATE;
END $$;

-- Notifications are removed through their public provider before finalization.
CREATE FUNCTION workflow_finalize_source_removal(selected_module text,selected_type text,selected_source uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
DECLARE instance_ids uuid[]; item_ids uuid[]; window_ids uuid[]; owned_table text; owned_row record;
BEGIN
    PERFORM workflow_lock_source_removal(selected_module,selected_type,selected_source);
    SELECT COALESCE(array_agg(id ORDER BY id),'{}') INTO instance_ids FROM process_instances
        WHERE source_module=selected_module AND source_type=selected_type AND source_id=selected_source;
    IF EXISTS(SELECT 1 FROM process_instances WHERE id=ANY(instance_ids) AND definition->>'status'='completed')
        OR EXISTS(SELECT 1 FROM correspondence_records WHERE instance_id=ANY(instance_ids))
        OR EXISTS(SELECT 1 FROM process_action_history WHERE instance_id=ANY(instance_ids) AND correspondence_id IS NOT NULL)
    THEN RAISE EXCEPTION 'WORKFLOW_SOURCE_REMOVAL_HAS_ACCEPTED_OR_FOREIGN_EVIDENCE'; END IF;
    IF EXISTS(SELECT 1 FROM notification_receipts r JOIN process_notification_sources s ON s.id=r.source_id
        WHERE r.source_kind='process' AND s.instance_id=ANY(instance_ids))
    THEN RAISE EXCEPTION 'WORKFLOW_SOURCE_REMOVAL_NOTIFICATIONS_REMAIN'; END IF;
    SELECT COALESCE(array_agg(id ORDER BY id),'{}'),COALESCE(array_agg(DISTINCT workflow_window_id),'{}')
        INTO item_ids,window_ids FROM work_items WHERE module_code=selected_module
        AND source_resource_type=selected_type AND source_resource_id=selected_source;
    PERFORM 1 FROM workflow_windows WHERE id=ANY(window_ids) ORDER BY id FOR UPDATE;
    IF EXISTS(SELECT 1 FROM work_items WHERE workflow_window_id=ANY(window_ids) AND NOT(id=ANY(item_ids)))
        OR EXISTS(SELECT 1 FROM workflow_windows WHERE id=ANY(window_ids)
            AND (module_code<>selected_module OR workflow_code<>'process_step'))
        OR EXISTS(SELECT 1 FROM process_notification_sources WHERE work_item_id=ANY(item_ids) AND NOT(instance_id=ANY(instance_ids)))
    THEN RAISE EXCEPTION 'WORKFLOW_SOURCE_REMOVAL_SHARED_WORK'; END IF;
    FOREACH owned_table IN ARRAY ARRAY['process_action_history','process_action_events','process_instance_revisions'] LOOP
        FOR owned_row IN EXECUTE format('SELECT to_jsonb(r) row_data FROM %I r WHERE instance_id=ANY($1)',owned_table) USING instance_ids LOOP
            INSERT INTO workflow_evidence_removal_scope VALUES(pg_current_xact_id(),pg_backend_pid(),owned_table,
                encode(sha256(convert_to(owned_row.row_data::text,'UTF8')),'hex')) ON CONFLICT DO NOTHING;
        END LOOP;
    END LOOP;
    DELETE FROM process_action_history WHERE instance_id=ANY(instance_ids);
    DELETE FROM process_action_events WHERE instance_id=ANY(instance_ids);
    DELETE FROM process_instance_revisions WHERE instance_id=ANY(instance_ids);
    DELETE FROM process_participants WHERE instance_id=ANY(instance_ids);
    DELETE FROM process_notification_sources WHERE instance_id=ANY(instance_ids);
    DELETE FROM workflow_mutation_receipts WHERE (operation='process_start' AND resource_id=selected_source)
        OR (operation='process_action' AND resource_id=ANY(instance_ids));
    DELETE FROM workflow_windows WHERE id=ANY(window_ids);
    DELETE FROM process_instances WHERE id=ANY(instance_ids);
    DELETE FROM workflow_source_removal_scope WHERE transaction_id=pg_current_xact_id() AND backend_pid=pg_backend_pid()
        AND source_module=selected_module AND source_type=selected_type AND source_id=selected_source;
END $$;

CREATE FUNCTION budget_finalize_draft_removal(selected_project uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path FROM CURRENT AS $$
BEGIN
    PERFORM 1 FROM budget_projects WHERE id=selected_project FOR UPDATE;
    IF NOT EXISTS(SELECT 1 FROM budget_draft_removal_scope s WHERE s.transaction_id=pg_current_xact_id()
        AND s.backend_pid=pg_backend_pid() AND s.project_id=selected_project AND s.relation_name='budget_projects')
    THEN RAISE EXCEPTION 'BUDGET_PROJECT_REMOVAL_NOT_AUTHORIZED'; END IF;
    IF NOT budget_draft_project_deletable(selected_project) THEN RAISE EXCEPTION 'BUDGET_PROJECT_NOT_DELETABLE'; END IF;
    IF EXISTS(SELECT 1 FROM budget_project_attempts a JOIN process_instances i
        ON i.source_module='budget' AND i.source_type='budget_project_plan' AND i.source_id=a.id WHERE a.project_id=selected_project)
        OR EXISTS(SELECT 1 FROM budget_project_attempts a JOIN workflow_source_removal_scope s
        ON s.source_module='budget' AND s.source_type='budget_project_plan' AND s.source_id=a.id
        WHERE a.project_id=selected_project AND s.transaction_id=pg_current_xact_id() AND s.backend_pid=pg_backend_pid())
    THEN RAISE EXCEPTION 'BUDGET_PROJECT_REMOVAL_WORKFLOW_REMAINS'; END IF;
    DELETE FROM budget_project_revisions WHERE project_id=selected_project;
    DELETE FROM budget_history WHERE resource_kind='project' AND resource_id=selected_project;
    INSERT INTO budget_erased_intents SELECT budget_erased_intent_key(actor_id,request_id) FROM budget_receipts WHERE resource_id=selected_project ON CONFLICT DO NOTHING;
    DELETE FROM budget_receipts WHERE resource_id=selected_project;
    DELETE FROM budget_plan_lines WHERE project_id=selected_project;
    DELETE FROM budget_activities WHERE project_id=selected_project;
    DELETE FROM budget_project_attempts WHERE project_id=selected_project;
    DELETE FROM budget_projects WHERE id=selected_project;
    DELETE FROM budget_draft_removal_scope WHERE transaction_id=pg_current_xact_id() AND backend_pid=pg_backend_pid() AND project_id=selected_project;
END $$;

REVOKE ALL ON FUNCTION budget_guard_draft_removal(),workflow_guard_source_removal() FROM PUBLIC;
-- Definer resolution must ignore caller search_path and temporary shadow tables.
DO $$ DECLARE owned_schema text:=current_schema(); f record; BEGIN
    FOR f IN SELECT p.oid::regprocedure identity FROM pg_proc p
        WHERE p.pronamespace=owned_schema::regnamespace AND p.proname=ANY(ARRAY[
            'budget_draft_project_deletable','budget_year_templates_referenced','budget_guard_draft_removal','workflow_guard_source_removal',
            'budget_prepare_draft_removal','workflow_lock_source_removal','workflow_finalize_source_removal',
            'budget_finalize_draft_removal','budget_finalize_draft_year_removal',
            'budget_prepare_year_template_removal','workflow_finalize_year_template_removal']) LOOP
        EXECUTE format('ALTER FUNCTION %s SET search_path TO pg_catalog,%I,pg_temp',f.identity,owned_schema);
    END LOOP;
END $$;
