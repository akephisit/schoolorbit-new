-- Academic Core Phase B: remove the separately reconciled legacy schema.
-- The migration is intentionally destructive and must run while tenant writes
-- remain stopped. Every populated tenant requires the current Phase A marker.

SET LOCAL lock_timeout = '30s';

LOCK TABLE
    academic_core_cutover_audits,
    academic_core_entity_map,
    academic_years,
    academic_terms,
    bell_schedules,
    bell_schedule_periods,
    subjects,
    subject_versions,
    activities,
    activity_versions,
    curricula,
    curriculum_versions,
    study_programs,
    curriculum_course_requirements,
    curriculum_activity_requirements,
    grade_levels,
    homerooms,
    homeroom_advisors,
    student_academic_years,
    homeroom_placements,
    learning_offerings,
    learning_offering_targets,
    course_offering_details,
    activity_offering_details,
    learning_groups,
    learning_group_homerooms,
    learning_group_teachers,
    learning_group_students,
    learning_results,
    activity_result_details,
    course_assessment_plans,
    course_assessment_categories,
    course_assessment_items,
    academic_timetable_entries,
    timetable_entry_instructors,
    academic_exam_rounds,
    academic_exam_days,
    academic_exam_schedule_items,
    academic_exam_sessions,
    academic_exam_day_room_assignments,
    supervision_cycles,
    supervision_observations,
    academic_question_bank_questions,
    academic_question_bank_choices,
    admission_tracks,
    admission_applications,
    admission_room_assignments,
    calendar_events,
    calendar_event_targets,
    certificate_campaigns,
    student_class_enrollments,
    classroom_courses,
    classroom_course_instructors,
    activity_slots,
    activity_slot_classrooms,
    activity_slot_classroom_assignments,
    activity_slot_instructors,
    activity_groups,
    activity_group_instructors,
    activity_group_members,
    permissions,
    role_permissions,
    organization_permission_grants,
    organization_permission_delegations
IN ACCESS EXCLUSIVE MODE;

