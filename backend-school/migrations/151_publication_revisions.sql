-- Publication chronology is independent of deliberate civil effectivity.
-- Owner-only migration; runtime retains its existing DML-only privileges.
LOCK TABLE academic_delivery_versions, academic_timetable_versions,
    academic_term_change_sets, learning_offerings, learning_group_teachers IN ACCESS EXCLUSIVE MODE;

CREATE TEMP TABLE academic_151_preservation ON COMMIT DROP AS
SELECT 'delivery_publications_preserved'::text code,
    md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) fingerprint
FROM academic_delivery_versions v
UNION ALL SELECT 'timetable_publications_preserved',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM academic_timetable_versions v
UNION ALL SELECT 'teacher_episodes_preserved',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM learning_group_teachers v
UNION ALL SELECT 'offering_registry_preserved',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM learning_offerings v
UNION ALL SELECT 'publication_receipts_preserved',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM academic_term_change_sets v
UNION ALL SELECT 'downstream_sources_preserved',md5(COALESCE((SELECT string_agg(fingerprint,'' ORDER BY owner) FROM (
    SELECT 'blocks' owner,md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) fingerprint FROM academic_timetable_blocks v
    UNION ALL SELECT 'attendance_sessions',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM attendance_sessions v
    UNION ALL SELECT 'attendance_records',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY session_id,student_id),'')) FROM attendance_records v
    UNION ALL SELECT 'student_scores',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM learning_group_student_scores v
    UNION ALL SELECT 'course_results',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM academic_course_results v
    UNION ALL SELECT 'activity_results',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM academic_activity_results v
) preserved),''));

ALTER TABLE academic_delivery_versions ADD COLUMN publication_no BIGINT;
ALTER TABLE academic_timetable_versions ADD COLUMN publication_no BIGINT;
ALTER TABLE academic_delivery_versions DISABLE TRIGGER academic_delivery_versions_immutable;
ALTER TABLE academic_timetable_versions DISABLE TRIGGER academic_timetable_versions_published_immutable;
WITH numbered AS (SELECT id,row_number() OVER(PARTITION BY academic_term_id ORDER BY effective_from,id) n
    FROM academic_delivery_versions WHERE status='published')
UPDATE academic_delivery_versions v SET publication_no=numbered.n FROM numbered WHERE numbered.id=v.id;
WITH numbered AS (SELECT id,row_number() OVER(PARTITION BY academic_term_id ORDER BY effective_from,id) n
    FROM academic_timetable_versions WHERE status='published')
UPDATE academic_timetable_versions v SET publication_no=numbered.n FROM numbered WHERE numbered.id=v.id;
ALTER TABLE academic_delivery_versions ENABLE TRIGGER academic_delivery_versions_immutable;
ALTER TABLE academic_timetable_versions ENABLE TRIGGER academic_timetable_versions_published_immutable;

DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic_delivery_versions v JOIN academic_delivery_versions source ON source.id=v.source_version_id
        WHERE v.status='published' AND (source.status<>'published' OR source.academic_term_id<>v.academic_term_id))
    THEN RAISE EXCEPTION 'PUBLICATION_151_SOURCE_LINEAGE_INVALID' USING ERRCODE='check_violation'; END IF;
END $$;

ALTER TABLE academic_delivery_versions ADD CONSTRAINT academic_delivery_publication_number_shape
    CHECK ((status='published' AND publication_no IS NOT NULL AND publication_no>0) OR (status<>'published' AND publication_no IS NULL));
ALTER TABLE academic_timetable_versions ADD CONSTRAINT academic_timetable_publication_number_shape
    CHECK ((status='published' AND publication_no IS NOT NULL AND publication_no>0) OR (status<>'published' AND publication_no IS NULL));
CREATE UNIQUE INDEX academic_delivery_publication_number_key ON academic_delivery_versions(academic_term_id,publication_no) WHERE status='published';
CREATE UNIQUE INDEX academic_timetable_publication_number_key ON academic_timetable_versions(academic_term_id,publication_no) WHERE status='published';
DROP INDEX academic_delivery_versions_published_effective_key;
DROP INDEX academic_timetable_versions_published_effective_key;

