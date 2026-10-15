-- One canonical learning-area registry; organization rows own membership and grants.
-- Applied under maintenance. Old binaries must not be restored after this cutover.
DO $$
BEGIN
    IF (SELECT count(*) FROM organization_units WHERE code = 'SCHOOL' AND unit_type = 'school' AND is_active) <> 1 THEN
        RAISE EXCEPTION 'ORGANIZATION_116_ROOT_INVALID';
    END IF;
    IF EXISTS (SELECT 1 FROM organization_units WHERE unit_type = 'subject_group' AND subject_group_id IS NULL)
       OR EXISTS (SELECT 1 FROM organization_units WHERE subject_group_id IS NOT NULL
                  AND unit_type <> 'subject_group' AND code <> 'ACAD-ACT') THEN
        RAISE EXCEPTION 'ORGANIZATION_116_UNMAPPED_UNIT';
    END IF;
    IF EXISTS (SELECT 1 FROM organization_units WHERE subject_group_id IS NOT NULL
               GROUP BY subject_group_id HAVING count(*) > 1) THEN
        RAISE EXCEPTION 'ORGANIZATION_116_DUPLICATE_MAPPING';
    END IF;
    IF EXISTS (SELECT 1 FROM subject_groups WHERE btrim(name_th) = '') THEN
        RAISE EXCEPTION 'ORGANIZATION_116_NAME_INVALID';
    END IF;
    IF EXISTS (SELECT 1 FROM subject_groups WHERE is_active IS NULL)
       OR EXISTS (SELECT 1 FROM organization_units u JOIN subject_groups g ON g.id = u.subject_group_id
                  WHERE u.is_active IS DISTINCT FROM g.is_active) THEN
        RAISE EXCEPTION 'ORGANIZATION_116_STATUS_CONFLICT';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM organization_units u JOIN subject_groups g ON g.id = u.subject_group_id
                   WHERE u.code = 'ACAD-ACT' AND g.code = 'AC' AND u.unit_type = 'division') THEN
        RAISE EXCEPTION 'ORGANIZATION_116_ACTIVITY_MAPPING_INVALID';
    END IF;
    IF EXISTS (SELECT 1 FROM organization_units child JOIN organization_units parent ON parent.id = child.parent_unit_id
               WHERE parent.unit_type = 'subject_group') THEN
        RAISE EXCEPTION 'ORGANIZATION_116_SUBJECT_GROUP_HAS_CHILDREN';
    END IF;
    IF EXISTS (SELECT 1 FROM subjects s JOIN subject_groups g ON g.id = s.subject_group_id
               LEFT JOIN organization_units u ON u.subject_group_id = s.subject_group_id
               WHERE u.id IS DISTINCT FROM s.owning_organization_unit_id
                  OR (s.archived_at IS NULL AND (g.is_active IS NOT TRUE OR u.is_active IS NOT TRUE))) THEN
        RAISE EXCEPTION 'ORGANIZATION_116_SUBJECT_OWNER_CONFLICT';
    END IF;
END $$;

CREATE TABLE organization_code_counters (
    kind text PRIMARY KEY CHECK (kind IN ('MG', 'DV', 'SG', 'OU')),
    last_value bigint NOT NULL CHECK (last_value BETWEEN 1 AND 99999999999999999)
);

CREATE FUNCTION next_organization_code(code_kind text) RETURNS text
LANGUAGE plpgsql AS $$
DECLARE allocated bigint; candidate text;
BEGIN
    LOOP
        INSERT INTO organization_code_counters(kind, last_value) VALUES (code_kind, 1)
        ON CONFLICT (kind) DO UPDATE SET last_value = organization_code_counters.last_value + 1
        RETURNING last_value INTO allocated;
        candidate := code_kind || '-' || lpad(allocated::text, greatest(6, length(allocated::text)), '0');
        IF NOT EXISTS (SELECT 1 FROM organization_units WHERE code = candidate)
           AND NOT EXISTS (SELECT 1 FROM subject_groups WHERE code = candidate) THEN
            RETURN candidate;
        END IF;
    END LOOP;
END $$;

-- Include inactive identities when reconciling; never adopt a row by its name.
INSERT INTO organization_units(code, name, name_en, category, unit_type, parent_unit_id,
                               subject_group_id, is_active, display_order)
SELECT next_organization_code('SG'), g.name_th, nullif(g.name_en, ''), 'academic', 'subject_group',
       root.id, g.id, g.is_active, coalesce(g.display_order, 0)
FROM subject_groups g CROSS JOIN organization_units root
WHERE root.code = 'SCHOOL'
  AND NOT EXISTS (SELECT 1 FROM organization_units u WHERE u.subject_group_id = g.id);

DO $$
DECLARE moved bigint; tree_grants bigint;
BEGIN
    SELECT count(*) INTO moved FROM organization_units u
    WHERE u.unit_type = 'subject_group'
      AND u.parent_unit_id IS DISTINCT FROM (SELECT id FROM organization_units WHERE code = 'SCHOOL');
    SELECT count(*) INTO tree_grants FROM organization_permission_grants grant_row
    JOIN permissions p ON p.id = grant_row.permission_id
    WHERE p.scope = 'organization_tree';
    RAISE NOTICE 'ORGANIZATION_116_REPARENTED_COUNT=%, TREE_GRANT_REVIEW_COUNT=%', moved, tree_grants;
END $$;

UPDATE organization_units u
SET name = g.name_th, name_en = nullif(g.name_en, ''),
    parent_unit_id = root.id, display_order = coalesce(g.display_order, 0),
    metadata = u.metadata || jsonb_build_object('learningAreaSeparation',
        jsonb_build_object('migration', 116, 'previousParentId', u.parent_unit_id)), updated_at = now()
