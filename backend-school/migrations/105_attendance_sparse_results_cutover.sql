-- Old attendance binaries are prohibited after this transaction commits.
LOCK TABLE attendance_sessions, attendance_records, attendance_scan_events IN ACCESS EXCLUSIVE MODE;

-- Include any last old-runtime commit between expand and cutover.
UPDATE attendance_sessions s SET roster = COALESCE((
    SELECT jsonb_agg(jsonb_build_object(
        'studentId', r.student_id,
        'studentAcademicYearId', r.student_academic_year_id,
        'classNumber', r.class_number
    ) ORDER BY r.student_id)
    FROM attendance_records r WHERE r.session_id = s.id
), '[]'::jsonb);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM attendance_sessions s
        WHERE jsonb_array_length(s.roster) <> (SELECT count(*) FROM attendance_records r WHERE r.session_id=s.id)
    ) OR EXISTS (
        SELECT 1 FROM attendance_records r JOIN attendance_sessions s ON s.id=r.session_id
        WHERE NOT s.roster @> jsonb_build_array(jsonb_build_object(
            'studentId',r.student_id,'studentAcademicYearId',r.student_academic_year_id,'classNumber',r.class_number))
    ) THEN
        RAISE EXCEPTION 'ATTENDANCE_ROSTER_RECONCILIATION_FAILED';
    END IF;
    IF EXISTS(SELECT 1 FROM attendance_term_summaries WHERE category='flag' GROUP BY academic_term_id,student_id HAVING count(*)>1)
    OR EXISTS(SELECT 1 FROM attendance_term_teacher_summaries WHERE category='flag' GROUP BY academic_term_id,teacher_id,student_id HAVING count(*)>1) THEN
        RAISE EXCEPTION 'ATTENDANCE_ARCHIVED_CEREMONY_SCOPE_UNMAPPABLE';
    END IF;
    IF EXISTS (SELECT 1 FROM attendance_scan_events e JOIN attendance_records r
               ON r.session_id=e.session_id AND r.student_id=e.student_id WHERE r.result='unchecked') THEN
        RAISE EXCEPTION 'ATTENDANCE_UNCHECKED_CAPTURE_UNMAPPABLE';
    END IF;
END $$;

-- Fingerprints are bounded evidence; no source rows leave the tenant transaction.
CREATE TEMP TABLE attendance_preservation ON COMMIT DROP AS
SELECT 'records' resource,count(*) n,COALESCE(md5(string_agg(md5((to_jsonb(r)-'special_round_id')::text),'' ORDER BY r.session_id,r.student_id)),'') fingerprint
FROM attendance_records r WHERE result<>'unchecked'
UNION ALL SELECT 'captures',count(*),COALESCE(md5(string_agg(md5(to_jsonb(e)::text),'' ORDER BY e.id)),'') FROM attendance_scan_events e
UNION ALL SELECT 'sessions',count(*),COALESCE(md5(string_agg(md5((to_jsonb(t)-'kind'-'special_round_id'-'source_key')::text),'' ORDER BY t.id)),'') FROM attendance_sessions t
UNION ALL SELECT 'archives',count(*),COALESCE(md5(string_agg(md5((to_jsonb(t)-'category'-'scope_key'-'scope_label')::text),'' ORDER BY md5((to_jsonb(t)-'category'-'scope_key'-'scope_label')::text))),'') FROM attendance_term_summaries t
UNION ALL SELECT 'teacher_archives',count(*),COALESCE(md5(string_agg(md5((to_jsonb(t)-'category'-'scope_key'-'scope_label')::text),'' ORDER BY md5((to_jsonb(t)-'category'-'scope_key'-'scope_label')::text))),'') FROM attendance_term_teacher_summaries t;

-- Preserve actual ceremony meaning as historical special activity, never as arrival.
CREATE TEMP TABLE attendance_ceremony_conversion ON COMMIT DROP AS
SELECT s.id, uuid_generate_v5(s.id,'attendance-ceremony-template') template_id,
       uuid_generate_v5(s.id,'attendance-ceremony-round') round_id
FROM attendance_sessions s WHERE s.kind='flag';

INSERT INTO attendance_special_templates(id,academic_term_id,definition,created_by)
SELECT c.template_id,s.academic_term_id,jsonb_build_object(
    'title','หน้าเสาธง (ประวัติเดิม)', 'dates',jsonb_build_array(s.date),
    'startTime',s.start_time,'endTime',s.end_time,
    'counted',COALESCE(s.count_override,true),'notify',false,
    'groups',jsonb_build_array(jsonb_build_object(
        'name',s.title,'teacherIds',to_jsonb(s.teacher_ids),
        'homeroomIds',CASE WHEN s.homeroom_id IS NULL THEN '[]'::jsonb ELSE jsonb_build_array(s.homeroom_id) END,
        'audienceGroupIds','[]'::jsonb,'studentIds',COALESCE((SELECT jsonb_agg(p->'studentId') FROM jsonb_array_elements(s.roster) p),'[]'::jsonb),
        'useHomeroomAdvisors',false))),s.saved_by
FROM attendance_sessions s JOIN attendance_ceremony_conversion c ON c.id=s.id;
INSERT INTO attendance_special_rounds(id,template_id,date)
SELECT c.round_id,c.template_id,s.date FROM attendance_ceremony_conversion c JOIN attendance_sessions s ON s.id=c.id;

