SET LOCAL lock_timeout = '30s';
LOCK TABLE supervision_observations IN ACCESS EXCLUSIVE MODE;
LOCK TABLE supervision_evaluators IN ACCESS EXCLUSIVE MODE;
LOCK TABLE supervision_template_sections, supervision_template_items,
    supervision_evaluator_responses IN SHARE ROW EXCLUSIVE MODE;

-- Preserve evidence even when an imported submission has no submitted_at.
ALTER TABLE supervision_evaluators
    ADD COLUMN submission_reopened boolean NOT NULL DEFAULT false;

CREATE TEMP TABLE supervision_169_reopened_evaluators ON COMMIT DROP AS
SELECT e.id, to_jsonb(e) - 'status' - 'submission_reopened' AS retained_evaluator
FROM supervision_evaluators e
JOIN supervision_observations o ON o.id = e.observation_id
WHERE o.status IN ('planned', 'in_progress', 'evaluators_submitted', 'under_review', 'approved')
  AND e.is_required AND e.status = 'submitted'
  AND EXISTS (
      SELECT 1
      FROM supervision_template_sections s
      JOIN supervision_template_items i ON i.section_id = s.id
      WHERE s.template_id = o.template_id AND i.required
        AND NOT EXISTS (
            SELECT 1 FROM supervision_evaluator_responses r
            WHERE r.observation_id = o.id AND r.evaluator_id = e.id AND r.template_item_id = i.id
              AND (
                  (i.item_type = 'rating' AND r.rating_score <> 'NaN'::numeric)
                  OR (i.item_type = 'text' AND
                      -- Match the Unicode whitespace recognized in evaluation validation.
                      btrim(r.text_response, U&'\0009\000A\000B\000C\000D\0020\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000') <> '')
              )
        )
  );

ALTER TABLE supervision_evaluators DISABLE TRIGGER update_supervision_evaluators_updated_at;
UPDATE supervision_evaluators e
SET status = 'draft', submission_reopened = true
FROM supervision_169_reopened_evaluators reopened
WHERE e.id = reopened.id;
ALTER TABLE supervision_evaluators ENABLE TRIGGER update_supervision_evaluators_updated_at;

-- Capture the existing facts so the cutover cannot change booking or audit evidence.
CREATE TEMP TABLE supervision_169_status_changes ON COMMIT DROP AS
SELECT o.id, o.status AS previous_status,
       CASE WHEN EXISTS (
           SELECT 1 FROM supervision_evaluators e
           WHERE e.observation_id = o.id AND e.is_required
       ) AND NOT EXISTS (
           SELECT 1 FROM supervision_evaluators e
           WHERE e.observation_id = o.id AND e.is_required AND e.status <> 'submitted'
       ) THEN 'published' ELSE 'planned' END AS next_status,
       to_jsonb(o) - 'status' AS retained_observation
FROM supervision_observations o
WHERE o.status IN ('evaluators_submitted', 'under_review', 'approved');

-- Preserve updated_at because it determines which observation a teacher's report shows.
ALTER TABLE supervision_observations DISABLE TRIGGER update_supervision_observations_updated_at;
UPDATE supervision_observations o
SET status = changes.next_status
FROM supervision_169_status_changes changes
WHERE o.id = changes.id;
ALTER TABLE supervision_observations ENABLE TRIGGER update_supervision_observations_updated_at;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM supervision_169_reopened_evaluators reopened
        LEFT JOIN supervision_evaluators e ON e.id = reopened.id
        WHERE e.status IS DISTINCT FROM 'draft'
           OR e.submission_reopened IS DISTINCT FROM true
           OR to_jsonb(e) - 'status' - 'submission_reopened' IS DISTINCT FROM reopened.retained_evaluator
    ) THEN
        RAISE EXCEPTION 'SUPERVISION169_EVALUATOR_RECONCILIATION_FAILED' USING ERRCODE = 'check_violation';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM supervision_169_status_changes changes
        LEFT JOIN supervision_observations o ON o.id = changes.id
        WHERE o.status IS DISTINCT FROM changes.next_status
           OR to_jsonb(o) - 'status' IS DISTINCT FROM changes.retained_observation
    ) THEN
        RAISE EXCEPTION 'SUPERVISION169_STATUS_RECONCILIATION_FAILED' USING ERRCODE = 'check_violation';
    END IF;
