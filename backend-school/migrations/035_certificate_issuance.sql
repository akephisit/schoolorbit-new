-- Certificate campaigns, template design, approval, issuance, and verification.
--
-- Issued certificate rows are permanent snapshots. Templates remain live so later
-- renders use the current layout without storing one generated PDF per recipient.

ALTER TABLE files
    ADD COLUMN inspection_metadata JSONB NOT NULL DEFAULT '{"kind":"unknown"}'::jsonb,
    ADD CONSTRAINT files_inspection_metadata_check CHECK (
        jsonb_typeof(inspection_metadata) = 'object'
        AND jsonb_typeof(inspection_metadata -> 'kind') = 'string'
        AND inspection_metadata ->> 'kind' IN ('unknown', 'image', 'pdf', 'font')
    );

INSERT INTO permissions (code, name, module, action, scope, description)
VALUES
    (
        'certificate.read.own',
        'ดูเกียรติบัตรของตนเอง',
        'certificate',
        'read',
        'own',
        'ดูเกียรติบัตรที่เชื่อมกับบัญชีของตนเอง'
    ),
    (
        'certificate.read.organization_unit',
        'ดูเกียรติบัตรในหน่วยงาน',
        'certificate',
        'read',
        'organization_unit',
        'ดูโครงการ แบบ และรายชื่อผู้รับของหน่วยงานที่ได้รับสิทธิ์โดยตรง'
    ),
    (
        'certificate.read.school',
        'ดูเกียรติบัตรทั้งโรงเรียน',
        'certificate',
        'read',
        'school',
        'ดูโครงการ คำขอออก และเกียรติบัตรทั้งหมดของโรงเรียน'
    ),
    (
        'certificate.create.organization_unit',
        'สร้างโครงการเกียรติบัตรในหน่วยงาน',
        'certificate',
        'create',
        'organization_unit',
        'สร้างโครงการ แบบ และรายชื่อผู้รับในหน่วยงานที่ได้รับสิทธิ์โดยตรง'
    ),
    (
        'certificate.create.school',
        'สร้างโครงการเกียรติบัตรระดับโรงเรียน',
        'certificate',
        'create',
        'school',
        'สร้างโครงการ แบบ และรายชื่อผู้รับในขอบเขตทั้งโรงเรียน'
    ),
    (
        'certificate.update.organization_unit',
        'แก้ไขเกียรติบัตรในหน่วยงาน',
        'certificate',
        'update',
        'organization_unit',
        'แก้ไขโครงการ แบบ และรายชื่อผู้รับของหน่วยงานที่ได้รับสิทธิ์โดยตรง'
    ),
    (
        'certificate.update.school',
        'แก้ไขเกียรติบัตรทั้งโรงเรียน',
        'certificate',
        'update',
        'school',
        'แก้ไขโครงการ แบบ และรายชื่อผู้รับในขอบเขตทั้งโรงเรียน'
    ),
    (
        'certificate.delete.organization_unit',
        'ลบร่างเกียรติบัตรในหน่วยงาน',
        'certificate',
        'delete',
        'organization_unit',
        'ลบโครงการ แบบ หรือรายชื่อที่ยังไม่ออกเกียรติบัตรของหน่วยงานที่ได้รับสิทธิ์โดยตรง'
    ),
    (
        'certificate.delete.school',
        'ลบร่างเกียรติบัตรทั้งโรงเรียน',
        'certificate',
        'delete',
        'school',
        'ลบโครงการ แบบ หรือรายชื่อที่ยังไม่ออกเกียรติบัตรในขอบเขตทั้งโรงเรียน'
    ),
    (
        'certificate.submit.organization_unit',
        'ส่งคำขอออกเกียรติบัตรของหน่วยงาน',
        'certificate',
        'submit',
        'organization_unit',
        'ส่งรายชื่อที่พร้อมแล้วให้ผู้มีสิทธิ์ระดับโรงเรียนตรวจสอบและออกเลข'
    ),
    (
        'certificate.submit.school',
        'ส่งคำขอออกเกียรติบัตรระดับโรงเรียน',
        'certificate',
        'submit',
        'school',
        'ส่งรายชื่อระดับโรงเรียนเข้าสู่กระบวนการตรวจสอบและออกเลข'
    ),
    (
        'certificate.issue.school',
        'ออกเลขเกียรติบัตรทั้งโรงเรียน',
        'certificate',
        'issue',
        'school',
        'ตรวจสอบคำขอและออกเลขเกียรติบัตรแบบเรียงลำดับภายในธุรกรรม'
    ),
    (
        'certificate.revoke.school',
        'เพิกถอนเกียรติบัตรทั้งโรงเรียน',
        'certificate',
        'revoke',
        'school',
        'เพิกถอนเกียรติบัตรที่ออกแล้วโดยเก็บประวัติและเหตุผลไว้'
    ),
    (
        'certificate.download.organization_unit',
        'ดาวน์โหลดเกียรติบัตรในหน่วยงาน',
        'certificate',
        'download',
        'organization_unit',
        'สร้างและดาวน์โหลดไฟล์เกียรติบัตรของหน่วยงานที่ได้รับสิทธิ์โดยตรง'
    ),
    (
        'certificate.download.school',
        'ดาวน์โหลดเกียรติบัตรทั้งโรงเรียน',
        'certificate',
        'download',
        'school',
        'สร้างและดาวน์โหลดไฟล์เกียรติบัตรในขอบเขตทั้งโรงเรียน'
    )
