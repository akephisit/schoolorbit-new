-- Release 2 transactional cutover. SQLx wraps this migration in one transaction.
-- Keep source writes stopped throughout validation, reconciliation, and removal.
SET LOCAL lock_timeout = '30s';
LOCK TABLE learning_results, activity_result_details, academic_assessment_phase_controls,
    learning_group_score_items, course_offering_details, course_assessment_plans,
    course_assessment_phases, learning_groups, learning_group_students,
    student_academic_years, academic_terms, learning_offerings, activity_offering_details,
    permissions, role_permissions, organization_permission_grants,
    organization_permission_delegations IN ACCESS EXCLUSIVE MODE;

CREATE TEMP TABLE academic_060_source_counts ON COMMIT DROP AS
SELECT (SELECT count(*) FROM learning_results) AS results,
       (SELECT count(*) FROM activity_result_details) AS details,
       (SELECT count(*) FROM learning_group_score_items) AS items,
       (SELECT count(*) FROM academic_assessment_phase_controls) AS controls;

DO $$
DECLARE
    offering RECORD;
    total NUMERIC;
BEGIN
    IF EXISTS (
        SELECT 1 FROM learning_results r
        LEFT JOIN activity_result_details d ON d.learning_result_id = r.id
        WHERE r.kind <> 'activity' OR r.status <> 'recorded'
           OR d.learning_result_id IS NULL OR lower(btrim(d.outcome)) NOT IN ('pass','fail')
    ) OR EXISTS (
        SELECT 1 FROM activity_result_details d
        LEFT JOIN learning_results r ON r.id = d.learning_result_id WHERE r.id IS NULL
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_060_LEGACY_OUTCOME_INVALID: expected recorded activity pass/fail rows'
            USING ERRCODE = 'check_violation';
    END IF;
    IF EXISTS (
        SELECT 1 FROM learning_results r
        LEFT JOIN learning_groups g ON g.id = r.learning_group_id
        LEFT JOIN activity_offering_details a ON a.learning_offering_id = r.learning_offering_id
        LEFT JOIN student_academic_years s ON s.id = r.student_academic_year_id
        WHERE g.id IS NULL OR a.learning_offering_id IS NULL OR s.id IS NULL
           OR (g.learning_offering_id,g.academic_term_id,g.academic_year_id)
              IS DISTINCT FROM (r.learning_offering_id,r.academic_term_id,r.academic_year_id)
           OR (a.academic_term_id,a.academic_year_id)
              IS DISTINCT FROM (r.academic_term_id,r.academic_year_id)
           OR (s.academic_year_id,s.student_id)
              IS DISTINCT FROM (r.academic_year_id,r.student_id)
           OR NOT EXISTS (
               SELECT 1 FROM learning_group_students member
               WHERE member.learning_group_id = r.learning_group_id
                 AND member.student_academic_year_id = r.student_academic_year_id
                 AND member.academic_term_id = r.academic_term_id
                 AND member.academic_year_id = r.academic_year_id
           )
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_060_LEGACY_CONTEXT_MISMATCH: reconcile activity offering/group/student context'
            USING ERRCODE = 'check_violation';
    END IF;
    IF EXISTS (
        SELECT 1 FROM academic_terms t
        LEFT JOIN academic_assessment_phase_controls c ON c.academic_term_id=t.id
        GROUP BY t.id HAVING count(c.id) <> 4 OR count(DISTINCT c.phase_code) <> 4
    ) OR EXISTS (
        SELECT 1 FROM academic_assessment_phase_controls c
        JOIN academic_terms t ON t.id=c.academic_term_id
        WHERE c.academic_year_id <> t.academic_year_id
           OR c.phase_code NOT IN ('before_midterm','midterm','after_midterm','final')
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_060_PHASE_CONTROL_CONTEXT_INVALID' USING ERRCODE = 'check_violation';
    END IF;
    IF EXISTS (
        SELECT 1 FROM learning_group_score_items i
        LEFT JOIN learning_groups g ON g.id=i.learning_group_id
        LEFT JOIN course_assessment_plans p ON p.id=i.course_assessment_plan_id
        LEFT JOIN course_assessment_phases phase ON phase.id=i.assessment_phase_id
        WHERE g.id IS NULL OR p.id IS NULL OR phase.id IS NULL
           OR (i.learning_offering_id,i.academic_term_id,i.academic_year_id)
              IS DISTINCT FROM (g.learning_offering_id,g.academic_term_id,g.academic_year_id)
           OR (i.learning_offering_id,i.academic_term_id,i.academic_year_id)
              IS DISTINCT FROM (p.learning_offering_id,p.academic_term_id,p.academic_year_id)
           OR phase.plan_id <> p.id
    ) THEN
        RAISE EXCEPTION 'ACADEMIC_060_SCORE_ITEM_CONTEXT_INVALID' USING ERRCODE = 'check_violation';
    END IF;
    FOR offering IN SELECT grading_policy FROM course_offering_details LOOP
        BEGIN
            IF jsonb_typeof(offering.grading_policy) <> 'object'
               OR offering.grading_policy->>'totalScore' IS NULL
               OR btrim(offering.grading_policy->>'totalScore') = '' THEN
                RAISE EXCEPTION 'missing total';
            END IF;
            total := (offering.grading_policy->>'totalScore')::numeric;
            IF total::text IN ('NaN','Infinity','-Infinity') OR total < 0
               OR total > 99999999.99 OR total <> round(total,2) THEN
                RAISE EXCEPTION 'unrepresentable total';
            END IF;
        EXCEPTION WHEN OTHERS THEN
            RAISE EXCEPTION 'ACADEMIC_060_OFFERING_TOTAL_INVALID: totalScore must be an exact nonnegative NUMERIC(10,2)'
                USING ERRCODE = 'check_violation';
        END;
    END LOOP;
END;
$$;

CREATE TABLE academic_gradebook_phase_controls (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    phase_code TEXT NOT NULL CHECK (phase_code IN ('before_midterm','midterm','after_midterm','final')),
    score_entry_enabled BOOLEAN NOT NULL DEFAULT false,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id) ON DELETE CASCADE,
    UNIQUE (academic_term_id,phase_code)
);
INSERT INTO academic_gradebook_phase_controls
SELECT id,academic_term_id,academic_year_id,phase_code,score_entry_enabled,row_version,
       updated_by,created_at,updated_at FROM academic_assessment_phase_controls;
ALTER TABLE academic_assessment_phase_controls DROP COLUMN score_entry_enabled;

ALTER TABLE learning_group_score_items
    ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('active','cancelled')),
    ADD COLUMN cancelled_at TIMESTAMPTZ,
    ADD COLUMN cancelled_by UUID REFERENCES users(id) ON DELETE SET NULL,
    ADD CONSTRAINT learning_group_score_items_cancelled_shape_check CHECK (
        (lifecycle='active' AND cancelled_at IS NULL AND cancelled_by IS NULL)
        OR (lifecycle='cancelled' AND cancelled_at IS NOT NULL)
    ),
    ADD CONSTRAINT learning_group_score_items_id_context_key
        UNIQUE (id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id);
ALTER TABLE course_offering_details ADD CONSTRAINT course_offering_details_subject_context_key
    UNIQUE (learning_offering_id,subject_id,academic_term_id,academic_year_id);
ALTER TABLE activity_offering_details ADD CONSTRAINT activity_offering_details_context_key
    UNIQUE (learning_offering_id,academic_term_id,academic_year_id);

CREATE TABLE learning_group_student_scores (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    score_item_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    score NUMERIC(10,2) NOT NULL CHECK (score >= 0 AND score <> 'NaN'::numeric),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (score_item_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_group_score_items(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (student_academic_year_id,academic_year_id)
        REFERENCES student_academic_years(id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (score_item_id,student_academic_year_id)
);

CREATE TABLE learning_group_phase_confirmations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    course_assessment_plan_id UUID NOT NULL,
    assessment_phase_id UUID NOT NULL,
    phase_row_version BIGINT NOT NULL CHECK (phase_row_version > 0),
    blank_score_count BIGINT NOT NULL DEFAULT 0 CHECK (blank_score_count >= 0),
    roster_checksum TEXT NOT NULL CHECK (roster_checksum ~ '^[0-9a-f]{64}$'),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    source_snapshot JSONB NOT NULL CHECK (jsonb_typeof(source_snapshot) = 'object'),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    confirmed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (course_assessment_plan_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES course_assessment_plans(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (assessment_phase_id,course_assessment_plan_id) REFERENCES course_assessment_phases(id,plan_id) ON DELETE RESTRICT,
    UNIQUE (learning_group_id,assessment_phase_id)
);

CREATE TABLE academic_grading_policy_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    version_no INTEGER NOT NULL UNIQUE CHECK (version_no > 0),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    lifecycle TEXT NOT NULL DEFAULT 'draft' CHECK (lifecycle IN ('draft','active','retired')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    activated_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((lifecycle='draft' AND activated_at IS NULL) OR (lifecycle IN ('active','retired') AND activated_at IS NOT NULL))
);
CREATE UNIQUE INDEX academic_grading_policy_versions_one_active ON academic_grading_policy_versions((true)) WHERE lifecycle='active';

CREATE TABLE academic_learner_evaluation_policy_versions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    version_no INTEGER NOT NULL UNIQUE CHECK (version_no > 0),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    lifecycle TEXT NOT NULL DEFAULT 'draft' CHECK (lifecycle IN ('draft','active','retired')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    activated_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((lifecycle='draft' AND activated_at IS NULL) OR (lifecycle IN ('active','retired') AND activated_at IS NOT NULL))
);
CREATE UNIQUE INDEX academic_learner_evaluation_policy_versions_one_active ON academic_learner_evaluation_policy_versions((true)) WHERE lifecycle='active';

CREATE TABLE academic_grading_policy_bands (
    policy_version_id UUID NOT NULL REFERENCES academic_grading_policy_versions(id) ON DELETE RESTRICT,
    grade NUMERIC(10,2) NOT NULL CHECK (grade IN (0,1,1.5,2,2.5,3,3.5,4)),
    lower_bound NUMERIC(10,2) NOT NULL CHECK (lower_bound >= 0 AND lower_bound <> 'NaN'::numeric),
    PRIMARY KEY (policy_version_id,grade),
    UNIQUE (policy_version_id,lower_bound)
);

CREATE TABLE academic_learner_evaluation_policy_bands (
    policy_version_id UUID NOT NULL REFERENCES academic_learner_evaluation_policy_versions(id) ON DELETE RESTRICT,
    quality_level SMALLINT NOT NULL CHECK (quality_level BETWEEN 0 AND 3),
    lower_bound NUMERIC(10,2) NOT NULL CHECK (lower_bound BETWEEN 0 AND 3),
    PRIMARY KEY (policy_version_id,quality_level),
    UNIQUE (policy_version_id,lower_bound)
);

CREATE TABLE learning_group_result_overrides (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('manual_zero','incomplete','insufficient_attendance')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,subject_id,academic_term_id,academic_year_id)
        REFERENCES course_offering_details(learning_offering_id,subject_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (student_academic_year_id,academic_year_id)
        REFERENCES student_academic_years(id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (learning_group_id,student_academic_year_id)
);

CREATE TABLE learning_group_result_confirmations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    policy_version_id UUID NOT NULL REFERENCES academic_grading_policy_versions(id) ON DELETE RESTRICT,
    roster_checksum TEXT NOT NULL CHECK (roster_checksum ~ '^[0-9a-f]{64}$'),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    source_snapshot JSONB NOT NULL CHECK (jsonb_typeof(source_snapshot) = 'object'),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    confirmed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,subject_id,academic_term_id,academic_year_id)
        REFERENCES course_offering_details(learning_offering_id,subject_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (learning_group_id)
);

CREATE TABLE academic_learner_evaluation_criteria (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    domain TEXT NOT NULL CHECK (domain IN ('desirable_characteristic','reading_thinking_writing')),
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    applicability TEXT NOT NULL DEFAULT 'all' CHECK (applicability IN ('all','primary','secondary')),
    lifecycle TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('active','inactive')),
    display_order INTEGER NOT NULL DEFAULT 0,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (id,domain)
);

CREATE TABLE subject_term_evaluation_criteria (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    domain TEXT NOT NULL CHECK (domain IN ('desirable_characteristic','reading_thinking_writing')),
    school_criterion_id UUID,
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    lifecycle TEXT NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('active','inactive')),
    display_order INTEGER NOT NULL DEFAULT 0,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (school_criterion_id,domain) REFERENCES academic_learner_evaluation_criteria(id,domain) ON DELETE RESTRICT,
    UNIQUE (id,subject_id,academic_term_id,academic_year_id,domain),
    UNIQUE (subject_id,academic_term_id,domain,school_criterion_id)
);

CREATE TABLE academic_learner_evaluation_controls (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    domain TEXT NOT NULL CHECK (domain IN ('desirable_characteristic','reading_thinking_writing')),
    entry_enabled BOOLEAN NOT NULL DEFAULT false,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (academic_term_id,domain)
);

CREATE TABLE learning_group_student_evaluations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    domain TEXT NOT NULL CHECK (domain IN ('desirable_characteristic','reading_thinking_writing')),
    subject_term_criterion_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    quality_level SMALLINT NOT NULL CHECK (quality_level BETWEEN 0 AND 3),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,subject_id,academic_term_id,academic_year_id)
        REFERENCES course_offering_details(learning_offering_id,subject_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (student_academic_year_id,academic_year_id)
        REFERENCES student_academic_years(id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (subject_term_criterion_id,subject_id,academic_term_id,academic_year_id,domain)
        REFERENCES subject_term_evaluation_criteria(id,subject_id,academic_term_id,academic_year_id,domain) ON DELETE RESTRICT,
    UNIQUE (learning_group_id,student_academic_year_id,subject_term_criterion_id)
);

CREATE TABLE learning_group_evaluation_confirmations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    domain TEXT NOT NULL CHECK (domain IN ('desirable_characteristic','reading_thinking_writing')),
    roster_checksum TEXT NOT NULL CHECK (roster_checksum ~ '^[0-9a-f]{64}$'),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    source_snapshot JSONB NOT NULL CHECK (jsonb_typeof(source_snapshot) = 'object'),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    confirmed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,subject_id,academic_term_id,academic_year_id)
        REFERENCES course_offering_details(learning_offering_id,subject_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (learning_group_id,domain)
);

CREATE TABLE subject_term_evaluation_locks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    domain TEXT NOT NULL CHECK (domain IN ('desirable_characteristic','reading_thinking_writing')),
    roster_checksum TEXT NOT NULL CHECK (roster_checksum ~ '^[0-9a-f]{64}$'),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    source_snapshot JSONB NOT NULL CHECK (jsonb_typeof(source_snapshot) = 'object'),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    locked_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    locked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (subject_id,academic_term_id,domain),
    UNIQUE (id,subject_id,academic_term_id,academic_year_id,domain)
);

CREATE TABLE subject_term_student_evaluations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    domain TEXT NOT NULL CHECK (domain IN ('desirable_characteristic','reading_thinking_writing')),
    subject_term_criterion_id UUID NOT NULL,
    evaluation_lock_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    quality_level SMALLINT NOT NULL CHECK (quality_level BETWEEN 0 AND 3),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,subject_id,academic_term_id,academic_year_id)
        REFERENCES course_offering_details(learning_offering_id,subject_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (student_academic_year_id,academic_year_id)
        REFERENCES student_academic_years(id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (subject_term_criterion_id,subject_id,academic_term_id,academic_year_id,domain)
        REFERENCES subject_term_evaluation_criteria(id,subject_id,academic_term_id,academic_year_id,domain) ON DELETE RESTRICT,
    FOREIGN KEY (evaluation_lock_id,subject_id,academic_term_id,academic_year_id,domain)
        REFERENCES subject_term_evaluation_locks(id,subject_id,academic_term_id,academic_year_id,domain) ON DELETE RESTRICT,
    UNIQUE (evaluation_lock_id,student_academic_year_id,subject_term_criterion_id)
);

CREATE TABLE academic_course_result_locks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE RESTRICT,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    policy_version_id UUID NOT NULL REFERENCES academic_grading_policy_versions(id) ON DELETE RESTRICT,
    policy_snapshot JSONB NOT NULL CHECK (jsonb_typeof(policy_snapshot) = 'object'),
    roster_checksum TEXT NOT NULL CHECK (roster_checksum ~ '^[0-9a-f]{64}$'),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    source_snapshot JSONB NOT NULL CHECK (jsonb_typeof(source_snapshot) = 'object'),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    locked_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    locked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (academic_term_id,academic_year_id) REFERENCES academic_terms(id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (subject_id,academic_term_id),
    UNIQUE (id,subject_id,academic_term_id,academic_year_id)
);

CREATE TABLE academic_course_results (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    subject_id UUID NOT NULL,
    course_result_lock_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    calculated_score NUMERIC(10,2) NOT NULL CHECK (calculated_score >= 0 AND calculated_score <> 'NaN'::numeric),
    calculated_grade NUMERIC(10,2) NOT NULL CHECK (calculated_grade IN (0,1,1.5,2,2.5,3,3.5,4)),
    outcome TEXT NOT NULL CHECK (outcome IN ('numeric','incomplete','insufficient_attendance')),
    numeric_grade NUMERIC(10,2),
    selection_source TEXT NOT NULL CHECK (selection_source IN ('derived','manual_zero','incomplete','insufficient_attendance')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,subject_id,academic_term_id,academic_year_id)
        REFERENCES course_offering_details(learning_offering_id,subject_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (student_academic_year_id,academic_year_id)
        REFERENCES student_academic_years(id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (course_result_lock_id,subject_id,academic_term_id,academic_year_id)
        REFERENCES academic_course_result_locks(id,subject_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    CHECK ((outcome='numeric' AND numeric_grade IS NOT NULL AND numeric_grade IN (0,1,1.5,2,2.5,3,3.5,4)) OR (outcome IN ('incomplete','insufficient_attendance') AND numeric_grade IS NULL)),
    CHECK ((selection_source='derived' AND outcome='numeric' AND numeric_grade=calculated_grade) OR (selection_source='manual_zero' AND outcome='numeric' AND numeric_grade=0) OR (selection_source=outcome AND outcome IN ('incomplete','insufficient_attendance'))),
    UNIQUE (course_result_lock_id,student_academic_year_id),
    UNIQUE (subject_id,academic_term_id,student_academic_year_id)
);

CREATE TABLE academic_activity_evaluations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('pass','fail')),
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    migration_provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES activity_offering_details(learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (student_academic_year_id,academic_year_id)
        REFERENCES student_academic_years(id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (learning_group_id,student_academic_year_id)
);

CREATE TABLE academic_activity_result_confirmations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    roster_checksum TEXT NOT NULL CHECK (roster_checksum ~ '^[0-9a-f]{64}$'),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    source_snapshot JSONB NOT NULL CHECK (jsonb_typeof(source_snapshot) = 'object'),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    confirmed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES activity_offering_details(learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (learning_group_id)
);

CREATE TABLE academic_activity_result_locks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    roster_checksum TEXT NOT NULL CHECK (roster_checksum ~ '^[0-9a-f]{64}$'),
    source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
    source_snapshot JSONB NOT NULL CHECK (jsonb_typeof(source_snapshot) = 'object'),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    locked_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    locked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES activity_offering_details(learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (learning_group_id),
    UNIQUE (id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
);

CREATE TABLE academic_activity_results (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    learning_group_id UUID NOT NULL,
    learning_offering_id UUID NOT NULL,
    academic_term_id UUID NOT NULL,
    academic_year_id UUID NOT NULL,
    activity_result_lock_id UUID NOT NULL,
    student_academic_year_id UUID NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('pass','fail')),
    row_version BIGINT NOT NULL DEFAULT 1 CHECK (row_version > 0),
    FOREIGN KEY (learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES learning_groups(id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES activity_offering_details(learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (student_academic_year_id,academic_year_id)
        REFERENCES student_academic_years(id,academic_year_id) ON DELETE RESTRICT,
    FOREIGN KEY (activity_result_lock_id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id)
        REFERENCES academic_activity_result_locks(id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id) ON DELETE RESTRICT,
    UNIQUE (activity_result_lock_id,student_academic_year_id),
    UNIQUE (learning_group_id,student_academic_year_id)
);

CREATE TABLE academic_result_corrections (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    course_result_id UUID REFERENCES academic_course_results(id) ON DELETE RESTRICT,
    activity_result_id UUID REFERENCES academic_activity_results(id) ON DELETE RESTRICT,
    subject_student_evaluation_id UUID REFERENCES subject_term_student_evaluations(id) ON DELETE RESTRICT,
    old_course_outcome TEXT,
    new_course_outcome TEXT,
    old_numeric_grade NUMERIC(10,2),
    new_numeric_grade NUMERIC(10,2),
    old_activity_outcome TEXT,
    new_activity_outcome TEXT,
    old_quality_level SMALLINT,
    new_quality_level SMALLINT,
    expected_effective_version BIGINT NOT NULL CHECK (expected_effective_version > 0),
    corrected_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    corrected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT academic_result_corrections_one_target CHECK (num_nonnulls(course_result_id,activity_result_id,subject_student_evaluation_id)=1),
    CONSTRAINT academic_result_corrections_outcome_family CHECK (
        (course_result_id IS NOT NULL
         AND old_course_outcome IS NOT NULL AND new_course_outcome IS NOT NULL
         AND ((old_course_outcome='numeric' AND old_numeric_grade IS NOT NULL AND old_numeric_grade IN (0,0.5,1,1.5,2,2.5,3,3.5,4))
              OR (old_course_outcome IN ('incomplete','insufficient_attendance') AND old_numeric_grade IS NULL))
         AND ((new_course_outcome='numeric' AND new_numeric_grade IS NOT NULL AND new_numeric_grade IN (0,0.5,1,1.5,2,2.5,3,3.5,4))
              OR (new_course_outcome IN ('incomplete','insufficient_attendance') AND new_numeric_grade IS NULL))
         AND num_nonnulls(old_activity_outcome,new_activity_outcome,old_quality_level,new_quality_level)=0)
        OR (activity_result_id IS NOT NULL
         AND old_activity_outcome IS NOT NULL AND old_activity_outcome IN ('pass','fail')
         AND new_activity_outcome IS NOT NULL AND new_activity_outcome IN ('pass','fail')
         AND num_nonnulls(old_course_outcome,new_course_outcome,old_numeric_grade,new_numeric_grade,old_quality_level,new_quality_level)=0)
        OR (subject_student_evaluation_id IS NOT NULL
         AND old_quality_level IS NOT NULL AND old_quality_level BETWEEN 0 AND 3
         AND new_quality_level IS NOT NULL AND new_quality_level BETWEEN 0 AND 3
         AND num_nonnulls(old_course_outcome,new_course_outcome,old_numeric_grade,new_numeric_grade,old_activity_outcome,new_activity_outcome)=0)
    ),
    UNIQUE (course_result_id,expected_effective_version),
    UNIQUE (activity_result_id,expected_effective_version),
    UNIQUE (subject_student_evaluation_id,expected_effective_version)
);

-- Seed school defaults. Draft versions receive their complete bands before activation.
INSERT INTO academic_grading_policy_versions (id,version_no,name)
VALUES ('06000000-0000-0000-0000-000000000001',1,'เกณฑ์การตัดเกรดเริ่มต้น');
INSERT INTO academic_grading_policy_bands (policy_version_id,grade,lower_bound)
SELECT '06000000-0000-0000-0000-000000000001',grade,lower_bound
FROM (VALUES (0,0),(1,50),(1.5,55),(2,60),(2.5,65),(3,70),(3.5,75),(4,80)) b(grade,lower_bound);
UPDATE academic_grading_policy_versions SET lifecycle='active',activated_at=now();

INSERT INTO academic_learner_evaluation_policy_versions (id,version_no,name)
VALUES ('06000000-0000-0000-0000-000000000002',1,'เกณฑ์สรุปผลการประเมินผู้เรียนเริ่มต้น');
INSERT INTO academic_learner_evaluation_policy_bands (policy_version_id,quality_level,lower_bound)
SELECT '06000000-0000-0000-0000-000000000002',quality_level,lower_bound
FROM (VALUES (0,0.00),(1,1.00),(2,1.50),(3,2.50)) b(quality_level,lower_bound);
UPDATE academic_learner_evaluation_policy_versions SET lifecycle='active',activated_at=now();

INSERT INTO academic_learner_evaluation_criteria (domain,name,display_order)
VALUES
    ('desirable_characteristic','รักชาติ ศาสน์ กษัตริย์',1),
    ('desirable_characteristic','ซื่อสัตย์สุจริต',2),
    ('desirable_characteristic','มีวินัย',3),
    ('desirable_characteristic','ใฝ่เรียนรู้',4),
    ('desirable_characteristic','อยู่อย่างพอเพียง',5),
    ('desirable_characteristic','มุ่งมั่นในการทำงาน',6),
    ('desirable_characteristic','รักความเป็นไทย',7),
    ('desirable_characteristic','มีจิตสาธารณะ',8),
    ('reading_thinking_writing','การอ่าน',1),
    ('reading_thinking_writing','การคิดวิเคราะห์',2),
    ('reading_thinking_writing','การเขียน',3);
INSERT INTO academic_learner_evaluation_controls (academic_term_id,academic_year_id,domain)
SELECT t.id,t.academic_year_id,d.domain FROM academic_terms t
CROSS JOIN (VALUES ('desirable_characteristic'),('reading_thinking_writing')) d(domain);

-- Preserve exact totals, including a deliberate zero; never silently round old JSON values.
ALTER TABLE course_offering_details ADD COLUMN assessment_total_score NUMERIC(10,2);
ALTER TABLE course_offering_details DISABLE TRIGGER course_offering_details_published_immutable;
UPDATE course_offering_details
SET assessment_total_score=(grading_policy->>'totalScore')::numeric(10,2),
    migration_provenance=migration_provenance - 'legacyGradingPolicy';
-- Drain the existing deferred exact-subtype check before subsequent ALTER TABLE.
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE course_offering_details ENABLE TRIGGER course_offering_details_published_immutable;
ALTER TABLE course_offering_details
    ALTER COLUMN assessment_total_score SET NOT NULL,
    ALTER COLUMN assessment_total_score SET DEFAULT 100.00,
    ADD CONSTRAINT course_offering_details_assessment_total_check
        CHECK (assessment_total_score >= 0 AND assessment_total_score <> 'NaN'::numeric);

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
    JOIN academic_terms term ON term.id = plan.academic_term_id
    JOIN course_assessment_phases phase ON phase.plan_id = plan.id
    WHERE plan.assessment_coordinator_id IS NOT NULL
      AND EXISTS (
          SELECT 1
          FROM learning_groups coordinator_group
          JOIN learning_group_teachers coordinator_teacher
            ON coordinator_teacher.learning_group_id = coordinator_group.id
           AND coordinator_teacher.teacher_id = plan.assessment_coordinator_id
           AND coordinator_teacher.starts_on <= LEAST(
               GREATEST(current_date, term.start_date), term.planned_end_date
           )
           AND (
               coordinator_teacher.ends_on IS NULL
               OR coordinator_teacher.ends_on >= LEAST(
                   GREATEST(current_date, term.start_date), term.planned_end_date
               )
           )
          JOIN users coordinator
            ON coordinator.id = coordinator_teacher.teacher_id
           AND coordinator.status = 'active'
          WHERE coordinator_group.learning_offering_id = plan.learning_offering_id
            AND coordinator_group.status <> 'closed'
      )
    GROUP BY plan.id, detail.assessment_total_score
    HAVING count(phase.id) = 4
       AND count(DISTINCT phase.phase_code) = 4
       AND sum(phase.max_score) =
           detail.assessment_total_score
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
JOIN learning_group_homerooms coverage
  ON coverage.learning_group_id = learning_group.id
JOIN homerooms homeroom
  ON homeroom.id = coverage.homeroom_id
 AND homeroom.academic_year_id = ready_plan.academic_year_id
 AND homeroom.is_active
WHERE phase.phase_code IN ('midterm', 'final')
  AND phase.exam_arrangement = 'in_timetable'
  AND phase.exam_duration_minutes IS NOT NULL
  AND phase.exam_duration_minutes > 0;

ALTER TABLE course_offering_details DROP COLUMN grading_policy;

INSERT INTO academic_activity_evaluations (
    id,learning_group_id,learning_offering_id,academic_term_id,academic_year_id,
    student_academic_year_id,outcome,evidence,migration_provenance,row_version,created_at,updated_at
)
SELECT r.id,r.learning_group_id,r.learning_offering_id,r.academic_term_id,r.academic_year_id,
       r.student_academic_year_id,lower(btrim(d.outcome)),d.evidence,
       jsonb_build_object('migration',60,'legacyResult',r.migration_provenance,
                          'legacyDetail',d.migration_provenance),
       r.row_version,r.created_at,r.updated_at
FROM learning_results r JOIN activity_result_details d ON d.learning_result_id=r.id;

-- Compare every identity and copied value before either legacy relation is removed.
DO $$
BEGIN
    IF (SELECT results FROM academic_060_source_counts) <> (SELECT count(*) FROM academic_activity_evaluations)
       OR (SELECT details FROM academic_060_source_counts) <> (SELECT count(*) FROM academic_activity_evaluations)
       OR (SELECT items FROM academic_060_source_counts) <> (SELECT count(*) FROM learning_group_score_items)
       OR (SELECT controls FROM academic_060_source_counts) <> (SELECT count(*) FROM academic_gradebook_phase_controls)
       OR EXISTS (
           SELECT 1 FROM learning_results r
           JOIN activity_result_details d ON d.learning_result_id=r.id
           LEFT JOIN academic_activity_evaluations t ON t.id=r.id
           WHERE t.id IS NULL
              OR (t.learning_group_id,t.learning_offering_id,t.academic_term_id,t.academic_year_id,
                  t.student_academic_year_id,t.outcome,t.evidence,t.row_version,t.created_at,t.updated_at)
                 IS DISTINCT FROM
                 (r.learning_group_id,r.learning_offering_id,r.academic_term_id,r.academic_year_id,
                  r.student_academic_year_id,lower(btrim(d.outcome)),d.evidence,r.row_version,r.created_at,r.updated_at)
       )
    THEN
        RAISE EXCEPTION 'ACADEMIC_060_SOURCE_TARGET_RECONCILIATION_FAILED' USING ERRCODE = 'check_violation';
    END IF;
END;
$$;
DROP TABLE activity_result_details;
DROP TABLE learning_results;

-- Official initial snapshots and correction history are never updated or deleted.
CREATE FUNCTION academic_reject_official_result_mutation()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'ACADEMIC_OFFICIAL_RESULT_IMMUTABLE' USING ERRCODE = 'check_violation';
END;
$$;
CREATE TRIGGER subject_term_evaluation_locks_immutable
BEFORE UPDATE OR DELETE ON subject_term_evaluation_locks
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
CREATE TRIGGER subject_term_student_evaluations_immutable
BEFORE UPDATE OR DELETE ON subject_term_student_evaluations
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
CREATE TRIGGER academic_course_result_locks_immutable
BEFORE UPDATE OR DELETE ON academic_course_result_locks
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
CREATE TRIGGER academic_course_results_immutable
BEFORE UPDATE OR DELETE ON academic_course_results
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
CREATE TRIGGER academic_activity_result_locks_immutable
BEFORE UPDATE OR DELETE ON academic_activity_result_locks
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
CREATE TRIGGER academic_activity_results_immutable
BEFORE UPDATE OR DELETE ON academic_activity_results
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();
CREATE TRIGGER academic_result_corrections_immutable
BEFORE UPDATE OR DELETE ON academic_result_corrections
FOR EACH ROW EXECUTE FUNCTION academic_reject_official_result_mutation();

-- Exact generated-contract definitions; grants follow capability lineage, never role names.
INSERT INTO permissions (code,name,module,action,scope,description,is_active)
VALUES
    ('academic_gradebook.read.assigned','ดูสมุดคะแนนที่รับผิดชอบ','academic_gradebook','read','assigned','ดูรายการคะแนนและคะแนนผู้เรียนของรายวิชาที่ตนเองได้รับมอบหมาย',true),
    ('academic_gradebook.read.organization_unit','ดูสมุดคะแนนในกลุ่มสาระ','academic_gradebook','read','organization_unit','ดูรายการคะแนนและคะแนนผู้เรียนของรายวิชาในกลุ่มสาระเดียวกัน',true),
    ('academic_gradebook.read.school','ดูสมุดคะแนนทั้งโรงเรียน','academic_gradebook','read','school','ดูรายการคะแนนและคะแนนผู้เรียนของรายวิชาทั้งโรงเรียน',true),
    ('academic_gradebook.manage.assigned','จัดการสมุดคะแนนที่รับผิดชอบ','academic_gradebook','manage','assigned','สร้างและแก้ไขรายการคะแนนและคะแนนผู้เรียนของรายวิชาที่ตนเองได้รับมอบหมาย',true),
    ('academic_gradebook.manage.school','จัดการสมุดคะแนนทั้งโรงเรียน','academic_gradebook','manage','school','จัดการรายการคะแนน คะแนนผู้เรียน และช่วงเวลาเปิดรับคะแนนทั้งโรงเรียน',true),
    ('academic_result.read.assigned','ดูผลการเรียนที่รับผิดชอบ','academic_result','read','assigned','ดูผลการเรียนของรายวิชาหรือกิจกรรมที่ตนเองได้รับมอบหมาย',true),
    ('academic_result.read.organization_unit','ดูผลการเรียนในกลุ่มสาระ','academic_result','read','organization_unit','ดูผลการเรียนของรายวิชาหรือกิจกรรมในกลุ่มสาระเดียวกัน',true),
    ('academic_result.read.school','ดูผลการเรียนทั้งโรงเรียน','academic_result','read','school','ดูผลการเรียนของรายวิชาและกิจกรรมทั้งโรงเรียน',true),
    ('academic_result.manage.assigned','จัดการผลการเรียนที่รับผิดชอบ','academic_result','manage','assigned','ยืนยันผลการเรียนของรายวิชาหรือกิจกรรมที่ตนเองได้รับมอบหมาย',true),
    ('academic_result.manage.school','จัดการผลการเรียนทั้งโรงเรียน','academic_result','manage','school','จัดการนโยบายการตัดผลและการเตรียมผลการเรียนทั้งโรงเรียน',true),
    ('academic_result.lock.school','ล็อกผลการเรียนทั้งโรงเรียน','academic_result','lock','school','ล็อกผลการเรียนเริ่มต้นของรายวิชาและกิจกรรมเป็นข้อมูลอย่างเป็นทางการที่แก้ไขไม่ได้',true),
    ('academic_result.correct.school','แก้ไขผลการเรียนทั้งโรงเรียน','academic_result','correct','school','บันทึกการแก้ไขผลการเรียนหลังล็อกโดยเก็บประวัติแบบต่อท้าย',true),
    ('academic_learner_evaluation.read.assigned','ดูผลประเมินผู้เรียนที่รับผิดชอบ','academic_learner_evaluation','read','assigned','ดูผลประเมินคุณลักษณะและการอ่าน คิดวิเคราะห์ และเขียนของรายวิชาที่ตนเองได้รับมอบหมาย',true),
    ('academic_learner_evaluation.read.organization_unit','ดูผลประเมินผู้เรียนในกลุ่มสาระ','academic_learner_evaluation','read','organization_unit','ดูผลประเมินคุณลักษณะและการอ่าน คิดวิเคราะห์ และเขียนของรายวิชาในกลุ่มสาระเดียวกัน',true),
    ('academic_learner_evaluation.read.school','ดูผลประเมินผู้เรียนทั้งโรงเรียน','academic_learner_evaluation','read','school','ดูผลประเมินคุณลักษณะและการอ่าน คิดวิเคราะห์ และเขียนทั้งโรงเรียน',true),
    ('academic_learner_evaluation.manage.assigned','จัดการผลประเมินผู้เรียนที่รับผิดชอบ','academic_learner_evaluation','manage','assigned','บันทึกและยืนยันผลประเมินผู้เรียนของรายวิชาที่ตนเองได้รับมอบหมาย',true),
    ('academic_learner_evaluation.manage.school','จัดการผลประเมินผู้เรียนทั้งโรงเรียน','academic_learner_evaluation','manage','school','จัดการเกณฑ์ นโยบาย และช่วงเวลาเปิดรับผลประเมินผู้เรียนทั้งโรงเรียน',true),
    ('academic_learner_evaluation.lock.school','ล็อกผลประเมินผู้เรียนทั้งโรงเรียน','academic_learner_evaluation','lock','school','ล็อกผลประเมินผู้เรียนรายวิชาแยกตามด้านเป็นข้อมูลอย่างเป็นทางการที่แก้ไขไม่ได้',true),
    ('academic_learner_evaluation.correct.school','แก้ไขผลประเมินผู้เรียนทั้งโรงเรียน','academic_learner_evaluation','correct','school','บันทึกการแก้ไขผลประเมินผู้เรียนหลังล็อกโดยเก็บประวัติแบบต่อท้าย',true);

CREATE TEMP TABLE academic_060_permission_map ON COMMIT DROP AS
SELECT source.id AS source_id,target.id AS target_id,target.code AS target_code
FROM permissions source CROSS JOIN permissions target
WHERE source.module='academic_assessment'
  AND target.module IN ('academic_gradebook','academic_result','academic_learner_evaluation')
  AND ((source.action=target.action AND source.scope=target.scope)
       OR (source.action='manage' AND source.scope='school' AND target.scope='school'
           AND target.action IN ('lock','correct')));

INSERT INTO role_permissions (role_id,permission_id,created_at)
SELECT s.role_id,m.target_id,s.created_at
FROM role_permissions s JOIN academic_060_permission_map m ON m.source_id=s.permission_id
ON CONFLICT DO NOTHING;
INSERT INTO organization_permission_grants (organization_unit_id,permission_id,created_at,created_by,position_code)
SELECT s.organization_unit_id,m.target_id,s.created_at,s.created_by,s.position_code
FROM organization_permission_grants s JOIN academic_060_permission_map m ON m.source_id=s.permission_id
ON CONFLICT DO NOTHING;
INSERT INTO organization_permission_delegations (
    id,from_user_id,to_user_id,permission_id,organization_unit_id,reason,
    started_at,expires_at,revoked_at,created_at
)
SELECT uuid_generate_v5('5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
           'release-two-delegation:' || s.id::text || ':' || m.target_code),
       s.from_user_id,s.to_user_id,m.target_id,s.organization_unit_id,s.reason,
       s.started_at,s.expires_at,s.revoked_at,s.created_at
FROM organization_permission_delegations s JOIN academic_060_permission_map m ON m.source_id=s.permission_id
ON CONFLICT DO NOTHING;

CREATE INDEX learning_group_student_scores_roster_idx ON learning_group_student_scores(learning_group_id,student_academic_year_id);
CREATE INDEX learning_group_student_evaluations_roster_idx ON learning_group_student_evaluations(learning_group_id,domain,student_academic_year_id);
CREATE INDEX subject_term_evaluation_criteria_scope_idx ON subject_term_evaluation_criteria(subject_id,academic_term_id,domain,display_order);
CREATE INDEX subject_term_student_evaluations_student_idx ON subject_term_student_evaluations(student_academic_year_id,academic_term_id,domain);