CREATE TABLE academic_delivery_teacher_cutoffs (
    assignment_id UUID NOT NULL REFERENCES learning_group_teachers(id) ON DELETE RESTRICT,
    change_set_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    effective_on DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(assignment_id,change_set_id),
    FOREIGN KEY(change_set_id,academic_term_id,academic_year_id)
        REFERENCES academic_term_change_sets(id,academic_term_id,academic_year_id) ON DELETE RESTRICT
);
CREATE TABLE academic_delivery_offering_effectivity (
    learning_offering_id UUID NOT NULL,
    change_set_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    effect_kind TEXT NOT NULL CHECK(effect_kind IN ('activation','cutoff')),
    effective_on DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(learning_offering_id,change_set_id),
    FOREIGN KEY(learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_offerings(id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY(change_set_id,academic_term_id,academic_year_id)
        REFERENCES academic_term_change_sets(id,academic_term_id,academic_year_id) ON DELETE RESTRICT
);

CREATE FUNCTION academic_protect_delivery_effectivity() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE revision academic_term_change_sets%ROWTYPE;
BEGIN
    IF TG_OP<>'INSERT' THEN RAISE EXCEPTION 'ACADEMIC_DELIVERY_EFFECTIVITY_IMMUTABLE' USING ERRCODE='check_violation'; END IF;
    SELECT * INTO revision FROM academic_term_change_sets
    WHERE id=NEW.change_set_id AND status='draft' FOR SHARE;
    IF revision.id IS NULL OR NEW.academic_term_id<>revision.academic_term_id
        OR NEW.academic_year_id<>revision.academic_year_id
        OR NEW.effective_on<>COALESCE(revision.effective_from,revision.reference_date)
        OR NULLIF(current_setting('schoolorbit.academic_change_set_id',true),'')::uuid IS DISTINCT FROM revision.id
    THEN RAISE EXCEPTION 'ACADEMIC_DELIVERY_EFFECTIVITY_PROVENANCE_INVALID' USING ERRCODE='check_violation'; END IF;
    IF TG_TABLE_NAME='academic_delivery_teacher_cutoffs' THEN
        IF NOT EXISTS(SELECT 1 FROM learning_group_teachers t WHERE t.id=NEW.assignment_id
            AND t.academic_term_id=revision.academic_term_id AND t.academic_year_id=revision.academic_year_id)
        THEN RAISE EXCEPTION 'ACADEMIC_DELIVERY_EFFECTIVITY_CONTEXT_INVALID' USING ERRCODE='check_violation'; END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER academic_delivery_teacher_cutoffs_guard BEFORE INSERT OR UPDATE OR DELETE ON academic_delivery_teacher_cutoffs
    FOR EACH ROW EXECUTE FUNCTION academic_protect_delivery_effectivity();
CREATE TRIGGER academic_delivery_offering_effectivity_guard BEFORE INSERT OR UPDATE OR DELETE ON academic_delivery_offering_effectivity
    FOR EACH ROW EXECUTE FUNCTION academic_protect_delivery_effectivity();

-- Pending evidence is visible to overlap checks in this transaction, but may
-- never become committed effectivity without the same accepted publication.
CREATE FUNCTION academic_require_accepted_delivery_effectivity() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE revision_id UUID; chosen DATE;
BEGIN
    IF TG_TABLE_NAME='learning_group_teachers' THEN
        revision_id:=NEW.started_by_change_set_id; chosen:=NEW.starts_on;
        IF revision_id IS NULL THEN RETURN NEW; END IF;
    ELSE
        revision_id:=NEW.change_set_id; chosen:=NEW.effective_on;
    END IF;
    IF NOT EXISTS(SELECT 1 FROM academic_term_change_sets c JOIN academic_delivery_versions v ON v.id=c.target_delivery_version_id
        WHERE c.id=revision_id AND c.status='published' AND v.status='published'
          AND c.effective_from=chosen AND v.effective_from=chosen
          AND c.academic_term_id=NEW.academic_term_id AND c.academic_year_id=NEW.academic_year_id
          AND v.academic_term_id=NEW.academic_term_id AND v.academic_year_id=NEW.academic_year_id)
    THEN RAISE EXCEPTION 'ACADEMIC_DELIVERY_EFFECTIVITY_NOT_PUBLISHED' USING ERRCODE='check_violation'; END IF;
    RETURN NEW;
END $$;
CREATE CONSTRAINT TRIGGER academic_delivery_teacher_cutoffs_accepted AFTER INSERT ON academic_delivery_teacher_cutoffs
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION academic_require_accepted_delivery_effectivity();
CREATE CONSTRAINT TRIGGER academic_delivery_offering_effectivity_accepted AFTER INSERT ON academic_delivery_offering_effectivity
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION academic_require_accepted_delivery_effectivity();
CREATE CONSTRAINT TRIGGER academic_delivery_teacher_episodes_accepted AFTER INSERT ON learning_group_teachers
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION academic_require_accepted_delivery_effectivity();

CREATE VIEW academic_delivery_effective_teacher_episodes AS
SELECT t.*, LEAST(t.ends_on,(SELECT min(c.effective_on)-1 FROM academic_delivery_teacher_cutoffs c WHERE c.assignment_id=t.id)) AS effective_ends_on
FROM learning_group_teachers t;

-- Complete immutable graph membership owns offering availability. Sparse/global
-- MIN cutoffs cannot represent a newer retention overriding a future older stop.
CREATE VIEW academic_delivery_effective_offering_spans AS
SELECT (o->>'id')::uuid learning_offering_id,v.id delivery_version_id,v.academic_term_id,v.academic_year_id,
    v.effective_from starts_on,bounds.ends_on
FROM academic_delivery_versions v
CROSS JOIN LATERAL (SELECT min(n.effective_from)-1 ends_on FROM academic_delivery_versions n
    WHERE n.academic_term_id=v.academic_term_id AND n.status='published' AND n.publication_no>v.publication_no) bounds
CROSS JOIN LATERAL jsonb_array_elements(v.snapshot->'offerings') o
WHERE v.status='published' AND (bounds.ends_on IS NULL OR bounds.ends_on>=v.effective_from);

CREATE FUNCTION academic_delivery_applicable_publication(term UUID,on_date DATE) RETURNS UUID LANGUAGE SQL STABLE AS $$
    SELECT id FROM academic_delivery_versions WHERE academic_term_id=term AND status='published' AND effective_from<=on_date
    ORDER BY publication_no DESC LIMIT 1;
$$;
CREATE FUNCTION academic_timetable_applicable_publication(term UUID,on_date DATE) RETURNS UUID LANGUAGE SQL STABLE AS $$
    SELECT id FROM academic_timetable_versions WHERE academic_term_id=term AND status='published' AND effective_from<=on_date
    ORDER BY publication_no DESC LIMIT 1;
$$;
CREATE FUNCTION academic_timetable_latest_publication(term UUID) RETURNS UUID LANGUAGE SQL STABLE AS $$
    SELECT id FROM academic_timetable_versions WHERE academic_term_id=term AND status='published' ORDER BY publication_no DESC LIMIT 1;
$$;
CREATE FUNCTION academic_delivery_offering_available(offering UUID,on_date DATE) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
    SELECT COALESCE((SELECT EXISTS(SELECT 1 FROM jsonb_array_elements(version.snapshot->'offerings') entry WHERE (entry->>'id')::uuid=offering)
        FROM learning_offerings registry JOIN academic_delivery_versions version
            ON version.id=academic_delivery_applicable_publication(registry.academic_term_id,on_date)
        WHERE registry.id=offering),false);
$$;

ALTER TABLE learning_group_teachers DROP CONSTRAINT learning_group_teachers_episode_key;
ALTER TABLE learning_group_teachers ADD CONSTRAINT learning_group_teachers_episode_provenance_key
    UNIQUE NULLS NOT DISTINCT(learning_group_id,teacher_id,starts_on,started_by_change_set_id);
CREATE OR REPLACE FUNCTION academic_validate_group_teacher_interval() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE boundary DATE;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.learning_group_id::text||':'||NEW.teacher_id::text,0));
    SELECT min(effective_on)-1 INTO boundary FROM academic_delivery_teacher_cutoffs WHERE assignment_id=NEW.id;
    boundary:=LEAST(NEW.ends_on,boundary);
    IF boundary IS NOT NULL AND boundary<NEW.starts_on THEN RETURN NEW; END IF;
    IF EXISTS(SELECT 1 FROM academic_delivery_effective_teacher_episodes t
        WHERE t.learning_group_id=NEW.learning_group_id AND t.teacher_id=NEW.teacher_id AND t.id<>NEW.id
          AND CASE WHEN t.effective_ends_on<t.starts_on THEN 'empty'::daterange ELSE daterange(t.starts_on,t.effective_ends_on,'[]') END && daterange(NEW.starts_on,boundary,'[]'))
    THEN RAISE EXCEPTION 'ACADEMIC_GROUP_TEACHER_INTERVAL_OVERLAP' USING ERRCODE='check_violation'; END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION academic_protect_published_group_teachers() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE revision academic_term_change_sets%ROWTYPE;