ON CONFLICT (code) DO UPDATE SET
    name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    updated_at = NOW();

WITH own_permission AS (
    SELECT id
    FROM permissions
    WHERE code = 'certificate.read.own'
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT roles.id, own_permission.id, NOW()
FROM roles
CROSS JOIN own_permission
WHERE roles.is_active IS TRUE
  AND roles.user_type IN ('staff', 'student')
ON CONFLICT (role_id, permission_id) DO NOTHING;

WITH school_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'certificate.read.school',
        'certificate.create.school',
        'certificate.update.school',
        'certificate.delete.school',
        'certificate.submit.school',
        'certificate.issue.school',
        'certificate.revoke.school',
        'certificate.download.school'
    )
),
admin_roles AS (
    SELECT id
    FROM roles
    WHERE user_type = 'staff'
      AND (
        upper(code) IN ('ADMIN', 'SUPER_ADMIN', 'SCHOOL_ADMIN')
        OR lower(name) IN ('admin', 'administrator', 'super admin', 'school admin')
        OR lower(COALESCE(name_en, '')) IN (
            'admin',
            'administrator',
            'system admin',
            'super admin',
            'school admin'
        )
      )
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT admin_roles.id, school_permissions.id, NOW()
FROM admin_roles
CROSS JOIN school_permissions
ON CONFLICT (role_id, permission_id) DO NOTHING;

WITH unit_permissions AS (
    SELECT id
    FROM permissions
    WHERE code IN (
        'certificate.read.organization_unit',
        'certificate.create.organization_unit',
        'certificate.update.organization_unit',
        'certificate.delete.organization_unit',
        'certificate.submit.organization_unit',
        'certificate.download.organization_unit'
    )
),
active_units AS (
    SELECT id
    FROM organization_units
    WHERE is_active IS TRUE
),
eligible_positions AS (
    SELECT position_code
    FROM (
        VALUES ('head'), ('deputy_head'), ('coordinator')
    ) AS positions(position_code)
)
INSERT INTO organization_permission_grants (
    organization_unit_id,
    permission_id,
    created_at,
    position_code
)
SELECT active_units.id, unit_permissions.id, NOW(), eligible_positions.position_code
FROM active_units
CROSS JOIN unit_permissions
CROSS JOIN eligible_positions
ON CONFLICT DO NOTHING;

CREATE TABLE certificate_academic_year_counters (
    academic_year_id UUID PRIMARY KEY
        REFERENCES academic_years(id) ON DELETE RESTRICT,
    next_activity_sequence INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_academic_year_counters_next_sequence_check
        CHECK (next_activity_sequence BETWEEN 1 AND 10000)
);

CREATE TABLE certificate_campaigns (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL
        REFERENCES academic_years(id) ON DELETE RESTRICT,
    owner_organization_unit_id UUID
        REFERENCES organization_units(id) ON DELETE RESTRICT,
    name VARCHAR(200) NOT NULL,
    event_date DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    activity_sequence INTEGER,
    next_certificate_sequence INTEGER NOT NULL DEFAULT 1,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_campaigns_name_check
        CHECK (btrim(name) <> ''),
    CONSTRAINT certificate_campaigns_status_check
        CHECK (status IN ('draft', 'active', 'closed', 'archived')),
    CONSTRAINT certificate_campaigns_activity_sequence_check
        CHECK (activity_sequence BETWEEN 1 AND 9999),
    CONSTRAINT certificate_campaigns_next_sequence_check
        CHECK (next_certificate_sequence BETWEEN 1 AND 1000000)
);

CREATE UNIQUE INDEX certificate_campaigns_year_activity_unique
    ON certificate_campaigns (academic_year_id, activity_sequence)
    WHERE activity_sequence IS NOT NULL;

CREATE TABLE certificate_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL
        REFERENCES certificate_campaigns(id) ON DELETE CASCADE,
    name VARCHAR(200) NOT NULL,
    normalized_name VARCHAR(200) NOT NULL,
    background_file_id UUID REFERENCES files(id) ON DELETE RESTRICT,
    crop_box_x DOUBLE PRECISION,
    crop_box_y DOUBLE PRECISION,
    crop_box_width DOUBLE PRECISION,
    crop_box_height DOUBLE PRECISION,
    media_box_x DOUBLE PRECISION,
    media_box_y DOUBLE PRECISION,
    media_box_width DOUBLE PRECISION,
    media_box_height DOUBLE PRECISION,
    page_rotation SMALLINT,
    paper_label VARCHAR(50),
    safe_margin_points DOUBLE PRECISION NOT NULL DEFAULT 28.3464566929,
    show_safe_area BOOLEAN NOT NULL DEFAULT TRUE,
    allowed_recipient_types TEXT[] NOT NULL
        DEFAULT ARRAY['student', 'staff', 'external']::TEXT[],
    layout JSONB NOT NULL DEFAULT '{"schemaVersion":1,"elements":[]}'::jsonb,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_templates_name_check
        CHECK (btrim(name) <> '' AND btrim(normalized_name) <> ''),
    CONSTRAINT certificate_templates_background_geometry_check CHECK (
        num_nonnulls(
            background_file_id,
            crop_box_x,
            crop_box_y,
            crop_box_width,
            crop_box_height,
            media_box_x,
            media_box_y,
            media_box_width,
            media_box_height,
            page_rotation,
            paper_label
        ) IN (0, 11)
    ),
    CONSTRAINT certificate_templates_geometry_dimensions_check CHECK (
        background_file_id IS NULL
        OR (
            crop_box_width > 0
            AND crop_box_height > 0
            AND media_box_width > 0
            AND media_box_height > 0
            AND crop_box_x::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
            AND crop_box_y::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
            AND crop_box_width::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
            AND crop_box_height::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
            AND media_box_x::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
            AND media_box_y::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
            AND media_box_width::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
            AND media_box_height::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
            AND btrim(paper_label) <> ''
        )
    ),
    CONSTRAINT certificate_templates_page_rotation_check
        CHECK (page_rotation IS NULL OR page_rotation IN (0, 90, 180, 270)),
    CONSTRAINT certificate_templates_safe_margin_check CHECK (
        safe_margin_points >= 0
        AND safe_margin_points::TEXT NOT IN ('NaN', 'Infinity', '-Infinity')
    ),
    CONSTRAINT certificate_templates_recipient_types_check CHECK (
        cardinality(allowed_recipient_types) BETWEEN 1 AND 3
        AND allowed_recipient_types <@ ARRAY['student', 'staff', 'external']::TEXT[]
        AND array_position(allowed_recipient_types, NULL) IS NULL
    ),
    CONSTRAINT certificate_templates_layout_check CHECK (
        jsonb_typeof(layout) = 'object'
        AND layout @> '{"schemaVersion":1}'::jsonb
        AND jsonb_typeof(layout -> 'elements') = 'array'
    ),
    UNIQUE (campaign_id, normalized_name),
    UNIQUE (id, campaign_id)
);

