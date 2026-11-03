-- Terms own live weekday configuration. The former annual value remains an
-- initial first-term preset, including for years that have no terms yet.
LOCK TABLE academic_years, academic_terms IN SHARE ROW EXCLUSIVE MODE;

DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM academic_years
        WHERE school_days = ''
           OR school_days !~ '^(MON|TUE|WED|THU|FRI|SAT|SUN)(,(MON|TUE|WED|THU|FRI|SAT|SUN))*$'
           OR cardinality(string_to_array(school_days, ',')) <>
              (SELECT count(DISTINCT day) FROM unnest(string_to_array(school_days, ',')) day)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_SCHOOL_DAYS155_INVALID_YEAR_PRESET' USING ERRCODE='23514';
    END IF;
END $$;

CREATE TEMP TABLE academic_school_days155_mapping ON COMMIT DROP AS
SELECT term.id, term.academic_year_id, year.school_days
FROM academic_terms term JOIN academic_years year ON year.id=term.academic_year_id;

CREATE TEMP TABLE academic_school_days155_before (
    relation_name text PRIMARY KEY,
    row_count bigint NOT NULL,
    fingerprint text NOT NULL
) ON COMMIT DROP;

-- Hash complete rows individually, then aggregate fixed-size hashes. This
-- preserves historical facts without materializing their payload in the audit.
DO $$ DECLARE relation_name text; BEGIN
    FOR relation_name IN SELECT name FROM unnest(ARRAY[
        'academic_years','academic_terms','bell_schedules','bell_schedule_periods',
        'academic_term_transition_receipts','academic_year_transition_receipts',
        'academic_timetable_versions','academic_timetable_blocks',
        'attendance_settings','attendance_days','attendance_sessions',
        'attendance_records','attendance_term_archives','academic_context_cutover_audit'
    ]) name ORDER BY name LOOP
        IF to_regclass(current_schema()||'.'||relation_name) IS NOT NULL THEN
            EXECUTE format('LOCK TABLE %I IN SHARE MODE',relation_name);
            EXECUTE format($sql$
                INSERT INTO academic_school_days155_before
                SELECT %L,count(*),encode(sha256(convert_to(coalesce(string_agg(fingerprint,'' ORDER BY fingerprint),''),'UTF8')),'hex')
                FROM (SELECT encode(sha256(convert_to(to_jsonb(row)::text,'UTF8')),'hex') fingerprint FROM %I row) facts
            $sql$,relation_name,relation_name);
        END IF;
    END LOOP;
END $$;

ALTER TABLE academic_terms ADD COLUMN school_days varchar(50);
CREATE TEMP TABLE academic_school_days155_trigger ON COMMIT DROP AS
SELECT tgenabled FROM pg_trigger
WHERE tgrelid='academic_terms'::regclass AND tgname='update_academic_semesters_updated_at';
ALTER TABLE academic_terms DISABLE TRIGGER update_academic_semesters_updated_at;
UPDATE academic_terms term SET school_days=mapping.school_days
FROM academic_school_days155_mapping mapping WHERE mapping.id=term.id;
DO $$ DECLARE original_state "char"; BEGIN
    SELECT tgenabled INTO STRICT original_state FROM academic_school_days155_trigger;
    CASE original_state
        WHEN 'O' THEN ALTER TABLE academic_terms ENABLE TRIGGER update_academic_semesters_updated_at;
        WHEN 'A' THEN ALTER TABLE academic_terms ENABLE ALWAYS TRIGGER update_academic_semesters_updated_at;
        WHEN 'R' THEN ALTER TABLE academic_terms ENABLE REPLICA TRIGGER update_academic_semesters_updated_at;
        WHEN 'D' THEN ALTER TABLE academic_terms DISABLE TRIGGER update_academic_semesters_updated_at;
        ELSE RAISE EXCEPTION 'ACADEMIC_SCHOOL_DAYS155_TRIGGER_STATE_INVALID' USING ERRCODE='23514';
    END CASE;
END $$;

DO $$ BEGIN
    IF (SELECT count(*) FROM academic_school_days155_mapping) <> (SELECT count(*) FROM academic_terms)
       OR EXISTS (
           SELECT 1 FROM academic_school_days155_mapping mapping
           FULL JOIN academic_terms term ON term.id=mapping.id
           WHERE term.id IS NULL OR mapping.id IS NULL
              OR term.academic_year_id IS DISTINCT FROM mapping.academic_year_id
              OR term.school_days IS DISTINCT FROM mapping.school_days
       ) THEN
        RAISE EXCEPTION 'ACADEMIC_SCHOOL_DAYS155_MAPPING_FAILED' USING ERRCODE='23514';
    END IF;
END $$;

ALTER TABLE academic_terms ALTER COLUMN school_days SET NOT NULL;
ALTER TABLE academic_terms ALTER COLUMN school_days SET DEFAULT 'MON,TUE,WED,THU,FRI';
ALTER TABLE academic_terms ADD CONSTRAINT academic_terms_school_days_check CHECK (
    school_days ~ '^(MON|TUE|WED|THU|FRI|SAT|SUN)(,(MON|TUE|WED|THU|FRI|SAT|SUN))*$'
);
ALTER TABLE academic_years RENAME COLUMN school_days TO default_school_days;
COMMENT ON COLUMN academic_years.default_school_days IS 'Initial weekday preset for a first term; never live term configuration';
COMMENT ON COLUMN academic_terms.school_days IS 'Independent canonical school weekdays for this term';

DO $$ DECLARE evidence record; current_count bigint; current_hash text; projection text; BEGIN
    FOR evidence IN SELECT * FROM academic_school_days155_before ORDER BY relation_name LOOP
        projection := CASE evidence.relation_name
            WHEN 'academic_terms' THEN 'to_jsonb(row)-''school_days'''
            WHEN 'academic_years' THEN '(to_jsonb(row)-''default_school_days'')||jsonb_build_object(''school_days'',row.default_school_days)'
            ELSE 'to_jsonb(row)' END;
        EXECUTE format($sql$
            SELECT count(*),encode(sha256(convert_to(coalesce(string_agg(fingerprint,'' ORDER BY fingerprint),''),'UTF8')),'hex')
            FROM (SELECT encode(sha256(convert_to((%s)::text,'UTF8')),'hex') fingerprint FROM %I row) facts
        $sql$,projection,evidence.relation_name) INTO current_count,current_hash;
        IF current_count <> evidence.row_count OR current_hash <> evidence.fingerprint THEN
            RAISE EXCEPTION 'ACADEMIC_SCHOOL_DAYS155_PRESERVATION_FAILED' USING ERRCODE='23514';
        END IF;
    END LOOP;
END $$;

CREATE TABLE academic_term_school_days_migration_audit (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    term_count bigint NOT NULL,
    mapping_fingerprint text NOT NULL,
    preservation_evidence jsonb NOT NULL,
    reconciled_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
INSERT INTO academic_term_school_days_migration_audit(term_count,mapping_fingerprint,preservation_evidence)
SELECT (SELECT count(*) FROM academic_school_days155_mapping),
       (SELECT encode(sha256(convert_to(coalesce(string_agg(to_jsonb(mapping)::text,'' ORDER BY id),''),'UTF8')),'hex')
        FROM academic_school_days155_mapping mapping),
       (SELECT jsonb_agg(to_jsonb(evidence) ORDER BY relation_name) FROM academic_school_days155_before evidence);
