/// Permission Registry - Centralized list of all permissions in the system
/// This is the single source of truth for permission codes
use serde::{Deserialize, Serialize};

/// Permission definition structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDef {
    pub code: &'static str,
    pub module: &'static str,
    pub action: &'static str,
    pub scope: &'static str,
    pub description: &'static str,
}

/// Permission constants for type-safe usage in handlers
pub mod codes {
    // Staff permissions
    pub const STAFF_READ_ALL: &str = "staff.read.all";
    pub const STAFF_CREATE_ALL: &str = "staff.create.all";
    pub const STAFF_UPDATE_ALL: &str = "staff.update.all";
    pub const STAFF_DELETE_ALL: &str = "staff.delete.all";

    // Role permissions
    pub const ROLES_READ_ALL: &str = "roles.read.all";
    pub const ROLES_CREATE_ALL: &str = "roles.create.all";
    pub const ROLES_UPDATE_ALL: &str = "roles.update.all";
    pub const ROLES_DELETE_ALL: &str = "roles.delete.all";
    pub const ROLES_ASSIGN_ALL: &str = "roles.assign.all";
    pub const ROLES_REMOVE_ALL: &str = "roles.remove.all";

    // Menu permissions
    pub const MENU_READ_ALL: &str = "menu.read.all";
    pub const MENU_CREATE_ALL: &str = "menu.create.all";
    pub const MENU_UPDATE_ALL: &str = "menu.update.all";
    pub const MENU_DELETE_ALL: &str = "menu.delete.all";

    // Settings permissions
    pub const SETTINGS_READ: &str = "settings.read";
    pub const SETTINGS_UPDATE: &str = "settings.update";

    // Feature toggles permissions
    pub const FEATURES_READ_ALL: &str = "features.read.all";
    pub const FEATURES_UPDATE_ALL: &str = "features.update.all";
}

/// Complete list of all permissions in the system
pub const ALL_PERMISSIONS: &[PermissionDef] = &[
    // Staff permissions
    PermissionDef {
        code: codes::STAFF_READ_ALL,
        module: "staff",
        action: "read",
        scope: "all",
        description: "ดูข้อมูลบุคลากรทั้งหมด",
    },
    PermissionDef {
        code: codes::STAFF_CREATE_ALL,
        module: "staff",
        action: "create",
        scope: "all",
        description: "สร้างบุคลากรใหม่",
    },
    PermissionDef {
        code: codes::STAFF_UPDATE_ALL,
        module: "staff",
        action: "update",
        scope: "all",
        description: "แก้ไขข้อมูลบุคลากร",
    },
    PermissionDef {
        code: codes::STAFF_DELETE_ALL,
        module: "staff",
        action: "delete",
        scope: "all",
        description: "ลบบุคลากร",
    },
    // Role permissions
    PermissionDef {
        code: codes::ROLES_READ_ALL,
        module: "roles",
        action: "read",
        scope: "all",
        description: "ดูข้อมูลบทบาทและฝ่ายทั้งหมด",
    },
    PermissionDef {
        code: codes::ROLES_CREATE_ALL,
        module: "roles",
        action: "create",
        scope: "all",
        description: "สร้างบทบาทและฝ่ายใหม่",
    },
    PermissionDef {
        code: codes::ROLES_UPDATE_ALL,
        module: "roles",
        action: "update",
        scope: "all",
        description: "แก้ไขบทบาทและฝ่าย",
    },
    PermissionDef {
        code: codes::ROLES_DELETE_ALL,
        module: "roles",
        action: "delete",
        scope: "all",
        description: "ลบบทบาทและฝ่าย",
    },
    PermissionDef {
        code: codes::ROLES_ASSIGN_ALL,
        module: "roles",
        action: "assign",
        scope: "all",
        description: "มอบหมายบทบาทให้ผู้ใช้",
    },
    PermissionDef {
        code: codes::ROLES_REMOVE_ALL,
        module: "roles",
        action: "remove",
        scope: "all",
        description: "ถอนบทบาทจากผู้ใช้",
    },
    // Menu permissions
    PermissionDef {
        code: codes::MENU_READ_ALL,
        module: "menu",
        action: "read",
        scope: "all",
        description: "ดูเมนูทั้งหมด",
    },
    PermissionDef {
        code: codes::MENU_CREATE_ALL,
        module: "menu",
        action: "create",
        scope: "all",
        description: "สร้างเมนูใหม่",
    },
    PermissionDef {
        code: codes::MENU_UPDATE_ALL,
        module: "menu",
        action: "update",
        scope: "all",
        description: "แก้ไขเมนู",
    },
    PermissionDef {
        code: codes::MENU_DELETE_ALL,
        module: "menu",
        action: "delete",
        scope: "all",
        description: "ลบเมนู",
    },
    // Settings permissions
    PermissionDef {
        code: codes::SETTINGS_READ,
        module: "settings",
        action: "read",
        scope: "all",
        description: "ดูการตั้งค่าระบบ",
    },
    PermissionDef {
        code: codes::SETTINGS_UPDATE,
        module: "settings",
        action: "update",
        scope: "all",
        description: "แก้ไขการตั้งค่าระบบ",
    },
    // Feature toggles permissions
    PermissionDef {
        code: codes::FEATURES_READ_ALL,
        module: "features",
        action: "read",
        scope: "all",
        description: "ดูการตั้งค่าฟีเจอร์",
    },
    PermissionDef {
        code: codes::FEATURES_UPDATE_ALL,
        module: "features",
        action: "update",
        scope: "all",
        description: "แก้ไขการตั้งค่าฟีเจอร์",
    },
];
