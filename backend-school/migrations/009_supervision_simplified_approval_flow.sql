-- Simplify teaching supervision approval flow.
--
-- The workflow no longer has a separate "submit for review" step. Existing
-- in-flight rows are normalized to the nearest current workflow state:
-- evaluators_submitted -> approved -> published -> completed.

UPDATE supervision_observations
SET status = 'evaluators_submitted',
    updated_at = now()
WHERE status = 'under_review';

UPDATE supervision_observations
SET status = 'completed',
    updated_at = now()
WHERE status = 'acknowledged';
