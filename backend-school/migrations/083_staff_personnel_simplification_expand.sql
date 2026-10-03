-- Migration-only expansion under coordinated maintenance. No product dual writes.
LOCK TABLE staff_info, staff_reference_items IN ACCESS EXCLUSIVE MODE;

DO $$
DECLARE bad_count bigint;
BEGIN
    SELECT count(*) INTO bad_count FROM staff_info i
    LEFT JOIN staff_reference_items p ON p.id=i.job_position_id AND p.kind='job_position'
    LEFT JOIN staff_reference_items m ON m.id=i.major_id AND m.kind='major'
    LEFT JOIN staff_reference_items u ON u.id=i.university_id AND u.kind='university'
    WHERE (i.job_position_id IS NOT NULL AND p.id IS NULL)
       OR (i.major_id IS NOT NULL AND m.id IS NULL)
       OR (i.university_id IS NOT NULL AND u.id IS NULL)
       OR m.name ~ '[[:cntrl:]]' OR u.name ~ '[[:cntrl:]]';
    IF bad_count <> 0 THEN
        RAISE EXCEPTION 'PERSONNEL_SIMPLIFICATION_SOURCE_INVALID count=%', bad_count;
    END IF;
END $$;

CREATE TABLE staff_job_positions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    code varchar(64) NOT NULL UNIQUE CHECK (code ~ '^[a-z0-9_]+$'),
    name varchar(200) NOT NULL CHECK (char_length(name)>0 AND name !~ '[[:cntrl:]]'),
    is_active boolean NOT NULL,
    is_selectable boolean NOT NULL,
    display_order integer NOT NULL,
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    CONSTRAINT staff_job_positions_selection_check CHECK (NOT is_selectable OR (is_active AND code IN (
        'teacher','assistant_teacher','contract_teacher','government_employee_teacher',
        'school_director','deputy_school_director','support_staff'
    )))
);

INSERT INTO staff_job_positions(id,code,name,is_active,is_selectable,display_order,created_at,updated_at)
SELECT id,code,name,is_active,is_active AND code IN (
    'teacher','assistant_teacher','contract_teacher','government_employee_teacher',
    'school_director','deputy_school_director','support_staff'
),display_order,created_at,updated_at FROM staff_reference_items WHERE kind='job_position';

ALTER TABLE staff_info
    ADD COLUMN major varchar(200),
    ADD COLUMN university varchar(200),
    ADD CONSTRAINT staff_info_major_text_check CHECK (major IS NULL OR (char_length(major)>0 AND major=btrim(major) AND major !~ '[[:cntrl:]]')),
    ADD CONSTRAINT staff_info_university_text_check CHECK (university IS NULL OR (char_length(university)>0 AND university=btrim(university) AND university !~ '[[:cntrl:]]'));

-- Preserve the pre-migration edit timestamp; all writers are excluded by the table lock.
ALTER TABLE staff_info DISABLE TRIGGER update_staff_info_updated_at;
UPDATE staff_info i SET major=m.name,university=u.name
FROM staff_info source
LEFT JOIN staff_reference_items m ON m.id=source.major_id AND m.kind='major'
LEFT JOIN staff_reference_items u ON u.id=source.university_id AND u.kind='university'
WHERE i.id=source.id;
ALTER TABLE staff_info ENABLE TRIGGER update_staff_info_updated_at;