BEGIN
    IF NOT EXISTS(SELECT 1 FROM learning_groups WHERE id=CASE WHEN TG_OP='INSERT' THEN NEW.learning_group_id ELSE OLD.learning_group_id END
        AND status IN ('published','closed')) THEN RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END; END IF;
    IF TG_OP<>'INSERT' THEN RAISE EXCEPTION 'ACADEMIC_PUBLISHED_GROUP_TEACHERS_IMMUTABLE' USING ERRCODE='check_violation'; END IF;
    SELECT * INTO revision FROM academic_term_change_sets WHERE id=NULLIF(current_setting('schoolorbit.academic_change_set_id',true),'')::uuid AND status='draft' FOR SHARE;
    IF revision.id IS NULL OR NEW.started_by_change_set_id IS DISTINCT FROM revision.id
        OR NEW.ended_by_change_set_id IS NOT NULL OR NEW.ends_on IS NOT NULL
        OR NEW.starts_on IS DISTINCT FROM COALESCE(revision.effective_from,revision.reference_date)
        OR NEW.academic_term_id IS DISTINCT FROM revision.academic_term_id OR NEW.academic_year_id IS DISTINCT FROM revision.academic_year_id
        OR NEW.created_by IS NULL OR NEW.updated_by IS NULL
        OR NOT EXISTS(SELECT 1 FROM academic_delivery_versions v,jsonb_array_elements(v.snapshot->'offerings') o,
            jsonb_array_elements(o->'groups') g,jsonb_array_elements(g->'teachers') t
            WHERE v.id=revision.target_delivery_version_id AND (g->>'id')::uuid=NEW.learning_group_id
              AND (t->>'assignmentId')::uuid=NEW.id AND (t->>'teacherId')::uuid=NEW.teacher_id AND t->>'role'=NEW.role)
    THEN RAISE EXCEPTION 'ACADEMIC_TEACHER_CHANGE_PROVENANCE_INVALID' USING ERRCODE='check_violation'; END IF;
    RETURN NEW;
