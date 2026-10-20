-- Display membership remains derived from direct assignments. It never grants rights.
-- Preserve every chosen primary flag; ambiguous historical overlaps require remediation.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM organization_members a
        JOIN organization_units au ON au.id = a.organization_unit_id
        JOIN organization_members b ON b.user_id = a.user_id AND b.id > a.id
        JOIN organization_units bu ON bu.id = b.organization_unit_id
        WHERE a.is_primary IS TRUE AND b.is_primary IS TRUE
          AND (au.unit_type = 'subject_group') = (bu.unit_type = 'subject_group')
          AND a.started_at < coalesce(a.ended_at, 'infinity'::date)
          AND b.started_at < coalesce(b.ended_at, 'infinity'::date)
          AND a.started_at < coalesce(b.ended_at, 'infinity'::date)
          AND b.started_at < coalesce(a.ended_at, 'infinity'::date)
    ) THEN
        RAISE EXCEPTION 'ORGANIZATION_132_PRIMARY_OVERLAP';
    END IF;
END $$;

DROP INDEX idx_org_members_one_active_primary;

CREATE FUNCTION validate_organization_member_primary() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE learning_area boolean;
BEGIN
    -- One per-person serialization owner also covers raw SQL and bulk staff saves.
    PERFORM id FROM users WHERE id = NEW.user_id FOR UPDATE;
    SELECT unit_type = 'subject_group' INTO learning_area
    FROM organization_units WHERE id = NEW.organization_unit_id FOR SHARE;
    IF NEW.is_primary IS TRUE AND NEW.started_at < coalesce(NEW.ended_at, 'infinity'::date)
       AND EXISTS (
        SELECT 1 FROM organization_members other_member
        JOIN organization_units other_unit ON other_unit.id = other_member.organization_unit_id
        WHERE other_member.user_id = NEW.user_id AND other_member.id <> NEW.id
          AND other_member.is_primary IS TRUE
          AND (other_unit.unit_type = 'subject_group') = learning_area
          AND other_member.started_at < coalesce(other_member.ended_at, 'infinity'::date)
          AND other_member.started_at < coalesce(NEW.ended_at, 'infinity'::date)
          AND NEW.started_at < coalesce(other_member.ended_at, 'infinity'::date)
    ) THEN
        RAISE EXCEPTION 'ORGANIZATION_PRIMARY_DOMAIN_CONFLICT' USING ERRCODE = '23505';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER organization_members_primary_domain BEFORE INSERT OR UPDATE ON organization_members
FOR EACH ROW EXECUTE FUNCTION validate_organization_member_primary();

CREATE FUNCTION protect_organization_membership_domain() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.unit_type = 'subject_group') IS DISTINCT FROM (OLD.unit_type = 'subject_group')
       AND EXISTS (SELECT 1 FROM organization_members WHERE organization_unit_id = OLD.id) THEN
        RAISE EXCEPTION 'ORGANIZATION_MEMBERSHIP_DOMAIN_IMMUTABLE' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER organization_units_membership_domain BEFORE UPDATE OF unit_type ON organization_units
FOR EACH ROW EXECUTE FUNCTION protect_organization_membership_domain();

-- These five Workflow definitions own functional organization selectors without SQL FKs.
-- Extract only their typed wire key; null user-selector fields do not reference a unit.
CREATE FUNCTION organization_selector_unit_ids(definition jsonb) RETURNS SETOF uuid
LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE selector jsonb; selector_text text;
BEGIN
    FOR selector IN
        SELECT DISTINCT value FROM jsonb_path_query(definition,
            '$.**.organizationUnitId') AS selectors(value)
    LOOP
        IF selector = 'null'::jsonb THEN CONTINUE; END IF;
        selector_text := selector #>> '{}';
        IF jsonb_typeof(selector) <> 'string' THEN
            RAISE EXCEPTION 'WORKFLOW_ORGANIZATION_SELECTOR_INVALID' USING ERRCODE = '23514';
        END IF;
        -- PostgreSQL accepts preserved uppercase, compact and braced UUID spellings;
        -- the typed wire parser also accepts the URN form. Do not rewrite the JSON.
        IF lower(left(selector_text, 9)) = 'urn:uuid:' THEN
            selector_text := substr(selector_text, 10);
        END IF;
        BEGIN
            RETURN NEXT selector_text::uuid;
        EXCEPTION WHEN invalid_text_representation THEN
            RAISE EXCEPTION 'WORKFLOW_ORGANIZATION_SELECTOR_INVALID' USING ERRCODE = '23514';
        END;
    END LOOP;
END $$;

