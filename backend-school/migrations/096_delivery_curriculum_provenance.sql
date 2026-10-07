-- Record new preparation sources; never infer a publication for legacy actual graphs.
CREATE TABLE learning_offering_curriculum_sources (
 learning_offering_id uuid NOT NULL REFERENCES learning_offerings(id) ON DELETE CASCADE,
 publication_id uuid NOT NULL REFERENCES curriculum_publications(id) ON DELETE RESTRICT,
 PRIMARY KEY(learning_offering_id,publication_id)
);
CREATE TRIGGER offering_curriculum_sources_no_update BEFORE UPDATE ON learning_offering_curriculum_sources FOR EACH ROW EXECUTE FUNCTION curriculum_history_immutable();
ALTER TABLE learning_delivery_apply_runs ADD COLUMN curriculum_sources jsonb NOT NULL DEFAULT '[]'::jsonb CHECK(jsonb_typeof(curriculum_sources)='array');
CREATE FUNCTION curriculum_capture_offering_source() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE requirement uuid; catalog uuid; expected_kind text; selected_publication uuid;
BEGIN
 expected_kind:=CASE WHEN TG_TABLE_NAME='course_offering_details' THEN 'course' ELSE 'activity' END;
 requirement:=CASE WHEN expected_kind='course' THEN (to_jsonb(NEW)->>'curriculum_course_requirement_id')::uuid ELSE (to_jsonb(NEW)->>'curriculum_activity_requirement_id')::uuid END;
 catalog:=CASE WHEN expected_kind='course' THEN (to_jsonb(NEW)->>'subject_version_id')::uuid ELSE (to_jsonb(NEW)->>'activity_version_id')::uuid END;
 IF requirement IS NULL THEN RETURN NEW; END IF;
 SELECT e.current_publication_id INTO selected_publication FROM curriculum_requirement_sources source
 JOIN curriculum_levels l ON l.id=source.curriculum_level_id JOIN curriculum_editions e ON e.id=l.edition_id
 WHERE source.id=requirement AND source.resource_kind=expected_kind AND source.catalog_version_id=catalog FOR SHARE OF e;
 IF selected_publication IS NULL OR NOT EXISTS(
 SELECT 1 FROM (SELECT id,publication_id FROM curriculum_publication_courses UNION ALL SELECT id,publication_id FROM curriculum_publication_activities) content
 WHERE content.id=requirement AND content.publication_id=selected_publication)
 THEN RAISE EXCEPTION 'CURRICULUM_OFFERING_SOURCE_INVALID'; END IF;
 INSERT INTO learning_offering_curriculum_sources VALUES(NEW.learning_offering_id,selected_publication) ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;
CREATE TRIGGER course_offering_curriculum_source AFTER INSERT ON course_offering_details FOR EACH ROW EXECUTE FUNCTION curriculum_capture_offering_source();
CREATE TRIGGER activity_offering_curriculum_source AFTER INSERT ON activity_offering_details FOR EACH ROW EXECUTE FUNCTION curriculum_capture_offering_source();

CREATE OR REPLACE FUNCTION academic_capture_delivery_snapshot(
    selected_term UUID, selected_date DATE, selected_targets JSONB,
    recorded_before TIMESTAMPTZ DEFAULT NULL
) RETURNS JSONB LANGUAGE SQL STABLE AS $$
    SELECT jsonb_build_object('offerings', COALESCE(jsonb_agg(
        jsonb_build_object(
            'id', offering.id,
            'curriculumSources',COALESCE((SELECT jsonb_agg(jsonb_build_object(
                'editionId',p.edition_id,'publicationId',p.id,'publicationNo',p.publication_no,
                'revisionYear',p.revision_year,'editionName',p.name) ORDER BY p.edition_id,p.publication_no)
                FROM learning_offering_curriculum_sources source JOIN curriculum_publications p ON p.id=source.publication_id
                WHERE source.learning_offering_id=offering.id),'[]'::jsonb),
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