CREATE TABLE certificate_template_assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL
        REFERENCES certificate_templates(id) ON DELETE CASCADE,
    file_id UUID NOT NULL UNIQUE REFERENCES files(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL,
    display_name VARCHAR(200) NOT NULL,
    font_family VARCHAR(200),
    font_weight SMALLINT,
    rights_confirmed_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    rights_confirmed_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_template_assets_kind_check
        CHECK (kind IN ('image', 'font')),
    CONSTRAINT certificate_template_assets_display_name_check
        CHECK (btrim(display_name) <> ''),
    CONSTRAINT certificate_template_assets_kind_fields_check CHECK (
        (
            kind = 'image'
            AND font_family IS NULL
            AND font_weight IS NULL
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
            AND rights_confirmed_by IS NOT NULL
            AND rights_confirmed_at IS NOT NULL
        )
    )
);

CREATE TABLE certificate_import_batches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL
        REFERENCES certificate_campaigns(id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    row_count INTEGER NOT NULL,
    custom_headers TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    ready_count INTEGER NOT NULL DEFAULT 0,
    review_count INTEGER NOT NULL DEFAULT 0,
    invalid_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'processed',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_import_batches_source_check
        CHECK (source IN ('xlsx', 'csv', 'manual', 'account_search', 'replacement')),
    CONSTRAINT certificate_import_batches_row_count_check
        CHECK (row_count BETWEEN 1 AND 5000),
    CONSTRAINT certificate_import_batches_custom_headers_check CHECK (
        cardinality(custom_headers) <= 64
        AND array_position(custom_headers, NULL) IS NULL
    ),
    CONSTRAINT certificate_import_batches_counts_check CHECK (
        ready_count >= 0
        AND review_count >= 0
        AND invalid_count >= 0
        AND ready_count + review_count + invalid_count = row_count
    ),
    CONSTRAINT certificate_import_batches_status_check
        CHECK (status = 'processed'),
    UNIQUE (id, campaign_id)
);

