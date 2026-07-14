-- Calendar tags are independent labels that can be attached to many events.
-- Deleting a tag only removes its junction rows; calendar events remain intact.
CREATE TABLE calendar_tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(80) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_calendar_tags_name_unique
    ON calendar_tags (LOWER(name));

CREATE INDEX idx_calendar_tags_name
    ON calendar_tags (name);

CREATE TRIGGER update_calendar_tags_updated_at
    BEFORE UPDATE ON calendar_tags
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE calendar_event_tags (
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES calendar_tags(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (event_id, tag_id)
);

CREATE INDEX idx_calendar_event_tags_tag_event
    ON calendar_event_tags (tag_id, event_id);

COMMENT ON TABLE calendar_tags IS
    'Reusable labels for calendar events. Hard deletion is safe because only calendar_event_tags rows cascade.';
COMMENT ON TABLE calendar_event_tags IS
    'Many-to-many calendar event tags. Deleting either side removes only the association.';
