-- Canonical personnel data. Run through the tenant migrator under maintenance.
-- Legacy education must map exactly; every cleanup follows locked reconciliation.
LOCK TABLE staff_info IN ACCESS EXCLUSIVE MODE;

CREATE FUNCTION staff_reference_display_name(value text) RETURNS text
LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$ SELECT btrim(regexp_replace(value, '[[:space:]]+', ' ', 'g')) $$;

CREATE TEMP TABLE personnel_source ON COMMIT DROP AS SELECT * FROM staff_info;
CREATE TEMP TABLE personnel_education_aliases (
    alias text PRIMARY KEY,
    code varchar(100) NOT NULL
) ON COMMIT DROP;
INSERT INTO personnel_education_aliases (alias, code) VALUES
    ('primary', 'primary'), ('ประถมศึกษา', 'primary'),
    ('lower_secondary', 'lower_secondary'), ('มัธยมศึกษาตอนต้น', 'lower_secondary'), ('ม.3', 'lower_secondary'),
    ('upper_secondary', 'upper_secondary'), ('มัธยมศึกษาตอนปลาย', 'upper_secondary'), ('ม.6', 'upper_secondary'),
    ('vocational_certificate', 'vocational_certificate'), ('ปวช.', 'vocational_certificate'), ('ประกาศนียบัตรวิชาชีพ', 'vocational_certificate'),
    ('higher_vocational', 'higher_vocational'), ('ปวส.', 'higher_vocational'), ('ประกาศนียบัตรวิชาชีพชั้นสูง', 'higher_vocational'),
    ('diploma', 'diploma'), ('อนุปริญญา', 'diploma'),
    ('bachelor', 'bachelor'), ('ปริญญาตรี', 'bachelor'), ('ป.ตรี', 'bachelor'), ('bachelor''s degree', 'bachelor'), ('bachelor degree', 'bachelor'),
    ('master', 'master'), ('ปริญญาโท', 'master'), ('ป.โท', 'master'), ('master''s degree', 'master'), ('master degree', 'master'),
    ('doctorate', 'doctorate'), ('ปริญญาเอก', 'doctorate'), ('ป.เอก', 'doctorate'), ('doctoral degree', 'doctorate'), ('phd', 'doctorate'), ('ph.d.', 'doctorate'),
    ('other', 'other'), ('อื่น ๆ', 'other'), ('อื่นๆ', 'other');

DO $$
DECLARE bad_count bigint;
BEGIN
    SELECT count(*) INTO bad_count
    FROM personnel_source source
    LEFT JOIN personnel_education_aliases aliases
      ON aliases.alias = lower(staff_reference_display_name(source.education_level))
    WHERE nullif(staff_reference_display_name(source.education_level), '') IS NOT NULL
      AND aliases.code IS NULL;
    IF bad_count <> 0 THEN
        RAISE EXCEPTION 'PERSONNEL_EDUCATION_UNMAPPED count=%', bad_count;
    END IF;
    SELECT count(*) INTO bad_count FROM personnel_source
    WHERE staff_reference_display_name(major) ~ '[[:cntrl:]]'
       OR staff_reference_display_name(university) ~ '[[:cntrl:]]';
    IF bad_count <> 0 THEN
        RAISE EXCEPTION 'PERSONNEL_REFERENCE_INVALID count=%', bad_count;
    END IF;
END $$;

CREATE TABLE staff_reference_items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    kind varchar(32) NOT NULL CHECK (kind IN ('job_position', 'major', 'university')),
    code varchar(64) NOT NULL CHECK (code ~ '^[a-z0-9_]+$'),
    name varchar(200) NOT NULL CHECK (char_length(name) > 0 AND name = staff_reference_display_name(name) AND name !~ '[[:cntrl:]]'),
    normalized_name text GENERATED ALWAYS AS (lower(staff_reference_display_name(name))) STORED NOT NULL,
    is_active boolean NOT NULL DEFAULT true,
    display_order integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT staff_reference_items_kind_code_key UNIQUE (kind, code),
    CONSTRAINT staff_reference_items_kind_name_key UNIQUE (kind, normalized_name),
    CONSTRAINT staff_reference_items_id_kind_key UNIQUE (id, kind)
);
INSERT INTO staff_reference_items (kind, code, name, display_order) VALUES
    ('job_position', 'teacher', 'ครู', 10),
    ('job_position', 'assistant_teacher', 'ครูผู้ช่วย', 20),
    ('job_position', 'contract_teacher', 'ครูอัตราจ้าง', 30),
    ('job_position', 'government_employee_teacher', 'พนักงานราชการ (ครู)', 40),
    ('job_position', 'school_director', 'ผู้อำนวยการสถานศึกษา', 50),
    ('job_position', 'deputy_school_director', 'รองผู้อำนวยการสถานศึกษา', 60),
    ('job_position', 'support_staff', 'เจ้าหน้าที่สนับสนุน', 70);

WITH names AS (
    SELECT kind, min(name) AS name
    FROM (
        SELECT 'major' AS kind, staff_reference_display_name(major) AS name FROM personnel_source
        UNION ALL
        SELECT 'university', staff_reference_display_name(university) FROM personnel_source
    ) source_names
    WHERE nullif(name, '') IS NOT NULL
    GROUP BY kind, lower(name)
), identified AS MATERIALIZED (
    SELECT gen_random_uuid() AS id, kind, name FROM names
)
INSERT INTO staff_reference_items (id, kind, code, name)
SELECT id, kind, 'ref_' || replace(id::text, '-', ''), name FROM identified;