CREATE TABLE certificate_candidates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL
        REFERENCES certificate_campaigns(id) ON DELETE CASCADE,
    batch_id UUID,
    template_id UUID,
    recipient_type TEXT NOT NULL,
    matched_user_id UUID REFERENCES users(id) ON DELETE RESTRICT,
    lookup_student_id VARCHAR(50),
    lookup_staff_username VARCHAR(100),
    imported_title VARCHAR(100),
    imported_first_name VARCHAR(100) NOT NULL DEFAULT '',
    imported_last_name VARCHAR(100) NOT NULL DEFAULT '',
    account_title VARCHAR(100),
    account_first_name VARCHAR(100),
    account_last_name VARCHAR(100),
    selected_name_source TEXT,
    activity_item VARCHAR(500),
    award_or_role VARCHAR(500),
    custom_values JSONB NOT NULL DEFAULT '{}'::jsonb,
    match_status TEXT NOT NULL,
    validation_status TEXT NOT NULL,
    validation_codes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    duplicate_confirmed BOOLEAN NOT NULL DEFAULT FALSE,
    replacement_for_certificate_id UUID UNIQUE,
    issued_certificate_id UUID UNIQUE,
    deleted_at TIMESTAMPTZ,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_candidates_recipient_type_check
        CHECK (recipient_type IN ('student', 'staff', 'external')),
    CONSTRAINT certificate_candidates_lookup_check
        CHECK (num_nonnulls(lookup_student_id, lookup_staff_username) <= 1),
    CONSTRAINT certificate_candidates_lookup_nonblank_check CHECK (
        (lookup_student_id IS NULL OR btrim(lookup_student_id) <> '')
        AND (lookup_staff_username IS NULL OR btrim(lookup_staff_username) <> '')
    ),
    CONSTRAINT certificate_candidates_external_account_check
        CHECK (recipient_type <> 'external' OR matched_user_id IS NULL),
    CONSTRAINT certificate_candidates_selected_name_source_check
        CHECK (selected_name_source IS NULL OR selected_name_source IN ('file', 'account')),
    CONSTRAINT certificate_candidates_account_name_check CHECK (
        selected_name_source <> 'account'
        OR (
            matched_user_id IS NOT NULL
            AND account_first_name IS NOT NULL
            AND btrim(account_first_name) <> ''
            AND account_last_name IS NOT NULL
            AND btrim(account_last_name) <> ''
        )
    ),
    CONSTRAINT certificate_candidates_match_status_check CHECK (
        match_status IN (
            'matched',
            'name_mismatch',
            'not_found',
            'inactive',
            'external_confirmed',
            'not_applicable'
        )
    ),
    CONSTRAINT certificate_candidates_validation_status_check
        CHECK (validation_status IN ('ready', 'needs_review', 'invalid')),
    CONSTRAINT certificate_candidates_validation_codes_check CHECK (
        cardinality(validation_codes) <= 64
        AND array_position(validation_codes, NULL) IS NULL
    ),
    CONSTRAINT certificate_candidates_custom_values_check
        CHECK (jsonb_typeof(custom_values) = 'object'),
    CONSTRAINT certificate_candidates_issued_lookup_cleared_check CHECK (
        issued_certificate_id IS NULL
        OR (lookup_student_id IS NULL AND lookup_staff_username IS NULL)
    ),
    FOREIGN KEY (batch_id, campaign_id)
        REFERENCES certificate_import_batches(id, campaign_id) ON DELETE RESTRICT,
    FOREIGN KEY (template_id, campaign_id)
        REFERENCES certificate_templates(id, campaign_id) ON DELETE RESTRICT,
    UNIQUE (id, campaign_id)
);