DO $$
DECLARE
    delivery_audit academic_core_cutover_audits%ROWTYPE;
    mapping_audit academic_core_cutover_audits%ROWTYPE;
    marker_audit academic_core_cutover_audits%ROWTYPE;
    marker_found BOOLEAN;
    populated_tenant BOOLEAN;
    marker_xmin BIGINT;
    current_consumer_counts JSONB;
    current_checksum TEXT;
    expected_mapping_count BIGINT;
    resolved_mapping_count BIGINT;
    relation_name TEXT;
    has_stale_row BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM academic_years
        UNION ALL SELECT 1 FROM academic_terms
        UNION ALL SELECT 1 FROM subject_versions
        UNION ALL SELECT 1 FROM activity_versions
        UNION ALL SELECT 1 FROM curricula
        UNION ALL SELECT 1 FROM homerooms
        UNION ALL SELECT 1 FROM student_class_enrollments
        UNION ALL SELECT 1 FROM classroom_courses
        UNION ALL SELECT 1 FROM activity_slots
        UNION ALL SELECT 1 FROM activity_groups
    ) INTO populated_tenant;

    SELECT * INTO delivery_audit
    FROM academic_core_cutover_audits
    WHERE migration_version = 42
    FOR UPDATE;
    IF NOT FOUND OR delivery_audit.mapping_algorithm_version <> 'academic-core-v1' THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_DELIVERY_AUDIT_INVALID';
    END IF;

    SELECT encode(sha256(convert_to(
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
         FROM student_class_enrollments)
        || '|'
        || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
            FROM classroom_courses)
        || '|'
        || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
            FROM activity_slots)
        || '|'
        || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
            FROM activity_groups)
        || '|'
        || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
            FROM activity_group_members),
        'UTF8'
    )), 'hex') INTO current_checksum;
    IF btrim(delivery_audit.source_checksum) <> current_checksum THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_DELIVERY_CHECKSUM_MISMATCH';
    END IF;

    SELECT encode(sha256(convert_to(
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
         FROM student_academic_years)
        || '|'
        || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
            FROM homeroom_placements)
        || '|'
        || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
            FROM learning_offerings)
        || '|'
        || (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '')
            FROM learning_groups),
        'UTF8'
    )), 'hex') INTO current_checksum;
    IF btrim(delivery_audit.target_checksum) <> current_checksum THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_DELIVERY_CHECKSUM_MISMATCH';
    END IF;

    -- Preserve the public stale-marker error precedence before the deeper
    -- semantic projection checks below. A normal post-marker update has a
    -- newer xmin and can be rejected immediately; the bidirectional checks
    -- remain necessary for deletes and pre-marker transactions.
    SELECT xmin::text::bigint INTO marker_xmin
    FROM academic_core_cutover_audits
    WHERE migration_version = 44;
    marker_found := FOUND;

    IF marker_found THEN
        FOREACH relation_name IN ARRAY ARRAY[
            'academic_years', 'academic_terms', 'subjects', 'subject_versions',
            'activities', 'activity_versions', 'curricula', 'curriculum_versions',
            'study_programs', 'curriculum_course_requirements',
            'curriculum_activity_requirements', 'homerooms', 'student_academic_years',
            'homeroom_placements', 'learning_offerings', 'course_offering_details',
            'activity_offering_details', 'learning_groups', 'learning_group_homerooms',
            'learning_group_teachers', 'learning_group_students', 'learning_results',
            'activity_result_details', 'course_assessment_plans',
            'course_assessment_categories', 'course_assessment_items',
            'academic_timetable_entries', 'timetable_entry_instructors',
            'academic_exam_rounds', 'academic_exam_days', 'academic_exam_schedule_items',
            'academic_exam_sessions', 'academic_exam_day_room_assignments',
            'supervision_cycles', 'supervision_observations',
            'academic_question_bank_questions', 'academic_question_bank_choices',
            'admission_tracks', 'admission_applications', 'admission_room_assignments',
            'calendar_events', 'calendar_event_targets', 'certificate_campaigns',
            'student_class_enrollments', 'classroom_courses',
            'classroom_course_instructors', 'activity_slots',
            'activity_slot_classrooms', 'activity_slot_classroom_assignments',
            'activity_slot_instructors', 'activity_groups',
            'activity_group_instructors', 'activity_group_members'
        ] LOOP
            EXECUTE format(
                'SELECT EXISTS (SELECT 1 FROM %I candidate WHERE candidate.xmin::text::bigint > $1)',
                relation_name
            ) INTO has_stale_row USING marker_xmin;
            IF has_stale_row THEN
                RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_STALE';
            END IF;
        END LOOP;
    END IF;

    -- Migration 042 mapped every legacy delivery row that can be removed in Phase B.
    -- Resolve every one of those mappings against the physical target so deletes,
    -- including deletes committed by a transaction that began before the marker,
    -- cannot be hidden by xmin or by the aggregate delivery checksums.
    SELECT COUNT(*) INTO expected_mapping_count
    FROM academic_core_entity_map
    WHERE migration_version = 42;

    SELECT COUNT(*) INTO resolved_mapping_count
    FROM (
        SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN homerooms target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'homerooms'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN homeroom_advisors target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'homeroom_advisors'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN student_academic_years target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'student_academic_years'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN homeroom_placements target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'homeroom_placements'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN learning_offerings target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'learning_offerings'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN learning_offering_targets target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'learning_offering_targets'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN learning_groups target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'learning_groups'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN learning_group_homerooms target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'learning_group_homerooms'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN learning_group_teachers target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'learning_group_teachers'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN learning_group_students target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'learning_group_students'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN learning_results target ON target.id = map.target_id
        WHERE map.migration_version = 42 AND map.target_table = 'learning_results'
    ) resolved;
    IF resolved_mapping_count <> expected_mapping_count THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    -- Recompute the migration-42 student-year and placement projections from
    -- the still-present source rows. ID-only checks are not sufficient here:
    -- status, dates, classroom, program, and roster fields are all material
    -- data that would otherwise be lost when the source relations are dropped.
    IF EXISTS (
        WITH ranked AS (
            SELECT enrollment.student_id,
                   homeroom.academic_year_id,
                   homeroom.grade_level_id,
                   homeroom.study_program_id,
                   row_number() OVER (
                       PARTITION BY enrollment.student_id, homeroom.academic_year_id
                       ORDER BY enrollment.enrollment_date DESC,
                                enrollment.created_at DESC, enrollment.id DESC
                   ) AS choice_rank,
                   bool_or(enrollment.status = 'active') OVER (
                       PARTITION BY enrollment.student_id, homeroom.academic_year_id
                   ) AS has_active,
                   bool_or(enrollment.status = 'completed') OVER (
                       PARTITION BY enrollment.student_id, homeroom.academic_year_id
                   ) AS has_completed,
                   bool_or(enrollment.status = 'dropped') OVER (
                       PARTITION BY enrollment.student_id, homeroom.academic_year_id
                   ) AS has_dropped,
                   min(enrollment.created_at) OVER (
                       PARTITION BY enrollment.student_id, homeroom.academic_year_id
                   ) AS first_created_at,
                   max(enrollment.updated_at) OVER (
                       PARTITION BY enrollment.student_id, homeroom.academic_year_id
                   ) AS last_updated_at
            FROM student_class_enrollments enrollment
            JOIN homerooms homeroom ON homeroom.id = enrollment.class_room_id
        ), expected AS (
            SELECT uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'student-year:' || ranked.student_id::text || ':'
                           || ranked.academic_year_id::text
                   ) AS id,
                   ranked.student_id,
                   ranked.academic_year_id,
                   ranked.grade_level_id,
                   ranked.study_program_id,
                   CASE
                       WHEN ranked.has_active AND year.status = 'active' THEN 'active'
                       WHEN ranked.has_active AND year.status IN ('planning', 'ready') THEN 'planned'
                       WHEN ranked.has_completed AND year.status IN ('closed', 'archived') THEN 'completed'
                       WHEN ranked.has_dropped THEN 'withdrawn'
                       ELSE 'withdrawn'
                   END AS status,
                   ranked.first_created_at AS created_at,
                   ranked.last_updated_at AS updated_at
            FROM ranked
            JOIN academic_years year ON year.id = ranked.academic_year_id
            WHERE ranked.choice_rank = 1
        )
        SELECT 1
        FROM expected
        LEFT JOIN student_academic_years target ON target.id = expected.id
        WHERE target.id IS NULL
           OR target.student_id IS DISTINCT FROM expected.student_id
           OR target.academic_year_id IS DISTINCT FROM expected.academic_year_id
           OR target.grade_level_id IS DISTINCT FROM expected.grade_level_id
           OR target.study_program_id IS DISTINCT FROM expected.study_program_id
           OR target.created_at IS DISTINCT FROM expected.created_at
           OR target.updated_at IS DISTINCT FROM expected.updated_at
    ) OR EXISTS (
        WITH placement_intervals AS (
            SELECT enrollment.*,
                   homeroom.academic_year_id,
                   year.end_date AS academic_year_end,
                   lead(enrollment.enrollment_date) OVER (
                       PARTITION BY enrollment.student_id, homeroom.academic_year_id
                       ORDER BY enrollment.enrollment_date,
                                enrollment.created_at, enrollment.id
                   ) AS next_start_date
            FROM student_class_enrollments enrollment
            JOIN homerooms homeroom ON homeroom.id = enrollment.class_room_id
            JOIN academic_years year ON year.id = homeroom.academic_year_id
        )
        SELECT 1
        FROM placement_intervals source
        LEFT JOIN homeroom_placements target ON target.id = source.id
        WHERE target.id IS NULL
           OR target.student_academic_year_id IS DISTINCT FROM uuid_generate_v5(
               '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
               'student-year:' || source.student_id::text || ':'
                   || source.academic_year_id::text
           )
           OR target.academic_year_id IS DISTINCT FROM source.academic_year_id
           OR target.homeroom_id IS DISTINCT FROM source.class_room_id
           OR target.start_date IS DISTINCT FROM source.enrollment_date
           OR target.end_date IS DISTINCT FROM CASE
               WHEN source.status = 'active' THEN NULL
               WHEN source.next_start_date IS NOT NULL THEN source.next_start_date - 1
               ELSE source.academic_year_end
           END
           OR target.status IS DISTINCT FROM CASE
               WHEN source.status = 'active' THEN 'current' ELSE 'ended'
           END
           OR target.enrollment_type IS DISTINCT FROM source.enrollment_type
           OR target.class_number IS DISTINCT FROM source.class_number
           OR target.metadata IS DISTINCT FROM COALESCE(source.metadata, '{}'::jsonb)
           OR target.migration_provenance ->> 'legacyStatus'
              IS DISTINCT FROM source.status
           OR target.created_at IS DISTINCT FROM source.created_at
           OR target.updated_at IS DISTINCT FROM source.updated_at
    ) OR EXISTS (
        SELECT 1
        FROM homerooms
        WHERE study_program_id IS DISTINCT FROM uuid_generate_v5(
                  '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                  'program:' || legacy_curriculum_version_id::text
              )
           OR migration_provenance ->> 'legacyCurriculumVersionId'
              IS DISTINCT FROM legacy_curriculum_version_id::text
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    -- Resolve the source side as well. The delivery audit checksum predates
    -- several child relations, and a deleted row has no xmin left to inspect.
    -- Every durable mapping therefore has to resolve in both directions while
    -- the source and canonical relations are protected by the cleanup lock.
    SELECT COUNT(*) INTO resolved_mapping_count
    FROM (
        SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN homerooms source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'class_rooms'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN homeroom_advisors source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'classroom_advisors'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN student_class_enrollments source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'student_class_enrollments'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN classroom_courses source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'classroom_courses'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN classroom_course_instructors source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'classroom_course_instructors'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN activity_slots source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'activity_slots'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN activity_slot_classrooms source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'activity_slot_classrooms'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN activity_slot_classroom_assignments source ON source.id = map.source_id
        WHERE map.migration_version = 42
          AND map.source_table = 'activity_slot_classroom_assignments'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN activity_slot_instructors source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'activity_slot_instructors'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN activity_groups source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'activity_groups'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN activity_group_instructors source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'activity_group_instructors'
        UNION ALL SELECT map.source_table, map.source_id, map.target_table, map.target_id
        FROM academic_core_entity_map map
        JOIN activity_group_members source ON source.id = map.source_id
        WHERE map.migration_version = 42 AND map.source_table = 'activity_group_members'
    ) resolved;
    IF resolved_mapping_count <> expected_mapping_count THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM (
            SELECT 'student_class_enrollments'::text AS source_table, id AS source_id,
                   'homeroom_placements'::text AS target_table
            FROM student_class_enrollments
            UNION ALL SELECT 'classroom_courses', id, 'learning_groups'
            FROM classroom_courses
            UNION ALL SELECT 'classroom_course_instructors', id, 'learning_group_teachers'
            FROM classroom_course_instructors
            UNION ALL SELECT 'activity_slots', id, 'learning_offerings'
            FROM activity_slots
            UNION ALL SELECT 'activity_slot_classrooms', id, 'learning_offering_targets'
            FROM activity_slot_classrooms
            UNION ALL SELECT 'activity_slot_classroom_assignments', id,
                             'learning_group_homerooms'
            FROM activity_slot_classroom_assignments
            UNION ALL SELECT 'activity_slot_instructors', id, 'learning_group_teachers'
            FROM activity_slot_instructors
            UNION ALL SELECT 'activity_groups', id, 'learning_groups'
            FROM activity_groups
            UNION ALL SELECT 'activity_group_instructors', id, 'learning_group_teachers'
            FROM activity_group_instructors
            UNION ALL SELECT 'activity_group_members', id, 'learning_group_students'
            FROM activity_group_members
        ) source
        WHERE NOT EXISTS (
            SELECT 1
            FROM academic_core_entity_map map
            WHERE map.migration_version = 42
              AND map.source_table = source.source_table
              AND map.source_id = source.source_id
              AND map.target_table = source.target_table
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    -- Recompute every source-derived course value that survives Phase B.
    IF EXISTS (
        WITH expected AS (
            SELECT course.academic_semester_id AS academic_term_id,
                   term.academic_year_id,
                   course.subject_id AS subject_version_id,
                   version.subject_id,
                   version.code,
                   version.name_th,
                   stable.owning_organization_unit_id,
                   min(requirement.id::text)::uuid AS requirement_id,
                   min(course.settings::text)::jsonb AS grading_policy,
                   min(course.created_at) AS created_at,
                   max(course.updated_at) AS updated_at
            FROM classroom_courses course
            JOIN homerooms homeroom ON homeroom.id = course.classroom_id
            JOIN academic_terms term ON term.id = course.academic_semester_id
            JOIN subject_versions version ON version.id = course.subject_id
            JOIN subjects stable ON stable.id = version.subject_id
            LEFT JOIN curriculum_course_requirements requirement
              ON requirement.subject_version_id = course.subject_id
             AND requirement.study_program_id = homeroom.study_program_id
             AND requirement.grade_level_id = homeroom.grade_level_id
             AND (
                 academic_normalize_identity(requirement.recommended_term_code)
                     = academic_normalize_identity(term.legacy_term)
                 OR academic_normalize_identity(requirement.recommended_term_code)
                     = academic_normalize_identity(
                         term.migration_provenance ->> 'legacyTerm'
                     )
                 OR academic_normalize_identity(requirement.recommended_term_code)
                     = academic_normalize_identity(term.sequence_no::text)
                 OR academic_normalize_identity(requirement.recommended_term_code)
                     = academic_normalize_identity(term.code)
             )
            GROUP BY course.academic_semester_id, term.academic_year_id,
                     course.subject_id, version.subject_id, version.code,
                     version.name_th, stable.owning_organization_unit_id
        )
        SELECT 1
        FROM expected
        LEFT JOIN learning_offerings offering
          ON offering.id = uuid_generate_v5(
              '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
              'course-offering:' || expected.academic_term_id::text || ':'
                  || expected.subject_version_id::text
          )
        LEFT JOIN course_offering_details detail
          ON detail.learning_offering_id = offering.id
        LEFT JOIN subject_versions version ON version.id = expected.subject_version_id
        WHERE offering.id IS NULL OR detail.learning_offering_id IS NULL
           OR offering.academic_term_id IS DISTINCT FROM expected.academic_term_id
           OR offering.academic_year_id IS DISTINCT FROM expected.academic_year_id
           OR offering.kind IS DISTINCT FROM 'course'
           OR offering.code_snapshot IS DISTINCT FROM expected.code
           OR offering.name_snapshot IS DISTINCT FROM expected.name_th
           OR offering.source_requirement_kind
              IS DISTINCT FROM 'curriculum_course_requirement'
           OR offering.source_requirement_id IS DISTINCT FROM expected.requirement_id
           OR offering.status IS DISTINCT FROM 'published'
           OR offering.published_at IS DISTINCT FROM expected.updated_at
           OR offering.owning_organization_unit_id
              IS DISTINCT FROM expected.owning_organization_unit_id
           OR offering.created_at IS DISTINCT FROM expected.created_at
           OR offering.updated_at IS DISTINCT FROM expected.updated_at
           OR detail.academic_term_id IS DISTINCT FROM expected.academic_term_id
           OR detail.academic_year_id IS DISTINCT FROM expected.academic_year_id
           OR detail.subject_version_id IS DISTINCT FROM expected.subject_version_id
           OR detail.subject_id IS DISTINCT FROM expected.subject_id
           OR detail.curriculum_course_requirement_id
              IS DISTINCT FROM expected.requirement_id
           OR detail.credit IS DISTINCT FROM version.credit
           OR detail.hours IS DISTINCT FROM version.hours_per_semester::numeric(10,2)
           OR detail.grading_policy IS DISTINCT FROM CASE
               WHEN expected.grading_policy ? 'policyCode'
                AND expected.grading_policy ? 'totalScore'
               THEN expected.grading_policy
               ELSE jsonb_build_object(
                   'policyCode', COALESCE(
                       expected.grading_policy ->> 'policyCode', 'legacy_migrated'
                   ),
                   'totalScore', COALESCE(
                       expected.grading_policy ->> 'totalScore', '100.00'
                   ),
                   'passingScore', expected.grading_policy -> 'passingScore'
               )
           END
           OR (
               (NOT expected.grading_policy ? 'policyCode'
                OR NOT expected.grading_policy ? 'totalScore')
               AND detail.migration_provenance -> 'legacyGradingPolicy'
                   IS DISTINCT FROM expected.grading_policy
           )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: course offering projection';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM classroom_courses source
        JOIN homerooms homeroom ON homeroom.id = source.classroom_id
        JOIN subject_versions version ON version.id = source.subject_id
        JOIN academic_terms term ON term.id = source.academic_semester_id
        LEFT JOIN learning_groups target ON target.id = source.id
        WHERE target.id IS NULL
           OR target.learning_offering_id IS DISTINCT FROM uuid_generate_v5(
               '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
               'course-offering:' || source.academic_semester_id::text || ':'
                   || source.subject_id::text
           )
           OR target.academic_term_id IS DISTINCT FROM term.id
           OR target.academic_year_id IS DISTINCT FROM term.academic_year_id
           OR target.code IS DISTINCT FROM homeroom.code || '-' || version.code
           OR target.name IS DISTINCT FROM homeroom.name || ' · ' || version.name_th
           OR target.capacity IS DISTINCT FROM homeroom.capacity
           OR target.status IS DISTINCT FROM 'published'
           OR target.roster_status IS DISTINCT FROM 'published'
           OR target.roster_published_at IS DISTINCT FROM source.updated_at
           OR target.created_at IS DISTINCT FROM source.created_at
           OR target.updated_at IS DISTINCT FROM source.updated_at
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: course group projection';
    END IF;

    IF EXISTS (
        WITH expected AS (
            SELECT uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'offering-target:' || uuid_generate_v5(
                           '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                           'course-offering:' || course.academic_semester_id::text
                               || ':' || course.subject_id::text
                       )::text || ':' || homeroom.id::text
                   ) AS id,
                   uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'course-offering:' || course.academic_semester_id::text
                           || ':' || course.subject_id::text
                   ) AS learning_offering_id,
                   course.academic_semester_id AS academic_term_id,
                   homeroom.academic_year_id,
                   homeroom.id AS homeroom_id,
                   homeroom.grade_level_id,
                   homeroom.study_program_id
            FROM classroom_courses course
            JOIN homerooms homeroom ON homeroom.id = course.classroom_id
        ), actual AS (
            SELECT target.id, target.learning_offering_id,
                   target.academic_term_id, target.academic_year_id,
                   target.homeroom_id, target.grade_level_id,
                   target.study_program_id
            FROM learning_offering_targets target
            JOIN learning_offerings offering ON offering.id = target.learning_offering_id
            WHERE offering.migration_provenance ->> 'source' = 'classroom_courses'
              AND target.migration_provenance ->> 'migration' = '42'
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: course target projection';
    END IF;

    IF EXISTS (
        WITH expected AS (
            SELECT uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'group-homeroom:' || course.id::text || ':'
                           || homeroom.id::text
                   ) AS id,
                   course.id AS learning_group_id,
                   term.id AS academic_term_id,
                   term.academic_year_id,
                   homeroom.id AS homeroom_id,
                   'legacy_classroom_course'::text AS coverage_source
            FROM classroom_courses course
            JOIN homerooms homeroom ON homeroom.id = course.classroom_id
            JOIN academic_terms term ON term.id = course.academic_semester_id
        ), actual AS (
            SELECT target.id, target.learning_group_id, target.academic_term_id,
                   target.academic_year_id, target.homeroom_id,
                   target.coverage_source
            FROM learning_group_homerooms target
            JOIN learning_groups learning_group ON learning_group.id = target.learning_group_id
            WHERE learning_group.migration_provenance ->> 'source' = 'classroom_courses'
              AND target.migration_provenance ->> 'migration' = '42'
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: course coverage projection';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM classroom_course_instructors source
        JOIN classroom_courses course ON course.id = source.classroom_course_id
        JOIN academic_terms term ON term.id = course.academic_semester_id
        LEFT JOIN learning_group_teachers target ON target.id = source.id
        WHERE target.id IS NULL
           OR target.learning_group_id IS DISTINCT FROM source.classroom_course_id
           OR target.academic_term_id IS DISTINCT FROM term.id
           OR target.academic_year_id IS DISTINCT FROM term.academic_year_id
           OR target.teacher_id IS DISTINCT FROM source.instructor_id
           OR target.role IS DISTINCT FROM CASE
               WHEN source.role = 'primary' THEN 'primary' ELSE 'secondary'
           END
           OR target.created_at IS DISTINCT FROM source.created_at
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: course teacher projection';
    END IF;

    IF EXISTS (
        WITH expected AS (
            SELECT uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'course-roster:' || course.id::text || ':'
                           || placement.id::text
                   ) AS id,
                   course.id AS learning_group_id,
                   term.id AS academic_term_id,
                   term.academic_year_id,
                   placement.student_academic_year_id,
                   student_year.student_id,
                   'migration_homeroom_snapshot'::text AS roster_source,
                   GREATEST(placement.start_date, term.start_date) AS joined_at,
                   course.updated_at AS published_at,
                   placement.created_at AS created_at,
                   GREATEST(placement.updated_at, course.updated_at) AS updated_at
            FROM classroom_courses course
            JOIN academic_terms term ON term.id = course.academic_semester_id
            JOIN homeroom_placements placement
              ON placement.homeroom_id = course.classroom_id
            JOIN student_academic_years student_year
              ON student_year.id = placement.student_academic_year_id
            WHERE placement.start_date <= term.end_date
              AND COALESCE(placement.end_date, term.end_date) >= term.start_date
              AND placement.migration_provenance ->> 'legacyStatus'
                  IN ('active', 'completed', 'transferred')
        ), actual AS (
            SELECT target.id, target.learning_group_id, target.academic_term_id,
                   target.academic_year_id, target.student_academic_year_id,
                   target.student_id, target.roster_source, target.joined_at,
                   target.published_at, target.created_at, target.updated_at
            FROM learning_group_students target
            WHERE target.roster_source = 'migration_homeroom_snapshot'
              AND target.migration_provenance ->> 'migration' = '42'
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: course roster projection';
    END IF;

    -- Recompute the activity offering and group projections, including the
    -- source fields that do not participate in any ID checksum.
    IF EXISTS (
        WITH expected AS (
            SELECT slot.id,
                   slot.semester_id AS academic_term_id,
                   term.academic_year_id,
                   slot.activity_catalog_id AS activity_version_id,
                   version.activity_id,
                   stable.code,
                   version.name,
                   stable.owning_organization_unit_id,
                   min(requirement.id::text)::uuid AS requirement_id,
                   slot.registration_type,
                   version.scheduling_mode,
                   version.hours_per_week,
                   slot.created_at,
                   slot.updated_at
            FROM activity_slots slot
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN activity_versions version ON version.id = slot.activity_catalog_id
            JOIN activities stable ON stable.id = version.activity_id
            LEFT JOIN activity_slot_classrooms slot_homeroom
              ON slot_homeroom.slot_id = slot.id
            LEFT JOIN homerooms homeroom ON homeroom.id = slot_homeroom.classroom_id
            LEFT JOIN curriculum_activity_requirements requirement
              ON requirement.activity_version_id = slot.activity_catalog_id
             AND requirement.study_program_id = homeroom.study_program_id
             AND requirement.grade_level_id = homeroom.grade_level_id
             AND (
                 academic_normalize_identity(requirement.recommended_term_code)
                     = academic_normalize_identity(term.legacy_term)
                 OR academic_normalize_identity(requirement.recommended_term_code)
                     = academic_normalize_identity(
                         term.migration_provenance ->> 'legacyTerm'
                     )
                 OR academic_normalize_identity(requirement.recommended_term_code)
                     = academic_normalize_identity(term.sequence_no::text)
                 OR academic_normalize_identity(requirement.recommended_term_code)
                     = academic_normalize_identity(term.code)
             )
            GROUP BY slot.id, slot.semester_id, term.academic_year_id,
                     slot.activity_catalog_id, version.activity_id, stable.code,
                     version.name, stable.owning_organization_unit_id,
                     slot.registration_type, version.scheduling_mode,
                     version.hours_per_week, slot.created_at, slot.updated_at
        )
        SELECT 1
        FROM expected
        LEFT JOIN learning_offerings offering ON offering.id = expected.id
        LEFT JOIN activity_offering_details detail
          ON detail.learning_offering_id = offering.id
        WHERE offering.id IS NULL OR detail.learning_offering_id IS NULL
           OR offering.academic_term_id IS DISTINCT FROM expected.academic_term_id
           OR offering.academic_year_id IS DISTINCT FROM expected.academic_year_id
           OR offering.kind IS DISTINCT FROM 'activity'
           OR offering.code_snapshot IS DISTINCT FROM expected.code
           OR offering.name_snapshot IS DISTINCT FROM expected.name
           OR offering.source_requirement_kind IS DISTINCT FROM CASE
               WHEN expected.requirement_id IS NULL THEN NULL
               ELSE 'curriculum_activity_requirement'
           END
           OR offering.source_requirement_id IS DISTINCT FROM expected.requirement_id
           OR offering.status IS DISTINCT FROM 'published'
           OR offering.published_at IS DISTINCT FROM expected.updated_at
           OR offering.owning_organization_unit_id
              IS DISTINCT FROM expected.owning_organization_unit_id
           OR offering.created_at IS DISTINCT FROM expected.created_at
           OR offering.updated_at IS DISTINCT FROM expected.updated_at
           OR detail.academic_term_id IS DISTINCT FROM expected.academic_term_id
           OR detail.academic_year_id IS DISTINCT FROM expected.academic_year_id
           OR detail.activity_version_id IS DISTINCT FROM expected.activity_version_id
           OR detail.activity_id IS DISTINCT FROM expected.activity_id
           OR detail.curriculum_activity_requirement_id
              IS DISTINCT FROM expected.requirement_id
           OR detail.registration_type IS DISTINCT FROM expected.registration_type
           OR detail.scheduling_mode IS DISTINCT FROM expected.scheduling_mode
           OR detail.hours IS DISTINCT FROM expected.hours_per_week
           OR detail.attendance_requirement IS DISTINCT FROM jsonb_build_object(
               'minimumPercent', NULL,
               'requiredSessions', NULL
           )
           OR detail.pass_criteria IS DISTINCT FROM jsonb_build_object(
               'requireAttendance', false,
               'requireTeacherConfirmation', true,
               'outcomes', jsonb_build_array('pass', 'fail')
           )
           OR detail.migration_provenance -> 'legacyAttendanceRequirement'
              IS DISTINCT FROM '{}'::jsonb
           OR detail.migration_provenance -> 'legacyPassCriteria'
              IS DISTINCT FROM jsonb_build_object(
                  'outcomes', jsonb_build_array('pass', 'fail')
              )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: activity offering projection';
    END IF;

    IF EXISTS (
        WITH expected AS (
            SELECT coverage.id, slot.id AS learning_offering_id,
                   term.id AS academic_term_id, term.academic_year_id,
                   coverage.classroom_id AS homeroom_id,
                   homeroom.grade_level_id, homeroom.study_program_id
            FROM activity_slot_classrooms coverage
            JOIN activity_slots slot ON slot.id = coverage.slot_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN homerooms homeroom ON homeroom.id = coverage.classroom_id
        ), actual AS (
            SELECT map.source_id AS id, target.learning_offering_id,
                   target.academic_term_id, target.academic_year_id,
                   target.homeroom_id, target.grade_level_id,
                   target.study_program_id
            FROM academic_core_entity_map map
            JOIN learning_offering_targets target ON target.id = map.target_id
            WHERE map.migration_version = 42
              AND map.source_table = 'activity_slot_classrooms'
              AND map.target_table = 'learning_offering_targets'
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: activity target projection';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM activity_groups source
        JOIN activity_slots slot ON slot.id = source.slot_id
        JOIN academic_terms term ON term.id = slot.semester_id
        LEFT JOIN learning_groups target ON target.id = source.id
        WHERE target.id IS NULL
           OR target.learning_offering_id IS DISTINCT FROM slot.id
           OR target.academic_term_id IS DISTINCT FROM term.id
           OR target.academic_year_id IS DISTINCT FROM term.academic_year_id
           OR target.code IS DISTINCT FROM
              'ACT-' || upper(substr(replace(source.id::text, '-', ''), 1, 12))
           OR target.name IS DISTINCT FROM source.name
           OR target.description IS DISTINCT FROM source.description
           OR target.capacity IS DISTINCT FROM source.max_capacity
           OR target.status IS DISTINCT FROM CASE
               WHEN source.is_active THEN 'published' ELSE 'closed'
           END
           OR target.roster_status IS DISTINCT FROM CASE
               WHEN source.is_active THEN 'published' ELSE 'closed'
           END
           OR target.roster_published_at IS DISTINCT FROM source.updated_at
           OR target.created_at IS DISTINCT FROM source.created_at
           OR target.updated_at IS DISTINCT FROM source.updated_at
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: activity group projection';
    END IF;

    IF EXISTS (
        WITH uncovered AS (
            SELECT assignment.slot_id, assignment.classroom_id,
                   assignment.created_at, homeroom.name AS homeroom_name,
                   homeroom.capacity, term.id AS academic_term_id,
                   term.academic_year_id
            FROM activity_slot_classroom_assignments assignment
            JOIN activity_slots slot ON slot.id = assignment.slot_id
            JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN homerooms homeroom ON homeroom.id = assignment.classroom_id
            WHERE activity.scheduling_mode = 'independent'
              AND NOT EXISTS (
                  SELECT 1 FROM activity_groups activity_group
                  WHERE activity_group.slot_id = assignment.slot_id
                    AND (
                        (activity_group.allowed_classroom_ids IS NOT NULL
                         AND activity_group.allowed_classroom_ids
                             ? assignment.classroom_id::text)
                        OR (
                            activity_group.allowed_classroom_ids IS NULL
                            AND (SELECT COUNT(*) FROM activity_groups sibling
                                 WHERE sibling.slot_id = assignment.slot_id) = 1
                            AND (SELECT COUNT(*)
                                 FROM activity_slot_classroom_assignments sibling
                                 WHERE sibling.slot_id = assignment.slot_id) = 1
                        )
                    )
              )
        ), expected AS (
            SELECT uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'activity-group:' || slot_id::text || ':' || classroom_id::text
                   ) AS id,
                   slot_id AS learning_offering_id, academic_term_id,
                   academic_year_id,
                   'ACT-' || upper(substr(replace(slot_id::text, '-', ''), 1, 6))
                       || '-' || upper(substr(replace(classroom_id::text, '-', ''), 1, 6))
                       AS code,
                   'กิจกรรม · ' || homeroom_name AS name,
                   capacity, created_at
            FROM uncovered
        ), actual AS (
            SELECT target.id, target.learning_offering_id,
                   target.academic_term_id, target.academic_year_id,
                   target.code, target.name, target.capacity, target.created_at
            FROM learning_groups target
            WHERE target.migration_provenance ->> 'source'
                  = 'activity_slot_classroom_assignments'
              AND target.migration_provenance ->> 'generated' = 'true'
              AND target.status = 'published'
              AND target.roster_status = 'published'
              AND target.roster_published_at = target.created_at
              AND target.updated_at = target.created_at
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED: generated activity group projection';
    END IF;

    IF EXISTS (
        WITH expected AS (
            SELECT activity_group.id AS learning_group_id,
                   term.id AS academic_term_id, term.academic_year_id,
                   coverage.classroom_id AS homeroom_id,
                   'legacy_activity_slot'::text AS coverage_source
            FROM activity_groups activity_group
            JOIN activity_slots slot ON slot.id = activity_group.slot_id
            JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN activity_slot_classrooms coverage ON coverage.slot_id = slot.id
            WHERE activity.scheduling_mode = 'synchronized'
              AND (activity_group.allowed_classroom_ids IS NULL
                   OR activity_group.allowed_classroom_ids ? coverage.classroom_id::text)
            UNION
            SELECT activity_group.id, term.id, term.academic_year_id,
                   assignment.classroom_id,
                   'legacy_activity_assignment'::text
            FROM activity_groups activity_group
            JOIN activity_slots slot ON slot.id = activity_group.slot_id
            JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN activity_slot_classroom_assignments assignment
              ON assignment.slot_id = slot.id
            WHERE activity.scheduling_mode = 'independent'
              AND (
                  (activity_group.allowed_classroom_ids IS NOT NULL
                   AND activity_group.allowed_classroom_ids ? assignment.classroom_id::text)
                  OR (
                      activity_group.allowed_classroom_ids IS NULL
                      AND (SELECT COUNT(*) FROM activity_groups sibling
                           WHERE sibling.slot_id = slot.id) = 1
                      AND (SELECT COUNT(*)
                           FROM activity_slot_classroom_assignments sibling
                           WHERE sibling.slot_id = slot.id) = 1
                  )
              )
            UNION
            SELECT generated.id, term.id, term.academic_year_id,
                   assignment.classroom_id,
                   'generated_independent_assignment'::text
            FROM activity_slot_classroom_assignments assignment
            JOIN activity_slots slot ON slot.id = assignment.slot_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN learning_groups generated
              ON generated.id = uuid_generate_v5(
                  '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                  'activity-group:' || assignment.slot_id::text || ':'
                      || assignment.classroom_id::text
              )
        ), actual AS (
            SELECT target.learning_group_id, target.academic_term_id,
                   target.academic_year_id, target.homeroom_id,
                   target.coverage_source
            FROM learning_group_homerooms target
            JOIN learning_groups learning_group
              ON learning_group.id = target.learning_group_id
            JOIN learning_offerings offering
              ON offering.id = learning_group.learning_offering_id
            WHERE offering.kind = 'activity'
              AND target.migration_provenance ->> 'migration' = '42'
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) OR EXISTS (
        WITH candidates AS (
            SELECT 1 AS priority, source.id,
                   source.activity_group_id AS learning_group_id,
                   term.id AS academic_term_id, term.academic_year_id,
                   source.instructor_id AS teacher_id, source.role::text AS role
            FROM activity_group_instructors source
            JOIN activity_groups activity_group
              ON activity_group.id = source.activity_group_id
            JOIN activity_slots slot ON slot.id = activity_group.slot_id
            JOIN academic_terms term ON term.id = slot.semester_id
            UNION ALL
            SELECT 2, uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'activity-group-teacher:' || activity_group.id::text || ':'
                           || activity_group.instructor_id::text
                   ),
                   activity_group.id, term.id, term.academic_year_id,
                   activity_group.instructor_id, 'primary'::text
            FROM activity_groups activity_group
            JOIN activity_slots slot ON slot.id = activity_group.slot_id
            JOIN academic_terms term ON term.id = slot.semester_id
            WHERE activity_group.instructor_id IS NOT NULL
            UNION ALL
            SELECT 3, uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'activity-slot-teacher:' || source.id::text || ':'
                           || activity_group.id::text
                   ),
                   activity_group.id, term.id, term.academic_year_id,
                   source.user_id, 'assistant'::text
            FROM activity_slot_instructors source
            JOIN activity_slots slot ON slot.id = source.slot_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN learning_groups activity_group
              ON activity_group.learning_offering_id = slot.id
            UNION ALL
            SELECT 4, assignment.id, coverage.learning_group_id,
                   term.id, term.academic_year_id,
                   assignment.instructor_id, 'primary'::text
            FROM activity_slot_classroom_assignments assignment
            JOIN activity_slots slot ON slot.id = assignment.slot_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN learning_group_homerooms coverage
              ON coverage.homeroom_id = assignment.classroom_id
            JOIN learning_groups activity_group
              ON activity_group.id = coverage.learning_group_id
             AND activity_group.learning_offering_id = assignment.slot_id
        ), expected AS (
            SELECT DISTINCT ON (learning_group_id, teacher_id)
                   id, learning_group_id, academic_term_id, academic_year_id,
                   teacher_id, role
            FROM candidates
            ORDER BY learning_group_id, teacher_id, priority, id
        ), actual AS (
            SELECT target.id, target.learning_group_id, target.academic_term_id,
                   target.academic_year_id, target.teacher_id, target.role
            FROM learning_group_teachers target
            JOIN learning_groups learning_group
              ON learning_group.id = target.learning_group_id
            JOIN learning_offerings offering
              ON offering.id = learning_group.learning_offering_id
            WHERE offering.kind = 'activity'
              AND target.migration_provenance ->> 'migration' = '42'
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) OR EXISTS (
        WITH expected AS (
            SELECT member.id, member.activity_group_id AS learning_group_id,
                   term.id AS academic_term_id, term.academic_year_id,
                   student_year.id AS student_academic_year_id,
                   member.student_id,
                   'legacy_activity_member'::text AS roster_source,
                   GREATEST(member.enrolled_at::date, term.start_date) AS joined_at,
                   activity_group.updated_at AS published_at,
                   member.enrolled_at AS created_at,
                   activity_group.updated_at AS updated_at
            FROM activity_group_members member
            JOIN activity_groups activity_group
              ON activity_group.id = member.activity_group_id
            JOIN activity_slots slot ON slot.id = activity_group.slot_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN student_academic_years student_year
              ON student_year.student_id = member.student_id
             AND student_year.academic_year_id = term.academic_year_id
        ), actual AS (
            SELECT target.id, target.learning_group_id, target.academic_term_id,
                   target.academic_year_id, target.student_academic_year_id,
                   target.student_id, target.roster_source, target.joined_at,
                   target.published_at, target.created_at, target.updated_at
            FROM learning_group_students target
            WHERE target.roster_source = 'legacy_activity_member'
              AND target.migration_provenance ->> 'migration' = '42'
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) OR EXISTS (
        WITH expected AS (
            SELECT uuid_generate_v5(
                       '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                       'activity-result:' || member.id::text
                   ) AS id,
                   slot.id AS learning_offering_id,
                   activity_group.id AS learning_group_id,
                   term.id AS academic_term_id, term.academic_year_id,
                   student_year.id AS student_academic_year_id,
                   member.student_id, 'activity'::text AS kind,
                   'recorded'::text AS status, member.result::text AS outcome,
                   member.enrolled_at AS created_at,
                   activity_group.updated_at AS updated_at
            FROM activity_group_members member
            JOIN activity_groups activity_group
              ON activity_group.id = member.activity_group_id
            JOIN activity_slots slot ON slot.id = activity_group.slot_id
            JOIN academic_terms term ON term.id = slot.semester_id
            JOIN student_academic_years student_year
              ON student_year.student_id = member.student_id
             AND student_year.academic_year_id = term.academic_year_id
            WHERE member.result IS NOT NULL
        ), actual AS (
            SELECT target.id, target.learning_offering_id,
                   target.learning_group_id, target.academic_term_id,
                   target.academic_year_id, target.student_academic_year_id,
                   target.student_id, target.kind, target.status,
                   detail.outcome, target.created_at, target.updated_at
            FROM learning_results target
            JOIN activity_result_details detail
              ON detail.learning_result_id = target.id
            WHERE target.migration_provenance ->> 'source'
                  = 'activity_group_members'
              AND target.migration_provenance ->> 'migration' = '42'
        )
        SELECT 1 FROM (
            (SELECT * FROM expected EXCEPT SELECT * FROM actual)
            UNION ALL
            (SELECT * FROM actual EXCEPT SELECT * FROM expected)
        ) mismatch
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    -- Revalidate expanded migration-42 relationships that cannot have a
    -- one-source/one-target entity-map row. These checks compare the current
    -- source meaning with the canonical rows, so a long-running transaction
    -- cannot commit a destructive source/target drift behind the marker.
    IF EXISTS (
        SELECT 1
        FROM classroom_courses source
        WHERE NOT EXISTS (
            SELECT 1
            FROM learning_group_homerooms target
            WHERE target.learning_group_id = source.id
              AND target.homeroom_id = source.classroom_id
              AND target.academic_term_id = source.academic_semester_id
        )
    ) OR EXISTS (
        SELECT 1
        FROM classroom_courses course
        JOIN academic_terms term ON term.id = course.academic_semester_id
        JOIN homeroom_placements placement ON placement.homeroom_id = course.classroom_id
        JOIN student_academic_years student_year
          ON student_year.id = placement.student_academic_year_id
        WHERE placement.start_date <= term.end_date
          AND COALESCE(placement.end_date, term.end_date) >= term.start_date
          AND (placement.migration_provenance ->> 'legacyStatus')
              IN ('active', 'completed', 'transferred')
          AND NOT EXISTS (
              SELECT 1
              FROM learning_group_students target
              WHERE target.learning_group_id = course.id
                AND target.student_academic_year_id = placement.student_academic_year_id
                AND target.student_id = student_year.student_id
          )
    ) OR EXISTS (
        SELECT 1
        FROM classroom_course_instructors source
        WHERE NOT EXISTS (
            SELECT 1 FROM learning_group_teachers target
            WHERE target.learning_group_id = source.classroom_course_id
              AND target.teacher_id = source.instructor_id
        )
    ) OR EXISTS (
        SELECT 1
        FROM activity_group_instructors source
        WHERE NOT EXISTS (
            SELECT 1 FROM learning_group_teachers target
            WHERE target.learning_group_id = source.activity_group_id
              AND target.teacher_id = source.instructor_id
        )
    ) OR EXISTS (
        SELECT 1
        FROM activity_groups source
        WHERE source.instructor_id IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM learning_group_teachers target
              WHERE target.learning_group_id = source.id
                AND target.teacher_id = source.instructor_id
          )
    ) OR EXISTS (
        SELECT 1
        FROM activity_slot_instructors source
        JOIN learning_groups activity_group
          ON activity_group.learning_offering_id = source.slot_id
        WHERE NOT EXISTS (
            SELECT 1 FROM learning_group_teachers target
            WHERE target.learning_group_id = activity_group.id
              AND target.teacher_id = source.user_id
        )
    ) OR EXISTS (
        SELECT 1
        FROM activity_slot_classroom_assignments source
        JOIN learning_group_homerooms coverage
          ON coverage.homeroom_id = source.classroom_id
        JOIN learning_groups activity_group
          ON activity_group.id = coverage.learning_group_id
         AND activity_group.learning_offering_id = source.slot_id
        WHERE NOT EXISTS (
            SELECT 1 FROM learning_group_teachers target
            WHERE target.learning_group_id = activity_group.id
              AND target.teacher_id = source.instructor_id
        )
    ) OR EXISTS (
        SELECT 1
        FROM activity_group_members source
        WHERE NOT EXISTS (
            SELECT 1 FROM learning_group_students target
            WHERE target.id = source.id
              AND target.learning_group_id = source.activity_group_id
              AND target.student_id = source.student_id
        )
           OR (source.result IS NOT NULL AND NOT EXISTS (
               SELECT 1
               FROM academic_core_entity_map map
               JOIN activity_result_details detail ON detail.learning_result_id = map.target_id
               WHERE map.migration_version = 42
                 AND map.source_table = 'activity_group_members'
                 AND map.source_id = source.id
                 AND map.target_table = 'learning_results'
                 AND detail.outcome = source.result
           ))
    ) OR EXISTS (
        SELECT 1
        FROM activity_groups activity_group
        JOIN activity_slots slot ON slot.id = activity_group.slot_id
        JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
        JOIN activity_slot_classrooms coverage ON coverage.slot_id = slot.id
        WHERE activity.scheduling_mode = 'synchronized'
          AND (activity_group.allowed_classroom_ids IS NULL
               OR activity_group.allowed_classroom_ids ? coverage.classroom_id::text)
          AND NOT EXISTS (
              SELECT 1 FROM learning_group_homerooms target
              WHERE target.learning_group_id = activity_group.id
                AND target.homeroom_id = coverage.classroom_id
          )
    ) OR EXISTS (
        SELECT 1
        FROM activity_groups activity_group
        JOIN activity_slots slot ON slot.id = activity_group.slot_id
        JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
        JOIN activity_slot_classroom_assignments assignment
          ON assignment.slot_id = slot.id
        WHERE activity.scheduling_mode = 'independent'
          AND (
              (activity_group.allowed_classroom_ids IS NOT NULL
               AND activity_group.allowed_classroom_ids ? assignment.classroom_id::text)
              OR (
                  activity_group.allowed_classroom_ids IS NULL
                  AND (SELECT COUNT(*) FROM activity_groups sibling
                       WHERE sibling.slot_id = slot.id) = 1
                  AND (SELECT COUNT(*) FROM activity_slot_classroom_assignments sibling
                       WHERE sibling.slot_id = slot.id) = 1
              )
          )
          AND NOT EXISTS (
              SELECT 1 FROM learning_group_homerooms target
              WHERE target.learning_group_id = activity_group.id
                AND target.homeroom_id = assignment.classroom_id
          )
    ) OR EXISTS (
        SELECT 1
        FROM activity_slot_classroom_assignments assignment
        JOIN activity_slots slot ON slot.id = assignment.slot_id
        JOIN activity_versions activity ON activity.id = slot.activity_catalog_id
        WHERE activity.scheduling_mode = 'independent'
          AND NOT EXISTS (
              SELECT 1
              FROM activity_groups source_group
              WHERE source_group.slot_id = assignment.slot_id
                AND (
                    (source_group.allowed_classroom_ids IS NOT NULL
                     AND source_group.allowed_classroom_ids ? assignment.classroom_id::text)
                    OR (
                        source_group.allowed_classroom_ids IS NULL
                        AND (SELECT COUNT(*) FROM activity_groups sibling
                             WHERE sibling.slot_id = assignment.slot_id) = 1
                        AND (SELECT COUNT(*) FROM activity_slot_classroom_assignments sibling
                             WHERE sibling.slot_id = assignment.slot_id) = 1
                    )
                )
          )
          AND NOT EXISTS (
            SELECT 1
            FROM learning_groups generated
            JOIN learning_group_homerooms target
              ON target.learning_group_id = generated.id
             AND target.homeroom_id = assignment.classroom_id
            WHERE generated.id = uuid_generate_v5(
                '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
                'activity-group:' || assignment.slot_id::text || ':'
                    || assignment.classroom_id::text
            )
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    SELECT * INTO mapping_audit
    FROM academic_core_cutover_audits
    WHERE migration_version = 43
    FOR UPDATE;
    IF NOT FOUND
       OR mapping_audit.mapping_algorithm_version <> 'academic-core-v1'
       OR mapping_audit.source_counts <> mapping_audit.target_counts
       OR (SELECT COUNT(*) FROM jsonb_object_keys(mapping_audit.source_counts)) <> 20
       OR NOT mapping_audit.source_counts ?& ARRAY[
           'assessmentPlans', 'assessmentCategories', 'assessmentItems',
           'timetableEntries', 'timetableInstructors', 'examRounds', 'examDays',
           'examItems', 'examSessions', 'examRoomAssignments', 'supervisionCycles',
           'supervisionObservations', 'questions', 'questionChoices',
           'admissionTracks', 'admissionApplications', 'admissionAssignments',
           'calendarEvents', 'calendarTargets', 'certificateCampaigns'
       ] THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_MAPPING_AUDIT_INVALID';
    END IF;

    current_consumer_counts := jsonb_build_object(
        'assessmentPlans', (SELECT COUNT(*) FROM course_assessment_plans),
        'assessmentCategories', (SELECT COUNT(*) FROM course_assessment_categories),
        'assessmentItems', (SELECT COUNT(*) FROM course_assessment_items),
        'timetableEntries', (SELECT COUNT(*) FROM academic_timetable_entries),
        'timetableInstructors', (SELECT COUNT(*) FROM timetable_entry_instructors),
        'examRounds', (SELECT COUNT(*) FROM academic_exam_rounds),
        'examDays', (SELECT COUNT(*) FROM academic_exam_days),
        'examItems', (SELECT COUNT(*) FROM academic_exam_schedule_items),
        'examSessions', (SELECT COUNT(*) FROM academic_exam_sessions),
        'examRoomAssignments', (SELECT COUNT(*) FROM academic_exam_day_room_assignments),
        'supervisionCycles', (SELECT COUNT(*) FROM supervision_cycles),
        'supervisionObservations', (SELECT COUNT(*) FROM supervision_observations),
        'questions', (SELECT COUNT(*) FROM academic_question_bank_questions),
        'questionChoices', (SELECT COUNT(*) FROM academic_question_bank_choices),
        'admissionTracks', (SELECT COUNT(*) FROM admission_tracks),
        'admissionApplications', (SELECT COUNT(*) FROM admission_applications),
        'admissionAssignments', (SELECT COUNT(*) FROM admission_room_assignments),
        'calendarEvents', (SELECT COUNT(*) FROM calendar_events),
        'calendarTargets', (SELECT COUNT(*) FROM calendar_event_targets),
        'certificateCampaigns', (SELECT COUNT(*) FROM certificate_campaigns)
    );
    IF current_consumer_counts <> mapping_audit.source_counts THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    SELECT COALESCE(SUM(value::bigint), 0)
    INTO expected_mapping_count
    FROM jsonb_each_text(mapping_audit.source_counts);
    IF (SELECT COUNT(*) FROM academic_core_entity_map WHERE migration_version = 43)
       <> expected_mapping_count THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    SELECT COUNT(*) INTO resolved_mapping_count
    FROM (
        SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN course_assessment_plans target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'course_assessment_plans'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN course_assessment_categories target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'course_assessment_categories'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN course_assessment_items target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'course_assessment_items'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN academic_timetable_entries target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'academic_timetable_entries'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN timetable_entry_instructors target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'timetable_entry_instructors'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN academic_exam_rounds target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'academic_exam_rounds'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN academic_exam_days target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'academic_exam_days'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN academic_exam_schedule_items target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'academic_exam_schedule_items'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN academic_exam_sessions target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'academic_exam_sessions'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN academic_exam_day_room_assignments target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'academic_exam_day_room_assignments'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN supervision_cycles target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'supervision_cycles'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN supervision_observations target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'supervision_observations'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN academic_question_bank_questions target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'academic_question_bank_questions'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN academic_question_bank_choices target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'academic_question_bank_choices'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN admission_tracks target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'admission_tracks'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN admission_applications target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'admission_applications'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN admission_room_assignments target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'admission_room_assignments'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN calendar_events target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'calendar_events'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN calendar_event_targets target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'calendar_event_targets'
        UNION ALL SELECT map.source_table, map.source_id
        FROM academic_core_entity_map map
        JOIN certificate_campaigns target ON target.id = map.target_id
        WHERE map.migration_version = 43 AND map.target_table = 'certificate_campaigns'
    ) resolved;
    IF resolved_mapping_count <> expected_mapping_count THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    IF EXISTS (
        SELECT 1 FROM course_assessment_plans plan
        LEFT JOIN learning_offerings offering ON offering.id = plan.learning_offering_id
        LEFT JOIN course_offering_details detail
          ON detail.learning_offering_id = plan.learning_offering_id
        WHERE offering.id IS NULL OR detail.learning_offering_id IS NULL
           OR offering.kind <> 'course'
           OR offering.academic_term_id <> plan.academic_term_id
           OR offering.academic_year_id <> plan.academic_year_id
           OR detail.subject_version_id <> plan.subject_version_id
    ) OR EXISTS (
        SELECT 1 FROM academic_timetable_entries entry
        LEFT JOIN academic_terms term ON term.id = entry.academic_term_id
        LEFT JOIN learning_groups learning_group ON learning_group.id = entry.learning_group_id
        LEFT JOIN learning_offerings offering ON offering.id = entry.learning_offering_id
        WHERE term.id IS NULL OR term.academic_year_id <> entry.academic_year_id
           OR (entry.learning_group_id IS NOT NULL AND (
               learning_group.id IS NULL
               OR learning_group.learning_offering_id IS DISTINCT FROM entry.learning_offering_id
               OR learning_group.academic_term_id <> entry.academic_term_id
               OR learning_group.academic_year_id <> entry.academic_year_id
           ))
           OR (entry.learning_offering_id IS NOT NULL AND (
               offering.id IS NULL
               OR offering.academic_term_id <> entry.academic_term_id
               OR offering.academic_year_id <> entry.academic_year_id
           ))
    ) OR EXISTS (
        SELECT 1 FROM academic_exam_schedule_items item
        LEFT JOIN learning_groups learning_group ON learning_group.id = item.learning_group_id
        WHERE learning_group.id IS NULL
           OR learning_group.learning_offering_id <> item.learning_offering_id
           OR learning_group.academic_term_id <> item.academic_term_id
           OR learning_group.academic_year_id <> item.academic_year_id
    ) OR EXISTS (
        SELECT 1 FROM supervision_cycles cycle
        LEFT JOIN academic_years year ON year.id = cycle.academic_year_id
        LEFT JOIN academic_terms term ON term.id = cycle.academic_term_id
        WHERE year.id IS NULL
           OR (term.id IS NOT NULL AND term.academic_year_id <> cycle.academic_year_id)
    ) OR EXISTS (
        SELECT 1 FROM admission_room_assignments assignment
        WHERE assignment.student_id IS NOT NULL AND (
            assignment.student_academic_year_id IS NULL
            OR assignment.homeroom_placement_id IS NULL
        )
    ) OR (SELECT COUNT(*) FROM academic_years WHERE status = 'active') > 1
      OR (SELECT COUNT(*) FROM academic_terms WHERE status = 'active') > 1
      OR EXISTS (
          SELECT 1 FROM academic_terms term
          LEFT JOIN academic_years year ON year.id = term.academic_year_id
          WHERE term.status = 'active' AND (year.id IS NULL OR year.status <> 'active')
      )
    THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_FAILED';
    END IF;

    SELECT encode(sha256(convert_to(
        COALESCE(string_agg(source_table || ':' || source_id::text, ','
                            ORDER BY source_table, source_id), ''),
        'UTF8'
    )), 'hex') INTO current_checksum
    FROM academic_core_entity_map
    WHERE migration_version = 43;
    IF btrim(mapping_audit.source_checksum) <> current_checksum THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_MAPPING_CHECKSUM_MISMATCH';
    END IF;

    SELECT encode(sha256(convert_to(
        COALESCE(string_agg(target_table || ':' || target_id::text, ','
                            ORDER BY target_table, target_id), ''),
        'UTF8'
    )), 'hex') INTO current_checksum
    FROM academic_core_entity_map
    WHERE migration_version = 43;
    IF btrim(mapping_audit.target_checksum) <> current_checksum THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_MAPPING_CHECKSUM_MISMATCH';
    END IF;

    SELECT * INTO marker_audit
    FROM academic_core_cutover_audits
    WHERE migration_version = 44
    FOR UPDATE;
    marker_found := FOUND;

    IF populated_tenant AND NOT marker_found THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_MARKER_MISSING';
    END IF;

    IF marker_found THEN
        IF marker_audit.mapping_algorithm_version <> 'academic-core-v1-reconciliation'
           OR (SELECT COUNT(*) FROM jsonb_object_keys(marker_audit.source_counts)) <> 6
           OR (SELECT COUNT(*) FROM jsonb_object_keys(marker_audit.target_counts)) <> 6
           OR NOT marker_audit.source_counts ?& ARRAY[
               'ACADEMIC_CORE_RECON_SOURCE_TARGET_COUNTS',
               'ACADEMIC_CORE_RECON_ORPHAN_COUNTS',
               'ACADEMIC_CORE_RECON_CROSS_CONTEXT_COUNTS',
               'ACADEMIC_CORE_RECON_PERMISSION_PRINCIPAL_COUNTS',
               'ACADEMIC_CORE_RECON_ACTIVE_STATE_UNIQUENESS',
               'ACADEMIC_CORE_RECON_SORTED_ID_CHECKSUMS'
           ]
           OR NOT marker_audit.target_counts ?& ARRAY[
               'ACADEMIC_CORE_RECON_SOURCE_TARGET_COUNTS',
               'ACADEMIC_CORE_RECON_ORPHAN_COUNTS',
               'ACADEMIC_CORE_RECON_CROSS_CONTEXT_COUNTS',
               'ACADEMIC_CORE_RECON_PERMISSION_PRINCIPAL_COUNTS',
               'ACADEMIC_CORE_RECON_ACTIVE_STATE_UNIQUENESS',
               'ACADEMIC_CORE_RECON_SORTED_ID_CHECKSUMS'
           ]
           OR EXISTS (
               SELECT 1 FROM jsonb_each(marker_audit.source_counts)
               WHERE jsonb_typeof(value) <> 'number' OR value::text !~ '^-?[0-9]+$'
           )
           OR EXISTS (
               SELECT 1 FROM jsonb_each(marker_audit.target_counts)
               WHERE jsonb_typeof(value) <> 'number' OR value::text !~ '^-?[0-9]+$'
           ) THEN
            RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_MARKER_INVALID';
        END IF;

        -- A successful reconciliation does not require both aggregate maps to be
        -- identical. In particular, ACTIVE_STATE_UNIQUENESS records the bound (2)
        -- as its source and the actual active row count (0..2) as its target.
        IF (marker_audit.source_counts ->> 'ACADEMIC_CORE_RECON_SOURCE_TARGET_COUNTS')::bigint
              <> (marker_audit.target_counts ->> 'ACADEMIC_CORE_RECON_SOURCE_TARGET_COUNTS')::bigint
           OR (marker_audit.source_counts ->> 'ACADEMIC_CORE_RECON_ORPHAN_COUNTS')::bigint <> 0
           OR (marker_audit.target_counts ->> 'ACADEMIC_CORE_RECON_ORPHAN_COUNTS')::bigint <> 0
           OR (marker_audit.source_counts ->> 'ACADEMIC_CORE_RECON_CROSS_CONTEXT_COUNTS')::bigint <> 0
           OR (marker_audit.target_counts ->> 'ACADEMIC_CORE_RECON_CROSS_CONTEXT_COUNTS')::bigint <> 0
           OR (marker_audit.source_counts ->> 'ACADEMIC_CORE_RECON_PERMISSION_PRINCIPAL_COUNTS')::bigint
              <> (marker_audit.target_counts ->> 'ACADEMIC_CORE_RECON_PERMISSION_PRINCIPAL_COUNTS')::bigint
           OR (marker_audit.source_counts ->> 'ACADEMIC_CORE_RECON_ACTIVE_STATE_UNIQUENESS')::bigint <> 2
           OR (marker_audit.target_counts ->> 'ACADEMIC_CORE_RECON_ACTIVE_STATE_UNIQUENESS')::bigint NOT BETWEEN 0 AND 2
           OR (marker_audit.source_counts ->> 'ACADEMIC_CORE_RECON_SORTED_ID_CHECKSUMS')::bigint <> 1
           OR (marker_audit.target_counts ->> 'ACADEMIC_CORE_RECON_SORTED_ID_CHECKSUMS')::bigint <> 1 THEN
            RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_MARKER_INVALID';
        END IF;

        SELECT encode(sha256(convert_to(
            '{' || COALESCE(string_agg(
                to_jsonb(key)::text || ':' || value, ',' ORDER BY key
            ), '') || '}',
            'UTF8'
        )), 'hex') INTO current_checksum
        FROM jsonb_each_text(marker_audit.source_counts);
        IF btrim(marker_audit.source_checksum) <> current_checksum THEN
            RAISE EXCEPTION 'ACADEMIC_CORE_045_MARKER_CHECKSUM_MISMATCH';
        END IF;

        SELECT encode(sha256(convert_to(
            '{' || COALESCE(string_agg(
                to_jsonb(key)::text || ':' || value, ',' ORDER BY key
            ), '') || '}',
            'UTF8'
        )), 'hex') INTO current_checksum
        FROM jsonb_each_text(marker_audit.target_counts);
        IF btrim(marker_audit.target_checksum) <> current_checksum THEN
            RAISE EXCEPTION 'ACADEMIC_CORE_045_MARKER_CHECKSUM_MISMATCH';
        END IF;

        SELECT xmin::text::bigint INTO marker_xmin
        FROM academic_core_cutover_audits
        WHERE migration_version = 44;

        FOREACH relation_name IN ARRAY ARRAY[
            'academic_years', 'academic_terms', 'subjects', 'subject_versions',
            'activities', 'activity_versions', 'curricula', 'curriculum_versions',
            'study_programs', 'curriculum_course_requirements',
            'curriculum_activity_requirements', 'homerooms', 'student_academic_years',
            'homeroom_placements', 'learning_offerings', 'course_offering_details',
            'activity_offering_details', 'learning_groups', 'learning_group_homerooms',
            'learning_group_teachers', 'learning_group_students', 'learning_results',
            'activity_result_details', 'course_assessment_plans',
            'course_assessment_categories', 'course_assessment_items',
            'academic_timetable_entries', 'timetable_entry_instructors',
            'academic_exam_rounds', 'academic_exam_days', 'academic_exam_schedule_items',
            'academic_exam_sessions', 'academic_exam_day_room_assignments',
            'supervision_cycles', 'supervision_observations',
            'academic_question_bank_questions', 'academic_question_bank_choices',
            'admission_tracks', 'admission_applications', 'admission_room_assignments',
            'calendar_events', 'calendar_event_targets', 'certificate_campaigns',
            'student_class_enrollments', 'classroom_courses',
            'classroom_course_instructors', 'activity_slots',
            'activity_slot_classrooms', 'activity_slot_classroom_assignments',
            'activity_slot_instructors', 'activity_groups',
            'activity_group_instructors', 'activity_group_members'
        ] LOOP
            EXECUTE format(
                'SELECT EXISTS (SELECT 1 FROM %I candidate WHERE candidate.xmin::text::bigint > $1)',
                relation_name
            ) INTO has_stale_row USING marker_xmin;
            IF has_stale_row THEN
                RAISE EXCEPTION 'ACADEMIC_CORE_045_RECONCILIATION_STALE';
            END IF;
        END LOOP;
    END IF;
END;
$$;

CREATE TEMP TABLE academic_phase_b_permission_map (
    source_code TEXT NOT NULL,
    target_code TEXT NOT NULL,
    PRIMARY KEY (source_code, target_code)
) ON COMMIT DROP;

INSERT INTO academic_phase_b_permission_map (source_code, target_code)
VALUES
    ('academic_structure.read.all', 'academic_context.read.school'),
    ('academic_structure.read.all', 'academic_year.read.school'),
    ('academic_structure.read.all', 'academic_term.read.school'),
    ('academic_structure.read.all', 'academic_catalog.read.school'),
    ('academic_structure.manage.all', 'academic_context.read.school'),
    ('academic_structure.manage.all', 'academic_year.manage.school'),
    ('academic_structure.manage.all', 'academic_term.manage.school'),
    ('academic_structure.manage.all', 'academic_catalog.manage.school'),
    ('academic_classroom.read.all', 'homeroom.read.school'),
    ('academic_classroom.create.all', 'homeroom.manage.school'),
    ('academic_classroom.update.all', 'homeroom.manage.school'),
    ('academic_classroom.delete.all', 'homeroom.manage.school'),
    ('academic_enrollment.read.all', 'student_academic_year.read.school'),
    ('academic_enrollment.update.all', 'student_academic_year.manage.school'),
    ('academic_course_plan.read.all', 'learning_offering.read.school'),
    ('academic_course_plan.manage.all', 'learning_offering.manage.school'),
    ('academic_curriculum.read.all', 'academic_curriculum.read.school'),
    ('academic_curriculum.read.organization_tree', 'academic_curriculum.read.organization_tree'),
    ('academic_curriculum.create.all', 'academic_curriculum.manage.school'),
    ('academic_curriculum.update.all', 'academic_curriculum.manage.school'),
    ('academic_curriculum.delete.all', 'academic_curriculum.manage.school'),
    ('academic_curriculum.manage.organization_unit', 'academic_curriculum.manage.organization_unit'),
    ('academic_curriculum.manage.organization_tree', 'academic_curriculum.manage.organization_tree'),
    ('activity.read.all', 'academic_catalog.read.school'),
    ('activity.read.all', 'learning_offering.read.school'),
    ('activity.manage.all', 'academic_catalog.manage.school'),
    ('activity.manage.all', 'learning_offering.manage.school'),
    ('activity.manage_members.all', 'learning_offering.manage.school'),
    ('activity.manage.own', 'learning_offering.manage.assigned');

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM permissions
        WHERE is_active AND (
               code LIKE 'academic_structure.%'
            OR code LIKE 'academic_classroom.%'
            OR code LIKE 'academic_enrollment.%'
            OR code LIKE 'academic_course_plan.%'
            OR code IN (
                'academic_curriculum.read.all', 'academic_curriculum.create.all',
                'academic_curriculum.update.all', 'academic_curriculum.delete.all',
                'activity.read.all', 'activity.manage.all',
                'activity.manage_members.all', 'activity.manage.own',
                'academic_promotion.read.all', 'academic_promotion.execute.all'
            )
        )
    ) OR EXISTS (
        SELECT 1
        FROM role_permissions source_grant
        JOIN permissions source ON source.id = source_grant.permission_id
        JOIN academic_phase_b_permission_map mapping ON mapping.source_code = source.code
        JOIN permissions target ON target.code = mapping.target_code AND target.is_active
        WHERE NOT EXISTS (
            SELECT 1 FROM role_permissions target_grant
            WHERE target_grant.role_id = source_grant.role_id
              AND target_grant.permission_id = target.id
        )
    ) OR EXISTS (
        SELECT 1
        FROM organization_permission_grants source_grant
        JOIN permissions source ON source.id = source_grant.permission_id
        JOIN academic_phase_b_permission_map mapping ON mapping.source_code = source.code
        JOIN permissions target ON target.code = mapping.target_code AND target.is_active
        WHERE NOT EXISTS (
            SELECT 1 FROM organization_permission_grants target_grant
            WHERE target_grant.organization_unit_id = source_grant.organization_unit_id
              AND target_grant.position_code IS NOT DISTINCT FROM source_grant.position_code
              AND target_grant.permission_id = target.id
        )
    ) OR EXISTS (
        SELECT 1
        FROM organization_permission_delegations source_grant
        JOIN permissions source ON source.id = source_grant.permission_id
        JOIN academic_phase_b_permission_map mapping ON mapping.source_code = source.code
        JOIN permissions target ON target.code = mapping.target_code AND target.is_active
        WHERE NOT EXISTS (
            SELECT 1 FROM organization_permission_delegations target_grant
            WHERE target_grant.from_user_id = source_grant.from_user_id
              AND target_grant.to_user_id = source_grant.to_user_id
              AND target_grant.organization_unit_id IS NOT DISTINCT FROM source_grant.organization_unit_id
              AND target_grant.permission_id = target.id
              AND target_grant.started_at = source_grant.started_at
              AND target_grant.expires_at IS NOT DISTINCT FROM source_grant.expires_at
              AND target_grant.revoked_at IS NOT DISTINCT FROM source_grant.revoked_at
        )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_PERMISSION_RECONCILIATION_FAILED';
    END IF;
END;
$$;

CREATE TEMP TABLE academic_phase_b_cleanup_snapshot (
    counts JSONB NOT NULL,
    target_checksum CHAR(64) NOT NULL
) ON COMMIT DROP;

INSERT INTO academic_phase_b_cleanup_snapshot (counts, target_checksum)
SELECT jsonb_build_object(
           'legacyRelationsRemoved', 11,
           'legacyColumnsRemoved', 6,
           'legacyPermissionDefinitionsRemoved', (
               SELECT COUNT(*) FROM permissions
               WHERE code LIKE 'academic_structure.%'
                  OR code LIKE 'academic_classroom.%'
                  OR code LIKE 'academic_enrollment.%'
                  OR code LIKE 'academic_course_plan.%'
                  OR code IN (
                      'academic_curriculum.read.all', 'academic_curriculum.create.all',
                      'academic_curriculum.update.all', 'academic_curriculum.delete.all',
                      'activity.read.all', 'activity.manage.all',
                      'activity.manage_members.all', 'activity.manage.own',
                      'academic_promotion.read.all', 'academic_promotion.execute.all'
                  )
           ),
           'legacyPermissionGrantsRemoved', (
               SELECT COUNT(*) FROM (
                   SELECT role_id::text AS principal, permission_id FROM role_permissions
                   UNION ALL
                   SELECT organization_unit_id::text || ':' || COALESCE(position_code, ''), permission_id
                   FROM organization_permission_grants
                   UNION ALL
                   SELECT id::text, permission_id FROM organization_permission_delegations
               ) grants
               JOIN permissions permission ON permission.id = grants.permission_id
               WHERE permission.code LIKE 'academic_structure.%'
                  OR permission.code LIKE 'academic_classroom.%'
                  OR permission.code LIKE 'academic_enrollment.%'
                  OR permission.code LIKE 'academic_course_plan.%'
                  OR permission.code IN (
                      'academic_curriculum.read.all', 'academic_curriculum.create.all',
                      'academic_curriculum.update.all', 'academic_curriculum.delete.all',
                      'activity.read.all', 'activity.manage.all',
                      'activity.manage_members.all', 'activity.manage.own',
                      'academic_promotion.read.all', 'academic_promotion.execute.all'
                  )
           ),
           'targetRowsRetained', (
               SELECT COUNT(*) FROM academic_terms
           ) + (
               SELECT COUNT(*) FROM subject_versions
           ) + (
               SELECT COUNT(*) FROM activity_versions
           ) + (
               SELECT COUNT(*) FROM curricula
           ) + (
               SELECT COUNT(*) FROM curriculum_versions
           ) + (
               SELECT COUNT(*) FROM homerooms
           ) + (
               SELECT COUNT(*) FROM learning_offerings
           ) + (
               SELECT COUNT(*) FROM course_assessment_plans
           )
       ),
       encode(sha256(convert_to(concat_ws('|',
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM academic_terms),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM subject_versions),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM activity_versions),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM curricula),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM curriculum_versions),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM homerooms),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM learning_offerings),
           (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM course_assessment_plans)
       ), 'UTF8')), 'hex');

DELETE FROM role_permissions
WHERE permission_id IN (
    SELECT id FROM permissions
    WHERE code LIKE 'academic_structure.%'
       OR code LIKE 'academic_classroom.%'
       OR code LIKE 'academic_enrollment.%'
       OR code LIKE 'academic_course_plan.%'
       OR code IN (
           'academic_curriculum.read.all', 'academic_curriculum.create.all',
           'academic_curriculum.update.all', 'academic_curriculum.delete.all',
           'activity.read.all', 'activity.manage.all',
           'activity.manage_members.all', 'activity.manage.own',
           'academic_promotion.read.all', 'academic_promotion.execute.all'
       )
);

DELETE FROM organization_permission_grants
WHERE permission_id IN (
    SELECT id FROM permissions
    WHERE code LIKE 'academic_structure.%'
       OR code LIKE 'academic_classroom.%'
       OR code LIKE 'academic_enrollment.%'
       OR code LIKE 'academic_course_plan.%'
       OR code IN (
           'academic_curriculum.read.all', 'academic_curriculum.create.all',
           'academic_curriculum.update.all', 'academic_curriculum.delete.all',
           'activity.read.all', 'activity.manage.all',
           'activity.manage_members.all', 'activity.manage.own',
           'academic_promotion.read.all', 'academic_promotion.execute.all'
       )
);

DELETE FROM organization_permission_delegations
WHERE permission_id IN (
    SELECT id FROM permissions
    WHERE code LIKE 'academic_structure.%'
       OR code LIKE 'academic_classroom.%'
       OR code LIKE 'academic_enrollment.%'
       OR code LIKE 'academic_course_plan.%'
       OR code IN (
           'academic_curriculum.read.all', 'academic_curriculum.create.all',
           'academic_curriculum.update.all', 'academic_curriculum.delete.all',
           'activity.read.all', 'activity.manage.all',
           'activity.manage_members.all', 'activity.manage.own',
           'academic_promotion.read.all', 'academic_promotion.execute.all'
       )
);

DELETE FROM permissions
WHERE code LIKE 'academic_structure.%'
   OR code LIKE 'academic_classroom.%'
   OR code LIKE 'academic_enrollment.%'
   OR code LIKE 'academic_course_plan.%'
   OR code IN (
       'academic_curriculum.read.all', 'academic_curriculum.create.all',
       'academic_curriculum.update.all', 'academic_curriculum.delete.all',
       'activity.read.all', 'activity.manage.all',
       'activity.manage_members.all', 'activity.manage.own',
       'academic_promotion.read.all', 'academic_promotion.execute.all'
   );

DROP TABLE activity_group_members;
DROP TABLE activity_group_instructors;
DROP TABLE activity_groups;
DROP TABLE activity_slot_classroom_assignments;
DROP TABLE activity_slot_classrooms;
DROP TABLE activity_slot_instructors;
DROP TABLE activity_slots;
DROP TABLE classroom_course_instructors;
DROP TABLE classroom_courses;
DROP TABLE student_class_enrollments;
DROP TABLE IF EXISTS classroom_course_preferred_rooms;
DROP TABLE academic_core_entity_map;

DROP FUNCTION refresh_course_primary_instructor(UUID);
DROP FUNCTION trg_asc_cascade_assignments();
DROP FUNCTION trg_asc_cleanup_empty_slot();
DROP FUNCTION trg_cc_sync_junction();
DROP FUNCTION trg_cci_sync_primary();
DROP FUNCTION academic_normalize_identity(TEXT);

ALTER TABLE academic_years DROP COLUMN is_active;
ALTER TABLE academic_terms DROP COLUMN is_active, DROP COLUMN legacy_term;
ALTER TABLE grade_levels DROP COLUMN next_grade_level_id;
ALTER TABLE homerooms DROP COLUMN legacy_curriculum_version_id;
ALTER TABLE bell_schedule_periods DROP COLUMN academic_year_id;

DO $$
DECLARE
    cleanup_source_checksum TEXT;
    cleanup_target_checksum TEXT;
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_class
        WHERE relnamespace = current_schema()::regnamespace
          AND relname = ANY(ARRAY[
              'student_class_enrollments', 'classroom_courses',
              'classroom_course_instructors', 'classroom_course_preferred_rooms',
              'activity_slots', 'activity_slot_classrooms',
              'activity_slot_classroom_assignments', 'activity_slot_instructors',
              'activity_groups', 'activity_group_instructors',
              'activity_group_members', 'academic_core_entity_map'
          ])
    ) OR EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = current_schema()
          AND (table_name, column_name) IN (
              ('academic_years', 'is_active'),
              ('academic_terms', 'is_active'),
              ('academic_terms', 'legacy_term'),
              ('grade_levels', 'next_grade_level_id'),
              ('homerooms', 'legacy_curriculum_version_id'),
              ('bell_schedule_periods', 'academic_year_id'),
              ('admission_tracks', 'study_plan_id'),
              ('admission_tracks', 'curriculum_version_id'),
              ('admission_room_assignments', 'class_room_id'),
              ('academic_timetable_entries', 'academic_semester_id'),
              ('academic_timetable_entries', 'legacy_classroom_course_id'),
              ('academic_timetable_entries', 'legacy_activity_slot_id'),
              ('academic_exam_schedule_items', 'academic_semester_id'),
              ('academic_exam_schedule_items', 'legacy_classroom_course_id'),
              ('supervision_cycles', 'academic_year'),
              ('supervision_cycles', 'semester'),
              ('supervision_cycles', 'academic_semester_id'),
              ('supervision_observations', 'academic_semester_id')
          )
    ) OR EXISTS (
        SELECT 1 FROM permissions
        WHERE code LIKE 'academic_structure.%'
           OR code LIKE 'academic_classroom.%'
           OR code LIKE 'academic_enrollment.%'
           OR code LIKE 'academic_course_plan.%'
           OR code IN (
               'academic_curriculum.read.all', 'academic_curriculum.create.all',
               'academic_curriculum.update.all', 'academic_curriculum.delete.all',
               'activity.read.all', 'activity.manage.all',
               'activity.manage_members.all', 'activity.manage.own',
               'academic_promotion.read.all', 'academic_promotion.execute.all'
           )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_CLEANUP_MANIFEST_REMAINS';
    END IF;

    IF to_regclass('academic_terms') IS NULL
       OR to_regclass('subject_versions') IS NULL
       OR to_regclass('activity_versions') IS NULL
       OR to_regclass('curricula') IS NULL
       OR to_regclass('curriculum_versions') IS NULL
       OR to_regclass('homerooms') IS NULL
       OR to_regclass('learning_offerings') IS NULL
       OR to_regclass('course_assessment_plans') IS NULL THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_TARGET_MANIFEST_MISSING';
    END IF;

    SELECT btrim(snapshot.target_checksum) INTO cleanup_source_checksum
    FROM academic_phase_b_cleanup_snapshot snapshot;
    SELECT encode(sha256(convert_to(concat_ws('|',
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM academic_terms),
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM subject_versions),
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM activity_versions),
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM curricula),
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM curriculum_versions),
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM homerooms),
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM learning_offerings),
        (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM course_assessment_plans)
    ), 'UTF8')), 'hex') INTO cleanup_target_checksum;
    IF cleanup_source_checksum <> cleanup_target_checksum THEN
        RAISE EXCEPTION 'ACADEMIC_CORE_045_TARGET_CHECKSUM_MISMATCH';
    END IF;

    INSERT INTO academic_core_cutover_audits (
        migration_version, mapping_algorithm_version, source_counts, target_counts,
        source_checksum, target_checksum
    )
    SELECT 45, 'academic-core-v1-cleanup', snapshot.counts, snapshot.counts,
           cleanup_source_checksum, cleanup_target_checksum
    FROM academic_phase_b_cleanup_snapshot snapshot;
END;
$$;
