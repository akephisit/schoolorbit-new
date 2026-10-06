-- Curriculum editions are shared school resources; preserve the corresponding
-- read/manage capability when retiring their organization ownership scopes.
INSERT INTO permissions(id,code,name,module,action,scope,description,is_active)
VALUES
    (gen_random_uuid(),'academic_curriculum.read.school','ดูหลักสูตรทั้งโรงเรียน','academic_curriculum','read','school','ดูฉบับหลักสูตร ระดับการศึกษาและแผนการเรียนของโรงเรียน',true),
    (gen_random_uuid(),'academic_curriculum.manage.school','จัดการหลักสูตรทั้งโรงเรียน','academic_curriculum','manage','school','จัดการและเผยแพร่ฉบับหลักสูตร ระดับการศึกษาและแผนการเรียนของโรงเรียน',true)
ON CONFLICT(code) DO UPDATE SET is_active=true;

CREATE TEMP TABLE curriculum_094_permission_mapping ON COMMIT DROP AS
SELECT old.id AS old_id,current.id AS current_id
FROM permissions old JOIN permissions current ON current.module=old.module
    AND current.action=old.action AND current.scope='school'
WHERE old.module='academic_curriculum' AND old.scope IN ('organization_unit','organization_tree') AND old.is_active;

INSERT INTO role_permissions(role_id,permission_id,created_at)
SELECT grant_row.role_id,m.current_id,min(grant_row.created_at)
FROM role_permissions grant_row JOIN curriculum_094_permission_mapping m ON m.old_id=grant_row.permission_id
GROUP BY grant_row.role_id,m.current_id ON CONFLICT DO NOTHING;

INSERT INTO organization_permission_grants(organization_unit_id,permission_id,position_code,created_at,created_by)
SELECT DISTINCT ON (grant_row.organization_unit_id,m.current_id,grant_row.position_code)
       grant_row.organization_unit_id,m.current_id,grant_row.position_code,grant_row.created_at,grant_row.created_by
FROM organization_permission_grants grant_row JOIN curriculum_094_permission_mapping m ON m.old_id=grant_row.permission_id
ORDER BY grant_row.organization_unit_id,m.current_id,grant_row.position_code,grant_row.created_at
ON CONFLICT DO NOTHING;

UPDATE organization_permission_delegations delegation SET permission_id=m.current_id
FROM curriculum_094_permission_mapping m WHERE delegation.permission_id=m.old_id;

UPDATE permissions SET is_active=false
WHERE module='academic_curriculum' AND scope IN ('organization_unit','organization_tree');
