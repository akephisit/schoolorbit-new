-- Migration 123: Reset organization unit baseline
-- ORG-BASELINE-V1
--
-- This migration makes the active organization tree deterministic for both
-- fresh databases and existing tenant databases. It preserves existing row ids
-- by updating rows by code. Unreferenced non-baseline units are deleted, while
-- referenced non-baseline units are archived to avoid breaking history such as
-- members, grants, role context, menus, or delegations.

-- Canonicalize known old unit codes before the baseline upsert.
UPDATE organization_units
SET code = 'SUBJ-OC',
    name = 'กลุ่มสาระการเรียนรู้การงานอาชีพ',
    name_en = 'Occupations Department',
    updated_at = NOW()
WHERE code = 'SUBJ-OT'
  AND NOT EXISTS (
      SELECT 1
      FROM organization_units existing
      WHERE existing.code = 'SUBJ-OC'
  );

UPDATE organization_units
SET code = 'STU-DIS',
    name = 'งานวินัยและกิจการนักเรียน',
    name_en = 'Student Discipline and Affairs',
    updated_at = NOW()
WHERE code = 'PER-DIS'
  AND NOT EXISTS (
      SELECT 1
      FROM organization_units existing
      WHERE existing.code = 'STU-DIS'
  );

UPDATE organization_units
SET code = 'STU-ADV',
    name = 'งานระบบดูแลช่วยเหลือนักเรียน',
    name_en = 'Student Care and Advisor System',
    updated_at = NOW()
WHERE code = 'PER-ADV'
  AND NOT EXISTS (
      SELECT 1
      FROM organization_units existing
      WHERE existing.code = 'STU-ADV'
  );

