-- Migration 127: Canonicalize permission code contracts
-- PERMISSION-CODE-CONTRACTS-V1
--
-- Permission codes should be `module.action.scope`. Preserve permission ids so
-- role_permissions, organization_permission_grants, and delegations remain
-- attached to the same permission rows. Text-based menu filters are renamed
-- separately because menu_items.required_permission is not a permission FK.

WITH permission_renames (
    old_code,
    new_code,
    new_name,
    new_module,
    new_action,
    new_scope,
    new_description
) AS (
    VALUES
        ('settings.read', 'settings.read.all', 'ดูการตั้งค่า', 'settings', 'read', 'all', 'ดูการตั้งค่าระบบ'),
        ('settings.update', 'settings.update.all', 'แก้ไขการตั้งค่า', 'settings', 'update', 'all', 'แก้ไขการตั้งค่าระบบ'),
        ('dashboard', 'dashboard.read.own', 'แดชบอร์ด', 'dashboard', 'read', 'own', 'ดูหน้าแดชบอร์ด'),
        ('student.create', 'student.create.all', 'เพิ่มนักเรียน', 'student', 'create', 'all', 'สร้างนักเรียนใหม่'),
        ('student.delete', 'student.delete.all', 'ลบนักเรียน', 'student', 'delete', 'all', 'ลบนักเรียน'),
        ('organization_work.create', 'organization_work.create.own', 'สร้าง/ส่งงาน', 'organization_work', 'create', 'own', 'ส่งงานหรือสร้างบันทึกงานใหม่'),
        ('activity.members.manage', 'activity.manage_members.all', 'จัดการสมาชิกกิจกรรม', 'activity', 'manage_members', 'all', 'จัดการสมาชิกในกิจกรรมพัฒนาผู้เรียน'),
        ('admission.verify', 'admission.verify.all', 'ตรวจสอบใบสมัคร', 'admission', 'verify', 'all', 'ยืนยัน/ปฏิเสธใบสมัครของผู้สมัคร'),
        ('admission.scores', 'admission.scores.all', 'กรอกคะแนนและจัดห้อง', 'admission', 'scores', 'all', 'กรอกคะแนนสอบ, เรียงคะแนน, จัดห้องเรียน'),
        ('admission.enroll', 'admission.enroll.all', 'รับมอบตัว', 'admission', 'enroll', 'all', 'รับมอบตัวและสร้าง account นักเรียนในระบบ')
)
UPDATE permissions p
SET code = permission_renames.new_code,
    name = permission_renames.new_name,
    module = permission_renames.new_module,
    action = permission_renames.new_action,
    scope = permission_renames.new_scope,
    description = permission_renames.new_description,
    updated_at = NOW()
FROM permission_renames
WHERE p.code = permission_renames.old_code
  AND NOT EXISTS (
      SELECT 1
      FROM permissions existing
      WHERE existing.code = permission_renames.new_code
  );