END $$;

CREATE FUNCTION academic_delivery_inherit_teachers(graph JSONB,revision UUID,on_date DATE) RETURNS JSONB LANGUAGE SQL STABLE AS $$
    SELECT jsonb_build_object('offerings',COALESCE(jsonb_agg(jsonb_set(o,'{groups}',COALESCE((
        SELECT jsonb_agg(jsonb_set(g,'{teachers}',COALESCE((SELECT bg->'teachers'
            FROM academic_term_change_sets c JOIN academic_delivery_versions b ON b.id=c.base_delivery_version_id,
                jsonb_array_elements(b.snapshot->'offerings') bo,jsonb_array_elements(bo->'groups') bg
            WHERE c.id=revision AND (bg->>'id')::uuid=(g->>'id')::uuid),
            CASE WHEN EXISTS(SELECT 1 FROM learning_groups staged WHERE staged.id=(g->>'id')::uuid AND staged.status='draft')
            THEN (SELECT COALESCE(jsonb_agg(jsonb_build_object(
                    'assignmentId',assignment.id,'teacherId',assignment.teacher_id,
                    'displayName',concat_ws(' ',teacher.title,teacher.first_name,teacher.last_name),'role',assignment.role)
                    ORDER BY assignment.teacher_id,assignment.id),'[]'::jsonb)
                FROM learning_group_teachers assignment JOIN users teacher ON teacher.id=assignment.teacher_id
                WHERE assignment.learning_group_id=(g->>'id')::uuid)
            ELSE (SELECT COALESCE(jsonb_agg(t ORDER BY t->>'assignmentId'),'[]'::jsonb)
                FROM jsonb_array_elements(g->'teachers') t
                JOIN academic_delivery_effective_teacher_episodes episode ON episode.id=(t->>'assignmentId')::uuid
                    AND episode.learning_group_id=(g->>'id')::uuid
                WHERE episode.starts_on<=on_date AND (episode.effective_ends_on IS NULL OR episode.effective_ends_on>=on_date)) END
            )) ORDER BY g->>'id')
        FROM jsonb_array_elements(o->'groups') g),'[]'::jsonb)) ORDER BY o->>'id'),'[]'::jsonb))
    FROM jsonb_array_elements(graph->'offerings') o;
