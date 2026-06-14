-- Teaching supervision foundation

CREATE TABLE supervision_templates (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    title character varying(200) NOT NULL,
    description text,
    status character varying(20) DEFAULT 'draft' NOT NULL,
    rating_min integer DEFAULT 1 NOT NULL,
    rating_max integer DEFAULT 5 NOT NULL,
    created_by uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT supervision_templates_status_check CHECK (status IN ('draft', 'active', 'archived')),
    CONSTRAINT supervision_templates_rating_range_check CHECK (rating_min < rating_max)
);

CREATE TABLE supervision_template_sections (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    template_id uuid NOT NULL REFERENCES supervision_templates(id) ON DELETE CASCADE,
    title character varying(200) NOT NULL,
    description text,
    sort_order integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE supervision_template_items (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    section_id uuid NOT NULL REFERENCES supervision_template_sections(id) ON DELETE CASCADE,
    label text NOT NULL,
    description text,
    item_type character varying(20) NOT NULL,
    required boolean DEFAULT true NOT NULL,
    sort_order integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT supervision_template_items_type_check CHECK (item_type IN ('rating', 'text'))
);

CREATE TABLE supervision_template_steps (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    template_id uuid NOT NULL REFERENCES supervision_templates(id) ON DELETE CASCADE,
    step_order integer NOT NULL,
    step_code character varying(100) NOT NULL,
    label character varying(200) NOT NULL,
    actor_kind character varying(40) NOT NULL,
    actor_permission character varying(120),
    organization_position_code character varying(100),
    action_kind character varying(40) NOT NULL,
    required boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT supervision_template_steps_actor_kind_check CHECK (
        actor_kind IN ('supervisor', 'observed_teacher', 'permission', 'organization_position')
    ),
    CONSTRAINT supervision_template_steps_action_kind_check CHECK (
        action_kind IN ('submit', 'approve', 'return_for_revision', 'publish', 'acknowledge', 'sign')
    ),
    CONSTRAINT supervision_template_steps_permission_check CHECK (
        (actor_kind = 'permission' AND actor_permission IS NOT NULL)
        OR (actor_kind <> 'permission' AND actor_permission IS NULL)
    ),
    CONSTRAINT supervision_template_steps_position_check CHECK (
        (actor_kind = 'organization_position' AND organization_position_code IS NOT NULL)
        OR (actor_kind <> 'organization_position' AND organization_position_code IS NULL)
    )
);

CREATE TABLE supervision_cycles (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    academic_year integer NOT NULL,
    semester character varying(20) NOT NULL,
    academic_semester_id uuid REFERENCES academic_semesters(id) ON DELETE SET NULL,
    title character varying(200) NOT NULL,
    description text,
    template_id uuid NOT NULL REFERENCES supervision_templates(id) ON DELETE RESTRICT,
    booking_opens_at timestamp with time zone,
    booking_closes_at timestamp with time zone,
    starts_at timestamp with time zone NOT NULL,
    ends_at timestamp with time zone NOT NULL,
    status character varying(20) DEFAULT 'draft' NOT NULL,
    created_by uuid REFERENCES users(id) ON DELETE SET NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT supervision_cycles_status_check CHECK (status IN ('draft', 'open', 'closed', 'archived')),
    CONSTRAINT supervision_cycles_window_check CHECK (starts_at <= ends_at),
    CONSTRAINT supervision_cycles_booking_window_check CHECK (
        booking_opens_at IS NULL
        OR booking_closes_at IS NULL
        OR booking_opens_at <= booking_closes_at
    )
);

CREATE TABLE supervision_cycle_targets (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cycle_id uuid NOT NULL REFERENCES supervision_cycles(id) ON DELETE CASCADE,
    target_type character varying(40) NOT NULL,
    target_id uuid,
    required_observations integer DEFAULT 1 NOT NULL,
    priority integer DEFAULT 100 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT supervision_cycle_targets_type_check CHECK (
        target_type IN ('school', 'organization_unit', 'subject_group', 'staff')
    ),
    CONSTRAINT supervision_cycle_targets_required_check CHECK (required_observations > 0),
    CONSTRAINT supervision_cycle_targets_shape_check CHECK (
        (target_type = 'school' AND target_id IS NULL)
        OR (target_type <> 'school' AND target_id IS NOT NULL)
    )
);

CREATE TABLE supervision_observations (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    cycle_id uuid NOT NULL REFERENCES supervision_cycles(id) ON DELETE CASCADE,
    observed_user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    requested_by uuid REFERENCES users(id) ON DELETE SET NULL,
    approved_by uuid REFERENCES users(id) ON DELETE SET NULL,
    template_id uuid NOT NULL REFERENCES supervision_templates(id) ON DELETE RESTRICT,
    timetable_entry_id uuid REFERENCES academic_timetable_entries(id) ON DELETE SET NULL,
    manual_subject_name character varying(200),
    manual_classroom_label character varying(120),
    manual_room_label character varying(120),
    manual_observed_at timestamp with time zone,
    manual_period_label character varying(120),
    manual_reason text,
    lesson_snapshot jsonb DEFAULT '{}'::jsonb NOT NULL,
    status character varying(40) DEFAULT 'requested' NOT NULL,
    requested_at timestamp with time zone DEFAULT now() NOT NULL,
    approved_at timestamp with time zone,
    cancelled_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT supervision_observations_status_check CHECK (
        status IN (
            'requested',
            'planned',
            'in_progress',
            'evaluators_submitted',
            'under_review',
            'returned',
            'approved',
            'published',
            'acknowledged',
            'completed',
            'cancelled'
        )
    ),
    CONSTRAINT supervision_observations_lesson_source_check CHECK (
        timetable_entry_id IS NOT NULL
        OR (
            manual_subject_name IS NOT NULL
            AND manual_classroom_label IS NOT NULL
            AND manual_observed_at IS NOT NULL
            AND manual_period_label IS NOT NULL
            AND manual_reason IS NOT NULL
        )
    )
);

CREATE TABLE supervision_evaluators (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    observation_id uuid NOT NULL REFERENCES supervision_observations(id) ON DELETE CASCADE,
    evaluator_user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_label character varying(120),
    is_required boolean DEFAULT true NOT NULL,
    status character varying(20) DEFAULT 'assigned' NOT NULL,
    submitted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT supervision_evaluators_status_check CHECK (status IN ('assigned', 'draft', 'submitted')),
    CONSTRAINT supervision_evaluators_unique_user UNIQUE (observation_id, evaluator_user_id)
);

CREATE TABLE supervision_evaluator_responses (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    observation_id uuid NOT NULL REFERENCES supervision_observations(id) ON DELETE CASCADE,
    evaluator_id uuid NOT NULL REFERENCES supervision_evaluators(id) ON DELETE CASCADE,
    template_item_id uuid NOT NULL REFERENCES supervision_template_items(id) ON DELETE RESTRICT,
    rating_score numeric(6,2),
    text_response text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT supervision_evaluator_responses_score_check CHECK (
        rating_score IS NULL OR rating_score >= 0
    ),
    CONSTRAINT supervision_evaluator_responses_unique_item UNIQUE (evaluator_id, template_item_id)
);

CREATE TABLE supervision_actions (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY,
    observation_id uuid NOT NULL REFERENCES supervision_observations(id) ON DELETE CASCADE,
    actor_user_id uuid REFERENCES users(id) ON DELETE SET NULL,
    action_kind character varying(60) NOT NULL,
    from_status character varying(40),
    to_status character varying(40),
    comment text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE INDEX idx_supervision_template_sections_template ON supervision_template_sections(template_id, sort_order);
CREATE INDEX idx_supervision_template_items_section ON supervision_template_items(section_id, sort_order);
CREATE INDEX idx_supervision_template_steps_template ON supervision_template_steps(template_id, step_order);
CREATE INDEX idx_supervision_cycles_status ON supervision_cycles(status);
CREATE INDEX idx_supervision_cycles_semester ON supervision_cycles(academic_year, semester);
CREATE INDEX idx_supervision_cycle_targets_cycle ON supervision_cycle_targets(cycle_id, target_type, target_id);
CREATE INDEX idx_supervision_observations_cycle_status ON supervision_observations(cycle_id, status);
CREATE INDEX idx_supervision_observations_observed ON supervision_observations(observed_user_id, status);
CREATE INDEX idx_supervision_observations_timetable ON supervision_observations(timetable_entry_id) WHERE timetable_entry_id IS NOT NULL;
CREATE INDEX idx_supervision_evaluators_observation ON supervision_evaluators(observation_id, status);
CREATE INDEX idx_supervision_evaluators_user ON supervision_evaluators(evaluator_user_id, status);
CREATE INDEX idx_supervision_responses_observation ON supervision_evaluator_responses(observation_id);
CREATE INDEX idx_supervision_actions_observation ON supervision_actions(observation_id, created_at);

CREATE TRIGGER update_supervision_templates_updated_at
    BEFORE UPDATE ON supervision_templates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_supervision_template_sections_updated_at
    BEFORE UPDATE ON supervision_template_sections
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_supervision_template_items_updated_at
    BEFORE UPDATE ON supervision_template_items
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_supervision_template_steps_updated_at
    BEFORE UPDATE ON supervision_template_steps
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_supervision_cycles_updated_at
    BEFORE UPDATE ON supervision_cycles
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_supervision_cycle_targets_updated_at
    BEFORE UPDATE ON supervision_cycle_targets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_supervision_observations_updated_at
    BEFORE UPDATE ON supervision_observations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_supervision_evaluators_updated_at
    BEFORE UPDATE ON supervision_evaluators
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_supervision_evaluator_responses_updated_at
    BEFORE UPDATE ON supervision_evaluator_responses
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

COMMENT ON TABLE supervision_cycles IS 'รอบการนิเทศการสอนตามปีการศึกษาและภาคเรียน';
COMMENT ON TABLE supervision_cycle_targets IS 'กลุ่มเป้าหมายของรอบนิเทศและจำนวนครั้งที่ต้องนิเทศ';
COMMENT ON TABLE supervision_templates IS 'แบบประเมินนิเทศการสอนระดับโรงเรียน';
COMMENT ON TABLE supervision_template_items IS 'หัวข้อประเมินนิเทศแบบ rating หรือ text';
COMMENT ON TABLE supervision_observations IS 'รายการจองและผลนิเทศการสอนรายคาบ';
COMMENT ON TABLE supervision_evaluators IS 'ผู้ประเมินที่ได้รับมอบหมายในรายการนิเทศ';
COMMENT ON TABLE supervision_evaluator_responses IS 'คำตอบของผู้ประเมินแต่ละคน';
COMMENT ON TABLE supervision_actions IS 'ประวัติสถานะและการรับทราบของรายการนิเทศ';
