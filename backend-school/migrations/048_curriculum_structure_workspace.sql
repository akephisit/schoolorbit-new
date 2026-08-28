-- Curriculum Structure Workspace
--
-- Replace free-form curriculum term codes and duplicated requirement metrics
-- with version-owned term slots and catalog-owned workload values.

ALTER TABLE activity_versions
    ADD COLUMN hours_per_term NUMERIC(10,2),
    ADD CONSTRAINT activity_versions_hours_per_term_check
        CHECK (hours_per_term IS NULL OR hours_per_term >= 0);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM (
            SELECT recommended_term_code
            FROM curriculum_course_requirements
            UNION ALL
            SELECT recommended_term_code
            FROM curriculum_activity_requirements
        ) requirement
        WHERE requirement.recommended_term_code IS NULL
           OR btrim(requirement.recommended_term_code) = ''
           OR upper(btrim(requirement.recommended_term_code)) !~
              '^((TERM-)?[1-9][0-9]*|SUMMER|REMEDIAL|CUSTOM-[1-9][0-9]*)$'
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_048_TERM_CODE_UNMAPPABLE';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM curriculum_course_requirements requirement
        JOIN subject_versions version ON version.id = requirement.subject_version_id
        WHERE requirement.credit IS DISTINCT FROM version.credit
           OR requirement.hours IS DISTINCT FROM version.hours_per_semester::numeric(10,2)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_048_COURSE_METRIC_MISMATCH';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM curriculum_activity_requirements requirement
        JOIN activity_versions version ON version.id = requirement.activity_version_id
        WHERE requirement.hours IS DISTINCT FROM version.hours_per_week::numeric(10,2)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_048_ACTIVITY_WEEKLY_METRIC_MISMATCH';
    END IF;
END
$$;

CREATE TABLE curriculum_term_slots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    curriculum_version_id UUID NOT NULL
        REFERENCES curriculum_versions(id) ON DELETE RESTRICT,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    term_type TEXT NOT NULL
        CHECK (term_type IN ('regular', 'summer', 'remedial', 'custom')),
    type_occurrence INTEGER NOT NULL CHECK (type_occurrence > 0),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT curriculum_term_slots_version_sequence_key
        UNIQUE (curriculum_version_id, sequence),
    CONSTRAINT curriculum_term_slots_version_type_occurrence_key
        UNIQUE (curriculum_version_id, term_type, type_occurrence),
    CONSTRAINT curriculum_term_slots_id_version_key
        UNIQUE (id, curriculum_version_id)
);

ALTER TABLE curriculum_course_requirements
    DISABLE TRIGGER curriculum_course_requirements_published_immutable;

ALTER TABLE curriculum_activity_requirements
    DISABLE TRIGGER curriculum_activity_requirements_published_immutable;

WITH raw_codes AS (
    SELECT curriculum_version_id,
           upper(btrim(recommended_term_code)) AS term_code
    FROM curriculum_course_requirements
    UNION
    SELECT curriculum_version_id,
           upper(btrim(recommended_term_code)) AS term_code
    FROM curriculum_activity_requirements
),
parsed_slots AS (
    SELECT DISTINCT
           curriculum_version_id,
           CASE
               WHEN term_code ~ '^((TERM-)?[1-9][0-9]*)$' THEN 'regular'
               WHEN term_code = 'SUMMER' THEN 'summer'
               WHEN term_code = 'REMEDIAL' THEN 'remedial'
               ELSE 'custom'
           END AS term_type,
           CASE
               WHEN term_code ~ '^TERM-' THEN substring(term_code FROM 6)::integer
               WHEN term_code ~ '^[1-9][0-9]*$' THEN term_code::integer
               WHEN term_code ~ '^CUSTOM-' THEN substring(term_code FROM 8)::integer
               ELSE 1
           END AS type_occurrence
    FROM raw_codes
),
ordered_slots AS (
    SELECT curriculum_version_id,
           term_type,
           type_occurrence,
           row_number() OVER (
               PARTITION BY curriculum_version_id
               ORDER BY CASE term_type
                            WHEN 'regular' THEN 1
                            WHEN 'summer' THEN 2
                            WHEN 'remedial' THEN 3
                            ELSE 4
                        END,
                        type_occurrence
           )::integer AS sequence
    FROM parsed_slots
)
INSERT INTO curriculum_term_slots (
    id, curriculum_version_id, sequence, term_type, type_occurrence, name
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'curriculum-term-slot:' || curriculum_version_id::text || ':'
               || term_type || ':' || type_occurrence::text
       ),
       curriculum_version_id,
       sequence,
       term_type,
       type_occurrence,
       CASE term_type
           WHEN 'regular' THEN 'ภาคเรียนที่ ' || type_occurrence::text
           WHEN 'summer' THEN 'ภาคฤดูร้อน'
           WHEN 'remedial' THEN 'ภาคซ่อมเสริม'
           ELSE 'ภาคเรียนกำหนดเอง ' || type_occurrence::text
       END
FROM ordered_slots;

ALTER TABLE curriculum_course_requirements
    ADD COLUMN term_slot_id UUID;

ALTER TABLE curriculum_activity_requirements
    ADD COLUMN term_slot_id UUID;

