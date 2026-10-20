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

-- Inspect all referencing foreign keys, including CASCADE/SET NULL and future owners.
-- The deleting service holds FOR UPDATE on the unit before this check, so FK writers wait.
CREATE FUNCTION organization_unit_reference_category(target_id uuid) RETURNS text LANGUAGE plpgsql AS $$
DECLARE reference record; referenced boolean;
BEGIN
    FOR reference IN
        SELECT c.conrelid::regclass AS relation, a.attname AS column_name,
               cl.relname AS table_name
        FROM pg_constraint c
        JOIN pg_class cl ON cl.oid = c.conrelid
        JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = c.conkey[1]
        WHERE c.contype = 'f' AND c.confrelid = 'organization_units'::regclass
        ORDER BY cl.relname, c.conname
    LOOP
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
    RETURN NULL;
END $$;