CREATE TABLE certificate_issue_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL
        REFERENCES certificate_campaigns(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'pending',
    submitted_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    reviewed_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at TIMESTAMPTZ,
    returned_at TIMESTAMPTZ,
    withdrawn_at TIMESTAMPTZ,
    issued_at TIMESTAMPTZ,
    return_note VARCHAR(500),
    issue_codes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_issue_requests_status_check CHECK (
        status IN ('pending', 'reviewing', 'returned', 'withdrawn', 'issued')
    ),
    CONSTRAINT certificate_issue_requests_return_note_check CHECK (
        return_note IS NULL OR btrim(return_note) <> ''
    ),
    CONSTRAINT certificate_issue_requests_issue_codes_check CHECK (
        cardinality(issue_codes) <= 64
        AND array_position(issue_codes, NULL) IS NULL
    ),
    CONSTRAINT certificate_issue_requests_transition_fields_check CHECK (
        (
            status = 'pending'
            AND reviewed_by IS NULL
            AND reviewed_at IS NULL
            AND returned_at IS NULL
            AND withdrawn_at IS NULL
            AND issued_at IS NULL
            AND return_note IS NULL
        )
        OR
        (
            status = 'reviewing'
            AND reviewed_by IS NOT NULL
            AND reviewed_at IS NOT NULL
            AND returned_at IS NULL
            AND withdrawn_at IS NULL
            AND issued_at IS NULL
            AND return_note IS NULL
        )
        OR
        (
            status = 'returned'
            AND reviewed_by IS NOT NULL
            AND reviewed_at IS NOT NULL
            AND returned_at IS NOT NULL
            AND withdrawn_at IS NULL
            AND issued_at IS NULL
            AND return_note IS NOT NULL
        )
        OR
        (
            status = 'withdrawn'
            AND num_nonnulls(reviewed_by, reviewed_at) IN (0, 2)
            AND returned_at IS NULL
            AND withdrawn_at IS NOT NULL
            AND issued_at IS NULL
            AND return_note IS NULL
        )
        OR
        (
            status = 'issued'
            AND reviewed_by IS NOT NULL
            AND reviewed_at IS NOT NULL
            AND returned_at IS NULL
            AND withdrawn_at IS NULL
            AND issued_at IS NOT NULL
            AND return_note IS NULL
        )
    ),
    UNIQUE (id, campaign_id)
);

CREATE TABLE certificate_issue_request_items (
    request_id UUID NOT NULL,
    candidate_id UUID NOT NULL,
    campaign_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (request_id, candidate_id),
    FOREIGN KEY (request_id, campaign_id)
        REFERENCES certificate_issue_requests(id, campaign_id) ON DELETE RESTRICT,
    FOREIGN KEY (candidate_id, campaign_id)
        REFERENCES certificate_candidates(id, campaign_id) ON DELETE RESTRICT
);

CREATE TABLE certificate_candidate_issue_locks (
    candidate_id UUID PRIMARY KEY
        REFERENCES certificate_candidates(id) ON DELETE RESTRICT,
    request_id UUID NOT NULL
        REFERENCES certificate_issue_requests(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (request_id, candidate_id)
        REFERENCES certificate_issue_request_items(request_id, candidate_id)
        ON DELETE RESTRICT
);

CREATE TABLE certificate_issue_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL UNIQUE
        REFERENCES certificate_issue_requests(id) ON DELETE RESTRICT,
    idempotency_key UUID NOT NULL,
    issued_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    outcome TEXT NOT NULL,
    issued_count INTEGER NOT NULL DEFAULT 0,
    first_certificate_sequence INTEGER,
    last_certificate_sequence INTEGER,
    issue_codes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificate_issue_runs_outcome_check
        CHECK (outcome IN ('issued', 'returned')),
    CONSTRAINT certificate_issue_runs_issue_codes_check CHECK (
        cardinality(issue_codes) <= 64
        AND array_position(issue_codes, NULL) IS NULL
    ),
    CONSTRAINT certificate_issue_runs_sequence_check CHECK (
        (
            outcome = 'returned'
            AND issued_count = 0
            AND first_certificate_sequence IS NULL
            AND last_certificate_sequence IS NULL
        )
        OR
        (
            outcome = 'issued'
            AND issued_count BETWEEN 1 AND 5000
            AND first_certificate_sequence BETWEEN 1 AND 999999
            AND last_certificate_sequence BETWEEN 1 AND 999999
            AND last_certificate_sequence >= first_certificate_sequence
            AND issued_count =
                last_certificate_sequence - first_certificate_sequence + 1
        )
    ),
    UNIQUE (request_id, idempotency_key)
);

