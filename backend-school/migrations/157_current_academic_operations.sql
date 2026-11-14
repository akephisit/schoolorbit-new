-- One normalized current Delivery and Timetable per explicit term.
-- The matching release runs under universal maintenance. Recover forward.
SET LOCAL lock_timeout='30s';
LOCK TABLE academic_exam_rounds,academic_exam_schedule_items,academic_exam_sessions,academic_opening_policy,academic_terms,learning_offerings,course_offering_details,activity_offering_details,
    learning_groups,learning_group_teachers,learning_group_students,learning_group_homerooms,
    learning_offering_targets,learning_group_preferred_rooms,learning_group_roster_tracking,
    academic_delivery_versions,academic_timetable_versions,academic_term_change_sets,
    academic_term_change_items,academic_teacher_handoff_runs,academic_timetable_blocks,
    academic_timetable_block_groups,academic_timetable_block_group_instructors,
    academic_timetable_block_homerooms,academic_timetable_block_teachers,
    academic_timetable_block_group_sync,academic_delivery_revision_audiences,
    academic_delivery_revision_audience_rooms,academic_delivery_teacher_cutoffs,
    academic_delivery_offering_effectivity,attendance_sessions,attendance_records,
    attendance_scan_events,files,file_versions,file_derivatives,file_operations,file_storage_cleanup_watches,
    attendance_notifications,notification_receipts,notification_presentations,attendance_score_changes,attendance_module_state,
    supervision_observations IN ACCESS EXCLUSIVE MODE;
SET CONSTRAINTS ALL IMMEDIATE;

CREATE TEMP TABLE academic157_opening_policy ON COMMIT DROP AS
SELECT to_jsonb(policy)-'require_published_offerings'-'require_published_timetable'
    ||jsonb_build_object('require_offerings',require_published_offerings,'require_timetable',require_published_timetable) value
FROM academic_opening_policy policy;
ALTER TABLE academic_opening_policy RENAME COLUMN require_published_offerings TO require_offerings;
ALTER TABLE academic_opening_policy RENAME COLUMN require_published_timetable TO require_timetable;

CREATE TEMP TABLE academic157_context ON COMMIT DROP AS
SELECT (clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date cutover_date;
CREATE TEMP TABLE academic157_sources ON COMMIT DROP AS
SELECT t.id academic_term_id,t.academic_year_id,
    academic_delivery_applicable_publication(t.id,c.cutover_date) delivery_id,
    academic_timetable_applicable_publication(t.id,c.cutover_date) timetable_id
FROM academic_terms t CROSS JOIN academic157_context c;

-- Unopened preparation policy: a single saved draft OR single future plan,
-- never an arbitrary newest candidate. A prior activation forbids this path.
DO $$ DECLARE term record; candidates uuid[]; BEGIN
    FOR term IN SELECT s.*,t.status FROM academic157_sources s JOIN academic_terms t ON t.id=s.academic_term_id LOOP
        IF term.delivery_id IS NULL THEN
            SELECT array_agg(id ORDER BY id) INTO candidates FROM academic_delivery_versions
                WHERE academic_term_id=term.academic_term_id AND status IN('draft','published');
            IF cardinality(candidates)>0 THEN
                IF term.status NOT IN('planning','ready') OR cardinality(candidates)<>1
                    OR EXISTS(SELECT 1 FROM academic_term_transition_receipts WHERE academic_term_id=term.academic_term_id AND action='activate') THEN
                    RAISE EXCEPTION 'ACADEMIC_CURRENT_DELIVERY_SOURCE_AMBIGUOUS'; END IF;
                UPDATE academic157_sources SET delivery_id=candidates[1] WHERE academic_term_id=term.academic_term_id;
            ELSIF EXISTS(SELECT 1 FROM learning_offerings WHERE academic_term_id=term.academic_term_id) THEN
                RAISE EXCEPTION 'ACADEMIC_CURRENT_DELIVERY_SOURCE_MISSING';
            END IF;
        END IF;
        IF term.timetable_id IS NULL THEN
            SELECT array_agg(id ORDER BY id) INTO candidates FROM academic_timetable_versions
                WHERE academic_term_id=term.academic_term_id AND status IN('draft','published');
            IF cardinality(candidates)>0 THEN
                IF term.status NOT IN('planning','ready') OR cardinality(candidates)<>1
                    OR EXISTS(SELECT 1 FROM academic_term_transition_receipts WHERE academic_term_id=term.academic_term_id AND action='activate') THEN
                    RAISE EXCEPTION 'ACADEMIC_CURRENT_TIMETABLE_SOURCE_AMBIGUOUS'; END IF;
                UPDATE academic157_sources SET timetable_id=candidates[1] WHERE academic_term_id=term.academic_term_id;
            ELSIF EXISTS(SELECT 1 FROM academic_timetable_blocks WHERE academic_term_id=term.academic_term_id) THEN
                RAISE EXCEPTION 'ACADEMIC_CURRENT_TIMETABLE_SOURCE_MISSING';
            END IF;
        END IF;
    END LOOP;
END $$;

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic157_sources s JOIN academic_delivery_versions v ON v.id=s.delivery_id
        WHERE jsonb_typeof(v.snapshot) IS DISTINCT FROM 'object' OR jsonb_typeof(v.snapshot->'offerings') IS DISTINCT FROM 'array') THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_GRAPH_UNMAPPABLE'; END IF;
    IF EXISTS(SELECT 1 FROM academic157_sources s JOIN academic_delivery_versions v ON v.id=s.delivery_id
        CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') o
        WHERE jsonb_typeof(o) IS DISTINCT FROM 'object' OR o->>'id' IS NULL OR NOT pg_input_is_valid(o->>'id','uuid')
          OR o->>'weeklyPeriodTarget' IS NULL OR NOT pg_input_is_valid(o->>'weeklyPeriodTarget','int')
          OR jsonb_typeof(o->'groups') IS DISTINCT FROM 'array' OR jsonb_typeof(o->'targets') IS DISTINCT FROM 'array'
          OR jsonb_typeof(o->'catalog') IS DISTINCT FROM 'object' OR jsonb_typeof(o->'homeroomIds') IS DISTINCT FROM 'array') THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_GRAPH_UNMAPPABLE'; END IF;
    IF EXISTS(SELECT 1 FROM academic157_sources s JOIN academic_delivery_versions v ON v.id=s.delivery_id
        CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') o CROSS JOIN LATERAL jsonb_array_elements(o->'groups') g
        WHERE jsonb_typeof(g) IS DISTINCT FROM 'object' OR g->>'id' IS NULL OR NOT pg_input_is_valid(g->>'id','uuid')
          OR jsonb_typeof(g->'teachers') IS DISTINCT FROM 'array' OR jsonb_typeof(g->'homeroomIds') IS DISTINCT FROM 'array'
          OR jsonb_typeof(g->'preferredRoomIds') IS DISTINCT FROM 'array') THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_GRAPH_UNMAPPABLE'; END IF;
END $$;

CREATE TEMP TABLE academic157_offerings ON COMMIT DROP AS
SELECT s.academic_term_id,s.academic_year_id,o.graph,(o.graph->>'id')::uuid id
FROM academic157_sources s JOIN academic_delivery_versions v ON v.id=s.delivery_id
CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') o(graph);
CREATE TEMP TABLE academic157_groups ON COMMIT DROP AS
SELECT o.academic_term_id,o.academic_year_id,o.id offering_id,g.graph,(g.graph->>'id')::uuid id
FROM academic157_offerings o CROSS JOIN LATERAL jsonb_array_elements(o.graph->'groups') g(graph);
CREATE TEMP TABLE academic157_teachers ON COMMIT DROP AS
SELECT g.academic_term_id,g.academic_year_id,g.id group_id,(t.graph->>'assignmentId')::uuid id,
    (t.graph->>'teacherId')::uuid teacher_id,t.graph->>'role' role
FROM academic157_groups g CROSS JOIN LATERAL jsonb_array_elements(g.graph->'teachers') t(graph);