$$;
CREATE FUNCTION academic_delivery_project_teacher_ids(graph JSONB,revision UUID,on_date DATE) RETURNS JSONB LANGUAGE SQL STABLE AS $$
    SELECT jsonb_build_object('offerings',COALESCE(jsonb_agg(jsonb_set(o,'{groups}',COALESCE((
        SELECT jsonb_agg(jsonb_set(g,'{teachers}',COALESCE((SELECT jsonb_agg(
            CASE WHEN EXISTS(SELECT 1 FROM academic_delivery_effective_teacher_episodes e WHERE e.id=(t->>'assignmentId')::uuid
                AND (e.starts_on>on_date OR e.effective_ends_on IS NOT NULL))
            THEN jsonb_set(t,'{assignmentId}',to_jsonb(uuid_generate_v5(revision,'carried-teacher-episode:'||((t->>'assignmentId')::uuid)::text))) ELSE t END
            ORDER BY t->>'teacherId',t->>'assignmentId') FROM jsonb_array_elements(g->'teachers') t),'[]'::jsonb)) ORDER BY g->>'id')
        FROM jsonb_array_elements(o->'groups') g),'[]'::jsonb)) ORDER BY o->>'id'),'[]'::jsonb))
    FROM jsonb_array_elements(graph->'offerings') o;
$$;

CREATE FUNCTION academic_delivery_accept_effectivity(revision_id UUID,actor UUID) RETURNS JSONB LANGUAGE plpgsql AS $$
DECLARE revision academic_term_change_sets%ROWTYPE; graph JSONB; cutoffs UUID[]; inserted UUID[]; groups UUID[];
BEGIN
    SELECT * INTO revision FROM academic_term_change_sets WHERE id=revision_id AND status='draft' FOR UPDATE;
    IF revision.id IS NULL THEN RAISE EXCEPTION 'ACADEMIC_DELIVERY_EFFECTIVITY_PROVENANCE_INVALID' USING ERRCODE='check_violation'; END IF;
    SELECT snapshot INTO graph FROM academic_delivery_versions WHERE id=revision.target_delivery_version_id AND status='draft' FOR UPDATE;
    PERFORM set_config('schoolorbit.academic_change_set_id',revision.id::text,true);
    WITH changed AS (INSERT INTO academic_delivery_teacher_cutoffs(assignment_id,change_set_id,academic_term_id,academic_year_id,effective_on)
        SELECT e.id,revision.id,revision.academic_term_id,revision.academic_year_id,revision.reference_date
        FROM academic_delivery_effective_teacher_episodes e
        WHERE e.academic_term_id=revision.academic_term_id AND (e.effective_ends_on IS NULL OR e.effective_ends_on>=revision.reference_date)
          AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(graph->'offerings') o,jsonb_array_elements(o->'groups') g,
              jsonb_array_elements(g->'teachers') t WHERE (t->>'assignmentId')::uuid=e.id)
        RETURNING assignment_id) SELECT COALESCE(array_agg(assignment_id),'{}') INTO cutoffs FROM changed;
    WITH changed AS (INSERT INTO learning_group_teachers(id,learning_group_id,academic_term_id,academic_year_id,teacher_id,role,starts_on,started_by_change_set_id,created_by,updated_by)
        SELECT (t->>'assignmentId')::uuid,(g->>'id')::uuid,revision.academic_term_id,revision.academic_year_id,
            (t->>'teacherId')::uuid,t->>'role',revision.reference_date,revision.id,actor,actor
        FROM jsonb_array_elements(graph->'offerings') o,jsonb_array_elements(o->'groups') g,jsonb_array_elements(g->'teachers') t
        WHERE NOT EXISTS(SELECT 1 FROM learning_group_teachers e WHERE e.id=(t->>'assignmentId')::uuid)
        RETURNING id) SELECT COALESCE(array_agg(id),'{}') INTO inserted FROM changed;
    INSERT INTO academic_delivery_offering_effectivity(learning_offering_id,change_set_id,academic_term_id,academic_year_id,effect_kind,effective_on)
    SELECT o.id,revision.id,revision.academic_term_id,revision.academic_year_id,
        CASE WHEN EXISTS(SELECT 1 FROM jsonb_array_elements(graph->'offerings') selected WHERE (selected->>'id')::uuid=o.id) THEN 'activation' ELSE 'cutoff' END,
        revision.reference_date FROM learning_offerings o WHERE o.academic_term_id=revision.academic_term_id AND o.status='published';
    SELECT COALESCE(array_agg(DISTINCT learning_group_id),'{}') INTO groups FROM learning_group_teachers WHERE id=ANY(cutoffs||inserted);
    RETURN jsonb_build_object('cutoffAssignmentIds',cutoffs,'createdAssignmentIds',inserted,'affectedGroupIds',groups);
