-- Delivery owns immutable academic graphs; timetable owns placement revisions.
-- This expansion does not mutate legacy identities or their immutable receipts.
CREATE TABLE academic_delivery_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    source_version_id UUID,
    effective_from DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published', 'cancelled')),
    snapshot JSONB NOT NULL,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    published_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    published_at TIMESTAMPTZ,
    publication_idempotency_key UUID,
    publication_request_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_delivery_versions_context_key UNIQUE (id, academic_term_id, academic_year_id),
    CONSTRAINT academic_delivery_versions_term_context_fkey
        FOREIGN KEY (academic_term_id, academic_year_id)
        REFERENCES academic_terms(id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT academic_delivery_versions_source_context_fkey
        FOREIGN KEY (source_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_delivery_versions(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    CONSTRAINT academic_delivery_versions_snapshot_shape_check CHECK (
        jsonb_typeof(snapshot) = 'object'
        AND snapshot ? 'offerings'
        AND jsonb_typeof(snapshot->'offerings') = 'array'
    ),
    CONSTRAINT academic_delivery_versions_publication_check CHECK (
        (status = 'published' AND published_by IS NOT NULL AND published_at IS NOT NULL
         AND publication_idempotency_key IS NOT NULL AND publication_request_hash ~ '^[0-9a-f]{64}$')
        OR (status IN ('draft','cancelled') AND published_by IS NULL AND published_at IS NULL
            AND publication_idempotency_key IS NULL AND publication_request_hash IS NULL)
    )
);

CREATE UNIQUE INDEX academic_delivery_versions_published_effective_key
    ON academic_delivery_versions(academic_term_id, effective_from) WHERE status='published';
CREATE UNIQUE INDEX academic_delivery_versions_publication_idempotency_key
    ON academic_delivery_versions(publication_idempotency_key) WHERE publication_idempotency_key IS NOT NULL;
CREATE INDEX academic_delivery_versions_term_revision_idx
    ON academic_delivery_versions(academic_term_id, effective_from DESC, created_at DESC, id);

CREATE FUNCTION academic_protect_delivery_version() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status IN ('published','cancelled') THEN
        RAISE EXCEPTION 'ACADEMIC_DELIVERY_VERSION_IMMUTABLE' USING ERRCODE='check_violation';
    END IF;
    RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
CREATE TRIGGER academic_delivery_versions_immutable
    BEFORE UPDATE OR DELETE ON academic_delivery_versions
    FOR EACH ROW EXECUTE FUNCTION academic_protect_delivery_version();

-- Only validated IDs and recorded teacher intervals enter a graph. The explicit
-- target input belongs to delivery; this function does not read a timetable.
-- recorded_before limits historical captures to groups that existed at publication.
CREATE FUNCTION academic_capture_delivery_snapshot(
    selected_term UUID, selected_date DATE, selected_targets JSONB,
    recorded_before TIMESTAMPTZ DEFAULT NULL
) RETURNS JSONB LANGUAGE SQL STABLE AS $$
    SELECT jsonb_build_object('offerings', COALESCE(jsonb_agg(
        jsonb_build_object(
            'id', offering.id,
            'kind', offering.kind,
            'code', offering.code_snapshot,
            'name', offering.name_snapshot,
            'owningOrganizationUnitId', offering.owning_organization_unit_id,
            'sourceRequirementKind', offering.source_requirement_kind,
            'sourceRequirementId', offering.source_requirement_id,
            'weeklyPeriodTarget', target.weekly_period_target,
            'catalog', CASE WHEN offering.kind='course' THEN jsonb_build_object(
                'kind', 'course', 'subjectVersionId', course.subject_version_id,
                'subjectId', course.subject_id,
                'curriculumCourseRequirementId', course.curriculum_course_requirement_id,
                'credit', course.credit::TEXT, 'hours', course.hours::TEXT,
                'standardPeriodsPerWeek', subject_version.periods_per_week,
                'assessmentTotalScore', course.assessment_total_score::TEXT
            ) ELSE jsonb_build_object(
                'kind', 'activity', 'activityVersionId', activity.activity_version_id,
                'activityId', activity.activity_id,
                'curriculumActivityRequirementId', activity.curriculum_activity_requirement_id,
                'registrationType', activity.registration_type,
                'schedulingMode', activity.scheduling_mode,
                'hours', activity.hours::TEXT, 'capacity', activity.capacity,
                'attendanceRequirement', activity.attendance_requirement,
                'passCriteria', activity.pass_criteria
            ) END,
            'targets', COALESCE((SELECT jsonb_agg(jsonb_build_object(
                'id', coverage.id, 'targetKind', coverage.target_kind,
                'homeroomId', coverage.homeroom_id,
                'gradeLevelId', coverage.grade_level_id, 'studyProgramId', coverage.study_program_id
            ) ORDER BY coverage.id) FROM learning_offering_targets coverage
                WHERE coverage.learning_offering_id=offering.id), '[]'::JSONB),
            'homeroomIds', COALESCE((SELECT jsonb_agg(homeroom.id ORDER BY homeroom.id)
                FROM homerooms homeroom WHERE homeroom.academic_year_id=offering.academic_year_id
                AND EXISTS(SELECT 1 FROM learning_offering_targets audience
                    WHERE audience.learning_offering_id=offering.id AND (
                        (audience.target_kind='homeroom' AND audience.homeroom_id=homeroom.id)
                        OR (audience.target_kind='grade_program' AND audience.grade_level_id=homeroom.grade_level_id
                            AND audience.study_program_id=homeroom.study_program_id AND homeroom.is_active)))),'[]'::JSONB),
            'groups', COALESCE((SELECT jsonb_agg(jsonb_build_object(
                'id', learning_group.id, 'code', learning_group.code, 'name', learning_group.name,
                'description', learning_group.description, 'capacity', learning_group.capacity,
                'homeroomIds', COALESCE((SELECT jsonb_agg(coverage.homeroom_id ORDER BY coverage.homeroom_id)
                    FROM learning_group_homerooms coverage WHERE coverage.learning_group_id=learning_group.id),'[]'::JSONB),
                'preferredRoomIds', COALESCE((SELECT jsonb_agg(preference.room_id ORDER BY preference.rank, preference.room_id)
                    FROM learning_group_preferred_rooms preference WHERE preference.learning_group_id=learning_group.id),'[]'::JSONB),
                'teachers', COALESCE((SELECT jsonb_agg(jsonb_build_object(
                    'assignmentId', assignment.id, 'teacherId', assignment.teacher_id, 'displayName', concat_ws(' ',teacher.title,teacher.first_name,teacher.last_name), 'role', assignment.role
                ) ORDER BY assignment.teacher_id, assignment.id) FROM learning_group_teachers assignment JOIN users teacher ON teacher.id=assignment.teacher_id
                    WHERE assignment.learning_group_id=learning_group.id
                        AND assignment.starts_on<=selected_date
                        AND (assignment.ends_on IS NULL OR assignment.ends_on>=selected_date)),'[]'::JSONB)
            ) ORDER BY learning_group.id) FROM learning_groups learning_group
                WHERE learning_group.learning_offering_id=offering.id
                  AND learning_group.status<>'cancelled'
                  AND (recorded_before IS NULL OR learning_group.created_at<=recorded_before)), '[]'::JSONB)
        ) ORDER BY offering.id
    ), '[]'::JSONB))
    FROM jsonb_to_recordset(selected_targets) AS target(id UUID, weekly_period_target INTEGER)
    JOIN learning_offerings offering ON offering.id=target.id AND offering.academic_term_id=selected_term
    LEFT JOIN course_offering_details course ON course.learning_offering_id=offering.id
    LEFT JOIN subject_versions subject_version ON subject_version.id=course.subject_version_id
    LEFT JOIN activity_offering_details activity ON activity.learning_offering_id=offering.id;
$$;

-- Pending teacher commands have stable prospective episode identities. Publication
-- materializes these exact IDs, so a draft never guesses a teacher by name.
CREATE FUNCTION academic_project_delivery_teacher_changes(graph JSONB, revision UUID)
RETURNS JSONB LANGUAGE SQL STABLE AS $$
    SELECT jsonb_build_object('offerings',COALESCE(jsonb_agg(
        jsonb_set(offering,'{groups}',COALESCE((SELECT jsonb_agg(
            jsonb_set(source_group,'{teachers}',COALESCE((SELECT jsonb_agg(teacher ORDER BY
                (teacher->>'teacherId')::UUID,(teacher->>'assignmentId')::UUID) FROM (
                SELECT teacher FROM jsonb_array_elements(source_group->'teachers') teacher
                WHERE NOT EXISTS(SELECT 1 FROM academic_term_change_items item
                    WHERE item.change_set_id=revision
                      AND item.learning_group_teacher_id=(teacher->>'assignmentId')::UUID
                      AND item.action_kind IN ('stop_group_teacher','adjust_group_teacher_role'))
                UNION ALL
                SELECT jsonb_build_object('assignmentId',uuid_generate_v5(item.id,'delivery-teacher-episode'),
                    'teacherId',item.teacher_id,'displayName',concat_ws(' ',staff.title,staff.first_name,staff.last_name),'role',item.teacher_role)
                FROM academic_term_change_items item JOIN users staff ON staff.id=item.teacher_id
                WHERE item.change_set_id=revision AND item.learning_group_id=(source_group->>'id')::UUID
                  AND item.action_kind IN ('add_group_teacher','adjust_group_teacher_role')
            ) projected),'[]'::JSONB)) ORDER BY (source_group->>'id')::UUID)
        FROM jsonb_array_elements(offering->'groups') source_group),'[]'::JSONB))
        ORDER BY (offering->>'id')::UUID),'[]'::JSONB))
    FROM jsonb_array_elements(graph->'offerings') offering;
$$;

-- Additive links remain nullable only until the next reconciliation migration.
ALTER TABLE academic_timetable_versions ADD COLUMN delivery_version_id UUID;
ALTER TABLE academic_timetable_versions ADD CONSTRAINT academic_timetable_versions_delivery_context_fkey
    FOREIGN KEY (delivery_version_id, academic_term_id, academic_year_id)
    REFERENCES academic_delivery_versions(id, academic_term_id, academic_year_id) ON DELETE RESTRICT;
ALTER TABLE academic_term_change_sets
    ADD COLUMN base_delivery_version_id UUID,
    ADD COLUMN target_delivery_version_id UUID;
ALTER TABLE academic_term_change_sets
    ADD CONSTRAINT academic_term_change_sets_base_delivery_context_fkey
        FOREIGN KEY (base_delivery_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_delivery_versions(id, academic_term_id, academic_year_id) ON DELETE RESTRICT,
    ADD CONSTRAINT academic_term_change_sets_target_delivery_context_fkey
        FOREIGN KEY (target_delivery_version_id, academic_term_id, academic_year_id)
        REFERENCES academic_delivery_versions(id, academic_term_id, academic_year_id) ON DELETE RESTRICT;

CREATE FUNCTION academic_delivery_revision_snapshot(revision UUID)
RETURNS JSONB LANGUAGE SQL STABLE AS $$
    WITH context AS (
        SELECT c.*,base.snapshot AS base_snapshot FROM academic_term_change_sets c
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
            AND offering.starts_on<=context.effective_from
            AND (offering.ends_on IS NULL OR offering.ends_on>=context.effective_from)
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
        SELECT learning_offering_id,weekly_period_target FROM academic_term_change_items
        WHERE change_set_id=revision AND action_kind='add_offering'
    )
    SELECT academic_project_delivery_teacher_changes(academic_capture_delivery_snapshot(
        context.academic_term_id,context.effective_from,
        COALESCE((SELECT jsonb_agg(jsonb_build_object('id',id,'weekly_period_target',weekly_period_target) ORDER BY id)
            FROM selected_targets),'[]'::JSONB)),revision) FROM context;
$$;