DO $$ BEGIN
    IF EXISTS(SELECT id FROM academic157_offerings GROUP BY id HAVING count(*)<>1)
        OR EXISTS(SELECT id FROM academic157_groups GROUP BY id HAVING count(*)<>1)
        OR EXISTS(SELECT id FROM academic157_teachers GROUP BY id HAVING count(*)<>1)
        OR EXISTS(SELECT group_id,teacher_id FROM academic157_teachers GROUP BY group_id,teacher_id HAVING count(*)<>1)
        OR EXISTS(SELECT 1 FROM academic157_offerings s LEFT JOIN learning_offerings o ON o.id=s.id
            WHERE o.id IS NULL OR (o.academic_term_id,o.academic_year_id,o.kind) IS DISTINCT FROM
                (s.academic_term_id,s.academic_year_id,s.graph->>'kind') OR (s.graph->>'weeklyPeriodTarget')::int<0
                OR NOT(s.graph ?& ARRAY['id','kind','code','name','catalog','weeklyPeriodTarget','targets','homeroomIds','groups']))
        OR EXISTS(SELECT 1 FROM academic157_groups s LEFT JOIN learning_groups g ON g.id=s.id
            WHERE g.id IS NULL OR (g.academic_term_id,g.academic_year_id,g.learning_offering_id) IS DISTINCT FROM
                (s.academic_term_id,s.academic_year_id,s.offering_id))
        OR EXISTS(SELECT 1 FROM academic157_teachers s LEFT JOIN learning_group_teachers t ON t.id=s.id
            WHERE t.id IS NULL OR (t.learning_group_id,t.teacher_id,t.academic_term_id,t.academic_year_id,t.role)
                IS DISTINCT FROM (s.group_id,s.teacher_id,s.academic_term_id,s.academic_year_id,s.role)) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_GRAPH_UNMAPPABLE'; END IF;
    IF EXISTS(SELECT 1 FROM academic_timetable_blocks b JOIN academic157_sources s ON s.timetable_id=b.timetable_version_id
        LEFT JOIN academic157_offerings o ON o.id=b.learning_offering_id
        WHERE b.is_active AND b.learning_offering_id IS NOT NULL AND o.id IS NULL)
       OR EXISTS(SELECT 1 FROM academic_timetable_block_groups bg JOIN academic_timetable_blocks b ON b.id=bg.block_id
        JOIN academic157_sources s ON s.timetable_id=b.timetable_version_id LEFT JOIN academic157_groups g ON g.id=bg.learning_group_id
        WHERE b.is_active AND bg.is_active AND (g.id IS NULL OR g.offering_id<>bg.learning_offering_id
          OR NOT(ARRAY(SELECT value::uuid FROM jsonb_array_elements_text(g.graph->'homeroomIds')) @> bg.homeroom_ids)))
       OR EXISTS(SELECT 1 FROM academic_timetable_block_group_instructors i JOIN academic_timetable_block_groups bg ON bg.id=i.block_group_id
        JOIN academic_timetable_blocks b ON b.id=bg.block_id JOIN academic157_sources s ON s.timetable_id=b.timetable_version_id
        WHERE b.is_active AND bg.is_active AND NOT EXISTS(SELECT 1 FROM academic157_teachers t WHERE t.group_id=bg.learning_group_id AND t.teacher_id=i.instructor_id AND t.role=i.role)) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_TIMETABLE_DELIVERY_MISMATCH'; END IF;
END $$;

-- Preserve accepted session identities while adopting the current placement
-- key only when its old occurrence has one exact retained placement. Dates,
-- times, rosters, record IDs and notification/source ledgers never change.
CREATE TEMP TABLE academic157_session_keys ON COMMIT DROP AS
SELECT session.id,session.source_key FROM attendance_sessions session
JOIN academic157_offerings offering ON offering.id=session.offering_id;
CREATE TEMP TABLE academic157_session_key_candidates ON COMMIT DROP AS
SELECT session.id,'placement:'||placed.id::text source_key
FROM attendance_sessions session JOIN academic157_sources selected ON selected.academic_term_id=session.academic_term_id
JOIN academic_timetable_blocks block ON block.timetable_version_id=selected.timetable_id AND block.is_active
JOIN academic_timetable_block_groups placed ON placed.block_id=block.id AND placed.is_active
    AND placed.learning_group_id=session.learning_group_id AND placed.learning_offering_id=session.offering_id
WHERE session.kind='lesson' AND session.source_key=session.learning_group_id::text||':'||block.bell_schedule_period_id::text
    AND block.day_of_week=(ARRAY['MON','TUE','WED','THU','FRI','SAT','SUN'])[extract(isodow FROM session.date)::int];
CREATE TEMP TABLE academic157_session_key_map ON COMMIT DROP AS
SELECT id,min(source_key) source_key FROM academic157_session_key_candidates GROUP BY id HAVING count(*)=1;
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic157_session_key_map mapping JOIN attendance_sessions own ON own.id=mapping.id
        JOIN attendance_sessions other ON other.academic_term_id=own.academic_term_id AND other.date=own.date
            AND other.kind=own.kind AND other.source_key=mapping.source_key AND other.id<>own.id)
       OR EXISTS(SELECT 1 FROM academic157_session_key_map mapping JOIN attendance_sessions own ON own.id=mapping.id
            GROUP BY own.academic_term_id,own.date,own.kind,mapping.source_key HAVING count(*)>1) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_ATTENDANCE_SOURCE_AMBIGUOUS'; END IF;
END $$;
CREATE TEMP TABLE academic157_supervision ON COMMIT DROP AS
SELECT id,lesson_snapshot,timetable_block_group_id,observed_at FROM supervision_observations;

CREATE TEMP TABLE academic157_removed_lessons ON COMMIT DROP AS
SELECT session.id,session.academic_term_id,session.date,session.roster FROM attendance_sessions session
WHERE session.kind='lesson' AND (
    EXISTS(SELECT 1 FROM learning_offerings o WHERE o.id=session.offering_id AND NOT EXISTS(SELECT 1 FROM academic157_offerings kept WHERE kept.id=o.id))
    OR EXISTS(SELECT 1 FROM learning_groups g WHERE g.id=session.learning_group_id AND NOT EXISTS(SELECT 1 FROM academic157_groups kept WHERE kept.id=g.id)));
-- These are the producer's exact component-delimited identities. A digest,
-- arrival or special notice never enters this selected source set.
-- Persisted session evidence, never today's group membership, bounds the
-- learners whose day totals can contain a deleted lesson contribution.
CREATE TEMP TABLE academic157_removed_lesson_students ON COMMIT DROP AS
SELECT lesson.id,lesson.academic_term_id,lesson.date,record.student_id
FROM academic157_removed_lessons lesson JOIN attendance_records record ON record.session_id=lesson.id
UNION
SELECT lesson.id,lesson.academic_term_id,lesson.date,(student->>'studentId')::uuid
FROM academic157_removed_lessons lesson CROSS JOIN LATERAL jsonb_array_elements(lesson.roster) student
WHERE pg_input_is_valid(student->>'studentId','uuid')
UNION
SELECT lesson.id,lesson.academic_term_id,lesson.date,effect.student_id
FROM academic157_removed_lessons lesson JOIN behavior_source_effect_snapshots effect
    ON effect.academic_term_id=lesson.academic_term_id AND effect.date=lesson.date AND effect.kind='lesson' AND effect.source_key=lesson.id::text;
CREATE TEMP TABLE academic157_removed_notices ON COMMIT DROP AS
SELECT notice.id,notice.notification_id,notice.recipient_id,notice.student_id,notice.academic_term_id,
    string_to_array(notice.dedup_key,':') components
FROM attendance_notifications notice JOIN academic157_removed_lessons lesson
    ON split_part(notice.dedup_key,':',2)=lesson.id::text
WHERE split_part(notice.dedup_key,':',1) IN('session','correction');
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic157_removed_notices notice
        JOIN academic157_removed_lessons lesson ON notice.components[2]=lesson.id::text
        WHERE notice.academic_term_id<>lesson.academic_term_id OR notice.student_id IS NULL
            OR notice.components[3] IS DISTINCT FROM notice.student_id::text
            OR NOT((notice.components[1]='session' AND cardinality(notice.components)=3)
                OR (notice.components[1]='correction' AND cardinality(notice.components)=4
                    AND CASE WHEN pg_input_is_valid(notice.components[4],'bigint') THEN notice.components[4]::bigint>0 ELSE false END)))
       OR EXISTS(SELECT 1 FROM notification_receipts receipt JOIN academic157_removed_notices notice ON receipt.source_id=notice.id
            WHERE receipt.source_kind='attendance' AND (receipt.notification_id<>notice.notification_id OR receipt.recipient_id<>notice.recipient_id)) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_ATTENDANCE_NOTICE_UNMAPPABLE'; END IF;
    -- The existing day provider deliberately protects closed cycles and
    -- suppressions. Refuse those exact selected effects instead of claiming
    -- that ordinary queued reconciliation would reverse them.
    IF EXISTS(SELECT 1 FROM behavior_entries entry JOIN behavior_daily_totals total ON total.id=entry.total_id
        JOIN academic157_removed_lessons lesson ON (entry.source_session_id=lesson.id OR entry.source_key=lesson.id::text)
        CROSS JOIN attendance_module_state module
        LEFT JOIN behavior_cycles current_cycle ON current_cycle.side=total.side AND current_cycle.state='current'
        WHERE entry.cancelled_at IS NULL AND (total.source<>'lesson' OR total.academic_term_id<>lesson.academic_term_id
            OR total.date<>lesson.date OR entry.source_key<>lesson.id::text OR (entry.source_session_id IS NOT NULL AND entry.source_session_id<>lesson.id) OR entry.source_generation<>module.generation
            OR current_cycle.id IS NULL OR total.cycle_id<>current_cycle.id OR lesson.date<current_cycle.starts_on
            OR EXISTS(SELECT 1 FROM behavior_suppressions suppression WHERE suppression.side=total.side AND suppression.source='lesson'
                AND (suppression.start_date IS NULL OR lesson.date BETWEEN suppression.start_date AND suppression.end_date)
                AND (suppression.academic_term_id IS NULL OR suppression.academic_term_id=lesson.academic_term_id)
                AND (suppression.academic_year_id IS NULL OR suppression.academic_year_id=(SELECT academic_year_id FROM academic_terms WHERE id=lesson.academic_term_id))))) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_BEHAVIOR_EFFECT_PROTECTED'; END IF;
    -- details_only cleanup intentionally removes entries while retaining
    -- totals and suppression evidence. The day provider cannot subtract a
    -- missing contribution; refuse such exact affected learner/day/side
    -- totals independently of Attendance archival state.
    IF EXISTS(SELECT 1 FROM behavior_daily_totals total
        JOIN academic157_removed_lesson_students learner ON learner.student_id=total.student_id
            AND learner.academic_term_id=total.academic_term_id AND learner.date=total.date
        LEFT JOIN behavior_cycles current_cycle ON current_cycle.side=total.side AND current_cycle.state='current'
        WHERE total.source='lesson' AND total.points>0 AND
            (total.details_purged OR total.points<>(SELECT COALESCE(sum(entry.points),0) FROM behavior_entries entry
                WHERE entry.total_id=total.id AND entry.cancelled_at IS NULL)
            OR current_cycle.id IS NULL OR total.cycle_id<>current_cycle.id OR total.date<current_cycle.starts_on
            OR EXISTS(SELECT 1 FROM behavior_suppressions suppression WHERE suppression.side=total.side AND suppression.source='lesson'
                AND (suppression.start_date IS NULL OR total.date BETWEEN suppression.start_date AND suppression.end_date)
                AND (suppression.academic_term_id IS NULL OR suppression.academic_term_id=total.academic_term_id)
                AND (suppression.academic_year_id IS NULL OR suppression.academic_year_id=(SELECT academic_year_id FROM academic_terms WHERE id=total.academic_term_id)))))
       OR EXISTS(SELECT 1 FROM behavior_source_effect_snapshots effect JOIN academic157_removed_lessons lesson
            ON effect.academic_term_id=lesson.academic_term_id AND effect.date=lesson.date AND effect.kind='lesson' AND effect.source_key=lesson.id::text
            WHERE effect.suppressed AND effect.action IN('merit','deduction') AND effect.raw_points>0) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_BEHAVIOR_EFFECT_PROTECTED'; END IF;
    -- Archived offering totals do not identify individual lesson sessions.
    -- Existing derived totals cannot be allocated by today's roster or used
    -- to enqueue a whole archived day, which would also erase other sources.
    IF EXISTS(SELECT 1 FROM attendance_term_summaries summary JOIN learning_offerings offering ON summary.scope_key=offering.id::text
        WHERE summary.category='lesson' AND summary.expected>0 AND NOT EXISTS(SELECT 1 FROM academic157_offerings kept WHERE kept.id=offering.id)
            AND EXISTS(SELECT 1 FROM behavior_daily_totals total WHERE total.academic_term_id=offering.academic_term_id
                AND total.source='lesson' AND total.points>0)) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_BEHAVIOR_ARCHIVE_UNMAPPABLE'; END IF;