END $$;

CREATE OR REPLACE FUNCTION academic_validate_timetable_delivery_source() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source academic_delivery_versions%ROWTYPE; applicable UUID;
BEGIN
    SELECT * INTO source FROM academic_delivery_versions WHERE id=NEW.delivery_version_id FOR SHARE;
    IF source.id IS NULL OR source.status<>'published' OR source.academic_term_id<>NEW.academic_term_id OR source.academic_year_id<>NEW.academic_year_id
    THEN RAISE EXCEPTION 'ACADEMIC_TIMETABLE_DELIVERY_SOURCE_INVALID' USING ERRCODE='check_violation'; END IF;
    IF NEW.status='published' AND (TG_OP='INSERT' OR OLD.status<>'published') THEN
        SELECT academic_delivery_applicable_publication(NEW.academic_term_id,NEW.effective_from) INTO applicable;
        IF applicable IS DISTINCT FROM source.id THEN RAISE EXCEPTION 'ACADEMIC_TIMETABLE_DELIVERY_DATE_MISMATCH' USING ERRCODE='check_violation'; END IF;
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION academic_delivery_revision_snapshot(revision UUID, candidate_date DATE DEFAULT NULL)
RETURNS JSONB LANGUAGE SQL STABLE AS $$
    WITH context AS (
        SELECT c.*,COALESCE(candidate_date,c.effective_from,c.reference_date) AS evaluation_date,base.snapshot AS base_snapshot FROM academic_term_change_sets c
        LEFT JOIN academic_delivery_versions base ON base.id=c.base_delivery_version_id
        WHERE c.id=revision
    ), base_targets AS (
        SELECT (offering->>'id')::UUID AS id,(offering->>'weeklyPeriodTarget')::INTEGER AS weekly_period_target
        FROM context,jsonb_array_elements(base_snapshot->'offerings') offering
        UNION ALL
        SELECT offering.id,COALESCE(subject_version.periods_per_week,activity_version.periods_per_week)
        FROM context JOIN learning_offerings offering ON offering.academic_term_id=context.academic_term_id
        LEFT JOIN course_offering_details course ON course.learning_offering_id=offering.id
        LEFT JOIN subject_versions subject_version ON subject_version.id=course.subject_version_id
        LEFT JOIN activity_offering_details activity ON activity.learning_offering_id=offering.id
        LEFT JOIN activity_versions activity_version ON activity_version.id=activity.activity_version_id
        WHERE context.base_delivery_version_id IS NULL AND offering.status IN ('draft','published')
            AND offering.starts_on<=context.evaluation_date
            AND (offering.ends_on IS NULL OR offering.ends_on>=context.evaluation_date)
            AND NOT EXISTS(SELECT 1 FROM academic_term_change_items item
                WHERE item.learning_offering_id=offering.id AND item.action_kind='add_offering'
                )
    ), selected_targets AS (
        SELECT base.id,COALESCE(adjustment.weekly_period_target,base.weekly_period_target) AS weekly_period_target
        FROM base_targets base LEFT JOIN academic_term_change_items adjustment
            ON adjustment.change_set_id=revision AND adjustment.learning_offering_id=base.id
                AND adjustment.action_kind='adjust_weekly_period_target'
        WHERE NOT EXISTS(SELECT 1 FROM academic_term_change_items stopped
            WHERE stopped.change_set_id=revision AND stopped.learning_offering_id=base.id
                AND stopped.action_kind='stop_offering')
        UNION ALL
        SELECT added.learning_offering_id,COALESCE(adjustment.weekly_period_target,added.weekly_period_target)
        FROM academic_term_change_items added
        LEFT JOIN academic_term_change_items adjustment
            ON adjustment.change_set_id=revision AND adjustment.learning_offering_id=added.learning_offering_id
                AND adjustment.action_kind='adjust_weekly_period_target'
        WHERE added.change_set_id=revision AND added.action_kind='add_offering'
    )
    , projected AS (
    SELECT academic_delivery_project_teacher_ids(academic_project_delivery_teacher_changes(academic_delivery_inherit_teachers(academic_capture_delivery_snapshot(
        context.academic_term_id,context.evaluation_date,
        COALESCE((SELECT jsonb_agg(jsonb_build_object('id',id,'weekly_period_target',weekly_period_target) ORDER BY id)
            FROM selected_targets),'[]'::JSONB)),revision,context.evaluation_date),revision),revision,context.evaluation_date) AS graph FROM context
    )
    , excluded AS (SELECT CASE WHEN cardinality(context.excluded_offering_ids)=0
            AND cardinality(context.excluded_group_ids)=0 THEN projected.graph
        ELSE (SELECT jsonb_build_object('offerings',COALESCE(jsonb_agg(
            jsonb_set(offering,'{groups}',COALESCE((SELECT jsonb_agg(source_group ORDER BY (source_group->>'id')::UUID)
                FROM jsonb_array_elements(offering->'groups') source_group
                WHERE NOT ((source_group->>'id')::UUID=ANY(context.excluded_group_ids))), '[]'::JSONB))
            ORDER BY (offering->>'id')::UUID), '[]'::JSONB))
            FROM jsonb_array_elements(projected.graph->'offerings') offering
            WHERE NOT ((offering->>'id')::UUID=ANY(context.excluded_offering_ids))) END
    AS graph FROM context CROSS JOIN projected)
    SELECT academic_delivery_apply_audiences(excluded.graph,revision) FROM excluded;
