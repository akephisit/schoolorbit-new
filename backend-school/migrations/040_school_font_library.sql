DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM certificate_template_assets WHERE kind = 'font'
        UNION ALL
        SELECT 1 FROM certificate_template_file_uploads
        WHERE purpose_code = 'certificate_template_font'
        UNION ALL
        SELECT 1
        FROM certificate_templates AS template
        CROSS JOIN LATERAL jsonb_array_elements(template.layout -> 'elements') AS element
        WHERE element ->> 'type' = 'text'
          AND element -> 'fontSource' ->> 'type' = 'asset'
    ) THEN
        RAISE EXCEPTION
            'legacy certificate template fonts must be empty before migration 040'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
END;
$$;

DROP INDEX certificate_template_assets_font_variant_unique_idx;

ALTER TABLE certificate_template_assets
    DROP CONSTRAINT certificate_template_assets_kind_fields_check,
    DROP CONSTRAINT certificate_template_assets_kind_check,
    DROP COLUMN font_family,
    DROP COLUMN font_weight,
    DROP COLUMN font_style,
    DROP COLUMN rights_confirmed_by,
    DROP COLUMN rights_confirmed_at,
    ADD CONSTRAINT certificate_template_assets_kind_check
        CHECK (kind = 'image');

ALTER TABLE certificate_template_file_uploads
    DROP CONSTRAINT certificate_template_file_uploads_purpose_check,
    ADD CONSTRAINT certificate_template_file_uploads_purpose_check CHECK (
        purpose_code IN (
            'certificate_template_background',
            'certificate_template_image'
        )
    );

ALTER TABLE files
    DROP CONSTRAINT files_certificate_template_private_check,
    ADD CONSTRAINT files_certificate_template_private_check CHECK (
        purpose_code NOT IN (
            'certificate',
            'certificate_template_background',
            'certificate_template_image',
            'school_font'
        )
        OR visibility = 'private'
    );

CREATE TABLE school_font_file_uploads (
    file_id UUID PRIMARY KEY,
    purpose_code VARCHAR(100) NOT NULL DEFAULT 'school_font'
        CHECK (purpose_code = 'school_font'),
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (file_id, purpose_code)
        REFERENCES files(id, purpose_code) ON DELETE CASCADE
);

CREATE TABLE certificate_school_font_file_uploads (
    file_id UUID PRIMARY KEY,
    purpose_code VARCHAR(100) NOT NULL DEFAULT 'school_font'
        CHECK (purpose_code = 'school_font'),
    template_id UUID NOT NULL
        REFERENCES certificate_templates(id) ON DELETE CASCADE,
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (file_id, purpose_code)
        REFERENCES files(id, purpose_code) ON DELETE CASCADE
);

CREATE INDEX certificate_school_font_file_uploads_template_idx
    ON certificate_school_font_file_uploads (template_id, created_at, file_id);

CREATE TABLE school_fonts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id UUID NOT NULL UNIQUE,
    purpose_code VARCHAR(100) NOT NULL DEFAULT 'school_font'
        CHECK (purpose_code = 'school_font'),
    display_name VARCHAR(200) NOT NULL CHECK (btrim(display_name) <> ''),
    font_family VARCHAR(200) NOT NULL CHECK (btrim(font_family) <> ''),
    normalized_family VARCHAR(200) NOT NULL CHECK (btrim(normalized_family) <> ''),
    font_weight SMALLINT NOT NULL
        CHECK (font_weight BETWEEN 100 AND 900 AND font_weight % 100 = 0),
    font_style TEXT NOT NULL CHECK (font_style IN ('normal', 'italic')),
    rights_confirmed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    rights_confirmed_at TIMESTAMPTZ NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (normalized_family, font_weight, font_style),
    FOREIGN KEY (file_id, purpose_code)
        REFERENCES files(id, purpose_code) ON DELETE RESTRICT
);

CREATE TRIGGER update_school_fonts_updated_at
    BEFORE UPDATE ON school_fonts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE certificate_template_font_references (
    template_id UUID NOT NULL REFERENCES certificate_templates(id) ON DELETE CASCADE,
    font_id UUID NOT NULL REFERENCES school_fonts(id) ON DELETE RESTRICT,
    PRIMARY KEY (template_id, font_id)
);

CREATE INDEX certificate_template_font_references_font_idx
    ON certificate_template_font_references (font_id, template_id);

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES (
    'font.manage.school',
    'จัดการคลังฟอนต์โรงเรียน',
    'font',
    'manage',
    'school',
    'ดู อัปโหลด และลบฟอนต์กลางของโรงเรียนโดยไม่ให้สิทธิ์แก้ไขระบบที่นำฟอนต์ไปใช้'
)
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    updated_at = NOW();