WITH baseline (
    code,
    name,
    name_en,
    description,
    parent_code,
    category,
    unit_type,
    display_order,
    subject_group_code
) AS (
    VALUES
        ('SCHOOL', 'โรงเรียน', 'School', 'หน่วยงานรากของโครงสร้างโรงเรียน', NULL, 'other', 'school', -1000, NULL),
        ('DIR-01', 'สำนักงานผู้อำนวยการ', 'Director Office', 'สำนักบริหารระดับโรงเรียนและงานเลขานุการผู้บริหาร', 'SCHOOL', 'administrative', 'management_group', 10, NULL),
        ('DIR-SEC', 'งานเลขานุการผู้อำนวยการ', 'Director Secretariat', 'ประสานงานผู้บริหารและเอกสารระดับโรงเรียน', 'DIR-01', 'administrative', 'division', 11, NULL),

        ('ACAD-01', 'กลุ่มบริหารงานวิชาการ', 'Academic Affairs', 'งานหลักสูตร การจัดการเรียนรู้ วัดผล และกลุ่มสาระการเรียนรู้', 'SCHOOL', 'academic', 'management_group', 20, NULL),
        ('ACAD-CUR', 'งานหลักสูตรสถานศึกษา', 'Curriculum Development', 'จัดทำและดูแลหลักสูตรสถานศึกษา', 'ACAD-01', 'academic', 'division', 21, NULL),
        ('ACAD-REG', 'งานทะเบียนและวัดผล', 'Registration and Measurement', 'ทะเบียนนักเรียน วัดผล ประเมินผล และเอกสารทางการศึกษา', 'ACAD-01', 'academic', 'division', 22, NULL),
        ('ACAD-ACT', 'งานกิจกรรมพัฒนาผู้เรียน', 'Learner Development Activities', 'ดูแลกิจกรรมพัฒนาผู้เรียน เช่น ชุมนุม ลูกเสือ และกิจกรรมแนะแนว', 'ACAD-01', 'academic', 'division', 23, NULL),
        ('SUBJ-TH', 'กลุ่มสาระการเรียนรู้ภาษาไทย', 'Thai Language Department', 'กลุ่มสาระการเรียนรู้ภาษาไทย', 'ACAD-01', 'academic', 'subject_group', 31, 'TH'),
        ('SUBJ-MA', 'กลุ่มสาระการเรียนรู้คณิตศาสตร์', 'Mathematics Department', 'กลุ่มสาระการเรียนรู้คณิตศาสตร์', 'ACAD-01', 'academic', 'subject_group', 32, 'MA'),
        ('SUBJ-SC', 'กลุ่มสาระการเรียนรู้วิทยาศาสตร์และเทคโนโลยี', 'Science and Technology Department', 'กลุ่มสาระการเรียนรู้วิทยาศาสตร์และเทคโนโลยี', 'ACAD-01', 'academic', 'subject_group', 33, 'SC'),
        ('SUBJ-SO', 'กลุ่มสาระการเรียนรู้สังคมศึกษา ศาสนา และวัฒนธรรม', 'Social Studies, Religion and Culture Department', 'กลุ่มสาระการเรียนรู้สังคมศึกษา ศาสนา และวัฒนธรรม', 'ACAD-01', 'academic', 'subject_group', 34, 'SO'),
        ('SUBJ-HP', 'กลุ่มสาระการเรียนรู้สุขศึกษาและพลศึกษา', 'Health and Physical Education Department', 'กลุ่มสาระการเรียนรู้สุขศึกษาและพลศึกษา', 'ACAD-01', 'academic', 'subject_group', 35, 'HP'),
        ('SUBJ-AR', 'กลุ่มสาระการเรียนรู้ศิลปะ', 'Arts Department', 'กลุ่มสาระการเรียนรู้ศิลปะ', 'ACAD-01', 'academic', 'subject_group', 36, 'AR'),
        ('SUBJ-OC', 'กลุ่มสาระการเรียนรู้การงานอาชีพ', 'Occupations Department', 'กลุ่มสาระการเรียนรู้การงานอาชีพ', 'ACAD-01', 'academic', 'subject_group', 37, 'OC'),
        ('SUBJ-EN', 'กลุ่มสาระการเรียนรู้ภาษาต่างประเทศ', 'Foreign Languages Department', 'กลุ่มสาระการเรียนรู้ภาษาต่างประเทศ', 'ACAD-01', 'academic', 'subject_group', 38, 'EN'),

        ('STU-01', 'กลุ่มบริหารงานกิจการนักเรียน', 'Student Affairs', 'งานวินัย ระบบดูแลช่วยเหลือนักเรียน และกิจการนักเรียน', 'SCHOOL', 'student_affairs', 'management_group', 30, NULL),
        ('STU-DIS', 'งานวินัยและกิจการนักเรียน', 'Student Discipline and Affairs', 'ดูแลวินัย ความประพฤติ และกิจการนักเรียน', 'STU-01', 'student_affairs', 'division', 31, NULL),
        ('STU-ADV', 'งานระบบดูแลช่วยเหลือนักเรียน', 'Student Care and Advisor System', 'ระบบดูแลช่วยเหลือนักเรียนและครูที่ปรึกษา', 'STU-01', 'student_affairs', 'division', 32, NULL),

        ('PER-01', 'กลุ่มบริหารงานบุคคล', 'Personnel Affairs', 'งานบุคลากร อัตรากำลัง และพัฒนาครู', 'SCHOOL', 'personnel', 'management_group', 40, NULL),
        ('PER-HR', 'งานทะเบียนประวัติและอัตรากำลัง', 'Personnel Records and Workforce Planning', 'ทะเบียนประวัติบุคลากร อัตรากำลัง และตำแหน่ง', 'PER-01', 'personnel', 'division', 41, NULL),
        ('PER-DEV', 'งานพัฒนาบุคลากร', 'Personnel Development', 'พัฒนาครูและบุคลากรทางการศึกษา', 'PER-01', 'personnel', 'division', 42, NULL),

        ('BUD-01', 'กลุ่มบริหารงานงบประมาณ', 'Budget and Planning', 'งานแผน งบประมาณ การเงิน บัญชี และพัสดุ', 'SCHOOL', 'budget', 'management_group', 50, NULL),
        ('BUD-PLA', 'งานแผนงานและนโยบาย', 'Policy and Planning', 'แผนงาน โครงการ และนโยบายโรงเรียน', 'BUD-01', 'budget', 'division', 51, NULL),
        ('BUD-FIN', 'งานการเงินและบัญชี', 'Finance and Accounting', 'การเงิน บัญชี และรายงานการเงิน', 'BUD-01', 'budget', 'division', 52, NULL),
        ('BUD-AST', 'งานพัสดุและสินทรัพย์', 'Procurement and Assets', 'พัสดุ ครุภัณฑ์ และสินทรัพย์โรงเรียน', 'BUD-01', 'budget', 'division', 53, NULL),

        ('GEN-01', 'กลุ่มบริหารงานทั่วไป', 'General Administration', 'งานธุรการ อาคารสถานที่ ประชาสัมพันธ์ และบริการทั่วไป', 'SCHOOL', 'general', 'management_group', 60, NULL),
        ('GEN-DOC', 'งานสารบรรณและรับส่งเอกสาร', 'Correspondence and Records', 'รับ ส่ง ลงทะเบียน และติดตามเอกสารราชการ', 'GEN-01', 'general', 'division', 61, NULL),
        ('GEN-BLD', 'งานอาคารสถานที่', 'Building and Grounds', 'อาคารสถานที่ ห้องเรียน และสภาพแวดล้อมโรงเรียน', 'GEN-01', 'general', 'division', 62, NULL),
        ('GEN-PR', 'งานประชาสัมพันธ์', 'Public Relations', 'ประชาสัมพันธ์และสื่อสารองค์กร', 'GEN-01', 'general', 'division', 63, NULL),
        ('GEN-HEL', 'งานอนามัยโรงเรียน', 'School Health', 'อนามัยโรงเรียนและบริการสุขภาพเบื้องต้น', 'GEN-01', 'general', 'division', 64, NULL)
),
resolved AS (
    SELECT
        baseline.*,
        sg.id AS subject_group_id
    FROM baseline
    LEFT JOIN subject_groups sg ON sg.code = baseline.subject_group_code
)
INSERT INTO organization_units (
    id,
    code,
    name,
    name_en,
    description,
    parent_unit_id,
    category,
    unit_type,
    is_active,
    display_order,
    subject_group_id,
    metadata
)
SELECT
    gen_random_uuid(),
    code,
    name,
    name_en,
    description,
    NULL,
    category,
    unit_type,
    true,
    display_order,
    subject_group_id,
    jsonb_build_object('baseline', 'ORG-BASELINE-V1')