$$;

CREATE OR REPLACE VIEW academic_delivery_latest_published_teacher_allocations AS
WITH latest AS (
    SELECT DISTINCT ON (academic_year_id, academic_term_id)
           id, academic_year_id, academic_term_id, snapshot
    FROM academic_delivery_versions
    WHERE status = 'published'
    ORDER BY academic_year_id, academic_term_id,
             publication_no DESC
)
SELECT DISTINCT latest.id AS delivery_version_id,
       latest.academic_year_id,
       latest.academic_term_id,
       offering.id AS learning_offering_id,
       learning_group.id AS learning_group_id,
       assignment.id AS assignment_id,
       assignment.teacher_id,
       teacher_graph->>'role' AS role
FROM latest
CROSS JOIN LATERAL jsonb_array_elements(latest.snapshot->'offerings') offering_graph
CROSS JOIN LATERAL jsonb_array_elements(offering_graph->'groups') group_graph
CROSS JOIN LATERAL jsonb_array_elements(group_graph->'teachers') teacher_graph
JOIN learning_offerings offering
  ON offering.id = (offering_graph->>'id')::uuid
 AND offering.academic_year_id = latest.academic_year_id
 AND offering.academic_term_id = latest.academic_term_id
JOIN learning_groups learning_group
  ON learning_group.id = (group_graph->>'id')::uuid
 AND learning_group.learning_offering_id = offering.id
 AND learning_group.academic_year_id = latest.academic_year_id
 AND learning_group.academic_term_id = latest.academic_term_id
 AND learning_group.status <> 'closed'
JOIN learning_group_teachers assignment
  ON assignment.id = (teacher_graph->>'assignmentId')::uuid
 AND assignment.teacher_id = (teacher_graph->>'teacherId')::uuid
 AND assignment.learning_group_id = learning_group.id
 AND assignment.academic_year_id = latest.academic_year_id
 AND assignment.academic_term_id = latest.academic_term_id;


