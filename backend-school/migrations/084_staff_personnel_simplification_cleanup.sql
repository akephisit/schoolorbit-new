-- The source remains locked until fresh evidence permits contraction.
LOCK TABLE staff_info, staff_reference_items, staff_job_positions, staff_personnel_simplification_audit IN ACCESS EXCLUSIVE MODE;

DO $$
DECLARE audit staff_personnel_simplification_audit%ROWTYPE; required text[]; bad_count bigint;
BEGIN
    SELECT * INTO audit FROM staff_personnel_simplification_audit WHERE migration_version=84;
    required := ARRAY['PERSONNEL_SIMPLIFICATION_STAFF_PRESERVED','PERSONNEL_SIMPLIFICATION_POSITIONS_PRESERVED',
        'PERSONNEL_SIMPLIFICATION_EDUCATION_TEXT_PRESERVED','PERSONNEL_SIMPLIFICATION_UNRELATED_FIELDS_PRESERVED'];
    IF audit.migration_version IS NULL OR NOT audit.passed OR audit.cutover_completed
       OR jsonb_array_length(audit.checks)<>4
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(audit.checks) c
            WHERE NOT coalesce((c->>'passed')::boolean,false)
               OR (c->>'count')::bigint IS DISTINCT FROM CASE
                  WHEN c->>'code'='PERSONNEL_SIMPLIFICATION_POSITIONS_PRESERVED' THEN audit.position_count
                  ELSE audit.source_count END)
       OR (SELECT array_agg(c->>'code' ORDER BY c->>'code') FROM jsonb_array_elements(audit.checks) c)
          IS DISTINCT FROM (SELECT array_agg(code ORDER BY code) FROM unnest(required) code)
       OR audit.source_count<>(SELECT count(*) FROM staff_info)
       OR audit.position_count<>(SELECT count(*) FROM staff_job_positions)
       OR audit.source_fingerprint IS DISTINCT FROM staff_personnel_simplification_source_fingerprint()
       OR audit.target_fingerprint IS DISTINCT FROM md5(staff_personnel_simplification_snapshot(false)::text)
       OR staff_personnel_simplification_snapshot(true) IS DISTINCT FROM staff_personnel_simplification_snapshot(false) THEN
        RAISE EXCEPTION 'PERSONNEL_SIMPLIFICATION_RECONCILIATION_REQUIRED';
    END IF;
    SELECT count(*) INTO bad_count FROM staff_info i
    LEFT JOIN staff_reference_items p ON p.id=i.job_position_id AND p.kind='job_position'
    LEFT JOIN staff_reference_items m ON m.id=i.major_id AND m.kind='major'
    LEFT JOIN staff_reference_items u ON u.id=i.university_id AND u.kind='university'
    WHERE (i.job_position_id IS NOT NULL AND p.id IS NULL)
       OR (i.major_id IS NOT NULL AND m.id IS NULL)
       OR (i.university_id IS NOT NULL AND u.id IS NULL);
    IF bad_count<>0 THEN
        RAISE EXCEPTION 'PERSONNEL_SIMPLIFICATION_SOURCE_INVALID count=%',bad_count;
    END IF;
END $$;

ALTER TABLE staff_info
    DROP CONSTRAINT staff_info_position_reference_fkey,
    DROP CONSTRAINT staff_info_major_reference_fkey,
    DROP CONSTRAINT staff_info_university_reference_fkey,
    DROP COLUMN job_position_kind,
    DROP COLUMN major_kind,
    DROP COLUMN university_kind,
    DROP COLUMN major_id,
    DROP COLUMN university_id,
    ADD CONSTRAINT staff_info_job_position_fkey FOREIGN KEY(job_position_id) REFERENCES staff_job_positions(id);

DROP FUNCTION staff_personnel_simplification_snapshot(boolean);
DROP FUNCTION staff_personnel_simplification_source_fingerprint();
DROP TABLE staff_reference_items;
DROP FUNCTION staff_reference_display_name(text);

UPDATE staff_personnel_simplification_audit SET cutover_completed=true,completed_at=now() WHERE migration_version=84;