ALTER TABLE attendance_sessions DROP CONSTRAINT attendance_sessions_kind_check;
ALTER TABLE attendance_sessions DROP CONSTRAINT attendance_sessions_check;
-- Update the composite relationship with its dependants in the same transaction.
ALTER TABLE attendance_records DROP CONSTRAINT attendance_records_session_id_special_round_id_fkey;
UPDATE attendance_sessions s SET kind='special',special_round_id=c.round_id,
    source_key='historical-ceremony:'||s.id::text
FROM attendance_ceremony_conversion c WHERE c.id=s.id;
UPDATE attendance_records r SET special_round_id=c.round_id
FROM attendance_ceremony_conversion c WHERE c.id=r.session_id;
ALTER TABLE attendance_records ADD FOREIGN KEY(session_id,special_round_id)
    REFERENCES attendance_sessions(id,special_round_id);
ALTER TABLE attendance_sessions ADD CHECK(kind IN ('arrival','lesson','special'));
ALTER TABLE attendance_sessions ADD CHECK((kind='special')=(special_round_id IS NOT NULL));

UPDATE attendance_special_rounds round SET participants=COALESCE((
    SELECT jsonb_agg(jsonb_build_object(
        'sessionId',s.id,'sourceKey',s.source_key,'title',s.title,'teacherIds',to_jsonb(s.teacher_ids),
        'students',s.roster,'startTime',s.start_time,'endTime',s.end_time,'countOverride',s.count_override
    ) ORDER BY s.id) FROM attendance_sessions s WHERE s.special_round_id=round.id
),'[]'::jsonb);

-- Archived aggregates use the canonical special category and keep their exact totals.
UPDATE attendance_term_summaries SET category='special',
    scope_key=uuid_generate_v5(academic_term_id,'attendance-archived-ceremony')::text,
    scope_label='หน้าเสาธง (ประวัติเดิม)' WHERE category='flag';
UPDATE attendance_term_teacher_summaries SET category='special',
    scope_key=uuid_generate_v5(academic_term_id,'attendance-archived-ceremony')::text,
    scope_label='หน้าเสาธง (ประวัติเดิม)' WHERE category='flag';

DO $$
BEGIN
    IF EXISTS (SELECT * FROM attendance_preservation EXCEPT SELECT * FROM (SELECT 'records' resource,count(*) n,COALESCE(md5(string_agg(md5((to_jsonb(r)-'special_round_id')::text),'' ORDER BY r.session_id,r.student_id)),'') fingerprint
FROM attendance_records r WHERE result<>'unchecked'
UNION ALL SELECT 'captures',count(*),COALESCE(md5(string_agg(md5(to_jsonb(e)::text),'' ORDER BY e.id)),'') FROM attendance_scan_events e
UNION ALL SELECT 'sessions',count(*),COALESCE(md5(string_agg(md5((to_jsonb(t)-'kind'-'special_round_id'-'source_key')::text),'' ORDER BY t.id)),'') FROM attendance_sessions t
UNION ALL SELECT 'archives',count(*),COALESCE(md5(string_agg(md5((to_jsonb(t)-'category'-'scope_key'-'scope_label')::text),'' ORDER BY md5((to_jsonb(t)-'category'-'scope_key'-'scope_label')::text))),'') FROM attendance_term_summaries t
UNION ALL SELECT 'teacher_archives',count(*),COALESCE(md5(string_agg(md5((to_jsonb(t)-'category'-'scope_key'-'scope_label')::text),'' ORDER BY md5((to_jsonb(t)-'category'-'scope_key'-'scope_label')::text))),'') FROM attendance_term_teacher_summaries t) current_evidence) THEN
        RAISE EXCEPTION 'ATTENDANCE_FACT_PRESERVATION_FAILED';
    END IF;
    IF EXISTS(SELECT 1 FROM attendance_special_rounds WHERE participants='[]'::jsonb) THEN
        RAISE EXCEPTION 'ATTENDANCE_SPECIAL_PARTICIPANTS_UNMAPPABLE';
    END IF;
END $$;

INSERT INTO audit_logs(action,entity_type,description,metadata)
SELECT 'migrate','attendance','attendance.sparse_roster.reconciled',jsonb_build_object(
    'sessions',(SELECT count(*) FROM attendance_sessions),
    'participants',(SELECT COALESCE(sum(jsonb_array_length(roster)),0) FROM attendance_sessions),
    'explicitResults',(SELECT count(*) FROM attendance_records WHERE result<>'unchecked'),
    'uncheckedRemoved',(SELECT count(*) FROM attendance_records WHERE result='unchecked'),
    'ceremoniesConverted',(SELECT count(*) FROM attendance_ceremony_conversion));

DELETE FROM attendance_records WHERE result='unchecked';
ALTER TABLE attendance_records ALTER COLUMN result DROP DEFAULT;
ALTER TABLE attendance_records DROP CONSTRAINT attendance_records_result_check;
ALTER TABLE attendance_records ADD CHECK(result IN ('present','late','absent','leave','activity'));
DROP TABLE attendance_calendar_snapshots;