-- These functions are temporary migration gates and are removed by 084.
CREATE FUNCTION staff_personnel_simplification_snapshot(source boolean) RETURNS jsonb
LANGUAGE plpgsql AS $$
DECLARE staff_rows jsonb; position_rows jsonb;
BEGIN
    IF source THEN
        SELECT coalesce(jsonb_agg(
            (to_jsonb(i)-ARRAY['major_id','university_id','job_position_kind','major_kind','university_kind','major','university'])
            || jsonb_build_object('major',m.name,'university',u.name) ORDER BY i.id
        ),'[]'::jsonb) INTO staff_rows
        FROM staff_info i
        LEFT JOIN staff_reference_items m ON m.id=i.major_id AND m.kind='major'
        LEFT JOIN staff_reference_items u ON u.id=i.university_id AND u.kind='university';
        SELECT coalesce(jsonb_agg(
            (to_jsonb(p)-ARRAY['kind','normalized_name']) || jsonb_build_object('is_selectable',p.is_active AND p.code IN (
                'teacher','assistant_teacher','contract_teacher','government_employee_teacher',
                'school_director','deputy_school_director','support_staff'
            )) ORDER BY p.id
        ),'[]'::jsonb) INTO position_rows FROM staff_reference_items p WHERE p.kind='job_position';
    ELSE
        SELECT coalesce(jsonb_agg(to_jsonb(i)-ARRAY['major_id','university_id','job_position_kind','major_kind','university_kind'] ORDER BY i.id),'[]'::jsonb)
        INTO staff_rows FROM staff_info i;
        SELECT coalesce(jsonb_agg(to_jsonb(p) ORDER BY p.id),'[]'::jsonb) INTO position_rows FROM staff_job_positions p;
    END IF;
    RETURN jsonb_build_object('staff',staff_rows,'positions',position_rows);
END $$;

CREATE FUNCTION staff_personnel_simplification_source_fingerprint() RETURNS text
LANGUAGE sql AS $$
    SELECT md5(jsonb_build_object(
        'staff',(SELECT coalesce(jsonb_agg(to_jsonb(i)-ARRAY['major','university'] ORDER BY i.id),'[]'::jsonb) FROM staff_info i),
        'references',(SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]'::jsonb) FROM staff_reference_items r)
    )::text)
$$;

CREATE TABLE staff_personnel_simplification_audit (
    migration_version bigint PRIMARY KEY CHECK (migration_version=84),
    passed boolean NOT NULL,
    cutover_completed boolean NOT NULL DEFAULT false,
    source_count bigint NOT NULL CHECK (source_count>=0),
    position_count bigint NOT NULL CHECK (position_count>=0),
    unused_education_option_count bigint NOT NULL CHECK (unused_education_option_count>=0),
    source_fingerprint text NOT NULL,
    target_fingerprint text NOT NULL,
    checks jsonb NOT NULL CHECK (jsonb_typeof(checks)='array'),
    completed_at timestamptz
);

DO $$
DECLARE staff_count bigint; position_count bigint; unused_count bigint;
BEGIN
    IF staff_personnel_simplification_snapshot(true) IS DISTINCT FROM staff_personnel_simplification_snapshot(false) THEN
        RAISE EXCEPTION 'PERSONNEL_SIMPLIFICATION_PRESERVATION_FAILED';
    END IF;
    SELECT count(*) INTO staff_count FROM staff_info;
    SELECT count(*) INTO position_count FROM staff_job_positions;
    SELECT count(*) INTO unused_count FROM staff_reference_items r
    WHERE r.kind IN ('major','university') AND NOT EXISTS (
        SELECT 1 FROM staff_info i WHERE (r.kind='major' AND i.major_id=r.id) OR (r.kind='university' AND i.university_id=r.id)
    );
    INSERT INTO staff_personnel_simplification_audit(migration_version,passed,source_count,position_count,unused_education_option_count,source_fingerprint,target_fingerprint,checks)
    VALUES (84,true,staff_count,position_count,unused_count,staff_personnel_simplification_source_fingerprint(),md5(staff_personnel_simplification_snapshot(false)::text),jsonb_build_array(
        jsonb_build_object('code','PERSONNEL_SIMPLIFICATION_STAFF_PRESERVED','passed',true,'count',staff_count),
        jsonb_build_object('code','PERSONNEL_SIMPLIFICATION_POSITIONS_PRESERVED','passed',true,'count',position_count),
        jsonb_build_object('code','PERSONNEL_SIMPLIFICATION_EDUCATION_TEXT_PRESERVED','passed',true,'count',staff_count),
        jsonb_build_object('code','PERSONNEL_SIMPLIFICATION_UNRELATED_FIELDS_PRESERVED','passed',true,'count',staff_count)
    ));
END $$;