END $$;
CREATE TEMP TABLE academic157_cancelled_receipts ON COMMIT DROP AS
SELECT receipt.id FROM notification_receipts receipt JOIN academic157_removed_notices notice
    ON receipt.source_kind='attendance' AND receipt.source_id=notice.id
    AND receipt.notification_id=notice.notification_id AND receipt.recipient_id=notice.recipient_id;
CREATE TEMP TABLE academic157_original_receipts ON COMMIT DROP AS SELECT id,to_jsonb(receipt) fact FROM notification_receipts receipt;
CREATE TEMP TABLE academic157_unrelated_presentations ON COMMIT DROP AS
SELECT receipt_id,to_jsonb(presentation) fact FROM notification_presentations presentation
WHERE NOT EXISTS(SELECT 1 FROM academic157_cancelled_receipts selected WHERE selected.id=presentation.receipt_id);

-- Preserve explicit ownership, including saved group-less sessions whose
-- offering is retained. Omitted groups are authorized deletion scope even
-- when another group in their offering remains current.
CREATE TEMP TABLE academic157_retained_sessions ON COMMIT DROP AS
SELECT session.id FROM attendance_sessions session JOIN academic157_offerings offering ON offering.id=session.offering_id
WHERE session.learning_group_id IS NULL OR EXISTS(SELECT 1 FROM academic157_groups kept WHERE kept.id=session.learning_group_id);
CREATE TEMP TABLE academic157_discarded_blocks ON COMMIT DROP AS
SELECT b.id FROM academic_timetable_blocks b
WHERE NOT b.is_active OR NOT EXISTS(SELECT 1 FROM academic157_sources s WHERE s.timetable_id=b.timetable_version_id);
CREATE TEMP TABLE academic157_discarded_groups ON COMMIT DROP AS
SELECT bg.id FROM academic_timetable_block_groups bg
WHERE NOT bg.is_active OR EXISTS(SELECT 1 FROM academic157_discarded_blocks b WHERE b.id=bg.block_id);
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic_timetable_block_group_sync sync
        JOIN academic157_discarded_groups discarded ON discarded.id=sync.linked_block_group_id
        WHERE NOT EXISTS(SELECT 1 FROM academic157_discarded_blocks b WHERE b.id=sync.block_id)
          AND sync.status='LINKED') THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_INACTIVE_SYNC_UNMAPPABLE';
    END IF;
END $$;
-- Capture only retained facts. The temporary JSON never survives this transaction.
CREATE TEMP TABLE academic157_preserved ON COMMIT DROP AS
SELECT 'memberships'::text resource,count(*) row_count,encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex') fingerprint
FROM learning_group_students r JOIN academic157_groups g ON g.id=r.learning_group_id
UNION ALL SELECT 'attendance',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r)-'source_key' ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM attendance_sessions r JOIN academic157_retained_sessions kept ON kept.id=r.id
UNION ALL SELECT 'scores',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM learning_group_student_scores r JOIN academic157_groups g ON g.id=r.learning_group_id
UNION ALL SELECT 'course_results',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_course_results r JOIN academic157_groups g ON g.id=r.learning_group_id
UNION ALL SELECT 'activity_results',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_activity_results r JOIN academic157_groups g ON g.id=r.learning_group_id
UNION ALL SELECT 'attendance_records',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.session_id,r.student_id),'[]')::text,'UTF8')),'hex')
FROM attendance_records r JOIN academic157_retained_sessions kept ON kept.id=r.session_id
UNION ALL SELECT 'supervision',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r)-'updated_at'-'timetable_block_group_id'-'learning_group_id'-'lesson_snapshot' ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM supervision_observations r
UNION ALL SELECT 'blocks',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r)-'timetable_version_id'-'timetable_id' ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_blocks r JOIN academic157_sources selected ON selected.timetable_id=r.timetable_version_id WHERE r.is_active
UNION ALL SELECT 'placed_groups',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_groups r JOIN academic_timetable_blocks b ON b.id=r.block_id
JOIN academic157_sources selected ON selected.timetable_id=b.timetable_version_id WHERE b.is_active AND r.is_active
UNION ALL SELECT 'structural_homerooms',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_homerooms r JOIN academic_timetable_blocks b ON b.id=r.block_id
JOIN academic157_sources selected ON selected.timetable_id=b.timetable_version_id WHERE b.is_active AND r.is_active
UNION ALL SELECT 'structural_teachers',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_teachers r JOIN academic_timetable_blocks b ON b.id=r.block_id
JOIN academic157_sources selected ON selected.timetable_id=b.timetable_version_id WHERE b.is_active AND r.is_active
UNION ALL SELECT 'placement_sync',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_group_sync r JOIN academic_timetable_blocks b ON b.id=r.block_id
JOIN academic157_sources selected ON selected.timetable_id=b.timetable_version_id WHERE b.is_active AND EXISTS(SELECT 1 FROM academic157_groups kept WHERE kept.id=r.learning_group_id)
UNION ALL SELECT 'placed_instructors',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_group_instructors r JOIN academic_timetable_block_groups bg ON bg.id=r.block_group_id
JOIN academic_timetable_blocks b ON b.id=bg.block_id JOIN academic157_sources selected ON selected.timetable_id=b.timetable_version_id
WHERE b.is_active AND bg.is_active
UNION ALL SELECT 'placed_exam_items',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(i) ORDER BY i.id),'[]')::text,'UTF8')),'hex')
FROM academic_exam_schedule_items i JOIN academic157_groups kept ON kept.id=i.learning_group_id
WHERE EXISTS(SELECT 1 FROM academic_exam_sessions session WHERE session.exam_schedule_item_id=i.id)
UNION ALL SELECT 'placed_exam_sessions',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(session) ORDER BY session.id),'[]')::text,'UTF8')),'hex')
FROM academic_exam_sessions session JOIN academic_exam_schedule_items item ON item.id=session.exam_schedule_item_id
JOIN academic157_groups kept ON kept.id=item.learning_group_id
UNION ALL SELECT 'exam_rounds',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(round)-'status'-'published_at'-'published_by' ORDER BY round.id),'[]')::text,'UTF8')),'hex') FROM academic_exam_rounds round;

CREATE TEMP TABLE academic157_original_coverage ON COMMIT DROP AS SELECT * FROM learning_group_homerooms;
CREATE TEMP TABLE academic157_original_preferences ON COMMIT DROP AS SELECT * FROM learning_group_preferred_rooms;

-- Match Notifications' irreversible consume-before-presentation protocol while
-- retaining receipt, inbox and Attendance dedup identities. A prefetched worker
-- must re-lock this terminal receipt and can never reopen its presentation.
UPDATE notification_receipts receipt SET presentation_consumed_at=clock_timestamp(),presentation_outcome='skipped'
FROM academic157_cancelled_receipts selected WHERE selected.id=receipt.id AND receipt.presentation_consumed_at IS NULL;
DELETE FROM notification_presentations presentation USING academic157_cancelled_receipts selected WHERE selected.id=presentation.receipt_id;
-- Ordinary dirty-day delivery preserves current arrival/special/manual facts.
-- It never changes a pre-existing reset intent or invents a new generation.
INSERT INTO attendance_score_changes(generation,academic_term_id,date,reset)
SELECT module.generation,lesson.academic_term_id,lesson.date,false FROM attendance_module_state module
CROSS JOIN (SELECT DISTINCT academic_term_id,date FROM academic157_removed_lessons) lesson
ON CONFLICT(generation,academic_term_id,date) DO UPDATE SET row_version=nextval('attendance_score_revision_seq'),queued_at=now();