FROM resolved
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    name_en = EXCLUDED.name_en,
    description = EXCLUDED.description,
    category = EXCLUDED.category,
    unit_type = EXCLUDED.unit_type,
    is_active = true,
    display_order = EXCLUDED.display_order,
    subject_group_id = EXCLUDED.subject_group_id,
    metadata = COALESCE(organization_units.metadata, '{}'::jsonb)
        || jsonb_build_object('baseline', 'ORG-BASELINE-V1'),
    updated_at = NOW();

WITH baseline (code, parent_code) AS (
    VALUES
        ('SCHOOL', NULL),
        ('DIR-01', 'SCHOOL'),
        ('DIR-SEC', 'DIR-01'),
        ('ACAD-01', 'SCHOOL'),
        ('ACAD-CUR', 'ACAD-01'),
        ('ACAD-REG', 'ACAD-01'),
        ('ACAD-ACT', 'ACAD-01'),
        ('SUBJ-TH', 'ACAD-01'),
        ('SUBJ-MA', 'ACAD-01'),
        ('SUBJ-SC', 'ACAD-01'),
        ('SUBJ-SO', 'ACAD-01'),
        ('SUBJ-HP', 'ACAD-01'),
        ('SUBJ-AR', 'ACAD-01'),
        ('SUBJ-OC', 'ACAD-01'),
        ('SUBJ-EN', 'ACAD-01'),
        ('STU-01', 'SCHOOL'),
        ('STU-DIS', 'STU-01'),
        ('STU-ADV', 'STU-01'),
        ('PER-01', 'SCHOOL'),
        ('PER-HR', 'PER-01'),
        ('PER-DEV', 'PER-01'),
        ('BUD-01', 'SCHOOL'),
        ('BUD-PLA', 'BUD-01'),
        ('BUD-FIN', 'BUD-01'),
        ('BUD-AST', 'BUD-01'),
        ('GEN-01', 'SCHOOL'),
        ('GEN-DOC', 'GEN-01'),
        ('GEN-BLD', 'GEN-01'),
        ('GEN-PR', 'GEN-01'),
        ('GEN-HEL', 'GEN-01')
),
resolved AS (
    SELECT child.id AS child_id,
           parent.id AS parent_id,
           baseline.parent_code
    FROM baseline
    JOIN organization_units child ON child.code = baseline.code
    LEFT JOIN organization_units parent ON parent.code = baseline.parent_code
)
UPDATE organization_units ou
SET parent_unit_id = resolved.parent_id,
    updated_at = NOW()
