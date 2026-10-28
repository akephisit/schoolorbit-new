-- Exclusions belong only to the selected revision. Published snapshot bytes are untouched.
ALTER TABLE academic_term_change_sets
    ADD COLUMN excluded_offering_ids UUID[] NOT NULL DEFAULT '{}',
    ADD COLUMN excluded_group_ids UUID[] NOT NULL DEFAULT '{}',
    ADD CONSTRAINT academic_term_change_sets_exclusion_shape CHECK (
        array_position(excluded_offering_ids,NULL) IS NULL
        AND array_position(excluded_group_ids,NULL) IS NULL);

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
        SELECT learning_offering_id,weekly_period_target FROM academic_term_change_items
        WHERE change_set_id=revision AND action_kind='add_offering'
    )
    , projected AS (
    SELECT academic_project_delivery_teacher_changes(academic_capture_delivery_snapshot(
        context.academic_term_id,context.evaluation_date,
        COALESCE((SELECT jsonb_agg(jsonb_build_object('id',id,'weekly_period_target',weekly_period_target) ORDER BY id)
            FROM selected_targets),'[]'::JSONB)),revision) AS graph FROM context
    )
    SELECT jsonb_build_object('offerings',COALESCE(jsonb_agg(
        jsonb_set(offering,'{groups}',COALESCE((SELECT jsonb_agg(source_group ORDER BY (source_group->>'id')::UUID)
            FROM jsonb_array_elements(offering->'groups') source_group
            WHERE NOT ((source_group->>'id')::UUID=ANY(context.excluded_group_ids))), '[]'::JSONB))
        ORDER BY (offering->>'id')::UUID), '[]'::JSONB))
    FROM context CROSS JOIN projected CROSS JOIN LATERAL jsonb_array_elements(projected.graph->'offerings') offering
    WHERE NOT ((offering->>'id')::UUID=ANY(context.excluded_offering_ids));
$$;