-- Remove only the retired mutability mechanisms, explicitly by owned name.
DROP TRIGGER learning_offerings_published_immutable ON learning_offerings;
DROP TRIGGER course_offering_details_published_immutable ON course_offering_details;
DROP TRIGGER activity_offering_details_published_immutable ON activity_offering_details;
DROP TRIGGER learning_offering_targets_published_immutable ON learning_offering_targets;
DROP TRIGGER learning_group_teachers_published_immutable ON learning_group_teachers;
DROP TRIGGER learning_group_teachers_interval_guard ON learning_group_teachers;
DROP TRIGGER academic_delivery_teacher_episodes_accepted ON learning_group_teachers;
DO $$ DECLARE relation_name text; BEGIN
    FOREACH relation_name IN ARRAY ARRAY['academic_timetable_blocks','academic_timetable_block_groups','academic_timetable_block_group_instructors',
        'academic_timetable_block_homerooms','academic_timetable_block_teachers','academic_timetable_block_group_sync'] LOOP
        EXECUTE format('DROP TRIGGER %I ON %I',relation_name||'_version_immutable',relation_name);
        EXECUTE format('DROP TRIGGER %I ON %I',relation_name||'_content_revision',relation_name);
    END LOOP;
END $$;

-- A saved supervision observation already owns a lesson snapshot. Detachment
-- preserves that fact without pretending that it was a manual observation.
ALTER TABLE supervision_observations DROP CONSTRAINT supervision_observations_lesson_source_check;
ALTER TABLE supervision_observations ADD CONSTRAINT supervision_observations_lesson_source_check CHECK(
    timetable_block_group_id IS NOT NULL OR
    (manual_subject_name IS NOT NULL AND manual_classroom_label IS NOT NULL AND manual_period_label IS NOT NULL AND manual_reason IS NOT NULL)
    OR (jsonb_typeof(lesson_snapshot)='object' AND lesson_snapshot<>'{}'::jsonb));
UPDATE supervision_observations obs SET timetable_block_group_id=NULL,
    lesson_snapshot=CASE WHEN obs.lesson_snapshot='{}'::jsonb THEN jsonb_build_object(
        'source','removed_timetable','timetableBlockGroupId',obs.timetable_block_group_id,'observedAt',obs.observed_at)
        ELSE obs.lesson_snapshot END
WHERE obs.timetable_block_group_id IN(SELECT bg.id FROM academic_timetable_block_groups bg
    JOIN academic157_discarded_groups discarded ON discarded.id=bg.id);

-- Removed operational placements have no restore stage. Actual Attendance is
-- independent; only exact Supervision source references above are detached.
DELETE FROM academic_timetable_block_group_sync sync
WHERE EXISTS(SELECT 1 FROM academic157_discarded_blocks b WHERE b.id=sync.block_id)
   OR NOT EXISTS(SELECT 1 FROM academic157_groups kept WHERE kept.id=sync.learning_group_id);
DELETE FROM academic_timetable_block_groups bg USING academic157_discarded_groups discarded WHERE discarded.id=bg.id;
DELETE FROM academic_timetable_block_homerooms WHERE NOT is_active;
DELETE FROM academic_timetable_block_teachers WHERE NOT is_active;
DELETE FROM academic_timetable_blocks b USING academic157_discarded_blocks discarded WHERE discarded.id=b.id;

-- Unscheduled items are live projections. Only an actual saved placement owns
-- a persisted item identity; retire import-stage copies without deleting facts.
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM pg_constraint WHERE contype='f' AND confrelid='academic_exam_schedule_items'::regclass
   AND conrelid<>'academic_exam_sessions'::regclass) THEN
  RAISE EXCEPTION 'ACADEMIC_CURRENT_EXAM_ITEM_DEPENDENT_UNMAPPABLE';
 END IF;
END $$;
DELETE FROM academic_exam_schedule_items item WHERE NOT EXISTS(SELECT 1 FROM academic_exam_sessions session WHERE session.exam_schedule_item_id=item.id);

CREATE TEMP TABLE academic157_registration ON COMMIT DROP AS
SELECT g.id,(o.status='published' AND g.status='published' AND g.roster_status='draft' AND a.registration_type='self') registration_open
FROM learning_groups g JOIN learning_offerings o ON o.id=g.learning_offering_id
LEFT JOIN activity_offering_details a ON a.learning_offering_id=o.id;

-- Private graph/command tables are removed only after source materialization.
DROP VIEW academic_exam_eligible_sources;
DROP VIEW academic_delivery_latest_published_teacher_allocations;
DROP VIEW academic_delivery_effective_offering_spans;
DROP VIEW academic_delivery_effective_teacher_episodes;
ALTER TABLE learning_offerings DROP CONSTRAINT learning_offerings_stop_change_set_context_fkey;
ALTER TABLE learning_group_teachers DROP CONSTRAINT learning_group_teachers_started_change_set_fkey,
    DROP CONSTRAINT learning_group_teachers_ended_change_set_fkey;
DROP TABLE academic_delivery_revision_audience_rooms;
DROP TABLE academic_delivery_revision_audiences;
DROP TABLE academic_delivery_teacher_cutoffs;
DROP TABLE academic_delivery_offering_effectivity;
DROP TABLE academic_teacher_handoff_runs;
DROP TABLE academic_term_change_items;
DROP TABLE academic_term_change_sets;

-- Fixed migration-only projection of File Platform's transaction-bound deletion
-- recipe. Provider I/O remains with its existing durable worker after commit.
CREATE FUNCTION pg_temp.academic157_remove_lesson_files(selected_offering uuid,selected_group uuid DEFAULT NULL)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE selected_file uuid;
BEGIN
    FOR selected_file IN SELECT DISTINCT scan.evidence_file_id FROM attendance_scan_events scan
        JOIN attendance_sessions session ON session.id=scan.session_id
        WHERE session.kind='lesson' AND session.offering_id=selected_offering
          AND (selected_group IS NULL OR session.learning_group_id=selected_group) ORDER BY scan.evidence_file_id LOOP
        PERFORM 1 FROM files WHERE id=selected_file AND purpose_code='attendance_evidence' AND visibility='private' FOR UPDATE;
        IF NOT FOUND OR EXISTS(SELECT 1 FROM file_storage_cleanup_watches WHERE file_id=selected_file AND namespace_fingerprint IS NULL)
            OR EXISTS(SELECT 1 FROM file_versions v WHERE v.file_id=selected_file AND v.storage_status<>'deleted'
                AND NOT EXISTS(SELECT 1 FROM file_storage_cleanup_watches w WHERE w.target_kind='version' AND w.target_id=v.id
                    AND w.file_id=v.file_id AND w.provider_code=v.provider_code AND w.storage_class=v.storage_class
                    AND w.object_key=v.object_key AND w.content_type=v.detected_mime_type AND w.namespace_fingerprint IS NOT NULL))
            OR EXISTS(SELECT 1 FROM file_derivatives d WHERE d.file_id=selected_file AND d.storage_status<>'deleted'
                AND NOT EXISTS(SELECT 1 FROM file_storage_cleanup_watches w WHERE w.target_kind='derivative' AND w.target_id=d.id
                    AND w.file_id=d.file_id AND w.provider_code=d.provider_code AND w.storage_class=d.storage_class
                    AND w.object_key=d.object_key AND w.content_type=d.detected_mime_type AND w.namespace_fingerprint IS NOT NULL)) THEN
            RAISE EXCEPTION 'ACADEMIC_CURRENT_ATTENDANCE_EVIDENCE_UNMAPPABLE'; END IF;
        UPDATE files SET lifecycle_status='delete_requested',delete_requested_at=COALESCE(delete_requested_at,now()),updated_at=now()
            WHERE id=selected_file AND lifecycle_status<>'deleted';
        UPDATE file_operations SET status='cancelled',last_error_code='file_delete_requested',completed_at=now(),
            lease_owner=NULL,leased_at=NULL,lease_expires_at=NULL
            WHERE file_id=selected_file AND operation_type IN('reconcile','generate_derivative') AND status IN('pending','leased','retryable_failure');
        UPDATE file_versions SET storage_status='delete_requested' WHERE file_id=selected_file AND storage_status<>'deleted';
        UPDATE file_derivatives SET storage_status='delete_requested' WHERE file_id=selected_file AND storage_status<>'deleted';
        INSERT INTO file_operations(file_id,file_version_id,operation_type,status,next_retry_at)
        SELECT v.file_id,v.id,'delete_object','pending',now() FROM file_versions v WHERE v.file_id=selected_file AND v.storage_status<>'deleted'
            AND NOT EXISTS(SELECT 1 FROM file_operations o WHERE o.file_version_id=v.id AND o.operation_type='delete_object'
                AND o.status IN('pending','leased','retryable_failure','succeeded'));
        INSERT INTO file_operations(file_id,file_derivative_id,operation_type,status,next_retry_at)
        SELECT d.file_id,d.id,'delete_object','pending',now() FROM file_derivatives d WHERE d.file_id=selected_file AND d.storage_status<>'deleted'
            AND NOT EXISTS(SELECT 1 FROM file_operations o WHERE o.file_derivative_id=d.id AND o.operation_type='delete_object'
                AND o.status IN('pending','leased','retryable_failure','succeeded'));
        UPDATE files f SET lifecycle_status='deleted',deleted_at=COALESCE(deleted_at,now()),updated_at=now()
            WHERE f.id=selected_file AND f.lifecycle_status='delete_requested'
              AND NOT EXISTS(SELECT 1 FROM file_versions v WHERE v.file_id=f.id AND v.storage_status<>'deleted')
              AND NOT EXISTS(SELECT 1 FROM file_derivatives d WHERE d.file_id=f.id AND d.storage_status<>'deleted');
    END LOOP;
    DELETE FROM attendance_scan_events scan USING attendance_sessions session
    WHERE session.id=scan.session_id AND session.kind='lesson' AND session.offering_id=selected_offering
        AND (selected_group IS NULL OR session.learning_group_id=selected_group);