FROM resolved
WHERE ou.id = resolved.child_id
  AND (
      resolved.parent_code IS NULL
      OR resolved.parent_id IS NOT NULL
  );

UPDATE organization_units ou
SET subject_group_id = sg.id,
    category = 'academic',
    unit_type = 'subject_group',
    updated_at = NOW()
FROM subject_groups sg
WHERE ou.code = 'SUBJ-' || sg.code
  AND ou.is_active = true;

CREATE TEMP TABLE IF NOT EXISTS tmp_org_baseline_menu_refs (
    organization_unit_id UUID PRIMARY KEY
);

TRUNCATE tmp_org_baseline_menu_refs;

DO $$
BEGIN
    IF to_regclass('public.department_menu_access') IS NOT NULL THEN
        EXECUTE '
            INSERT INTO tmp_org_baseline_menu_refs (organization_unit_id)
            SELECT DISTINCT department_id
            FROM department_menu_access
            ON CONFLICT (organization_unit_id) DO NOTHING
        ';
    END IF;
END $$;

WITH baseline (code) AS (
    VALUES
        ('SCHOOL'), ('DIR-01'), ('DIR-SEC'),
        ('ACAD-01'), ('ACAD-CUR'), ('ACAD-REG'), ('ACAD-ACT'),
        ('SUBJ-TH'), ('SUBJ-MA'), ('SUBJ-SC'), ('SUBJ-SO'), ('SUBJ-HP'), ('SUBJ-AR'), ('SUBJ-OC'), ('SUBJ-EN'),
        ('STU-01'), ('STU-DIS'), ('STU-ADV'),
        ('PER-01'), ('PER-HR'), ('PER-DEV'),
        ('BUD-01'), ('BUD-PLA'), ('BUD-FIN'), ('BUD-AST'),
        ('GEN-01'), ('GEN-DOC'), ('GEN-BLD'), ('GEN-PR'), ('GEN-HEL')
),
unreferenced_non_baseline_units AS (
    SELECT ou.id
    FROM organization_units ou
    WHERE ou.code NOT IN (SELECT code FROM baseline)
      AND NOT EXISTS (
          SELECT 1
          FROM organization_members om
          WHERE om.organization_unit_id = ou.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM organization_permission_grants opg
          WHERE opg.organization_unit_id = ou.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM organization_permission_delegations opd
          WHERE opd.organization_unit_id = ou.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM user_roles ur
          WHERE ur.organization_unit_id = ou.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM tmp_org_baseline_menu_refs menu_refs
          WHERE menu_refs.organization_unit_id = ou.id
      )
)
UPDATE organization_units child
SET parent_unit_id = NULL,
    updated_at = NOW()
WHERE child.parent_unit_id IN (
    SELECT id
    FROM unreferenced_non_baseline_units
)
  AND child.code NOT IN (SELECT code FROM baseline);