FROM subject_groups g CROSS JOIN organization_units root
WHERE u.subject_group_id = g.id AND u.unit_type = 'subject_group' AND root.code = 'SCHOOL';

CREATE UNIQUE INDEX organization_units_one_subject_group ON organization_units(subject_group_id)
WHERE subject_group_id IS NOT NULL;
DROP INDEX organization_units_one_active_subject_group;

CREATE FUNCTION sync_subject_group_organization() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE root_id uuid;
BEGIN
    SELECT id INTO STRICT root_id FROM organization_units WHERE code = 'SCHOOL' AND is_active;
    IF TG_OP = 'UPDATE' THEN
        IF NEW.code IS DISTINCT FROM OLD.code THEN
            RAISE EXCEPTION 'LEARNING_AREA_CODE_IMMUTABLE' USING ERRCODE = '23514';
        END IF;
        IF NEW.is_active IS FALSE AND OLD.is_active IS TRUE AND (
            OLD.code = 'AC' OR EXISTS (SELECT 1 FROM subjects WHERE subject_group_id = NEW.id AND archived_at IS NULL)
        ) THEN
            RAISE EXCEPTION 'LEARNING_AREA_IN_USE' USING ERRCODE = '23514';
        END IF;
    END IF;
    IF TG_OP = 'INSERT' THEN
        INSERT INTO organization_units(code, name, name_en, category, unit_type, parent_unit_id,
                                       subject_group_id, is_active, display_order)
        VALUES (NEW.code, NEW.name_th, nullif(NEW.name_en, ''), 'academic', 'subject_group', root_id,
                NEW.id, NEW.is_active, coalesce(NEW.display_order, 0));
    ELSE
        UPDATE organization_units SET name = NEW.name_th, name_en = nullif(NEW.name_en, ''),
            is_active = NEW.is_active, display_order = coalesce(NEW.display_order, 0), updated_at = now()
        WHERE subject_group_id = NEW.id AND unit_type = 'subject_group';
        IF NOT FOUND AND NEW.code <> 'AC' THEN
            RAISE EXCEPTION 'LEARNING_AREA_MAPPING_MISSING' USING ERRCODE = '23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER subject_groups_sync_organization AFTER INSERT OR UPDATE ON subject_groups
FOR EACH ROW EXECUTE FUNCTION sync_subject_group_organization();

-- Guard hierarchy even when a caller bypasses the UI or changes a unit's type.
CREATE FUNCTION validate_learning_area_hierarchy() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.unit_type = 'subject_group' THEN
        IF NEW.subject_group_id IS NULL OR NOT EXISTS (
            SELECT 1 FROM organization_units WHERE id = NEW.parent_unit_id AND code = 'SCHOOL'
        ) THEN
            RAISE EXCEPTION 'LEARNING_AREA_REQUIRES_SCHOOL_ROOT' USING ERRCODE = '23514';
        END IF;
    END IF;
    IF EXISTS (SELECT 1 FROM organization_units WHERE id = NEW.parent_unit_id AND unit_type = 'subject_group') THEN
        RAISE EXCEPTION 'LEARNING_AREA_CANNOT_HAVE_CHILDREN' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'UPDATE' AND (OLD.unit_type = 'subject_group' OR OLD.subject_group_id IS NOT NULL) AND
       (NEW.unit_type IS DISTINCT FROM OLD.unit_type OR NEW.subject_group_id IS DISTINCT FROM OLD.subject_group_id) THEN
        RAISE EXCEPTION 'LEARNING_AREA_MAPPING_IMMUTABLE' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER organization_units_learning_area_hierarchy BEFORE INSERT OR UPDATE ON organization_units
FOR EACH ROW EXECUTE FUNCTION validate_learning_area_hierarchy();

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM subject_groups g LEFT JOIN organization_units u ON u.subject_group_id = g.id
               WHERE u.id IS NULL OR (u.unit_type = 'subject_group' AND (
                   u.name IS DISTINCT FROM g.name_th OR u.is_active IS DISTINCT FROM g.is_active OR
                   u.parent_unit_id IS DISTINCT FROM (SELECT id FROM organization_units WHERE code = 'SCHOOL')))) THEN
        RAISE EXCEPTION 'ORGANIZATION_116_RECONCILIATION_FAILED';
    END IF;
END $$;

-- Serialize catalog creation/restoration with learning-area deactivation. Lock the
-- registry before its projection, matching the registry update's lock order.
CREATE FUNCTION validate_subject_learning_area() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE owner_id uuid;
BEGIN
    IF NEW.archived_at IS NULL THEN
        PERFORM id FROM subject_groups WHERE id = NEW.subject_group_id AND is_active FOR SHARE;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'LEARNING_AREA_AFFILIATION_INVALID' USING ERRCODE = '23514';
        END IF;
        SELECT id INTO owner_id FROM organization_units
        WHERE subject_group_id = NEW.subject_group_id AND is_active FOR SHARE;
        IF owner_id IS DISTINCT FROM NEW.owning_organization_unit_id THEN
            RAISE EXCEPTION 'LEARNING_AREA_AFFILIATION_INVALID' USING ERRCODE = '23514';
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER subjects_learning_area_affiliation
BEFORE INSERT OR UPDATE OF subject_group_id, owning_organization_unit_id, archived_at ON subjects
FOR EACH ROW EXECUTE FUNCTION validate_subject_learning_area();