DO $$
DECLARE missing_count bigint;
BEGIN
    SELECT count(*) INTO missing_count FROM (
        SELECT definition FROM process_templates
        UNION ALL SELECT definition FROM process_template_versions
        UNION ALL SELECT definition FROM process_instances
        UNION ALL SELECT definition FROM process_instance_revisions
        UNION ALL SELECT definition FROM correspondence_routing_settings
    ) definitions
    CROSS JOIN LATERAL organization_selector_unit_ids(definitions.definition) refs(unit_id)
    LEFT JOIN organization_units unit ON unit.id = refs.unit_id
    WHERE unit.id IS NULL;
    IF missing_count > 0 THEN
        RAISE EXCEPTION 'ORGANIZATION_132_WORKFLOW_SELECTOR_ORPHANS count=%', missing_count;
    END IF;
END $$;

CREATE FUNCTION guard_workflow_organization_selectors() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE unit_id uuid;
BEGIN
    FOR unit_id IN
        SELECT DISTINCT refs.id FROM organization_selector_unit_ids(NEW.definition) refs(id)
        ORDER BY refs.id
    LOOP
        PERFORM id FROM organization_units WHERE id = unit_id FOR KEY SHARE;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'WORKFLOW_ORGANIZATION_SELECTOR_MISSING' USING ERRCODE = '23503';
        END IF;
    END LOOP;
    RETURN NEW;
END $$;

CREATE TRIGGER process_templates_organization_selectors
BEFORE INSERT OR UPDATE OF definition ON process_templates
FOR EACH ROW EXECUTE FUNCTION guard_workflow_organization_selectors();
CREATE TRIGGER process_template_versions_organization_selectors
BEFORE INSERT OR UPDATE OF definition ON process_template_versions
FOR EACH ROW EXECUTE FUNCTION guard_workflow_organization_selectors();
CREATE TRIGGER process_instances_organization_selectors
BEFORE INSERT OR UPDATE OF definition ON process_instances
FOR EACH ROW EXECUTE FUNCTION guard_workflow_organization_selectors();
CREATE TRIGGER process_instance_revisions_organization_selectors
BEFORE INSERT OR UPDATE OF definition ON process_instance_revisions
FOR EACH ROW EXECUTE FUNCTION guard_workflow_organization_selectors();
CREATE TRIGGER correspondence_routing_settings_organization_selectors
BEFORE INSERT OR UPDATE OF definition ON correspondence_routing_settings
FOR EACH ROW EXECUTE FUNCTION guard_workflow_organization_selectors();

-- Inspect all referencing foreign keys, including CASCADE/SET NULL and future owners.
-- The deleting service holds FOR UPDATE on the unit before this check, so FK writers wait.
CREATE FUNCTION organization_unit_reference_category(target_id uuid) RETURNS text LANGUAGE plpgsql AS $$
DECLARE reference record; referenced boolean;
BEGIN
    FOR reference IN
        SELECT c.conrelid::regclass AS relation, a.attname AS column_name,
               cl.relname AS table_name, cardinality(c.conkey) AS key_count, referenced_column.attname AS referenced_column_name
        FROM pg_constraint c
        JOIN pg_class cl ON cl.oid = c.conrelid
        JOIN LATERAL unnest(c.conkey, c.confkey) AS key_columns(local_key, foreign_key) ON true
        JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = key_columns.local_key
        JOIN pg_attribute referenced_column ON referenced_column.attrelid = c.confrelid AND referenced_column.attnum = key_columns.foreign_key
        WHERE c.contype = 'f' AND c.confrelid = 'organization_units'::regclass
        ORDER BY cl.relname, c.conname
    LOOP
        -- A future non-ID reference cannot be mapped to this UUID safely. Refuse deletion.
        IF reference.referenced_column_name <> 'id' THEN
            RETURN 'business_records';
        END IF;
        EXECUTE format('SELECT EXISTS (SELECT 1 FROM %s WHERE %I = $1)', reference.relation, reference.column_name)
            INTO referenced USING target_id;
        IF referenced THEN
            RETURN CASE reference.table_name
                WHEN 'organization_units' THEN 'children'
                WHEN 'organization_members' THEN 'memberships'
                WHEN 'organization_permission_grants' THEN 'permissions'
                WHEN 'organization_permission_delegations' THEN 'delegations'
                WHEN 'user_roles' THEN 'roles'
                ELSE 'business_records' END;
        END IF;
    END LOOP;
    IF EXISTS (
        SELECT 1 FROM (
            SELECT definition FROM process_templates
            UNION ALL SELECT definition FROM process_template_versions
            UNION ALL SELECT definition FROM process_instances
            UNION ALL SELECT definition FROM process_instance_revisions
            UNION ALL SELECT definition FROM correspondence_routing_settings
        ) definitions
        CROSS JOIN LATERAL organization_selector_unit_ids(definitions.definition) refs(unit_id)
        WHERE refs.unit_id = target_id
    ) THEN
        RETURN 'workflow_records';
    END IF;
    RETURN NULL;
END $$;
