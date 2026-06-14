-- Work item foundation for assigned, time-bounded school work.

CREATE TABLE work_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_window_id UUID NOT NULL REFERENCES workflow_windows(id) ON DELETE CASCADE,
    module_code VARCHAR(100) NOT NULL,
    source_resource_type VARCHAR(100) NOT NULL,
    source_resource_id UUID,
    title TEXT NOT NULL,
    description TEXT,
    action_path TEXT NOT NULL,
    required_permission VARCHAR(255),
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    metadata JSONB NOT NULL DEFAULT '{"tags":[]}'::jsonb,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_items_status_check
        CHECK (status IN ('active', 'closed', 'cancelled', 'archived'))
);

CREATE TABLE work_item_assignees (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_item_id UUID NOT NULL REFERENCES work_items(id) ON DELETE CASCADE,
    assignee_type VARCHAR(32) NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    organization_unit_id UUID REFERENCES organization_units(id) ON DELETE CASCADE,
    position_code VARCHAR(100),
    status VARCHAR(32) NOT NULL DEFAULT 'assigned',
    read_at TIMESTAMPTZ,
    submitted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_item_assignees_type_check
        CHECK (assignee_type IN ('user', 'organization_unit', 'organization_position')),
    CONSTRAINT work_item_assignees_status_check
        CHECK (status IN ('assigned', 'read', 'submitted', 'dismissed')),
    CONSTRAINT work_item_assignees_target_shape_check CHECK (
        (
            assignee_type = 'user'
            AND user_id IS NOT NULL
            AND organization_unit_id IS NULL
            AND position_code IS NULL
        )
        OR (
            assignee_type = 'organization_unit'
            AND user_id IS NULL
            AND organization_unit_id IS NOT NULL
            AND position_code IS NULL
        )
        OR (
            assignee_type = 'organization_position'
            AND user_id IS NULL
            AND organization_unit_id IS NOT NULL
            AND position_code IS NOT NULL
        )
    )
);

CREATE INDEX idx_work_items_window_id ON work_items(workflow_window_id);
CREATE INDEX idx_work_items_module_status ON work_items(module_code, status);
CREATE INDEX idx_work_items_source ON work_items(source_resource_type, source_resource_id);
CREATE INDEX idx_work_item_assignees_work_item_id ON work_item_assignees(work_item_id);
CREATE INDEX idx_work_item_assignees_user_id ON work_item_assignees(user_id)
    WHERE assignee_type = 'user';
CREATE INDEX idx_work_item_assignees_org_unit ON work_item_assignees(organization_unit_id)
    WHERE assignee_type IN ('organization_unit', 'organization_position');

CREATE UNIQUE INDEX idx_work_item_assignees_unique_user
    ON work_item_assignees(work_item_id, user_id)
    WHERE assignee_type = 'user';

CREATE UNIQUE INDEX idx_work_item_assignees_unique_org_unit
    ON work_item_assignees(work_item_id, organization_unit_id)
    WHERE assignee_type = 'organization_unit';

CREATE UNIQUE INDEX idx_work_item_assignees_unique_org_position
    ON work_item_assignees(work_item_id, organization_unit_id, position_code)
    WHERE assignee_type = 'organization_position';

CREATE TRIGGER update_work_items_updated_at
    BEFORE UPDATE ON work_items
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_work_item_assignees_updated_at
    BEFORE UPDATE ON work_item_assignees
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

COMMENT ON TABLE work_items IS 'Assigned work items inside workflow windows; used for My Work inbox, not permanent sidebar modules';
COMMENT ON TABLE work_item_assignees IS 'Direct, organization-unit, or organization-position assignments for work items';
COMMENT ON COLUMN work_items.required_permission IS 'Optional operation permission required by the owning workflow endpoint; listing the assignment is not a permission grant';
