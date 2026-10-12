-- Remove only per-person notes. Calendar notes and round correction reasons remain.
-- The matching runtime must be deployed under maintenance before this contract cleanup.
LOCK TABLE attendance_records, audit_logs IN ACCESS EXCLUSIVE MODE;

CREATE TEMP TABLE attendance_note_facts ON COMMIT DROP AS
SELECT count(*) n, COALESCE(md5(string_agg(md5((to_jsonb(r)-'note')::text),''
    ORDER BY r.session_id,r.student_id)),'') fingerprint
FROM attendance_records r;

-- Historical result saves used [student UUID,result,note] before snapshots and
-- [student UUID,result,note,revision] after snapshots. Accept the matching new
-- named snapshots as well, so a compatible writer need not recreate old notes.
CREATE FUNCTION pg_temp.attendance_result_without_note(source JSONB, is_after BOOLEAN)
RETURNS JSONB LANGUAGE plpgsql AS $$
DECLARE
    item JSONB;
    student_value JSONB;
    status_value JSONB;
    revision_value JSONB;
    mapped JSONB;
    output JSONB := '[]'::jsonb;
BEGIN
    IF jsonb_typeof(source) IS DISTINCT FROM 'array' THEN
        RAISE EXCEPTION 'ATTENDANCE_NOTE_AUDIT_UNMAPPABLE';
    END IF;
    FOR item IN SELECT value FROM jsonb_array_elements(source) LOOP
        revision_value := NULL;
        IF jsonb_typeof(item) = 'array' THEN
            IF jsonb_array_length(item) <> (CASE WHEN is_after THEN 4 ELSE 3 END)
                OR jsonb_typeof(item->2) IS DISTINCT FROM 'string' THEN
                RAISE EXCEPTION 'ATTENDANCE_NOTE_AUDIT_UNMAPPABLE';
            END IF;
            student_value := item->0;
            status_value := item->1;
            IF is_after THEN revision_value := item->3; END IF;
        ELSIF jsonb_typeof(item) = 'object' THEN
            IF item-'studentId'-'result'-'rowVersion' <> '{}'::jsonb
                OR (is_after AND NOT (item ? 'rowVersion')) THEN
                RAISE EXCEPTION 'ATTENDANCE_NOTE_AUDIT_UNMAPPABLE';
            END IF;
            student_value := item->'studentId';
            status_value := item->'result';
            IF item ? 'rowVersion' THEN revision_value := item->'rowVersion'; END IF;
        ELSE
            RAISE EXCEPTION 'ATTENDANCE_NOTE_AUDIT_UNMAPPABLE';
        END IF;
        IF jsonb_typeof(student_value) IS DISTINCT FROM 'string'
            OR (student_value #>> '{}') !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
            OR jsonb_typeof(status_value) IS DISTINCT FROM 'string'
            OR (status_value #>> '{}') NOT IN ('unchecked','present','late','absent','leave','activity')
            OR (revision_value IS NOT NULL AND (
                jsonb_typeof(revision_value) IS DISTINCT FROM 'number'
                OR (revision_value #>> '{}') !~ '^[1-9][0-9]*$')) THEN
            RAISE EXCEPTION 'ATTENDANCE_NOTE_AUDIT_UNMAPPABLE';
        END IF;
        mapped := jsonb_build_object('studentId',student_value,'result',status_value);
        IF revision_value IS NOT NULL THEN
            mapped := mapped || jsonb_build_object('rowVersion',revision_value);
        END IF;
        output := output || jsonb_build_array(mapped);
    END LOOP;
    RETURN output;
END $$;

-- Audit entries survive session archival/reset; do not limit conversion to live sessions.
CREATE TEMP TABLE attendance_note_audits ON COMMIT DROP AS
SELECT id,
    pg_temp.attendance_result_without_note(old_values,false) old_values,
    pg_temp.attendance_result_without_note(new_values,true) new_values
FROM audit_logs
WHERE entity_type='attendance' AND action='update'
    AND (old_values IS NOT NULL OR new_values IS NOT NULL);

CREATE TEMP TABLE attendance_note_audit_evidence ON COMMIT DROP AS
SELECT count(*) n, COALESCE(md5(string_agg(md5((CASE WHEN mapped.id IS NOT NULL
    THEN to_jsonb(a) || jsonb_build_object('old_values',mapped.old_values,'new_values',mapped.new_values)
    ELSE to_jsonb(a) END)::text),'' ORDER BY a.id)),'') fingerprint
FROM audit_logs a LEFT JOIN attendance_note_audits mapped ON mapped.id=a.id;

UPDATE audit_logs a SET old_values=mapped.old_values,new_values=mapped.new_values
FROM attendance_note_audits mapped WHERE a.id=mapped.id;

-- Reconciliation is a gate before removing the column, not merely a report.
DO $$
BEGIN
    IF EXISTS (SELECT * FROM attendance_note_audit_evidence EXCEPT
        SELECT count(*),COALESCE(md5(string_agg(md5(to_jsonb(a)::text),'' ORDER BY a.id)),'') FROM audit_logs a)
        OR EXISTS (SELECT * FROM attendance_note_facts EXCEPT
        SELECT count(*),COALESCE(md5(string_agg(md5((to_jsonb(r)-'note')::text),'' ORDER BY r.session_id,r.student_id)),'') FROM attendance_records r) THEN
        RAISE EXCEPTION 'ATTENDANCE_NOTE_PRESERVATION_FAILED';
    END IF;
END $$;

ALTER TABLE attendance_records DROP COLUMN note;

DO $$
BEGIN
    IF EXISTS (SELECT * FROM attendance_note_facts EXCEPT
        SELECT count(*),COALESCE(md5(string_agg(md5(to_jsonb(r)::text),'' ORDER BY r.session_id,r.student_id)),'') FROM attendance_records r) THEN
        RAISE EXCEPTION 'ATTENDANCE_NOTE_PRESERVATION_FAILED';
    END IF;
END $$;

INSERT INTO audit_logs(action,entity_type,description,metadata)
SELECT 'migrate','attendance','attendance.person_notes.removed',jsonb_build_object(
    'recordsPreserved',(SELECT n FROM attendance_note_facts),
    'resultAuditsConverted',(SELECT count(*) FROM attendance_note_audits));
