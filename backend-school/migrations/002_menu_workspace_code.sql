ALTER TABLE menu_groups
ADD COLUMN workspace_code character varying(50) NOT NULL DEFAULT 'operations';

UPDATE menu_groups
SET workspace_code = CASE code
    WHEN 'main' THEN 'home'
    WHEN 'academic' THEN 'academic'
    WHEN 'personnel' THEN 'personnel'
    WHEN 'settings' THEN 'settings'
    WHEN 'general_admin' THEN 'operations'
    WHEN 'budget' THEN 'operations'
    ELSE 'operations'
END;

CREATE INDEX idx_menu_groups_workspace_code ON menu_groups (workspace_code);

COMMENT ON COLUMN menu_groups.workspace_code IS 'Stable sidebar workspace bucket. Routine opening/closing of school work belongs to workflow windows, not feature toggles.';