UPDATE curriculum_course_requirements requirement
SET term_slot_id = slot.id
FROM curriculum_term_slots slot
WHERE slot.curriculum_version_id = requirement.curriculum_version_id
  AND slot.term_type = CASE
      WHEN upper(btrim(requirement.recommended_term_code)) ~ '^((TERM-)?[1-9][0-9]*)$'
          THEN 'regular'
      WHEN upper(btrim(requirement.recommended_term_code)) = 'SUMMER' THEN 'summer'
      WHEN upper(btrim(requirement.recommended_term_code)) = 'REMEDIAL' THEN 'remedial'
      ELSE 'custom'
  END
  AND slot.type_occurrence = CASE
      WHEN upper(btrim(requirement.recommended_term_code)) ~ '^TERM-'
          THEN substring(upper(btrim(requirement.recommended_term_code)) FROM 6)::integer
      WHEN upper(btrim(requirement.recommended_term_code)) ~ '^[1-9][0-9]*$'
          THEN btrim(requirement.recommended_term_code)::integer
      WHEN upper(btrim(requirement.recommended_term_code)) ~ '^CUSTOM-'
          THEN substring(upper(btrim(requirement.recommended_term_code)) FROM 8)::integer
      ELSE 1
  END;

UPDATE curriculum_activity_requirements requirement
SET term_slot_id = slot.id
FROM curriculum_term_slots slot
WHERE slot.curriculum_version_id = requirement.curriculum_version_id
  AND slot.term_type = CASE
      WHEN upper(btrim(requirement.recommended_term_code)) ~ '^((TERM-)?[1-9][0-9]*)$'
          THEN 'regular'
      WHEN upper(btrim(requirement.recommended_term_code)) = 'SUMMER' THEN 'summer'
      WHEN upper(btrim(requirement.recommended_term_code)) = 'REMEDIAL' THEN 'remedial'
      ELSE 'custom'
  END
  AND slot.type_occurrence = CASE
      WHEN upper(btrim(requirement.recommended_term_code)) ~ '^TERM-'
          THEN substring(upper(btrim(requirement.recommended_term_code)) FROM 6)::integer
      WHEN upper(btrim(requirement.recommended_term_code)) ~ '^[1-9][0-9]*$'
          THEN btrim(requirement.recommended_term_code)::integer
      WHEN upper(btrim(requirement.recommended_term_code)) ~ '^CUSTOM-'
          THEN substring(upper(btrim(requirement.recommended_term_code)) FROM 8)::integer
      ELSE 1
  END;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM curriculum_course_requirements WHERE term_slot_id IS NULL
    ) OR EXISTS (
        SELECT 1 FROM curriculum_activity_requirements WHERE term_slot_id IS NULL
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_048_TERM_SLOT_BACKFILL_INCOMPLETE';
    END IF;
END
$$;

ALTER TABLE curriculum_course_requirements
    DROP CONSTRAINT curriculum_course_requirements_program_resource_key,
    DROP CONSTRAINT curriculum_course_requirements_credit_check,
    DROP CONSTRAINT curriculum_course_requirements_hours_check,
    ALTER COLUMN term_slot_id SET NOT NULL,
    ADD CONSTRAINT curriculum_course_requirements_term_slot_fkey
        FOREIGN KEY (term_slot_id, curriculum_version_id)
        REFERENCES curriculum_term_slots(id, curriculum_version_id) ON DELETE RESTRICT,
    ADD CONSTRAINT curriculum_course_requirements_program_resource_key
        UNIQUE (study_program_id, grade_level_id, term_slot_id, subject_version_id),
    DROP COLUMN recommended_term_code,
    DROP COLUMN credit,
    DROP COLUMN hours;

ALTER TABLE curriculum_activity_requirements
    DROP CONSTRAINT curriculum_activity_requirements_program_resource_key,
    DROP CONSTRAINT curriculum_activity_requirements_hours_check,
    ALTER COLUMN term_slot_id SET NOT NULL,
    ADD CONSTRAINT curriculum_activity_requirements_term_slot_fkey
        FOREIGN KEY (term_slot_id, curriculum_version_id)
        REFERENCES curriculum_term_slots(id, curriculum_version_id) ON DELETE RESTRICT,
    ADD CONSTRAINT curriculum_activity_requirements_program_resource_key
        UNIQUE (study_program_id, grade_level_id, term_slot_id, activity_version_id),
    DROP COLUMN recommended_term_code,
    DROP COLUMN hours;

ALTER TABLE curriculum_course_requirements
    ENABLE TRIGGER curriculum_course_requirements_published_immutable;

ALTER TABLE curriculum_activity_requirements
    ENABLE TRIGGER curriculum_activity_requirements_published_immutable;

CREATE TRIGGER curriculum_term_slots_published_immutable
BEFORE INSERT OR UPDATE OR DELETE ON curriculum_term_slots
FOR EACH ROW EXECUTE FUNCTION academic_prevent_published_curriculum_child_mutation();

CREATE INDEX curriculum_term_slots_version_order_idx
    ON curriculum_term_slots(curriculum_version_id, sequence, id);

CREATE INDEX curriculum_course_requirements_program_term_order_idx
    ON curriculum_course_requirements(study_program_id, term_slot_id, display_order, id);

CREATE INDEX curriculum_activity_requirements_program_term_order_idx
    ON curriculum_activity_requirements(study_program_id, term_slot_id, display_order, id);