WITH baseline (code) AS (
    VALUES
        ('SCHOOL'), ('DIR-01'), ('DIR-SEC'),
        ('ACAD-01'), ('ACAD-CUR'), ('ACAD-REG'), ('ACAD-ACT'),
        ('SUBJ-TH'), ('SUBJ-MA'), ('SUBJ-SC'), ('SUBJ-SO'), ('SUBJ-HP'), ('SUBJ-AR'), ('SUBJ-OC'), ('SUBJ-EN'),
        ('STU-01'), ('STU-DIS'), ('STU-ADV'),
        ('PER-01'), ('PER-HR'), ('PER-DEV'),
        ('BUD-01'), ('BUD-PLA'), ('BUD-FIN'), ('BUD-AST'),
        ('GEN-01'), ('GEN-DOC'), ('GEN-BLD'), ('GEN-PR'), ('GEN-HEL')
),
unreferenced_non_baseline_units AS (
    SELECT ou.id
    FROM organization_units ou
    WHERE ou.code NOT IN (SELECT code FROM baseline)
      AND NOT EXISTS (
          SELECT 1
          FROM organization_members om
          WHERE om.organization_unit_id = ou.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM organization_permission_grants opg
          WHERE opg.organization_unit_id = ou.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM organization_permission_delegations opd
          WHERE opd.organization_unit_id = ou.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM user_roles ur
          WHERE ur.organization_unit_id = ou.id
      )
      AND NOT EXISTS (
          SELECT 1
          FROM tmp_org_baseline_menu_refs menu_refs
          WHERE menu_refs.organization_unit_id = ou.id
      )
)
DELETE FROM organization_units ou
USING unreferenced_non_baseline_units unused
WHERE ou.id = unused.id;

WITH baseline (code) AS (
    VALUES
        ('SCHOOL'), ('DIR-01'), ('DIR-SEC'),
        ('ACAD-01'), ('ACAD-CUR'), ('ACAD-REG'), ('ACAD-ACT'),
        ('SUBJ-TH'), ('SUBJ-MA'), ('SUBJ-SC'), ('SUBJ-SO'), ('SUBJ-HP'), ('SUBJ-AR'), ('SUBJ-OC'), ('SUBJ-EN'),
        ('STU-01'), ('STU-DIS'), ('STU-ADV'),
        ('PER-01'), ('PER-HR'), ('PER-DEV'),
        ('BUD-01'), ('BUD-PLA'), ('BUD-FIN'), ('BUD-AST'),
        ('GEN-01'), ('GEN-DOC'), ('GEN-BLD'), ('GEN-PR'), ('GEN-HEL')
)
UPDATE organization_units ou
SET is_active = false,
    metadata = COALESCE(ou.metadata, '{}'::jsonb)
        || jsonb_build_object('archived_by', 'ORG-BASELINE-V1'),
    updated_at = NOW()
WHERE ou.code NOT IN (SELECT code FROM baseline);

DO $$
DECLARE
    missing_baseline_units INTEGER;
    invalid_parent_units INTEGER;
    invalid_subject_units INTEGER;
    active_extra_units INTEGER;
