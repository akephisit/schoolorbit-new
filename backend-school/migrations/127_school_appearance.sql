-- Appearance belongs to the existing tenant-local settings singleton. Reject
-- duplicate historical settings rather than choosing or discarding a school logo.
CREATE UNIQUE INDEX school_settings_singleton ON school_settings ((true));

ALTER TABLE school_settings
    ADD COLUMN appearance jsonb NOT NULL DEFAULT '{"palette":"teal","font":"kanit","density":"comfortable","primary":null}'::jsonb,
    ADD COLUMN appearance_revision bigint NOT NULL DEFAULT 0,
    ADD CONSTRAINT school_settings_appearance_revision_nonnegative CHECK (appearance_revision >= 0),
    ADD CONSTRAINT school_settings_appearance_valid CHECK (COALESCE(
        jsonb_typeof(appearance) = 'object'
        AND appearance ?& ARRAY['palette', 'font', 'density', 'primary']
        AND appearance - ARRAY['palette', 'font', 'density', 'primary'] = '{}'::jsonb
        AND jsonb_typeof(appearance->'palette') = 'string'
        AND appearance->>'palette' IN ('teal', 'blue', 'purple')
        AND jsonb_typeof(appearance->'font') = 'string'
        AND appearance->>'font' IN ('kanit', 'noto', 'sarabun')
        AND jsonb_typeof(appearance->'density') = 'string'
        AND appearance->>'density' IN ('comfortable', 'compact')
        AND (appearance->'primary' = 'null'::jsonb OR (
            jsonb_typeof(appearance->'primary') = 'string'
            AND appearance->>'primary' ~ '^#[0-9a-f]{6}$'
        )), false));