END $$;

DO $$ DECLARE offering record; relation_name text; omitted_group uuid; has_facts boolean; BEGIN
    FOR offering IN SELECT id FROM learning_offerings o WHERE NOT EXISTS(SELECT 1 FROM academic157_offerings s WHERE s.id=o.id) ORDER BY id LOOP
        PERFORM academic_remove_offering_results(offering.id);
        PERFORM pg_temp.academic157_remove_lesson_files(offering.id);
        DELETE FROM attendance_records r USING attendance_sessions s WHERE s.id=r.session_id AND s.kind='lesson' AND s.offering_id=offering.id;
        DELETE FROM attendance_sessions WHERE kind='lesson' AND offering_id=offering.id;
        DELETE FROM attendance_term_summaries WHERE category='lesson' AND scope_key=offering.id::text;
        DELETE FROM attendance_term_teacher_summaries WHERE category='lesson' AND scope_key=offering.id::text;
        DELETE FROM academic_exam_schedule_items WHERE learning_offering_id=offering.id;
        FOREACH relation_name IN ARRAY ARRAY['learning_group_phase_confirmations','learning_group_result_confirmations',
            'learning_group_evaluation_confirmations','academic_activity_result_confirmations','learning_group_student_scores',
            'learning_group_student_evaluations','learning_group_result_overrides','academic_activity_evaluations','learning_group_score_items'] LOOP
            EXECUTE format('DELETE FROM %I WHERE learning_offering_id=$1',relation_name) USING offering.id;
        END LOOP;
        DELETE FROM course_assessment_plans WHERE learning_offering_id=offering.id;
        UPDATE supervision_observations SET learning_group_id=NULL WHERE learning_group_id IN(SELECT id FROM learning_groups WHERE learning_offering_id=offering.id);
        DELETE FROM learning_groups WHERE learning_offering_id=offering.id;
        DELETE FROM learning_offerings WHERE id=offering.id;
    END LOOP;
    FOR omitted_group IN SELECT g.id FROM learning_groups g WHERE NOT EXISTS(SELECT 1 FROM academic157_groups s WHERE s.id=g.id) ORDER BY g.id LOOP
        IF EXISTS(SELECT 1 FROM attendance_term_summaries a JOIN learning_groups g ON a.scope_key=g.learning_offering_id::text
            WHERE g.id=omitted_group AND a.category='lesson') OR EXISTS(SELECT 1 FROM attendance_term_teacher_summaries a
            JOIN learning_groups g ON a.scope_key=g.learning_offering_id::text WHERE g.id=omitted_group AND a.category='lesson') THEN
            RAISE EXCEPTION 'ACADEMIC_CURRENT_GROUP_ARCHIVE_UNMAPPABLE'; END IF;
        PERFORM pg_temp.academic157_remove_lesson_files((SELECT learning_offering_id FROM learning_groups WHERE id=omitted_group),omitted_group);
        PERFORM academic_remove_group_results(omitted_group);
        DELETE FROM attendance_records r USING attendance_sessions s WHERE s.id=r.session_id AND s.kind='lesson' AND s.learning_group_id=omitted_group;
        DELETE FROM attendance_sessions WHERE kind='lesson' AND learning_group_id=omitted_group;
        DELETE FROM academic_exam_schedule_items WHERE learning_group_id=omitted_group;
        FOREACH relation_name IN ARRAY ARRAY['learning_group_phase_confirmations','learning_group_result_confirmations',
            'learning_group_evaluation_confirmations','academic_activity_result_confirmations','learning_group_student_scores',
            'learning_group_student_evaluations','learning_group_result_overrides','academic_activity_evaluations','learning_group_score_items'] LOOP
            EXECUTE format('DELETE FROM %I WHERE learning_group_id=$1',relation_name) USING omitted_group;
        END LOOP;
        UPDATE supervision_observations SET learning_group_id=NULL WHERE learning_group_id=omitted_group;
        DELETE FROM learning_groups WHERE id=omitted_group;
    END LOOP;
END $$;
DROP FUNCTION pg_temp.academic157_remove_lesson_files(uuid,uuid);

ALTER TABLE learning_offerings ADD COLUMN weekly_period_target integer;
UPDATE learning_offerings o SET weekly_period_target=(s.graph->>'weeklyPeriodTarget')::int,
    code_snapshot=s.graph->>'code',name_snapshot=s.graph->>'name',
    owning_organization_unit_id=(s.graph->>'owningOrganizationUnitId')::uuid,
    source_requirement_kind=s.graph->>'sourceRequirementKind',source_requirement_id=(s.graph->>'sourceRequirementId')::uuid
FROM academic157_offerings s WHERE s.id=o.id;
ALTER TABLE learning_offerings ALTER COLUMN weekly_period_target SET NOT NULL,
    ADD CONSTRAINT learning_offerings_weekly_period_target_check CHECK(weekly_period_target>=0);

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic157_offerings s JOIN course_offering_details d ON d.learning_offering_id=s.id
        WHERE (d.subject_version_id,d.subject_id) IS DISTINCT FROM
            ((s.graph->'catalog'->>'subjectVersionId')::uuid,(s.graph->'catalog'->>'subjectId')::uuid))
    OR EXISTS(SELECT 1 FROM academic157_offerings s JOIN activity_offering_details d ON d.learning_offering_id=s.id
        WHERE (d.activity_version_id,d.activity_id) IS DISTINCT FROM
            ((s.graph->'catalog'->>'activityVersionId')::uuid,(s.graph->'catalog'->>'activityId')::uuid)) THEN
        RAISE EXCEPTION 'ACADEMIC_CURRENT_CATALOG_PIN_MISMATCH'; END IF;
END $$;
UPDATE course_offering_details d SET credit=(s.graph->'catalog'->>'credit')::numeric,
    hours=(s.graph->'catalog'->>'hours')::numeric,assessment_total_score=(s.graph->'catalog'->>'assessmentTotalScore')::numeric
FROM academic157_offerings s WHERE s.id=d.learning_offering_id;
UPDATE activity_offering_details d SET registration_type=s.graph->'catalog'->>'registrationType',
    scheduling_mode=s.graph->'catalog'->>'schedulingMode',hours=(s.graph->'catalog'->>'hours')::numeric,
    capacity=(s.graph->'catalog'->>'capacity')::int,attendance_requirement=s.graph->'catalog'->'attendanceRequirement',pass_criteria=s.graph->'catalog'->'passCriteria'
FROM academic157_offerings s WHERE s.id=d.learning_offering_id;

DELETE FROM learning_offering_targets;
INSERT INTO learning_offering_targets(id,learning_offering_id,academic_term_id,academic_year_id,target_kind,homeroom_id,grade_level_id,study_program_id)
SELECT (target->>'id')::uuid,s.id,s.academic_term_id,s.academic_year_id,target->>'targetKind',
    (target->>'homeroomId')::uuid,(target->>'gradeLevelId')::uuid,(target->>'studyProgramId')::uuid
FROM academic157_offerings s CROSS JOIN LATERAL jsonb_array_elements(s.graph->'targets') target;
UPDATE learning_groups g SET code=s.graph->>'code',name=s.graph->>'name',description=s.graph->>'description',capacity=(s.graph->>'capacity')::int
FROM academic157_groups s WHERE s.id=g.id;
DELETE FROM learning_group_homerooms;
INSERT INTO learning_group_homerooms(id,learning_group_id,academic_term_id,academic_year_id,homeroom_id,coverage_source)
SELECT COALESCE((SELECT original.id FROM pg_temp.academic157_original_coverage original WHERE original.learning_group_id=g.id AND original.homeroom_id=room::uuid),
    uuid_generate_v5(g.id,'current-coverage:'||room)),g.id,g.academic_term_id,g.academic_year_id,room::uuid,COALESCE((SELECT original.coverage_source FROM pg_temp.academic157_original_coverage original WHERE original.learning_group_id=g.id AND original.homeroom_id=room::uuid),'current')
FROM academic157_groups g CROSS JOIN LATERAL jsonb_array_elements_text(g.graph->'homeroomIds') room;
DELETE FROM learning_group_preferred_rooms;
INSERT INTO learning_group_preferred_rooms(id,learning_group_id,academic_term_id,academic_year_id,room_id,rank,is_required,migration_provenance)
SELECT COALESCE((SELECT original.id FROM pg_temp.academic157_original_preferences original WHERE original.learning_group_id=g.id AND original.room_id=room::uuid),
    uuid_generate_v5(g.id,'current-room:'||room)),g.id,g.academic_term_id,g.academic_year_id,room::uuid,ordinal::int,COALESCE((SELECT original.is_required FROM pg_temp.academic157_original_preferences original WHERE original.learning_group_id=g.id AND original.room_id=room::uuid),false),
    COALESCE((SELECT original.migration_provenance FROM pg_temp.academic157_original_preferences original WHERE original.learning_group_id=g.id AND original.room_id=room::uuid),'{}')