ALTER TABLE staff_info
    ADD COLUMN job_position_id uuid,
    ADD COLUMN academic_rank varchar(32) CHECK (academic_rank IN ('none', 'not_applicable', 'proficient', 'senior_proficient', 'expert', 'senior_expert')),
    ADD COLUMN major_id uuid,
    ADD COLUMN university_id uuid,
    ADD COLUMN job_position_kind varchar(32) GENERATED ALWAYS AS ('job_position'::varchar) STORED,
    ADD COLUMN major_kind varchar(32) GENERATED ALWAYS AS ('major'::varchar) STORED,
    ADD COLUMN university_kind varchar(32) GENERATED ALWAYS AS ('university'::varchar) STORED,
    ADD CONSTRAINT staff_info_position_reference_fkey FOREIGN KEY (job_position_id, job_position_kind) REFERENCES staff_reference_items (id, kind),
    ADD CONSTRAINT staff_info_major_reference_fkey FOREIGN KEY (major_id, major_kind) REFERENCES staff_reference_items (id, kind),
    ADD CONSTRAINT staff_info_university_reference_fkey FOREIGN KEY (university_id, university_kind) REFERENCES staff_reference_items (id, kind);

UPDATE staff_info target SET education_level = aliases.code
FROM personnel_source source
LEFT JOIN personnel_education_aliases aliases
  ON aliases.alias = lower(staff_reference_display_name(source.education_level))
WHERE target.id = source.id;
UPDATE staff_info target SET major_id = reference.id
FROM personnel_source source JOIN staff_reference_items reference
  ON reference.kind = 'major' AND reference.normalized_name = lower(staff_reference_display_name(source.major))
WHERE target.id = source.id;
UPDATE staff_info target SET university_id = reference.id
FROM personnel_source source JOIN staff_reference_items reference
  ON reference.kind = 'university' AND reference.normalized_name = lower(staff_reference_display_name(source.university))
WHERE target.id = source.id;

CREATE TABLE staff_personnel_cutover_audit (
    migration_version bigint PRIMARY KEY CHECK (migration_version = 81),
    passed boolean NOT NULL CHECK (passed),
    source_count bigint NOT NULL CHECK (source_count >= 0),
    checks jsonb NOT NULL CHECK (jsonb_typeof(checks) = 'array' AND jsonb_array_length(checks) > 0),
    completed_at timestamptz NOT NULL DEFAULT now()
);

DO $$
DECLARE
    source_count bigint;
    current_count bigint;
    bad_count bigint;
BEGIN
    SELECT count(*) INTO source_count FROM personnel_source;
    SELECT count(*) INTO current_count FROM staff_info;
    SELECT count(*) INTO bad_count FROM personnel_source source
    LEFT JOIN staff_info target ON target.id = source.id
    WHERE target.id IS NULL OR
      (source.user_id, source.employment_type, source.teaching_license_number, source.teaching_license_expiry, source.metadata, source.created_at)
      IS DISTINCT FROM
      (target.user_id, target.employment_type, target.teaching_license_number, target.teaching_license_expiry, target.metadata, target.created_at);
    IF current_count <> source_count OR bad_count <> 0 THEN
        RAISE EXCEPTION 'PERSONNEL_PRESERVATION_FAILED count=%', bad_count;
    END IF;
    SELECT count(*) INTO bad_count FROM personnel_source source
    JOIN staff_info target ON target.id = source.id
    LEFT JOIN personnel_education_aliases aliases ON aliases.alias = lower(staff_reference_display_name(source.education_level))
    LEFT JOIN staff_reference_items major ON major.id = target.major_id AND major.kind = 'major'
    LEFT JOIN staff_reference_items university ON university.id = target.university_id AND university.kind = 'university'
    WHERE target.education_level IS DISTINCT FROM aliases.code
       OR nullif(lower(staff_reference_display_name(source.major)), '') IS DISTINCT FROM major.normalized_name
       OR nullif(lower(staff_reference_display_name(source.university)), '') IS DISTINCT FROM university.normalized_name
       OR target.job_position_id IS NOT NULL OR target.academic_rank IS NOT NULL;
    IF bad_count <> 0 THEN
        RAISE EXCEPTION 'PERSONNEL_RECONCILIATION_FAILED count=%', bad_count;
    END IF;
    INSERT INTO staff_personnel_cutover_audit (migration_version, passed, source_count, checks)
    VALUES (81, true, source_count, jsonb_build_array(
        jsonb_build_object('code', 'PERSONNEL_ROWS_PRESERVED', 'passed', true, 'count', source_count),
        jsonb_build_object('code', 'PERSONNEL_RELATIONSHIPS_PRESERVED', 'passed', true, 'count', source_count),
        jsonb_build_object('code', 'PERSONNEL_EDUCATION_MAPPED', 'passed', true, 'count', source_count),
        jsonb_build_object('code', 'PERSONNEL_REFERENCES_MAPPED', 'passed', true, 'count', source_count)
    ));
    IF NOT EXISTS (SELECT 1 FROM staff_personnel_cutover_audit WHERE migration_version = 81 AND passed AND jsonb_array_length(checks) = 4) THEN
        RAISE EXCEPTION 'PERSONNEL_RECONCILIATION_REQUIRED';
    END IF;
END $$;

ALTER TABLE staff_info
    ADD CONSTRAINT staff_info_education_code_check CHECK (education_level IN ('primary', 'lower_secondary', 'upper_secondary', 'vocational_certificate', 'higher_vocational', 'diploma', 'bachelor', 'master', 'doctorate', 'other')),
    DROP COLUMN major,
    DROP COLUMN university;
