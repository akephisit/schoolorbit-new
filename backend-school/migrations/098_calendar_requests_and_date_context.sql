-- Calendar dates stand independently of the academic context in the app header.
-- Preserve former context in provenance before retiring the event-level linkage.
CREATE TEMP TABLE calendar_098_snapshot ON COMMIT DROP AS
SELECT id, academic_year_id, academic_term_id,
       to_jsonb(event) - ARRAY['academic_year_id','academic_term_id','migration_provenance'] AS facts
FROM calendar_events event;

ALTER TABLE calendar_events DISABLE TRIGGER update_calendar_events_updated_at;
UPDATE calendar_events
SET migration_provenance = migration_provenance || jsonb_build_object(
    'dateCalendarContext', jsonb_build_object(
        'migration', 98, 'academicYearId', academic_year_id, 'academicTermId', academic_term_id
    )
);

ALTER TABLE calendar_events ENABLE TRIGGER update_calendar_events_updated_at;

-- A target's year remains its enrollment scope, independent of the event.
ALTER TABLE calendar_event_targets DROP CONSTRAINT calendar_event_targets_event_year_fkey;
ALTER TABLE calendar_event_targets ALTER COLUMN academic_year_id DROP NOT NULL;
ALTER TABLE calendar_event_targets ADD CONSTRAINT calendar_event_targets_enrollment_year_fkey
    FOREIGN KEY (academic_year_id) REFERENCES academic_years(id) ON DELETE RESTRICT;
ALTER TABLE calendar_events DROP CONSTRAINT calendar_events_term_context_fkey;
ALTER TABLE calendar_events DROP CONSTRAINT calendar_events_id_year_key;
ALTER TABLE calendar_events DROP COLUMN academic_term_id, DROP COLUMN academic_year_id;

DO $$
BEGIN
    IF (SELECT count(*) FROM calendar_098_snapshot) <> (SELECT count(*) FROM calendar_events)
       OR EXISTS (
           SELECT 1 FROM calendar_098_snapshot old
           LEFT JOIN calendar_events current ON current.id=old.id
           WHERE current.id IS NULL
              OR (to_jsonb(current) - 'migration_provenance') IS DISTINCT FROM old.facts
              OR current.migration_provenance->'dateCalendarContext'->'academicYearId'
                    IS DISTINCT FROM to_jsonb(old.academic_year_id)
              OR current.migration_provenance->'dateCalendarContext'->'academicTermId'
                    IS DISTINCT FROM COALESCE(to_jsonb(old.academic_term_id), 'null'::jsonb)
       ) THEN
        RAISE EXCEPTION 'CALENDAR_098_PRESERVATION_FAILED';
    END IF;
END $$;

CREATE TABLE calendar_event_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requested_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    title VARCHAR(200) NOT NULL CHECK (length(btrim(title)) > 0),
    description TEXT NOT NULL CHECK (length(btrim(description)) > 0 AND length(description) <= 5000),
    location VARCHAR(200),
    start_date DATE NOT NULL,
    end_date DATE NOT NULL CHECK (end_date >= start_date),
    all_day BOOLEAN NOT NULL,
    start_time TIME,
    end_time TIME,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    reviewed_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    reviewed_at TIMESTAMPTZ,
    rejection_reason VARCHAR(2000),
    event_id UUID UNIQUE REFERENCES calendar_events(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT calendar_requests_time_check CHECK (
        (all_day AND start_time IS NULL AND end_time IS NULL)
        OR (NOT all_day AND start_time IS NOT NULL AND end_time IS NOT NULL
            AND (end_date > start_date OR end_time > start_time))
    ),
    CONSTRAINT calendar_requests_review_check CHECK (
        (status='pending' AND reviewed_by IS NULL AND reviewed_at IS NULL AND event_id IS NULL AND rejection_reason IS NULL)
        OR (status='approved' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL AND event_id IS NOT NULL AND rejection_reason IS NULL)
        OR (status='rejected' AND reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL AND event_id IS NULL AND rejection_reason IS NOT NULL AND length(btrim(rejection_reason)) > 0)
    )
);
CREATE INDEX calendar_requests_own_queue_idx ON calendar_event_requests(requested_by, created_at DESC, id DESC);
CREATE INDEX calendar_requests_review_queue_idx ON calendar_event_requests(status, created_at DESC, id DESC);

INSERT INTO permissions(code,name,module,action,scope,description)
VALUES ('calendar.request.own','ส่งคำร้องเพิ่มกิจกรรมของตนเอง','calendar','request','own','ส่งและติดตามคำร้องเพิ่มกิจกรรมของตนเอง โดยต้องรอผู้ดูแลอนุมัติก่อนขึ้นปฏิทิน')
ON CONFLICT (code) DO UPDATE SET name=EXCLUDED.name,description=EXCLUDED.description,is_active=true;
INSERT INTO role_permissions(role_id,permission_id,created_at)
SELECT role.id,permission.id,now() FROM roles role CROSS JOIN permissions permission
WHERE role.user_type='staff' AND permission.code='calendar.request.own'
ON CONFLICT DO NOTHING;
INSERT INTO organization_permission_grants(organization_unit_id,permission_id,created_at,created_by,position_code)
SELECT unit.id,permission.id,now(),NULL,NULL
FROM organization_units unit CROSS JOIN permissions permission
WHERE unit.is_active AND permission.code='calendar.request.own'
ON CONFLICT DO NOTHING;