FROM academic157_groups g CROSS JOIN LATERAL jsonb_array_elements_text(g.graph->'preferredRoomIds') WITH ORDINALITY t(room,ordinal);
DELETE FROM learning_group_teachers t WHERE NOT EXISTS(SELECT 1 FROM academic157_teachers s WHERE s.id=t.id);

ALTER TABLE learning_groups ADD COLUMN registration_open boolean NOT NULL DEFAULT false;
UPDATE learning_groups g SET registration_open=COALESCE(r.registration_open,false) FROM academic157_registration r WHERE r.id=g.id;
ALTER TABLE learning_offerings DROP COLUMN status,DROP COLUMN published_at,DROP COLUMN publish_idempotency_key,
    DROP COLUMN starts_on,DROP COLUMN ends_on,DROP COLUMN stop_reason,DROP COLUMN stopped_at,DROP COLUMN stopped_by,DROP COLUMN stop_change_set_id;
ALTER TABLE learning_groups DROP COLUMN status,DROP COLUMN roster_status,DROP COLUMN roster_published_at,
    DROP COLUMN roster_source_hash,DROP COLUMN roster_publish_idempotency_key;
ALTER TABLE learning_group_teachers DROP COLUMN starts_on,DROP COLUMN ends_on,
    DROP COLUMN started_by_change_set_id,DROP COLUMN ended_by_change_set_id,
    ADD CONSTRAINT learning_group_teachers_current_teacher_key UNIQUE(learning_group_id,teacher_id);