WITH permission_renames (
    old_code,
    new_code,
    new_name,
    new_module,
    new_action,
    new_scope,
    new_description
) AS (
    VALUES
        ('settings.read', 'settings.read.all', 'ดูการตั้งค่า', 'settings', 'read', 'all', 'ดูการตั้งค่าระบบ'),
        ('settings.update', 'settings.update.all', 'แก้ไขการตั้งค่า', 'settings', 'update', 'all', 'แก้ไขการตั้งค่าระบบ'),
        ('dashboard', 'dashboard.read.own', 'แดชบอร์ด', 'dashboard', 'read', 'own', 'ดูหน้าแดชบอร์ด'),
        ('student.create', 'student.create.all', 'เพิ่มนักเรียน', 'student', 'create', 'all', 'สร้างนักเรียนใหม่'),
        ('student.delete', 'student.delete.all', 'ลบนักเรียน', 'student', 'delete', 'all', 'ลบนักเรียน'),
        ('organization_work.create', 'organization_work.create.own', 'สร้าง/ส่งงาน', 'organization_work', 'create', 'own', 'ส่งงานหรือสร้างบันทึกงานใหม่'),
        ('activity.members.manage', 'activity.manage_members.all', 'จัดการสมาชิกกิจกรรม', 'activity', 'manage_members', 'all', 'จัดการสมาชิกในกิจกรรมพัฒนาผู้เรียน'),
        ('admission.verify', 'admission.verify.all', 'ตรวจสอบใบสมัคร', 'admission', 'verify', 'all', 'ยืนยัน/ปฏิเสธใบสมัครของผู้สมัคร'),
        ('admission.scores', 'admission.scores.all', 'กรอกคะแนนและจัดห้อง', 'admission', 'scores', 'all', 'กรอกคะแนนสอบ, เรียงคะแนน, จัดห้องเรียน'),
        ('admission.enroll', 'admission.enroll.all', 'รับมอบตัว', 'admission', 'enroll', 'all', 'รับมอบตัวและสร้าง account นักเรียนในระบบ')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id,
           new_permissions.id AS new_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
INSERT INTO role_permissions (role_id, permission_id, created_at)
SELECT role_permissions.role_id, duplicate_pairs.new_id, role_permissions.created_at
FROM role_permissions
JOIN duplicate_pairs ON duplicate_pairs.old_id = role_permissions.permission_id
ON CONFLICT (role_id, permission_id) DO NOTHING;

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('settings.read', 'settings.read.all'),
        ('settings.update', 'settings.update.all'),
        ('dashboard', 'dashboard.read.own'),
        ('student.create', 'student.create.all'),
        ('student.delete', 'student.delete.all'),
        ('organization_work.create', 'organization_work.create.own'),
        ('activity.members.manage', 'activity.manage_members.all'),
        ('admission.verify', 'admission.verify.all'),
        ('admission.scores', 'admission.scores.all'),
        ('admission.enroll', 'admission.enroll.all')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id,
           new_permissions.id AS new_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
UPDATE organization_permission_delegations delegation
SET permission_id = duplicate_pairs.new_id
FROM duplicate_pairs
WHERE delegation.permission_id = duplicate_pairs.old_id;

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('settings.read', 'settings.read.all'),
        ('settings.update', 'settings.update.all'),
        ('dashboard', 'dashboard.read.own'),
        ('student.create', 'student.create.all'),
        ('student.delete', 'student.delete.all'),
        ('organization_work.create', 'organization_work.create.own'),
        ('activity.members.manage', 'activity.manage_members.all'),
        ('admission.verify', 'admission.verify.all'),
        ('admission.scores', 'admission.scores.all'),
        ('admission.enroll', 'admission.enroll.all')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id,
           new_permissions.id AS new_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
UPDATE organization_permission_grants grant_row
SET permission_id = duplicate_pairs.new_id
FROM duplicate_pairs
WHERE grant_row.permission_id = duplicate_pairs.old_id
  AND NOT EXISTS (
      SELECT 1
      FROM organization_permission_grants existing
      WHERE existing.organization_unit_id = grant_row.organization_unit_id
        AND existing.permission_id = duplicate_pairs.new_id
        AND existing.position_code IS NOT DISTINCT FROM grant_row.position_code
  );

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('settings.read', 'settings.read.all'),
        ('settings.update', 'settings.update.all'),
        ('dashboard', 'dashboard.read.own'),
        ('student.create', 'student.create.all'),
        ('student.delete', 'student.delete.all'),
        ('organization_work.create', 'organization_work.create.own'),
        ('activity.members.manage', 'activity.manage_members.all'),
        ('admission.verify', 'admission.verify.all'),
        ('admission.scores', 'admission.scores.all'),
        ('admission.enroll', 'admission.enroll.all')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
DELETE FROM organization_permission_grants grant_row
USING duplicate_pairs
WHERE grant_row.permission_id = duplicate_pairs.old_id;

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('settings.read', 'settings.read.all'),
        ('settings.update', 'settings.update.all'),
        ('dashboard', 'dashboard.read.own'),
        ('student.create', 'student.create.all'),
        ('student.delete', 'student.delete.all'),
        ('organization_work.create', 'organization_work.create.own'),
        ('activity.members.manage', 'activity.manage_members.all'),
        ('admission.verify', 'admission.verify.all'),
        ('admission.scores', 'admission.scores.all'),
        ('admission.enroll', 'admission.enroll.all')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
DELETE FROM role_permissions role_permission
USING duplicate_pairs
WHERE role_permission.permission_id = duplicate_pairs.old_id;

WITH permission_renames (old_code, new_code) AS (
    VALUES
        ('settings.read', 'settings.read.all'),
        ('settings.update', 'settings.update.all'),
        ('dashboard', 'dashboard.read.own'),
        ('student.create', 'student.create.all'),
        ('student.delete', 'student.delete.all'),
        ('organization_work.create', 'organization_work.create.own'),
        ('activity.members.manage', 'activity.manage_members.all'),
        ('admission.verify', 'admission.verify.all'),
        ('admission.scores', 'admission.scores.all'),
        ('admission.enroll', 'admission.enroll.all')
),
duplicate_pairs AS (
    SELECT old_permissions.id AS old_id
    FROM permission_renames
    JOIN permissions old_permissions ON old_permissions.code = permission_renames.old_code
    JOIN permissions new_permissions ON new_permissions.code = permission_renames.new_code
)
DELETE FROM permissions permission_row
USING duplicate_pairs
WHERE permission_row.id = duplicate_pairs.old_id;

WITH permission_text_renames (old_code, new_code) AS (
    VALUES
        ('settings.read', 'settings.read.all'),
        ('settings.update', 'settings.update.all'),
        ('dashboard', 'dashboard.read.own'),
        ('student.create', 'student.create.all'),
        ('student.delete', 'student.delete.all'),
        ('organization_work.create', 'organization_work.create.own'),
        ('activity.members.manage', 'activity.manage_members.all'),
        ('admission.verify', 'admission.verify.all'),
        ('admission.scores', 'admission.scores.all'),
        ('admission.enroll', 'admission.enroll.all')
)
UPDATE menu_items
SET required_permission = permission_text_renames.new_code,
    updated_at = NOW()
FROM permission_text_renames
WHERE menu_items.required_permission = permission_text_renames.old_code;

WITH permission_renames (
    new_code,
    new_name,
    new_module,
    new_action,
    new_scope,
    new_description
) AS (
    VALUES
        ('settings.read.all', 'ดูการตั้งค่า', 'settings', 'read', 'all', 'ดูการตั้งค่าระบบ'),
        ('settings.update.all', 'แก้ไขการตั้งค่า', 'settings', 'update', 'all', 'แก้ไขการตั้งค่าระบบ'),
        ('dashboard.read.own', 'แดชบอร์ด', 'dashboard', 'read', 'own', 'ดูหน้าแดชบอร์ด'),
        ('student.create.all', 'เพิ่มนักเรียน', 'student', 'create', 'all', 'สร้างนักเรียนใหม่'),
        ('student.delete.all', 'ลบนักเรียน', 'student', 'delete', 'all', 'ลบนักเรียน'),
        ('organization_work.create.own', 'สร้าง/ส่งงาน', 'organization_work', 'create', 'own', 'ส่งงานหรือสร้างบันทึกงานใหม่'),
        ('activity.manage_members.all', 'จัดการสมาชิกกิจกรรม', 'activity', 'manage_members', 'all', 'จัดการสมาชิกในกิจกรรมพัฒนาผู้เรียน'),
        ('admission.verify.all', 'ตรวจสอบใบสมัคร', 'admission', 'verify', 'all', 'ยืนยัน/ปฏิเสธใบสมัครของผู้สมัคร'),
        ('admission.scores.all', 'กรอกคะแนนและจัดห้อง', 'admission', 'scores', 'all', 'กรอกคะแนนสอบ, เรียงคะแนน, จัดห้องเรียน'),
        ('admission.enroll.all', 'รับมอบตัว', 'admission', 'enroll', 'all', 'รับมอบตัวและสร้าง account นักเรียนในระบบ')
)
INSERT INTO permissions (code, name, module, action, scope, description)
SELECT new_code, new_name, new_module, new_action, new_scope, new_description
FROM permission_renames
ON CONFLICT (code) DO UPDATE
SET name = EXCLUDED.name,
    module = EXCLUDED.module,
    action = EXCLUDED.action,
    scope = EXCLUDED.scope,
    description = EXCLUDED.description,
    updated_at = NOW();

DO $$
DECLARE
    legacy_count INTEGER;
BEGIN
    SELECT COUNT(*)
    INTO legacy_count
    FROM permissions
    WHERE code IN (
        'settings.read',
        'settings.update',
        'dashboard',
        'student.create',
        'student.delete',
        'organization_work.create',
        'activity.members.manage',
        'admission.verify',
        'admission.scores',
        'admission.enroll'
    );

    IF legacy_count > 0 THEN
        RAISE EXCEPTION 'Permission code canonicalization failed: % legacy permission(s) remain', legacy_count;
    END IF;
END $$;
