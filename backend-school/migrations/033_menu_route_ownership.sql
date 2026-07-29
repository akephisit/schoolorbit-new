ALTER TABLE menu_items
ADD COLUMN managed_by character varying(20) NOT NULL DEFAULT 'school';

ALTER TABLE menu_items
ADD CONSTRAINT menu_items_managed_by_check
CHECK (managed_by IN ('frontend', 'school', 'integration'));

COMMENT ON COLUMN menu_items.managed_by IS
    'Owner of menu identity and lifecycle. Frontend synchronization may delete only frontend-owned rows.';

CREATE INDEX idx_menu_items_frontend_managed_code
ON menu_items (code)
WHERE managed_by = 'frontend';
