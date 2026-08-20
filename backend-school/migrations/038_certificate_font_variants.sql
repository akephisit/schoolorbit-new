-- Persist the exact static OpenType face used by a certificate template.
-- Existing font assets predate style metadata and are normal faces.

ALTER TABLE certificate_template_assets
    ADD COLUMN font_style TEXT;

UPDATE certificate_template_assets
SET font_style = 'normal'
WHERE kind = 'font';

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM certificate_template_assets
        WHERE kind = 'font'
        GROUP BY
            template_id,
            lower(btrim(font_family)),
            font_weight,
            font_style
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION
            'duplicate certificate font variants must be resolved before migration 038'
            USING ERRCODE = 'unique_violation';
    END IF;
END;
$$;

ALTER TABLE certificate_template_assets
    DROP CONSTRAINT certificate_template_assets_kind_fields_check;

ALTER TABLE certificate_template_assets
    ADD CONSTRAINT certificate_template_assets_kind_fields_check CHECK (
        (
            kind = 'image'
            AND font_family IS NULL
            AND font_weight IS NULL
            AND font_style IS NULL
            AND rights_confirmed_by IS NULL
            AND rights_confirmed_at IS NULL
        )
        OR
        (
            kind = 'font'
            AND font_family IS NOT NULL
            AND btrim(font_family) <> ''
            AND font_weight IS NOT NULL
            AND font_weight BETWEEN 100 AND 900
            AND font_weight % 100 = 0
            AND font_style IS NOT NULL
            AND font_style IN ('normal', 'italic')
            AND rights_confirmed_by IS NOT NULL
            AND rights_confirmed_at IS NOT NULL
        )
    );

CREATE UNIQUE INDEX certificate_template_assets_font_variant_unique_idx
    ON certificate_template_assets (
        template_id,
        lower(btrim(font_family)),
        font_weight,
        font_style
    )
    WHERE kind = 'font';