END $$;

-- Record the cutover as history without generating new inbox or push work.
INSERT INTO supervision_actions (observation_id, action_kind, from_status, to_status)
SELECT id, 'result_status_migrated', previous_status, next_status
FROM supervision_169_status_changes;

ALTER TABLE supervision_observations
    DROP CONSTRAINT supervision_observations_status_check,
    ADD CONSTRAINT supervision_observations_status_check CHECK (
        status IN ('requested', 'planned', 'in_progress', 'returned',
                   'published', 'acknowledged', 'completed', 'cancelled')
    );

-- Preserve existing school-wide read/report access on fresh and deployed tenants.
LOCK TABLE permissions, role_permissions, organization_permission_grants,
    organization_permission_delegations IN SHARE ROW EXCLUSIVE MODE;
INSERT INTO permissions (code, name, module, action, scope, description)
VALUES ('supervision.read.school', 'ดูนิเทศทั้งโรงเรียน', 'supervision', 'read', 'school',
        'ดูรายการนิเทศการสอนและรายงานทั้งโรงเรียน')
ON CONFLICT (code) DO UPDATE SET is_active = true
WHERE NOT permissions.is_active;

DO $$
DECLARE
    approval_id uuid;
    read_id uuid;
BEGIN
    SELECT id INTO approval_id
    FROM permissions WHERE code = 'supervision.approve.school' AND is_active;
    IF approval_id IS NULL THEN
        RETURN;
    END IF;
    SELECT id INTO STRICT read_id FROM permissions WHERE code = 'supervision.read.school';

    INSERT INTO role_permissions (role_id, permission_id, created_at)
    SELECT role_id, read_id, created_at
    FROM role_permissions WHERE permission_id = approval_id
    ON CONFLICT DO NOTHING;

    INSERT INTO organization_permission_grants (
        organization_unit_id, permission_id, created_at, created_by, position_code
    )
    SELECT organization_unit_id, read_id, created_at, created_by, position_code
    FROM organization_permission_grants WHERE permission_id = approval_id
    ON CONFLICT DO NOTHING;

    IF EXISTS (
        SELECT 1 FROM role_permissions old_grant
        WHERE old_grant.permission_id = approval_id
          AND NOT EXISTS (
              SELECT 1 FROM role_permissions new_grant
              WHERE new_grant.role_id = old_grant.role_id AND new_grant.permission_id = read_id
          )
    ) OR EXISTS (
        SELECT 1 FROM organization_permission_grants old_grant
        WHERE old_grant.permission_id = approval_id
          AND NOT EXISTS (
              SELECT 1 FROM organization_permission_grants new_grant
              WHERE new_grant.organization_unit_id = old_grant.organization_unit_id
                AND new_grant.position_code IS NOT DISTINCT FROM old_grant.position_code
                AND new_grant.permission_id = read_id
          )
    ) THEN
        RAISE EXCEPTION 'SUPERVISION169_GRANT_RECONCILIATION_FAILED' USING ERRCODE = 'check_violation';
    END IF;

    DELETE FROM role_permissions WHERE permission_id = approval_id;
    DELETE FROM organization_permission_grants WHERE permission_id = approval_id;

    UPDATE organization_permission_delegations
    SET permission_id = read_id
    WHERE permission_id = approval_id AND revoked_at IS NULL
      AND (expires_at IS NULL OR expires_at > now());

    UPDATE permissions SET is_active = false, updated_at = now() WHERE id = approval_id;
END $$;