CREATE TABLE certificates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL
        REFERENCES certificate_campaigns(id) ON DELETE RESTRICT,
    template_id UUID NOT NULL,
    candidate_id UUID NOT NULL,
    issue_run_id UUID NOT NULL
        REFERENCES certificate_issue_runs(id) ON DELETE RESTRICT,
    academic_year_id UUID NOT NULL
        REFERENCES academic_years(id) ON DELETE RESTRICT,
    academic_year_value INTEGER NOT NULL,
    activity_sequence INTEGER NOT NULL,
    certificate_sequence INTEGER NOT NULL,
    check_digit SMALLINT NOT NULL,
    certificate_number VARCHAR(18) NOT NULL,
    recipient_type TEXT NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE RESTRICT,
    title_snapshot VARCHAR(100),
    first_name_snapshot VARCHAR(100) NOT NULL,
    last_name_snapshot VARCHAR(100) NOT NULL,
    template_name_snapshot VARCHAR(200) NOT NULL,
    activity_item_snapshot VARCHAR(500),
    award_or_role_snapshot VARCHAR(500),
    custom_values_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    school_name_snapshot VARCHAR(200) NOT NULL,
    owner_organization_unit_name_snapshot VARCHAR(200),
    issue_date DATE NOT NULL,
    status TEXT NOT NULL DEFAULT 'issued',
    qr_proof_encrypted TEXT NOT NULL,
    qr_proof_hash CHAR(64) NOT NULL,
    revoked_by UUID REFERENCES users(id) ON DELETE RESTRICT,
    revoked_at TIMESTAMPTZ,
    revocation_reason VARCHAR(500),
    replacement_for_certificate_id UUID UNIQUE,
    replaced_by_certificate_id UUID UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT certificates_number_shape_check CHECK (
        certificate_number ~ '^[0-9]{4}-[0-9]{4}-[0-9]{6}-[0-9]$'
    ),
    CONSTRAINT certificates_component_range_check CHECK (
        academic_year_value BETWEEN 0 AND 9999
        AND activity_sequence BETWEEN 1 AND 9999
        AND certificate_sequence BETWEEN 1 AND 999999
        AND check_digit BETWEEN 0 AND 9
    ),
    CONSTRAINT certificates_number_components_check CHECK (
        certificate_number =
            lpad(academic_year_value::TEXT, 4, '0')
            || '-' || lpad(activity_sequence::TEXT, 4, '0')
            || '-' || lpad(certificate_sequence::TEXT, 6, '0')
            || '-' || check_digit::TEXT
    ),
    CONSTRAINT certificates_recipient_type_check
        CHECK (recipient_type IN ('student', 'staff', 'external')),
    CONSTRAINT certificates_recipient_account_check CHECK (
        (recipient_type = 'external' AND user_id IS NULL)
        OR (recipient_type IN ('student', 'staff') AND user_id IS NOT NULL)
    ),
    CONSTRAINT certificates_snapshot_names_check CHECK (
        (title_snapshot IS NULL OR btrim(title_snapshot) <> '')
        AND btrim(first_name_snapshot) <> ''
        AND btrim(last_name_snapshot) <> ''
        AND btrim(template_name_snapshot) <> ''
        AND btrim(school_name_snapshot) <> ''
        AND (
            owner_organization_unit_name_snapshot IS NULL
            OR btrim(owner_organization_unit_name_snapshot) <> ''
        )
    ),
    CONSTRAINT certificates_custom_values_check
        CHECK (jsonb_typeof(custom_values_snapshot) = 'object'),
    CONSTRAINT certificates_status_check
        CHECK (status IN ('issued', 'revoked')),
    CONSTRAINT certificates_proof_ciphertext_check
        CHECK (btrim(qr_proof_encrypted) <> ''),
    CONSTRAINT certificates_proof_hash_check
        CHECK (qr_proof_hash ~ '^[0-9a-f]{64}$'),
    CONSTRAINT certificates_revocation_fields_check CHECK (
        (
            status = 'issued'
            AND revoked_by IS NULL
            AND revoked_at IS NULL
            AND revocation_reason IS NULL
        )
        OR
        (
            status = 'revoked'
            AND revoked_by IS NOT NULL
            AND revoked_at IS NOT NULL
            AND revocation_reason IS NOT NULL
            AND btrim(revocation_reason) <> ''
        )
    ),
    CONSTRAINT certificates_replacement_shape_check CHECK (
        (replacement_for_certificate_id IS NULL OR id <> replacement_for_certificate_id)
        AND (replaced_by_certificate_id IS NULL OR id <> replaced_by_certificate_id)
        AND (
            replacement_for_certificate_id IS NULL
            OR replaced_by_certificate_id IS NULL
            OR replacement_for_certificate_id <> replaced_by_certificate_id
        )
        AND (replaced_by_certificate_id IS NULL OR status = 'revoked')
    ),
    FOREIGN KEY (template_id, campaign_id)
        REFERENCES certificate_templates(id, campaign_id) ON DELETE RESTRICT,
    FOREIGN KEY (candidate_id, campaign_id)
        REFERENCES certificate_candidates(id, campaign_id) ON DELETE RESTRICT,
    UNIQUE (candidate_id),
    UNIQUE (campaign_id, certificate_sequence),
    UNIQUE (certificate_number),
    UNIQUE (qr_proof_hash)
);

