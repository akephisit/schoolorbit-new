-- Explicit revision audience intent. Absence is legacy; an empty parent is intentional.
CREATE TABLE academic_delivery_revision_audiences (
    change_set_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    PRIMARY KEY(change_set_id,learning_offering_id),
    UNIQUE(change_set_id,learning_offering_id,academic_term_id,academic_year_id),
    FOREIGN KEY(change_set_id,academic_term_id,academic_year_id)
        REFERENCES academic_term_change_sets(id,academic_term_id,academic_year_id) ON DELETE CASCADE,
    FOREIGN KEY(learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_offerings(id,academic_term_id,academic_year_id) ON DELETE RESTRICT
);
CREATE TABLE academic_delivery_revision_audience_rooms (
    change_set_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    homeroom_id UUID NOT NULL,
    PRIMARY KEY(change_set_id,learning_offering_id,homeroom_id),
    FOREIGN KEY(change_set_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES academic_delivery_revision_audiences(change_set_id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE CASCADE,
    FOREIGN KEY(homeroom_id,academic_year_id) REFERENCES homerooms(id,academic_year_id) ON DELETE RESTRICT
);
CREATE FUNCTION academic_protect_revision_audience() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE revision_id UUID; writable BOOLEAN;
BEGIN
    revision_id:=CASE WHEN TG_OP='DELETE' THEN OLD.change_set_id ELSE NEW.change_set_id END;
    SELECT revision.status='draft' AND version.status='draft' INTO writable
    FROM academic_term_change_sets revision JOIN academic_delivery_versions version ON version.id=revision.target_delivery_version_id
    WHERE revision.id=revision_id FOR UPDATE OF revision,version;
    IF NOT FOUND AND TG_OP='DELETE' THEN RETURN OLD; END IF;
    IF writable IS DISTINCT FROM TRUE OR (TG_OP='UPDATE' AND NEW.change_set_id IS DISTINCT FROM OLD.change_set_id) THEN
        RAISE EXCEPTION 'ACADEMIC_DELIVERY_AUDIENCE_IMMUTABLE' USING ERRCODE='check_violation';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE TRIGGER academic_delivery_revision_audiences_immutable BEFORE INSERT OR UPDATE OR DELETE ON academic_delivery_revision_audiences FOR EACH ROW EXECUTE FUNCTION academic_protect_revision_audience();
CREATE TRIGGER academic_delivery_revision_audience_rooms_immutable BEFORE INSERT OR UPDATE OR DELETE ON academic_delivery_revision_audience_rooms FOR EACH ROW EXECUTE FUNCTION academic_protect_revision_audience();

CREATE FUNCTION academic_delivery_apply_audiences(graph JSONB, revision UUID) RETURNS JSONB LANGUAGE SQL STABLE AS $$
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM academic_delivery_revision_audiences WHERE change_set_id=revision) THEN graph
    ELSE jsonb_set(graph,'{offerings}',COALESCE((
        SELECT jsonb_agg(CASE WHEN scope.learning_offering_id IS NULL THEN entry.offering
            ELSE jsonb_set(jsonb_set(jsonb_set(entry.offering,'{targets}',rooms.targets),'{homeroomIds}',rooms.ids),'{groups}',COALESCE((
                SELECT jsonb_agg(jsonb_set(g.item,'{homeroomIds}',coverage.ids) ORDER BY g.position)
                FROM jsonb_array_elements(entry.offering->'groups') WITH ORDINALITY g(item,position)
                CROSS JOIN LATERAL (SELECT COALESCE(jsonb_agg(room ORDER BY room),'[]'::JSONB) ids
                    FROM jsonb_array_elements(g.item->'homeroomIds') room
                    WHERE rooms.ids @> jsonb_build_array(room)) coverage
                WHERE jsonb_array_length(g.item->'homeroomIds')=0 OR jsonb_array_length(coverage.ids)>0
            ),'[]'::JSONB)) END ORDER BY entry.position)
        FROM jsonb_array_elements(graph->'offerings') WITH ORDINALITY entry(offering,position)
        LEFT JOIN academic_delivery_revision_audiences scope ON scope.change_set_id=revision AND scope.learning_offering_id=(entry.offering->>'id')::UUID
        CROSS JOIN LATERAL (
            SELECT COALESCE(jsonb_agg(to_jsonb(room.id) ORDER BY room.id),'[]'::JSONB) ids,
                COALESCE(jsonb_agg(jsonb_build_object(
                    'id',COALESCE((SELECT (old->>'id')::UUID FROM jsonb_array_elements(entry.offering->'targets') old WHERE old->>'targetKind'='homeroom' AND (old->>'homeroomId')::UUID=room.id ORDER BY old->>'id' LIMIT 1),
                        uuid_generate_v5('83c846da-e34e-5146-8ff1-f7caaa6e20a4'::UUID,'audience-target:'||(entry.offering->>'id')||':'||room.id::TEXT)),
                    'targetKind','homeroom','homeroomId',room.id,'gradeLevelId',room.grade_level_id,'studyProgramId',room.study_program_id) ORDER BY room.id),'[]'::JSONB) targets
            FROM academic_delivery_revision_audience_rooms allowed JOIN homerooms room ON room.id=allowed.homeroom_id
            WHERE allowed.change_set_id=revision AND allowed.learning_offering_id=scope.learning_offering_id
        ) rooms
        WHERE scope.learning_offering_id IS NULL OR jsonb_array_length(rooms.ids)>0
    ),'[]'::JSONB)) END;
$$;

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
    SELECT academic_project_delivery_teacher_changes(academic_capture_delivery_snapshot(
        context.academic_term_id,context.evaluation_date,
        COALESCE((SELECT jsonb_agg(jsonb_build_object('id',id,'weekly_period_target',weekly_period_target) ORDER BY id)
            FROM selected_targets),'[]'::JSONB)),revision) AS graph FROM context
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