CREATE TABLE academic_delivery_state(
    academic_term_id uuid PRIMARY KEY,
    academic_year_id uuid NOT NULL,
    row_version bigint NOT NULL DEFAULT 1 CHECK(row_version>0),
    FOREIGN KEY(academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT
);
INSERT INTO academic_delivery_state SELECT academic_term_id,academic_year_id,1 FROM academic157_sources;

CREATE TABLE academic_timetables(
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    academic_term_id uuid NOT NULL UNIQUE,
    academic_year_id uuid NOT NULL,
    bell_schedule_id uuid NOT NULL,
    row_version bigint NOT NULL DEFAULT 1 CHECK(row_version>0),
    created_at timestamptz NOT NULL DEFAULT now(),updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(id,academic_term_id,academic_year_id),
    FOREIGN KEY(academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY(academic_term_id,bell_schedule_id) REFERENCES academic_terms(id,bell_schedule_id) ON DELETE RESTRICT
);
INSERT INTO academic_timetables(id,academic_term_id,academic_year_id,bell_schedule_id,row_version,created_at,updated_at)
SELECT v.id,v.academic_term_id,v.academic_year_id,v.bell_schedule_id,v.row_version,v.created_at,v.updated_at
FROM academic_timetable_versions v JOIN academic157_sources s ON s.timetable_id=v.id;
ALTER TABLE academic_timetable_blocks DROP CONSTRAINT academic_timetable_blocks_version_context_fkey;
ALTER TABLE academic_timetable_blocks RENAME COLUMN timetable_version_id TO timetable_id;
ALTER TABLE academic_timetable_blocks ADD CONSTRAINT academic_timetable_blocks_current_context_fkey
    FOREIGN KEY(timetable_id,academic_term_id,academic_year_id) REFERENCES academic_timetables(id,academic_term_id,academic_year_id) ON DELETE RESTRICT;
DROP TABLE academic_timetable_versions;
DROP TABLE academic_delivery_versions;

ALTER TABLE academic_exam_rounds DROP COLUMN status,DROP COLUMN published_at,DROP COLUMN published_by;
UPDATE attendance_sessions session SET source_key=mapping.source_key FROM academic157_session_key_map mapping WHERE mapping.id=session.id;

CREATE FUNCTION academic_touch_delivery_revision() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE term uuid; item jsonb:=CASE WHEN TG_OP='DELETE' THEN to_jsonb(OLD) ELSE to_jsonb(NEW) END;
BEGIN
    IF TG_TABLE_NAME='learning_group_roster_tracking' THEN
        SELECT academic_term_id INTO term FROM learning_groups WHERE id=(item->>'learning_group_id')::uuid;
    ELSIF TG_TABLE_NAME='learning_offering_curriculum_sources' THEN
        SELECT academic_term_id INTO term FROM learning_offerings WHERE id=(item->>'learning_offering_id')::uuid;
    ELSE term:=(item->>'academic_term_id')::uuid; END IF;
    UPDATE academic_delivery_state SET row_version=row_version+1 WHERE academic_term_id=term;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
DO $$ DECLARE relation_name text; BEGIN
    FOREACH relation_name IN ARRAY ARRAY['learning_offerings','course_offering_details','activity_offering_details',
        'learning_offering_targets','learning_groups','learning_group_teachers','learning_group_students',
        'learning_group_homerooms','learning_group_preferred_rooms','learning_group_roster_tracking','learning_offering_curriculum_sources'] LOOP
        EXECUTE format('CREATE TRIGGER %I AFTER INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION academic_touch_delivery_revision()',relation_name||'_current_revision',relation_name);
    END LOOP;
END $$;

CREATE FUNCTION academic_touch_timetable_current_revision() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE current_id uuid; parent_block uuid; child_group uuid;
BEGIN
    IF TG_TABLE_NAME='academic_timetable_blocks' THEN
        current_id:=CASE WHEN TG_OP='DELETE' THEN OLD.timetable_id ELSE NEW.timetable_id END;
    ELSE
        IF TG_TABLE_NAME='academic_timetable_block_group_instructors' THEN
            child_group:=CASE WHEN TG_OP='DELETE' THEN OLD.block_group_id ELSE NEW.block_group_id END;
            SELECT block_id INTO parent_block FROM academic_timetable_block_groups WHERE id=child_group;
        ELSE parent_block:=CASE WHEN TG_OP='DELETE' THEN OLD.block_id ELSE NEW.block_id END; END IF;
        SELECT timetable_id INTO current_id FROM academic_timetable_blocks WHERE id=parent_block;
    END IF;
    UPDATE academic_timetables SET row_version=row_version+1,updated_at=now() WHERE id=current_id;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
DO $$ DECLARE relation_name text; BEGIN
    FOREACH relation_name IN ARRAY ARRAY['academic_timetable_blocks','academic_timetable_block_groups',
        'academic_timetable_block_group_instructors','academic_timetable_block_homerooms',
        'academic_timetable_block_teachers','academic_timetable_block_group_sync'] LOOP
        EXECUTE format('CREATE TRIGGER %I AFTER INSERT OR UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION academic_touch_timetable_current_revision()',relation_name||'_current_revision',relation_name);
    END LOOP;
END $$;

CREATE OR REPLACE FUNCTION assert_timetable_block_conflict_free(target_block_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_block academic_timetable_blocks%ROWTYPE;
    slot_lock_key BIGINT;
BEGIN
    SELECT * INTO target_block
    FROM academic_timetable_blocks
    WHERE id = target_block_id;

    IF target_block.id IS NULL OR NOT target_block.is_active THEN
        RETURN;
    END IF;

    slot_lock_key := hashtextextended(
        target_block.timetable_id::text || ':' || target_block.day_of_week || ':'
            || target_block.bell_schedule_period_id::text,
        0
    );
    PERFORM pg_advisory_xact_lock(slot_lock_key);

    IF EXISTS (
        SELECT 1
        FROM academic_timetable_block_groups candidate
        JOIN academic_timetable_block_groups occupied
          ON occupied.learning_group_id = candidate.learning_group_id
         AND occupied.block_id <> candidate.block_id
         AND occupied.is_active
        JOIN academic_timetable_blocks occupied_block ON occupied_block.id = occupied.block_id
        WHERE candidate.block_id = target_block_id
          AND candidate.is_active
          AND occupied_block.is_active
          AND occupied_block.timetable_id = target_block.timetable_id
          AND occupied_block.day_of_week = target_block.day_of_week
          AND occupied_block.bell_schedule_period_id = target_block.bell_schedule_period_id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_GROUP_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_homerooms AS (
            SELECT homeroom_id
            FROM academic_timetable_block_homerooms
            WHERE block_id = target_block_id AND is_active
            UNION
            SELECT coverage.homeroom_id
            FROM academic_timetable_block_groups block_group
            CROSS JOIN LATERAL unnest(block_group.homeroom_ids) AS coverage(homeroom_id)
            WHERE block_group.block_id = target_block_id AND block_group.is_active
        ), occupied_homerooms AS (
            SELECT homeroom.homeroom_id, homeroom.block_id
            FROM academic_timetable_block_homerooms homeroom
            JOIN academic_timetable_blocks block ON block.id = homeroom.block_id
            WHERE homeroom.block_id <> target_block_id
              AND homeroom.is_active AND block.is_active
              AND block.timetable_id = target_block.timetable_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT coverage.homeroom_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            CROSS JOIN LATERAL unnest(block_group.homeroom_ids) AS coverage(homeroom_id)
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block.timetable_id = target_block.timetable_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_homerooms candidate
        JOIN occupied_homerooms occupied USING (homeroom_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_teachers AS (
            SELECT teacher_id
            FROM academic_timetable_block_teachers
            WHERE block_id = target_block_id AND is_active
            UNION
            SELECT instructor.instructor_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_block_group_instructors instructor
              ON instructor.block_group_id = block_group.id
            WHERE block_group.block_id = target_block_id AND block_group.is_active
        ), occupied_teachers AS (
            SELECT teacher.teacher_id, teacher.block_id
            FROM academic_timetable_block_teachers teacher
            JOIN academic_timetable_blocks block ON block.id = teacher.block_id
            WHERE teacher.block_id <> target_block_id
              AND teacher.is_active AND block.is_active
              AND block.timetable_id = target_block.timetable_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT instructor.instructor_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            JOIN academic_timetable_block_group_instructors instructor
              ON instructor.block_group_id = block_group.id
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block.timetable_id = target_block.timetable_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_teachers candidate
        JOIN occupied_teachers occupied USING (teacher_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED'
            USING ERRCODE = 'check_violation';
    END IF;

    IF EXISTS (
        WITH candidate_rooms AS (
            SELECT room_id
            FROM academic_timetable_block_groups
            WHERE block_id = target_block_id AND is_active AND room_id IS NOT NULL
            UNION
            SELECT room_id
            FROM academic_timetable_block_homerooms
            WHERE block_id = target_block_id AND is_active AND room_id IS NOT NULL
        ), occupied_rooms AS (
            SELECT block_group.room_id, block_group.block_id
            FROM academic_timetable_block_groups block_group
            JOIN academic_timetable_blocks block ON block.id = block_group.block_id
            WHERE block_group.block_id <> target_block_id
              AND block_group.is_active AND block.is_active
              AND block_group.room_id IS NOT NULL
              AND block.timetable_id = target_block.timetable_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
            UNION
            SELECT homeroom.room_id, homeroom.block_id
            FROM academic_timetable_block_homerooms homeroom
            JOIN academic_timetable_blocks block ON block.id = homeroom.block_id
            WHERE homeroom.block_id <> target_block_id
              AND homeroom.is_active AND block.is_active
              AND homeroom.room_id IS NOT NULL
              AND block.timetable_id = target_block.timetable_id
              AND block.day_of_week = target_block.day_of_week
              AND block.bell_schedule_period_id = target_block.bell_schedule_period_id
        )
        SELECT 1
        FROM candidate_rooms candidate
        JOIN occupied_rooms occupied USING (room_id)
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_TIMETABLE_ROOM_CONFLICT'
            USING ERRCODE = 'check_violation';
    END IF;
END;
$$;

CREATE OR REPLACE VIEW academic_exam_eligible_sources AS
WITH ready_plan AS (
    SELECT plan.id,
           plan.learning_offering_id,
           plan.academic_term_id,
           plan.academic_year_id,
           plan.subject_version_id
    FROM course_assessment_plans plan
    JOIN course_offering_details detail
      ON detail.learning_offering_id = plan.learning_offering_id
     AND detail.subject_version_id = plan.subject_version_id
     AND detail.academic_term_id = plan.academic_term_id
     AND detail.academic_year_id = plan.academic_year_id
    JOIN course_assessment_phases phase ON phase.plan_id = plan.id
    WHERE plan.assessment_coordinator_id IS NOT NULL
      AND EXISTS (
          SELECT 1
          FROM learning_group_teachers allocation JOIN learning_groups allocated_group ON allocated_group.id=allocation.learning_group_id
          JOIN users coordinator
            ON coordinator.id = allocation.teacher_id
           AND coordinator.status = 'active'
          WHERE allocated_group.learning_offering_id = plan.learning_offering_id
            AND allocation.academic_year_id = plan.academic_year_id
            AND allocation.academic_term_id = plan.academic_term_id
            AND allocation.teacher_id = plan.assessment_coordinator_id
      )
    GROUP BY plan.id, detail.assessment_total_score
    HAVING count(phase.id) = 4
       AND count(DISTINCT phase.phase_code) = 4
       AND sum(phase.max_score) = detail.assessment_total_score
)
SELECT uuid_generate_v5(
           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'exam-source:' || phase.id::text || ':' || learning_group.id::text
               || ':' || homeroom.id::text
       ) AS source_id,
       ready_plan.academic_term_id,
       ready_plan.academic_year_id,
       phase.id AS assessment_phase_id,
       ready_plan.id AS course_assessment_plan_id,
       ready_plan.learning_offering_id,
       learning_group.id AS learning_group_id,
       homeroom.id AS homeroom_id,
       version.subject_id,
       homeroom.grade_level_id,
       phase.phase_code AS exam_kind,
       phase.exam_duration_minutes AS duration_minutes
FROM ready_plan
JOIN course_assessment_phases phase ON phase.plan_id = ready_plan.id
JOIN subject_versions version ON version.id = ready_plan.subject_version_id
JOIN learning_groups learning_group
  ON learning_group.learning_offering_id = ready_plan.learning_offering_id
 AND learning_group.academic_term_id = ready_plan.academic_term_id
 AND learning_group.academic_year_id = ready_plan.academic_year_id

JOIN learning_group_homerooms coverage ON coverage.learning_group_id = learning_group.id
JOIN homerooms homeroom
  ON homeroom.id = coverage.homeroom_id
 AND homeroom.academic_year_id = ready_plan.academic_year_id
 AND homeroom.is_active
WHERE phase.phase_code IN ('midterm', 'final')
  AND phase.exam_arrangement = 'in_timetable'
  AND phase.exam_duration_minutes IS NOT NULL
  AND phase.exam_duration_minutes > 0;

DROP FUNCTION academic_capture_delivery_snapshot(uuid,date,jsonb,timestamptz);
DROP FUNCTION academic_project_delivery_teacher_changes(jsonb,uuid);
DROP FUNCTION academic_delivery_revision_snapshot(uuid,date);
DROP FUNCTION academic_delivery_apply_audiences(jsonb,uuid);
DROP FUNCTION academic_delivery_teacher_basis(uuid);
DROP FUNCTION academic_delivery_inherit_teachers(jsonb,uuid,date);
DROP FUNCTION academic_delivery_project_teacher_ids(jsonb,uuid,date);
DROP FUNCTION academic_delivery_accept_effectivity(uuid,uuid);
DROP FUNCTION academic_delivery_applicable_publication(uuid,date);
DROP FUNCTION academic_timetable_applicable_publication(uuid,date);
DROP FUNCTION academic_timetable_latest_publication(uuid);
DROP FUNCTION academic_delivery_offering_available(uuid,date);
DROP FUNCTION academic_validate_delivery_construction();
DROP FUNCTION academic_require_published_construction();
DROP FUNCTION academic_protect_delivery_effectivity();
DROP FUNCTION academic_require_accepted_delivery_effectivity();
DROP FUNCTION academic_validate_group_teacher_interval();
DROP FUNCTION academic_protect_published_group_teachers();
DROP FUNCTION academic_protect_published_offering();
DROP FUNCTION academic_protect_published_offering_child();
DROP FUNCTION academic_protect_change_set();
DROP FUNCTION academic_protect_change_set_item();
DROP FUNCTION academic_protect_teacher_handoff_receipt();
DROP FUNCTION academic_protect_delivery_version();
DROP FUNCTION academic_protect_revision_audience();
DROP FUNCTION academic_protect_timetable_version();
DROP FUNCTION academic_protect_timetable_version_child();
DROP FUNCTION academic_protect_timetable_block_child();
DROP FUNCTION academic_validate_timetable_delivery_source();
DROP FUNCTION academic_touch_timetable_content_revision();
DROP FUNCTION assert_timetable_block_mutable(uuid);

DELETE FROM academic_audit_events WHERE entity_type IN(
    'academic_delivery_version','academic_term_change_set','academic_term_change_item',
    'academic_timetable_version','academic_teacher_handoff_run',
    'learning_offering','learning_group','learning_group_student')
 OR (entity_type='academic_timetable_block_group' AND event_code='academic_teacher_handoff.applied');

CREATE TABLE academic_current_operations_migration_audit(
    migration_version bigint PRIMARY KEY CHECK(migration_version=157),
    cutover_date date NOT NULL,
    passed boolean NOT NULL,
    source_fingerprint text NOT NULL CHECK(length(source_fingerprint)=64),
    checks jsonb NOT NULL CHECK(jsonb_typeof(checks)='array'),
    completed_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TRIGGER academic_current_operations_migration_audit_immutable
BEFORE UPDATE OR DELETE ON academic_current_operations_migration_audit
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();


CREATE TEMP TABLE academic157_after ON COMMIT DROP AS
SELECT 'memberships'::text resource,count(*) row_count,encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex') fingerprint
FROM learning_group_students r JOIN academic157_groups g ON g.id=r.learning_group_id
UNION ALL SELECT 'attendance',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r)-'source_key' ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM attendance_sessions r JOIN academic157_retained_sessions kept ON kept.id=r.id
UNION ALL SELECT 'scores',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM learning_group_student_scores r JOIN academic157_groups g ON g.id=r.learning_group_id
UNION ALL SELECT 'course_results',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_course_results r JOIN academic157_groups g ON g.id=r.learning_group_id
UNION ALL SELECT 'activity_results',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_activity_results r JOIN academic157_groups g ON g.id=r.learning_group_id
UNION ALL SELECT 'attendance_records',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.session_id,r.student_id),'[]')::text,'UTF8')),'hex')
FROM attendance_records r JOIN academic157_retained_sessions kept ON kept.id=r.session_id
UNION ALL SELECT 'supervision',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r)-'updated_at'-'timetable_block_group_id'-'learning_group_id'-'lesson_snapshot' ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM supervision_observations r
UNION ALL SELECT 'blocks',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r)-'timetable_version_id'-'timetable_id' ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_blocks r JOIN academic157_sources selected ON selected.timetable_id=r.timetable_id WHERE r.is_active
UNION ALL SELECT 'placed_groups',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_groups r JOIN academic_timetable_blocks b ON b.id=r.block_id
JOIN academic157_sources selected ON selected.timetable_id=b.timetable_id WHERE b.is_active AND r.is_active
UNION ALL SELECT 'structural_homerooms',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_homerooms r JOIN academic_timetable_blocks b ON b.id=r.block_id
JOIN academic157_sources selected ON selected.timetable_id=b.timetable_id WHERE b.is_active AND r.is_active
UNION ALL SELECT 'structural_teachers',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_teachers r JOIN academic_timetable_blocks b ON b.id=r.block_id
JOIN academic157_sources selected ON selected.timetable_id=b.timetable_id WHERE b.is_active AND r.is_active
UNION ALL SELECT 'placement_sync',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_group_sync r JOIN academic_timetable_blocks b ON b.id=r.block_id
JOIN academic157_sources selected ON selected.timetable_id=b.timetable_id WHERE b.is_active AND EXISTS(SELECT 1 FROM academic157_groups kept WHERE kept.id=r.learning_group_id)
UNION ALL SELECT 'placed_instructors',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(r) ORDER BY r.id),'[]')::text,'UTF8')),'hex')
FROM academic_timetable_block_group_instructors r JOIN academic_timetable_block_groups bg ON bg.id=r.block_group_id
JOIN academic_timetable_blocks b ON b.id=bg.block_id JOIN academic157_sources selected ON selected.timetable_id=b.timetable_id
WHERE b.is_active AND bg.is_active
UNION ALL SELECT 'placed_exam_items',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(i) ORDER BY i.id),'[]')::text,'UTF8')),'hex')
FROM academic_exam_schedule_items i JOIN academic157_groups kept ON kept.id=i.learning_group_id
WHERE EXISTS(SELECT 1 FROM academic_exam_sessions session WHERE session.exam_schedule_item_id=i.id)
UNION ALL SELECT 'placed_exam_sessions',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(session) ORDER BY session.id),'[]')::text,'UTF8')),'hex')
FROM academic_exam_sessions session JOIN academic_exam_schedule_items item ON item.id=session.exam_schedule_item_id
JOIN academic157_groups kept ON kept.id=item.learning_group_id
UNION ALL SELECT 'exam_rounds',count(*),encode(sha256(convert_to(COALESCE(jsonb_agg(to_jsonb(round)-'status'-'published_at'-'published_by' ORDER BY round.id),'[]')::text,'UTF8')),'hex') FROM academic_exam_rounds round;

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic157_opening_policy before FULL JOIN (SELECT to_jsonb(policy) value FROM academic_opening_policy policy) after USING(value) WHERE before.value IS NULL OR after.value IS NULL)
    OR EXISTS(SELECT 1 FROM academic157_preserved b FULL JOIN academic157_after a USING(resource)
        WHERE b.row_count IS DISTINCT FROM a.row_count OR b.fingerprint IS DISTINCT FROM a.fingerprint)
    OR EXISTS(SELECT 1 FROM academic_timetable_blocks WHERE NOT is_active)
    OR EXISTS(SELECT 1 FROM academic_timetable_block_groups WHERE NOT is_active)
    OR EXISTS(SELECT 1 FROM academic_timetable_block_homerooms WHERE NOT is_active)
    OR EXISTS(SELECT 1 FROM academic_timetable_block_teachers WHERE NOT is_active)
    OR (SELECT count(*) FROM learning_offerings)<>(SELECT count(*) FROM academic157_offerings)
    OR (SELECT count(*) FROM learning_groups)<>(SELECT count(*) FROM academic157_groups)
    OR (SELECT count(*) FROM learning_group_teachers)<>(SELECT count(*) FROM academic157_teachers)
    OR (SELECT count(*) FROM academic_timetables)<>(SELECT count(*) FROM academic157_sources WHERE timetable_id IS NOT NULL)
    OR EXISTS(SELECT 1 FROM academic157_teachers s LEFT JOIN learning_group_teachers t ON t.id=s.id
        WHERE t.id IS NULL OR t.teacher_id<>s.teacher_id OR t.learning_group_id<>s.group_id OR t.role<>s.role)
    OR EXISTS(SELECT 1 FROM academic_timetable_blocks b LEFT JOIN academic_timetables t ON t.id=b.timetable_id WHERE t.id IS NULL)
    OR EXISTS(SELECT 1 FROM academic157_session_keys original JOIN attendance_sessions actual ON actual.id=original.id
        LEFT JOIN academic157_session_key_map mapping ON mapping.id=original.id
        WHERE actual.source_key IS DISTINCT FROM COALESCE(mapping.source_key,original.source_key))
    OR EXISTS(SELECT 1 FROM academic157_supervision original LEFT JOIN supervision_observations actual ON actual.id=original.id
        WHERE actual.id IS NULL OR (actual.lesson_snapshot IS DISTINCT FROM original.lesson_snapshot
            AND NOT(original.lesson_snapshot='{}'::jsonb AND actual.timetable_block_group_id IS NULL
                AND actual.lesson_snapshot=jsonb_build_object('source','removed_timetable',
                    'timetableBlockGroupId',original.timetable_block_group_id,'observedAt',original.observed_at))))
    OR (SELECT count(*) FROM pg_trigger t JOIN pg_proc p ON p.oid=t.tgfoid
        WHERE t.tgrelid=ANY(ARRAY['subject_term_evaluation_locks','subject_term_student_evaluations',
            'academic_course_result_locks','academic_course_results','academic_activity_result_locks','academic_activity_results',
            'academic_result_corrections','academic_term_aggregate_revisions','academic_annual_result_revisions','academic_annual_result_term_sources']::regclass[])
          AND NOT t.tgisinternal AND t.tgenabled='O' AND p.proname='academic_guard_result_removal')<>10
    OR EXISTS(SELECT 1 FROM academic_result_removal_scope)
    OR EXISTS(SELECT 1 FROM academic157_original_receipts original FULL JOIN notification_receipts actual ON actual.id=original.id
        LEFT JOIN academic157_cancelled_receipts selected ON selected.id=original.id
        WHERE original.id IS NULL OR actual.id IS NULL
            OR (selected.id IS NULL AND to_jsonb(actual) IS DISTINCT FROM original.fact)
            OR (selected.id IS NOT NULL AND ((to_jsonb(actual)-'presentation_consumed_at'-'presentation_outcome')
                IS DISTINCT FROM (original.fact-'presentation_consumed_at'-'presentation_outcome')
                OR (original.fact->>'presentation_consumed_at' IS NULL AND
                    (actual.presentation_consumed_at IS NULL OR actual.presentation_outcome<>'skipped'))
                OR (original.fact->>'presentation_consumed_at' IS NOT NULL AND to_jsonb(actual) IS DISTINCT FROM original.fact))))
    OR EXISTS(SELECT 1 FROM academic157_unrelated_presentations original FULL JOIN
        (SELECT receipt_id,to_jsonb(presentation) fact FROM notification_presentations presentation
            WHERE NOT EXISTS(SELECT 1 FROM academic157_cancelled_receipts selected WHERE selected.id=presentation.receipt_id)) actual USING(receipt_id)
        WHERE original.fact IS DISTINCT FROM actual.fact)
    OR EXISTS(SELECT 1 FROM notification_presentations presentation JOIN academic157_cancelled_receipts selected ON selected.id=presentation.receipt_id)
    OR EXISTS(SELECT 1 FROM academic157_removed_lessons lesson CROSS JOIN attendance_module_state module
        WHERE NOT EXISTS(SELECT 1 FROM attendance_score_changes change WHERE change.generation=module.generation
            AND change.academic_term_id=lesson.academic_term_id AND change.date=lesson.date))
    THEN RAISE EXCEPTION 'ACADEMIC_CURRENT_RECONCILIATION_FAILED'; END IF;
END $$;
INSERT INTO academic_current_operations_migration_audit(migration_version,cutover_date,passed,source_fingerprint,checks)
SELECT 157,c.cutover_date,true,
    encode(sha256(convert_to(COALESCE((SELECT jsonb_agg(to_jsonb(s) ORDER BY academic_term_id)::text FROM academic157_sources s),'[]'),'UTF8')),'hex'),
    (SELECT jsonb_agg(jsonb_build_object('code',code,'passed',true,'mismatchCount',0) ORDER BY code)
    FROM unnest(ARRAY['CURRENT_DELIVERY_GRAPH_RECONCILED','CURRENT_TEACHERS_RECONCILED','CURRENT_TIMETABLE_IDS_RECONCILED',
        'CURRENT_FACTS_PRESERVED','CURRENT_SUPERVISION_PRESERVED','CURRENT_CATALOG_PINS_PRESERVED',
        'CURRENT_LEGACY_SCHEMA_REMOVED','CURRENT_SCOPE_GUARDS_ENABLED']) code)
FROM academic157_context c;
