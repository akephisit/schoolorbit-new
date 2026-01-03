-- ===================================================================
-- Migration 011: Dynamic Menu System with Feature Toggles
-- Description: Database-driven menu system with permission-based filtering
-- Date: 2025-12-23
-- ===================================================================

-- ===================================================================
-- 1. Menu Groups Table
-- ===================================================================
CREATE TABLE IF NOT EXISTS menu_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(100) NOT NULL,
    name_en VARCHAR(100),
    description TEXT,
    
    -- Display
    icon VARCHAR(50),  -- Icon name from lucide-svelte
    display_order INTEGER DEFAULT 0,
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_menu_groups_code ON menu_groups(code);
CREATE INDEX IF NOT EXISTS idx_menu_groups_active ON menu_groups(is_active);
CREATE INDEX IF NOT EXISTS idx_menu_groups_order ON menu_groups(display_order);

COMMENT ON TABLE menu_groups IS 'กลุ่มเมนู สำหรับจัดหมวดหมู่เมนูในระบบ';
COMMENT ON COLUMN menu_groups.icon IS 'ชื่อ icon จาก lucide-svelte เช่น layout-dashboard, shield';

-- ===================================================================
-- 2. Menu Items Table
-- ===================================================================
CREATE TABLE IF NOT EXISTS menu_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Basic Info
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(100) NOT NULL,
    name_en VARCHAR(100),
    description TEXT,
    
    -- Grouping & Hierarchy
    group_id UUID REFERENCES menu_groups(id) ON DELETE SET NULL,
    parent_id UUID REFERENCES menu_items(id) ON DELETE CASCADE,
    
    -- Navigation
    path VARCHAR(200) NOT NULL,
    icon VARCHAR(50),
    
    -- Permission
    required_permission VARCHAR(100),
    
    -- Display
    display_order INTEGER DEFAULT 0,
    is_active BOOLEAN DEFAULT true,
    
    -- Metadata
    metadata JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_menu_items_code ON menu_items(code);
CREATE INDEX IF NOT EXISTS idx_menu_items_group ON menu_items(group_id);
CREATE INDEX IF NOT EXISTS idx_menu_items_parent ON menu_items(parent_id);
CREATE INDEX IF NOT EXISTS idx_menu_items_permission ON menu_items(required_permission);
CREATE INDEX IF NOT EXISTS idx_menu_items_active ON menu_items(is_active);
CREATE INDEX IF NOT EXISTS idx_menu_items_order ON menu_items(display_order);

COMMENT ON TABLE menu_items IS 'รายการเมนูในระบบ';
COMMENT ON COLUMN menu_items.path IS 'URL path เช่น /attendance, /grades';
COMMENT ON COLUMN menu_items.required_permission IS 'Permission ที่ต้องการ เช่น attendance.read, staff.read.own';
COMMENT ON COLUMN menu_items.parent_id IS 'สำหรับ sub-menu (ถ้ามี)';

-- ===================================================================
-- 3. Feature Toggles Table
-- ===================================================================
CREATE TABLE IF NOT EXISTS feature_toggles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Feature Info
    code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(100) NOT NULL,
    name_en VARCHAR(100),
    description TEXT,
    
    -- Module Association
    module VARCHAR(50),
    
    -- Status
    is_enabled BOOLEAN DEFAULT true,
    
    -- Time-based Toggle (optional)
    enabled_from TIMESTAMPTZ,
    enabled_until TIMESTAMPTZ,
    
    -- Metadata
    metadata JSONB DEFAULT '{}',
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_feature_toggles_code ON feature_toggles(code);
CREATE INDEX IF NOT EXISTS idx_feature_toggles_module ON feature_toggles(module);
CREATE INDEX IF NOT EXISTS idx_feature_toggles_enabled ON feature_toggles(is_enabled);

COMMENT ON TABLE feature_toggles IS 'Feature toggles สำหรับเปิด/ปิดฟีเจอร์';
COMMENT ON COLUMN feature_toggles.enabled_from IS 'เปิดใช้งานตั้งแต่วันที่';
COMMENT ON COLUMN feature_toggles.enabled_until IS 'เปิดใช้งานถึงวันที่';

-- ===================================================================
-- 4. Menu Item Features Junction Table
-- ===================================================================
CREATE TABLE IF NOT EXISTS menu_item_features (
    menu_item_id UUID NOT NULL REFERENCES menu_items(id) ON DELETE CASCADE,
    feature_id UUID NOT NULL REFERENCES feature_toggles(id) ON DELETE CASCADE,
    PRIMARY KEY (menu_item_id, feature_id)
);

CREATE INDEX IF NOT EXISTS idx_menu_item_features_menu ON menu_item_features(menu_item_id);
CREATE INDEX IF NOT EXISTS idx_menu_item_features_feature ON menu_item_features(feature_id);

COMMENT ON TABLE menu_item_features IS 'เชื่อม menu items กับ feature toggles';