ALTER TABLE certificate_candidates
    ADD CONSTRAINT certificate_candidates_replacement_for_fkey
        FOREIGN KEY (replacement_for_certificate_id)
        REFERENCES certificates(id) ON DELETE RESTRICT,
    ADD CONSTRAINT certificate_candidates_issued_certificate_fkey
        FOREIGN KEY (issued_certificate_id)
        REFERENCES certificates(id) ON DELETE RESTRICT;

ALTER TABLE certificates
    ADD CONSTRAINT certificates_replacement_for_fkey
        FOREIGN KEY (replacement_for_certificate_id)
        REFERENCES certificates(id) ON DELETE RESTRICT,
    ADD CONSTRAINT certificates_replaced_by_fkey
        FOREIGN KEY (replaced_by_certificate_id)
        REFERENCES certificates(id) ON DELETE RESTRICT;

CREATE FUNCTION enforce_certificate_candidate_issue_lock_active_request()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    request_status TEXT;
BEGIN
    SELECT status
    INTO request_status
    FROM certificate_issue_requests
    WHERE id = NEW.request_id
    FOR KEY SHARE;

    IF request_status IS NULL THEN
        RAISE EXCEPTION 'certificate issue request does not exist'
            USING ERRCODE = 'foreign_key_violation';
    END IF;

    IF request_status NOT IN ('pending', 'reviewing') THEN
        RAISE EXCEPTION 'candidate lock requires a pending or reviewing request'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER enforce_certificate_candidate_issue_lock_active_request
    BEFORE INSERT OR UPDATE ON certificate_candidate_issue_locks
    FOR EACH ROW
    EXECUTE FUNCTION enforce_certificate_candidate_issue_lock_active_request();

CREATE FUNCTION release_certificate_candidate_issue_locks()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.status IN ('pending', 'reviewing')
       AND NEW.status IN ('returned', 'withdrawn', 'issued') THEN
        DELETE FROM certificate_candidate_issue_locks
        WHERE request_id = NEW.id;
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER release_certificate_candidate_issue_locks
    AFTER UPDATE OF status ON certificate_issue_requests
    FOR EACH ROW
    EXECUTE FUNCTION release_certificate_candidate_issue_locks();

CREATE FUNCTION prevent_certificate_issue_request_item_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'certificate issue request items are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER prevent_certificate_issue_request_item_update
    BEFORE UPDATE ON certificate_issue_request_items
    FOR EACH ROW
    EXECUTE FUNCTION prevent_certificate_issue_request_item_mutation();

CREATE TRIGGER prevent_certificate_issue_request_item_delete
    BEFORE DELETE ON certificate_issue_request_items
    FOR EACH ROW
    EXECUTE FUNCTION prevent_certificate_issue_request_item_mutation();

CREATE FUNCTION enforce_certificate_snapshot_immutability()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF ROW(
        NEW.id,
        NEW.campaign_id,
        NEW.template_id,
        NEW.candidate_id,
        NEW.issue_run_id,
        NEW.academic_year_id,
        NEW.academic_year_value,
        NEW.activity_sequence,
        NEW.certificate_sequence,
        NEW.check_digit,
        NEW.certificate_number,
        NEW.recipient_type,
        NEW.user_id,
        NEW.title_snapshot,
        NEW.first_name_snapshot,
        NEW.last_name_snapshot,
        NEW.template_name_snapshot,
        NEW.activity_item_snapshot,
        NEW.award_or_role_snapshot,
        NEW.custom_values_snapshot,
        NEW.school_name_snapshot,
        NEW.owner_organization_unit_name_snapshot,
        NEW.issue_date,
        NEW.qr_proof_encrypted,
        NEW.qr_proof_hash,
        NEW.replacement_for_certificate_id,
        NEW.created_at
    ) IS DISTINCT FROM ROW(
        OLD.id,
        OLD.campaign_id,
        OLD.template_id,
        OLD.candidate_id,
        OLD.issue_run_id,
        OLD.academic_year_id,
        OLD.academic_year_value,
        OLD.activity_sequence,
        OLD.certificate_sequence,
        OLD.check_digit,
        OLD.certificate_number,
        OLD.recipient_type,
        OLD.user_id,
        OLD.title_snapshot,
        OLD.first_name_snapshot,
        OLD.last_name_snapshot,
        OLD.template_name_snapshot,
        OLD.activity_item_snapshot,
        OLD.award_or_role_snapshot,
        OLD.custom_values_snapshot,
        OLD.school_name_snapshot,
        OLD.owner_organization_unit_name_snapshot,
        OLD.issue_date,
        OLD.qr_proof_encrypted,
        OLD.qr_proof_hash,
        OLD.replacement_for_certificate_id,
        OLD.created_at
    ) THEN
        RAISE EXCEPTION 'issued certificate snapshots are immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    IF OLD.status = 'revoked'
       AND ROW(
           NEW.status,
           NEW.revoked_by,
           NEW.revoked_at,
           NEW.revocation_reason
       ) IS DISTINCT FROM ROW(
           OLD.status,
           OLD.revoked_by,
           OLD.revoked_at,
           OLD.revocation_reason
       ) THEN
        RAISE EXCEPTION 'certificate revocation is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    IF OLD.replaced_by_certificate_id IS NOT NULL
       AND NEW.replaced_by_certificate_id IS DISTINCT FROM OLD.replaced_by_certificate_id THEN
        RAISE EXCEPTION 'certificate replacement link is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER enforce_certificate_snapshot_immutability
    BEFORE UPDATE ON certificates
    FOR EACH ROW
    EXECUTE FUNCTION enforce_certificate_snapshot_immutability();

