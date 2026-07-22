ALTER TABLE roles
    ADD COLUMN is_system boolean NOT NULL DEFAULT false;

ALTER TABLE organization_units
    ADD COLUMN is_system boolean NOT NULL DEFAULT false;

UPDATE roles
SET is_system = true
WHERE code = 'ADMIN';

UPDATE organization_units
SET is_system = true
WHERE code = 'SCHOOL';

COMMENT ON COLUMN roles.is_system IS
    'Protected system role; status cannot be deactivated through normal APIs';

COMMENT ON COLUMN organization_units.is_system IS
    'Protected system unit; status cannot be deactivated through normal APIs';