-- ===================================================================
-- 5. Insert Default Menu Groups
-- ===================================================================
INSERT INTO menu_groups (code, name, name_en, icon, display_order) VALUES
    ('main', 'เมนูหลัก', 'Main Menu', 'layout-dashboard', 1),
    ('settings', 'ตั้งค่า', 'Settings', 'settings', 2),
    ('other', 'อื่นๆ', 'Other', 'folder-open', 999)
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 6. Insert Default Menu Items
-- ===================================================================
INSERT INTO menu_items (code, name, name_en, path, icon, required_permission, group_id, display_order, description) VALUES
    -- Main Menu
    ('dashboard', 'แดชบอร์ด', 'Dashboard', '/dashboard', 'layout-dashboard', 'dashboard',
     (SELECT id FROM menu_groups WHERE code = 'main'), 1, 'หน้าหลักแสดงภาพรวม'),
     
    ('students', 'นักเรียน', 'Students', '/students', 'users', 'students',
     (SELECT id FROM menu_groups WHERE code = 'main'), 2, 'จัดการข้อมูลนักเรียน'),
     
    ('staff', 'บุคลากร', 'Staff', '/staff', 'graduation-cap', 'staff',
     (SELECT id FROM menu_groups WHERE code = 'main'), 3, 'จัดการข้อมูลบุคลากร'),
     
    ('subjects', 'รายวิชา', 'Subjects', '/subjects', 'book-open', 'subjects',
     (SELECT id FROM menu_groups WHERE code = 'main'), 4, 'จัดการรายวิชา'),
     
    ('classes', 'ห้องเรียน', 'Classes', '/classes', 'school', 'classes',
     (SELECT id FROM menu_groups WHERE code = 'main'), 5, 'จัดการห้องเรียน'),
     
    ('calendar', 'ปฏิทิน', 'Calendar', '/calendar', 'calendar', 'calendar',
     (SELECT id FROM menu_groups WHERE code = 'main'), 6, 'ปฏิทินกิจกรรม'),
     
    -- Admin Menu
    ('roles', 'จัดการบทบาท', 'Role Management', '/admin/roles', 'shield', 'roles',
     (SELECT id FROM menu_groups WHERE code = 'admin'), 1, 'จัดการบทบาทและสิทธิ์'),
     
    -- Settings Menu
    ('settings_general', 'ตั้งค่าทั่วไป', 'General Settings', '/settings', 'settings', 'settings',
     (SELECT id FROM menu_groups WHERE code = 'settings'), 1, 'ตั้งค่าระบบทั่วไป')
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 7. Insert Default Feature Toggles
-- ===================================================================
INSERT INTO feature_toggles (code, name, name_en, module, is_enabled, description) VALUES
    ('attendance_system', 'ระบบเช็คชื่อ', 'Attendance System', 'attendance', true, 
     'ระบบบันทึกการเข้าเรียน'),
     
    ('grade_management', 'ระบบจัดการคะแนน', 'Grade Management', 'grades', true,
     'ระบบบันทึกและจัดการคะแนน'),
     
    ('document_approval', 'ระบบอนุมัติเอกสาร', 'Document Approval', 'documents', true,
     'ระบบอนุมัติเอกสารออนไลน์'),
     
    ('calendar_events', 'ระบบปฏิทินกิจกรรม', 'Calendar Events', 'calendar', true,
     'ปฏิทินและการจัดการกิจกรรม')
ON CONFLICT (code) DO NOTHING;

-- ===================================================================
-- 8. Add Updated At Triggers
-- ===================================================================
DROP TRIGGER IF EXISTS update_menu_groups_updated_at ON menu_groups;
CREATE TRIGGER update_menu_groups_updated_at
    BEFORE UPDATE ON menu_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_menu_items_updated_at ON menu_items;
CREATE TRIGGER update_menu_items_updated_at
    BEFORE UPDATE ON menu_items
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

DROP TRIGGER IF EXISTS update_feature_toggles_updated_at ON feature_toggles;
CREATE TRIGGER update_feature_toggles_updated_at
    BEFORE UPDATE ON feature_toggles
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ===================================================================
-- 9. Verify the installation
-- ===================================================================
SELECT 
    mg.name as group_name,
    COUNT(mi.id) as menu_count
FROM menu_groups mg
LEFT JOIN menu_items mi ON mg.id = mi.group_id
WHERE mg.is_active = true
GROUP BY mg.id, mg.name, mg.display_order
ORDER BY mg.display_order;

-- ===================================================================
-- NOTES:
-- ===================================================================
-- Dynamic Menu System Features:
--   1. Menu items stored in database (not hardcoded)
--   2. Auto-grouping by menu_groups
--   3. Feature toggles can enable/disable menu items
--   4. Permission-based filtering
--   5. Support for sub-menus via parent_id
--   6. Time-based feature toggles (optional)
--
-- Usage:
--   - Frontend calls /api/menu/user
--   - Backend filters by user permissions & feature toggles
--   - Returns only accessible menu items grouped by category
--
-- Admin can:
--   - Add/remove menu items without code changes
--   - Enable/disable features dynamically
--   - Reorder menu items
--   - Toggle features by time period
-- ===================================================================
