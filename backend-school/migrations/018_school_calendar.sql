-- School-wide calendar V1: manual events, audience targets, public visibility, and daily reminders.

CREATE TABLE calendar_categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(120) NOT NULL,
    color VARCHAR(32) NOT NULL,
    order_index INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_calendar_categories_active_name_unique
    ON calendar_categories (lower(name))
    WHERE is_active = true;

CREATE INDEX idx_calendar_categories_order
    ON calendar_categories (is_active, order_index, name);

CREATE TRIGGER update_calendar_categories_updated_at
    BEFORE UPDATE ON calendar_categories
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    category_id UUID REFERENCES calendar_categories(id) ON DELETE SET NULL,
    title VARCHAR(200) NOT NULL,
    description TEXT,
    location VARCHAR(200),
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    all_day BOOLEAN NOT NULL DEFAULT true,
    start_time TIME,
    end_time TIME,
    is_public BOOLEAN NOT NULL DEFAULT false,
    source_type VARCHAR(50),
    source_id UUID,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT calendar_events_valid_date_range CHECK (end_date >= start_date),
    CONSTRAINT calendar_events_valid_time_range CHECK (
        all_day = true
        OR start_date <> end_date
        OR start_time IS NULL
        OR end_time IS NULL
        OR end_time > start_time
    )
);

CREATE INDEX idx_calendar_events_range
    ON calendar_events (start_date, end_date)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_calendar_events_public_range
    ON calendar_events (start_date, end_date)
    WHERE deleted_at IS NULL AND is_public = true;

CREATE INDEX idx_calendar_events_category
    ON calendar_events (category_id)
    WHERE deleted_at IS NULL;

CREATE TRIGGER update_calendar_events_updated_at
    BEFORE UPDATE ON calendar_events
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE calendar_event_targets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    audience_type VARCHAR(20) NOT NULL,
    grade_level_id UUID REFERENCES grade_levels(id) ON DELETE CASCADE,
    class_room_id UUID REFERENCES class_rooms(id) ON DELETE CASCADE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT calendar_event_targets_single_scope CHECK (grade_level_id IS NULL OR class_room_id IS NULL),
    CONSTRAINT calendar_event_targets_audience_type CHECK (
        audience_type IN ('all', 'staff', 'student', 'parent')
    ),
    CONSTRAINT calendar_event_targets_all_scope CHECK (
        audience_type <> 'all' OR (grade_level_id IS NULL AND class_room_id IS NULL)
    ),
    CONSTRAINT calendar_event_targets_staff_scope CHECK (
        audience_type <> 'staff' OR (grade_level_id IS NULL AND class_room_id IS NULL)
    )
);

CREATE INDEX idx_calendar_event_targets_event
    ON calendar_event_targets (event_id);

CREATE INDEX idx_calendar_event_targets_audience
    ON calendar_event_targets (audience_type, grade_level_id, class_room_id);

CREATE UNIQUE INDEX idx_calendar_event_targets_unique_global
    ON calendar_event_targets (event_id, audience_type)
    WHERE grade_level_id IS NULL AND class_room_id IS NULL;

CREATE UNIQUE INDEX idx_calendar_event_targets_unique_grade
    ON calendar_event_targets (event_id, audience_type, grade_level_id)
    WHERE grade_level_id IS NOT NULL AND class_room_id IS NULL;

CREATE UNIQUE INDEX idx_calendar_event_targets_unique_class
    ON calendar_event_targets (event_id, audience_type, class_room_id)
    WHERE class_room_id IS NOT NULL AND grade_level_id IS NULL;

CREATE TRIGGER update_calendar_event_targets_updated_at
    BEFORE UPDATE ON calendar_event_targets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE calendar_event_reminders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    days_before INTEGER NOT NULL,
    remind_on DATE NOT NULL,
    sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT calendar_event_reminders_days_positive CHECK (days_before > 0)
);

CREATE INDEX idx_calendar_event_reminders_due
    ON calendar_event_reminders (remind_on)
    WHERE sent_at IS NULL;

CREATE UNIQUE INDEX idx_calendar_event_reminders_unique_offset
    ON calendar_event_reminders (event_id, days_before);

CREATE TRIGGER update_calendar_event_reminders_updated_at
    BEFORE UPDATE ON calendar_event_reminders
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    (
        'calendar.read.school',
        'ดูปฏิทินโรงเรียน',
        'calendar',
        'read',
        'school',
        'ดูปฏิทินและกำหนดการของโรงเรียน'
    ),
    (
        'calendar.manage.school',
        'จัดการปฏิทินโรงเรียน',
        'calendar',
        'manage',
        'school',
        'สร้าง แก้ไข ลบ หมวดหมู่ กลุ่มผู้เห็น และการแจ้งเตือนของปฏิทินโรงเรียน'
    )
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description;