BEGIN
    IF EXISTS (
        SELECT 1
        FROM organization_units
        WHERE code = 'SUBJ-OT'
    ) THEN
        RAISE EXCEPTION 'Legacy organization unit code SUBJ-OT remains; expected SUBJ-OC';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM organization_units
        WHERE code IN ('PER-DIS', 'PER-ADV')
    ) THEN
        RAISE EXCEPTION 'Legacy student-affairs organization codes PER-DIS/PER-ADV remain; expected STU-DIS/STU-ADV';
    END IF;

    WITH baseline (code) AS (
        VALUES
            ('SCHOOL'), ('DIR-01'), ('DIR-SEC'),
            ('ACAD-01'), ('ACAD-CUR'), ('ACAD-REG'), ('ACAD-ACT'),
            ('SUBJ-TH'), ('SUBJ-MA'), ('SUBJ-SC'), ('SUBJ-SO'), ('SUBJ-HP'), ('SUBJ-AR'), ('SUBJ-OC'), ('SUBJ-EN'),
            ('STU-01'), ('STU-DIS'), ('STU-ADV'),
            ('PER-01'), ('PER-HR'), ('PER-DEV'),
            ('BUD-01'), ('BUD-PLA'), ('BUD-FIN'), ('BUD-AST'),
            ('GEN-01'), ('GEN-DOC'), ('GEN-BLD'), ('GEN-PR'), ('GEN-HEL')
    )
    SELECT COUNT(*)
    INTO missing_baseline_units
    FROM baseline
    LEFT JOIN organization_units ou ON ou.code = baseline.code AND ou.is_active = true
    WHERE ou.id IS NULL;

    IF missing_baseline_units > 0 THEN
        RAISE EXCEPTION 'Organization baseline failed: % baseline unit(s) missing or inactive', missing_baseline_units;
    END IF;

    WITH baseline (code, parent_code) AS (
        VALUES
            ('SCHOOL', NULL),
            ('DIR-01', 'SCHOOL'),
            ('DIR-SEC', 'DIR-01'),
            ('ACAD-01', 'SCHOOL'),
            ('ACAD-CUR', 'ACAD-01'),
            ('ACAD-REG', 'ACAD-01'),
            ('ACAD-ACT', 'ACAD-01'),
            ('SUBJ-TH', 'ACAD-01'),
            ('SUBJ-MA', 'ACAD-01'),
            ('SUBJ-SC', 'ACAD-01'),
            ('SUBJ-SO', 'ACAD-01'),
            ('SUBJ-HP', 'ACAD-01'),
            ('SUBJ-AR', 'ACAD-01'),
            ('SUBJ-OC', 'ACAD-01'),
            ('SUBJ-EN', 'ACAD-01'),
            ('STU-01', 'SCHOOL'),
            ('STU-DIS', 'STU-01'),
            ('STU-ADV', 'STU-01'),
            ('PER-01', 'SCHOOL'),
            ('PER-HR', 'PER-01'),
            ('PER-DEV', 'PER-01'),
            ('BUD-01', 'SCHOOL'),
            ('BUD-PLA', 'BUD-01'),
            ('BUD-FIN', 'BUD-01'),
            ('BUD-AST', 'BUD-01'),
            ('GEN-01', 'SCHOOL'),
            ('GEN-DOC', 'GEN-01'),
            ('GEN-BLD', 'GEN-01'),
            ('GEN-PR', 'GEN-01'),
            ('GEN-HEL', 'GEN-01')
    )
    SELECT COUNT(*)
    INTO invalid_parent_units
    FROM baseline
    JOIN organization_units child ON child.code = baseline.code
    LEFT JOIN organization_units parent ON parent.code = baseline.parent_code
    WHERE (
        baseline.parent_code IS NULL
        AND child.parent_unit_id IS NOT NULL
    ) OR (
        baseline.parent_code IS NOT NULL
        AND child.parent_unit_id IS DISTINCT FROM parent.id
    );

    IF invalid_parent_units > 0 THEN
        RAISE EXCEPTION 'Organization baseline failed: % unit(s) have invalid parent placement', invalid_parent_units;
    END IF;

    SELECT COUNT(*)
    INTO invalid_subject_units
    FROM organization_units ou
    LEFT JOIN subject_groups sg ON sg.id = ou.subject_group_id
    WHERE ou.is_active = true
      AND ou.code LIKE 'SUBJ-%'
      AND (
          ou.subject_group_id IS NULL
          OR sg.id IS NULL
          OR ou.code <> 'SUBJ-' || sg.code
          OR ou.category <> 'academic'
          OR ou.unit_type <> 'subject_group'
      );

    IF invalid_subject_units > 0 THEN
        RAISE EXCEPTION 'Organization baseline failed: % active subject-group unit(s) invalid', invalid_subject_units;
    END IF;

    WITH baseline (code) AS (
        VALUES
            ('SCHOOL'), ('DIR-01'), ('DIR-SEC'),
            ('ACAD-01'), ('ACAD-CUR'), ('ACAD-REG'), ('ACAD-ACT'),
            ('SUBJ-TH'), ('SUBJ-MA'), ('SUBJ-SC'), ('SUBJ-SO'), ('SUBJ-HP'), ('SUBJ-AR'), ('SUBJ-OC'), ('SUBJ-EN'),
            ('STU-01'), ('STU-DIS'), ('STU-ADV'),
            ('PER-01'), ('PER-HR'), ('PER-DEV'),
            ('BUD-01'), ('BUD-PLA'), ('BUD-FIN'), ('BUD-AST'),
            ('GEN-01'), ('GEN-DOC'), ('GEN-BLD'), ('GEN-PR'), ('GEN-HEL')
    )
    SELECT COUNT(*)
    INTO active_extra_units
    FROM organization_units ou
    WHERE ou.is_active = true
      AND ou.code NOT IN (SELECT code FROM baseline);

    IF active_extra_units > 0 THEN
        RAISE EXCEPTION 'Organization baseline failed: % active non-baseline unit(s) remain', active_extra_units;
    END IF;
END $$;
