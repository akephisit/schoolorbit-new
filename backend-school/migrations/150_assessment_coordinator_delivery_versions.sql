-- Delivery owns allocation membership. Assessment and exam eligibility consume
-- the same latest published graph without changing stored owners or episodes.
CREATE VIEW academic_delivery_latest_published_teacher_allocations AS
WITH latest AS (
    SELECT DISTINCT ON (academic_year_id, academic_term_id)
           id, academic_year_id, academic_term_id, snapshot
    FROM academic_delivery_versions
    WHERE status = 'published'
    ORDER BY academic_year_id, academic_term_id,
             effective_from DESC, published_at DESC, id DESC
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
          FROM academic_delivery_latest_published_teacher_allocations allocation
          JOIN users coordinator
            ON coordinator.id = allocation.teacher_id
           AND coordinator.status = 'active'
          WHERE allocation.learning_offering_id = plan.learning_offering_id
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
 AND learning_group.status <> 'closed'
JOIN learning_group_homerooms coverage ON coverage.learning_group_id = learning_group.id
JOIN homerooms homeroom
  ON homeroom.id = coverage.homeroom_id
 AND homeroom.academic_year_id = ready_plan.academic_year_id
 AND homeroom.is_active
WHERE phase.phase_code IN ('midterm', 'final')
  AND phase.exam_arrangement = 'in_timetable'
  AND phase.exam_duration_minutes IS NOT NULL
  AND phase.exam_duration_minutes > 0;
