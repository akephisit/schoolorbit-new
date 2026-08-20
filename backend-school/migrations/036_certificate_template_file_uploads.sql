-- Persist the certificate template selected at upload authorization time.
-- File Platform logical rows intentionally do not carry arbitrary owning-resource IDs,
-- so this domain relation prevents temporary backgrounds/assets from being reused
-- across templates after upload.

ALTER TABLE files
    ADD CONSTRAINT files_id_purpose_code_key UNIQUE (id, purpose_code),
    ADD CONSTRAINT files_certificate_template_private_check CHECK (
        purpose_code NOT IN (
            'certificate',
            'certificate_template_background',
            'certificate_template_image',
            'certificate_template_font'
        )
        OR visibility = 'private'
    );

CREATE TABLE certificate_template_file_uploads (
    file_id UUID PRIMARY KEY,
    template_id UUID NOT NULL
        REFERENCES certificate_templates(id) ON DELETE CASCADE,
    purpose_code VARCHAR(100) NOT NULL,
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_template_file_uploads_purpose_check CHECK (
        purpose_code IN (
            'certificate_template_background',
            'certificate_template_image',
            'certificate_template_font'
        )
    ),
    CONSTRAINT certificate_template_file_uploads_file_purpose_fkey
        FOREIGN KEY (file_id, purpose_code)
        REFERENCES files(id, purpose_code) ON DELETE CASCADE
);

CREATE INDEX idx_certificate_template_file_uploads_template
    ON certificate_template_file_uploads (template_id, created_at, file_id);

COMMENT ON TABLE certificate_template_file_uploads IS
    'Certificate-domain owning-resource relation captured after an authorized private upload.';