CREATE TABLE academic_publication_revisions_audit (
    migration_version INTEGER PRIMARY KEY CHECK(migration_version=151),
    passed BOOLEAN NOT NULL,
    checks JSONB NOT NULL,
    reconciled_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE FUNCTION academic_protect_publication_revisions_audit() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP<>'INSERT' OR NOT EXISTS(SELECT 1 FROM pg_class WHERE oid=TG_RELID AND relowner=current_user::regrole)
    THEN RAISE EXCEPTION 'PUBLICATION_151_AUDIT_IMMUTABLE' USING ERRCODE='check_violation'; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER academic_publication_revisions_audit_guard BEFORE INSERT OR UPDATE OR DELETE ON academic_publication_revisions_audit
    FOR EACH ROW EXECUTE FUNCTION academic_protect_publication_revisions_audit();

CREATE TEMP TABLE academic_151_after ON COMMIT DROP AS
SELECT 'delivery_publications_preserved'::text code,md5(COALESCE(string_agg(md5((to_jsonb(v)-'publication_no')::text),'' ORDER BY id),'')) fingerprint FROM academic_delivery_versions v
UNION ALL SELECT 'timetable_publications_preserved',md5(COALESCE(string_agg(md5((to_jsonb(v)-'publication_no')::text),'' ORDER BY id),'')) FROM academic_timetable_versions v
UNION ALL SELECT 'teacher_episodes_preserved',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM learning_group_teachers v
UNION ALL SELECT 'offering_registry_preserved',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM learning_offerings v
UNION ALL SELECT 'publication_receipts_preserved',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM academic_term_change_sets v
UNION ALL SELECT 'downstream_sources_preserved',md5(COALESCE((SELECT string_agg(fingerprint,'' ORDER BY owner) FROM (
    SELECT 'blocks' owner,md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) fingerprint FROM academic_timetable_blocks v
    UNION ALL SELECT 'attendance_sessions',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM attendance_sessions v
    UNION ALL SELECT 'attendance_records',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY session_id,student_id),'')) FROM attendance_records v
    UNION ALL SELECT 'student_scores',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM learning_group_student_scores v
    UNION ALL SELECT 'course_results',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM academic_course_results v
    UNION ALL SELECT 'activity_results',md5(COALESCE(string_agg(md5(to_jsonb(v)::text),'' ORDER BY id),'')) FROM academic_activity_results v
) preserved),''));
DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM academic_151_preservation b FULL JOIN academic_151_after a USING(code) WHERE b.fingerprint IS DISTINCT FROM a.fingerprint)
    THEN RAISE EXCEPTION 'PUBLICATION_151_PRESERVATION_FAILED' USING ERRCODE='check_violation'; END IF;
    IF EXISTS(SELECT 1 FROM academic_delivery_versions v,jsonb_array_elements(v.snapshot->'offerings') o,
        jsonb_array_elements(o->'groups') g,jsonb_array_elements(g->'teachers') t
        WHERE v.status='published' AND NOT EXISTS(SELECT 1 FROM learning_group_teachers e WHERE e.id=(t->>'assignmentId')::uuid
            AND e.learning_group_id=(g->>'id')::uuid AND e.teacher_id=(t->>'teacherId')::uuid
            AND e.academic_term_id=v.academic_term_id AND e.academic_year_id=v.academic_year_id AND e.role=t->>'role'
            AND EXISTS(SELECT 1 FROM learning_groups owned WHERE owned.id=e.learning_group_id AND owned.learning_offering_id=(o->>'id')::uuid)))
    THEN RAISE EXCEPTION 'PUBLICATION_151_ASSIGNMENT_CONTEXT_INVALID' USING ERRCODE='check_violation'; END IF;
    IF EXISTS(SELECT 1 FROM academic_delivery_versions WHERE (status='published') IS DISTINCT FROM (publication_no IS NOT NULL))
        OR EXISTS(SELECT 1 FROM academic_timetable_versions WHERE (status='published') IS DISTINCT FROM (publication_no IS NOT NULL))
        OR EXISTS(SELECT 1 FROM academic_delivery_versions a JOIN academic_delivery_versions b ON a.academic_term_id=b.academic_term_id
            WHERE a.status='published' AND b.status='published' AND a.effective_from<b.effective_from AND a.publication_no>=b.publication_no)
        OR EXISTS(SELECT 1 FROM academic_delivery_effective_offering_spans WHERE ends_on<starts_on)
    THEN RAISE EXCEPTION 'PUBLICATION_151_RECONCILIATION_FAILED' USING ERRCODE='check_violation'; END IF;
END $$;
INSERT INTO academic_publication_revisions_audit(migration_version,passed,checks)
SELECT 151,true,jsonb_agg(jsonb_build_object('code',code,'count',0,'passed',true) ORDER BY code)
FROM (SELECT code FROM academic_151_preservation UNION ALL SELECT 'publication_numbers_reconciled' UNION ALL SELECT 'projection_integrity_reconciled') evidence;
