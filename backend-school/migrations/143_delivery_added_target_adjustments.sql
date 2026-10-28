-- Pending weekly adjustments override the initial target of offerings added to the same draft.
-- Published snapshots, journal identities and every other projection rule remain unchanged.
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
    SELECT CASE WHEN cardinality(context.excluded_offering_ids)=0
            AND cardinality(context.excluded_group_ids)=0 THEN projected.graph
        ELSE (SELECT jsonb_build_object('offerings',COALESCE(jsonb_agg(
            jsonb_set(offering,'{groups}',COALESCE((SELECT jsonb_agg(source_group ORDER BY (source_group->>'id')::UUID)
                FROM jsonb_array_elements(offering->'groups') source_group
                WHERE NOT ((source_group->>'id')::UUID=ANY(context.excluded_group_ids))), '[]'::JSONB))
            ORDER BY (offering->>'id')::UUID), '[]'::JSONB))
            FROM jsonb_array_elements(projected.graph->'offerings') offering
            WHERE NOT ((offering->>'id')::UUID=ANY(context.excluded_offering_ids))) END
    FROM context CROSS JOIN projected;
$$;

-- Repair only the stale scalar in writable drafts that recorded an ignored adjustment.
-- Preserve array order, exclusions and every other captured fact; never recapture a graph.
WITH corrected AS (
    SELECT version.id AS version_id,revision.id AS revision_id,
        jsonb_set(version.snapshot,'{offerings}',repair.offerings,false) AS snapshot
    FROM academic_term_change_sets revision
    JOIN academic_delivery_versions version ON version.id=revision.target_delivery_version_id
    CROSS JOIN LATERAL (
        SELECT jsonb_agg(CASE
            WHEN adjustment.id IS NOT NULL AND entry.offering ? 'weeklyPeriodTarget'
                AND entry.offering->'weeklyPeriodTarget' IS DISTINCT FROM to_jsonb(adjustment.weekly_period_target)
            THEN jsonb_set(entry.offering,'{weeklyPeriodTarget}',to_jsonb(adjustment.weekly_period_target),false)
            ELSE entry.offering END ORDER BY entry.position) AS offerings,
            bool_or(adjustment.id IS NOT NULL AND entry.offering ? 'weeklyPeriodTarget'
                AND entry.offering->'weeklyPeriodTarget' IS DISTINCT FROM to_jsonb(adjustment.weekly_period_target)) AS changed
        FROM jsonb_array_elements(version.snapshot->'offerings') WITH ORDINALITY AS entry(offering,position)
        LEFT JOIN academic_term_change_items added
            ON added.change_set_id=revision.id AND added.action_kind='add_offering'
                AND added.learning_offering_id=(entry.offering->>'id')::UUID
        LEFT JOIN academic_term_change_items adjustment
            ON adjustment.change_set_id=revision.id AND adjustment.action_kind='adjust_weekly_period_target'
                AND adjustment.learning_offering_id=added.learning_offering_id
    ) repair
    WHERE revision.status='draft' AND version.status='draft' AND repair.changed
), updated_versions AS (
    UPDATE academic_delivery_versions version
    SET snapshot=corrected.snapshot,row_version=version.row_version+1,updated_at=now()
    FROM corrected WHERE version.id=corrected.version_id
    RETURNING corrected.revision_id
)
UPDATE academic_term_change_sets revision
SET row_version=revision.row_version+1,updated_at=now()
WHERE revision.id IN (SELECT revision_id FROM updated_versions);