CREATE FUNCTION prevent_certificate_delete()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'issued certificates cannot be deleted'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER prevent_certificate_delete
    BEFORE DELETE ON certificates
    FOR EACH ROW
    EXECUTE FUNCTION prevent_certificate_delete();

CREATE INDEX certificate_campaigns_year_status_event_idx
    ON certificate_campaigns (academic_year_id, status, event_date DESC);

CREATE INDEX certificate_campaigns_owner_status_event_idx
    ON certificate_campaigns (owner_organization_unit_id, status, event_date DESC)
    WHERE owner_organization_unit_id IS NOT NULL;

CREATE INDEX certificate_templates_campaign_active_idx
    ON certificate_templates (campaign_id, is_active, created_at);

CREATE INDEX certificate_template_assets_template_kind_idx
    ON certificate_template_assets (template_id, kind);

CREATE INDEX certificate_import_batches_campaign_created_idx
    ON certificate_import_batches (campaign_id, created_at DESC);

CREATE INDEX certificate_candidates_campaign_validation_idx
    ON certificate_candidates (campaign_id, validation_status, created_at)
    WHERE deleted_at IS NULL;

CREATE INDEX certificate_candidates_campaign_match_idx
    ON certificate_candidates (campaign_id, match_status)
    WHERE deleted_at IS NULL;

CREATE INDEX certificate_candidates_template_idx
    ON certificate_candidates (template_id)
    WHERE deleted_at IS NULL;

CREATE INDEX certificate_candidates_matched_user_idx
    ON certificate_candidates (matched_user_id)
    WHERE matched_user_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX certificate_candidates_student_lookup_idx
    ON certificate_candidates (lookup_student_id)
    WHERE lookup_student_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX certificate_candidates_staff_lookup_idx
    ON certificate_candidates (lookup_staff_username)
    WHERE lookup_staff_username IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX certificate_issue_requests_queue_idx
    ON certificate_issue_requests (status, submitted_at);

CREATE INDEX certificate_issue_requests_campaign_status_idx
    ON certificate_issue_requests (campaign_id, status, submitted_at DESC);

CREATE INDEX certificate_issue_request_items_candidate_idx
    ON certificate_issue_request_items (candidate_id, request_id);

CREATE INDEX certificate_candidate_issue_locks_request_idx
    ON certificate_candidate_issue_locks (request_id);

CREATE INDEX certificates_campaign_issue_date_idx
    ON certificates (campaign_id, issue_date DESC);

CREATE INDEX certificates_template_idx
    ON certificates (template_id);

CREATE INDEX certificates_user_status_idx
    ON certificates (user_id, status, issue_date DESC)
    WHERE user_id IS NOT NULL;

CREATE INDEX certificates_status_issue_date_idx
    ON certificates (status, issue_date DESC);

CREATE INDEX certificates_recipient_name_idx
    ON certificates (
        lower(first_name_snapshot),
        lower(last_name_snapshot),
        certificate_number
    );

CREATE TRIGGER update_certificate_academic_year_counters_updated_at
    BEFORE UPDATE ON certificate_academic_year_counters
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_certificate_campaigns_updated_at
    BEFORE UPDATE ON certificate_campaigns
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_certificate_templates_updated_at
    BEFORE UPDATE ON certificate_templates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_certificate_candidates_updated_at
    BEFORE UPDATE ON certificate_candidates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_certificate_issue_requests_updated_at
    BEFORE UPDATE ON certificate_issue_requests
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_certificates_updated_at
    BEFORE UPDATE ON certificates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
