/// Permission Registry - Centralized list of all permissions in the system
/// This is the single source of truth for permission codes
use serde::{Deserialize, Serialize};

/// Permission definition structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDef {
    pub code: &'static str,
    pub name: &'static str,      // Thai display name
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

    // Dashboard permission
    pub const DASHBOARD: &str = "dashboard";

    // Student permissions
    pub const STUDENT_READ_OWN: &str = "student.read.own";
    pub const STUDENT_UPDATE_OWN: &str = "student.update.own";
    pub const STUDENT_READ_ALL: &str = "student.read.all";
    pub const STUDENT_CREATE: &str = "student.create";
    pub const STUDENT_UPDATE_ALL: &str = "student.update.all";
    pub const STUDENT_DELETE: &str = "student.delete";

    // Achievement permissions
    pub const ACHIEVEMENT_READ_OWN: &str = "achievement.read.own";
    pub const ACHIEVEMENT_READ_ALL: &str = "achievement.read.all";
    pub const ACHIEVEMENT_CREATE_OWN: &str = "achievement.create.own";
    pub const ACHIEVEMENT_CREATE_ALL: &str = "achievement.create.all";
    pub const ACHIEVEMENT_UPDATE_OWN: &str = "achievement.update.own";
    pub const ACHIEVEMENT_UPDATE_ALL: &str = "achievement.update.all";
    pub const ACHIEVEMENT_DELETE_OWN: &str = "achievement.delete.own";
    pub const ACHIEVEMENT_DELETE_ALL: &str = "achievement.delete.all";
}

/// Complete list of all permissions in the system
pub const ALL_PERMISSIONS: &[PermissionDef] = &[
    // Staff permissions
    PermissionDef {
        code: codes::STAFF_READ_ALL,
        name: "ดูบุคลากรทั้งหมด",
        module: "staff",
        action: "read",
        scope: "all",
        description: "ดูข้อมูลบุคลากรทั้งหมด",
    },
    PermissionDef {
        code: codes::STAFF_CREATE_ALL,
        name: "เพิ่มบุคลากร",
        module: "staff",
        action: "create",
        scope: "all",
        description: "สร้างบุคลากรใหม่",
    },
    PermissionDef {
        code: codes::STAFF_UPDATE_ALL,
        name: "แก้ไขบุคลากร",
        module: "staff",
        action: "update",
        scope: "all",
        description: "แก้ไขข้อมูลบุคลากร",
    },
    PermissionDef {
        code: codes::STAFF_DELETE_ALL,
        name: "ลบบุคลากร",
        module: "staff",
        action: "delete",
        scope: "all",
        description: "ลบบุคลากร",
    },
    // Role permissions
    PermissionDef {
        code: codes::ROLES_READ_ALL,
        name: "ดูบทบาท",
        module: "roles",
        action: "read",
        scope: "all",
        description: "ดูข้อมูลบทบาทและฝ่ายทั้งหมด",
    },
    PermissionDef {
        code: codes::ROLES_CREATE_ALL,
        name: "สร้างบทบาท",
        module: "roles",
        action: "create",
        scope: "all",
        description: "สร้างบทบาทและฝ่ายใหม่",
    },
    PermissionDef {
        code: codes::ROLES_UPDATE_ALL,
        name: "แก้ไขบทบาท",
        module: "roles",
        action: "update",
        scope: "all",
        description: "แก้ไขบทบาทและฝ่าย",
    },
    PermissionDef {
        code: codes::ROLES_DELETE_ALL,
        name: "ลบบทบาท",
        module: "roles",
        action: "delete",
        scope: "all",
        description: "ลบบทบาทและฝ่าย",
    },
    PermissionDef {
        code: codes::ROLES_ASSIGN_ALL,
        name: "มอบหมายบทบาท",
        module: "roles",
        action: "assign",
        scope: "all",
        description: "มอบหมายบทบาทให้ผู้ใช้",
    },
    PermissionDef {
        code: codes::ROLES_REMOVE_ALL,
        name: "ถอนบทบาท",
        module: "roles",
        action: "remove",
        scope: "all",
        description: "ถอนบทบาทจากผู้ใช้",
    },
    // Menu permissions
    PermissionDef {
        code: codes::MENU_READ_ALL,
        name: "ดูเมนู",
        module: "menu",
        action: "read",
        scope: "all",
        description: "ดูเมนูทั้งหมด",
    },
    PermissionDef {
        code: codes::MENU_CREATE_ALL,
        name: "สร้างเมนู",
        module: "menu",
        action: "create",
        scope: "all",
        description: "สร้างเมนูใหม่",
    },
    PermissionDef {
        code: codes::MENU_UPDATE_ALL,
        name: "แก้ไขเมนู",
        module: "menu",
        action: "update",
        scope: "all",
        description: "แก้ไขเมนู",
    },
    PermissionDef {
        code: codes::MENU_DELETE_ALL,
        name: "ลบเมนู",
        module: "menu",
        action: "delete",
        scope: "all",
        description: "ลบเมนู",
    },
    // Settings permissions
    PermissionDef {
        code: codes::SETTINGS_READ,
        name: "ดูการตั้งค่า",
        module: "settings",
        action: "read",
        scope: "all",
        description: "ดูการตั้งค่าระบบ",
    },
    PermissionDef {
        code: codes::SETTINGS_UPDATE,
        name: "แก้ไขการตั้งค่า",
        module: "settings",
        action: "update",
        scope: "all",
        description: "แก้ไขการตั้งค่าระบบ",
    },
    // Feature toggles permissions
    PermissionDef {
        code: codes::FEATURES_READ_ALL,
        name: "ดูฟีเจอร์",
        module: "features",
        action: "read",
        scope: "all",
        description: "ดูการตั้งค่าฟีเจอร์",
    },
    PermissionDef {
        code: codes::FEATURES_UPDATE_ALL,
        name: "แก้ไขฟีเจอร์",
        module: "features",
        action: "update",
        scope: "all",
        description: "แก้ไขการตั้งค่าฟีเจอร์",
    },
    // Dashboard permission
    PermissionDef {
        code: codes::DASHBOARD,
        name: "แดชบอร์ด",
        module: "dashboard",
        action: "read",
        scope: "own",
        description: "ดูหน้าแดชบอร์ด",
    },
    // Student permissions
    PermissionDef {
        code: codes::STUDENT_READ_OWN,
        name: "ดูข้อมูลตนเอง",
        module: "student",
        action: "read",
        scope: "own",
        description: "นักเรียนดูข้อมูลตนเอง",
    },
    PermissionDef {
        code: codes::STUDENT_UPDATE_OWN,
        name: "แก้ไขข้อมูลตนเอง",
        module: "student",
        action: "update",
        scope: "own",
        description: "นักเรียนแก้ไขข้อมูลตนเอง (จำกัดฟิลด์)",
    },
    PermissionDef {
        code: codes::STUDENT_READ_ALL,
        name: "ดูนักเรียนทั้งหมด",
        module: "student",
        action: "read",
        scope: "all",
        description: "ดูข้อมูลนักเรียนทั้งหมด",
    },
    PermissionDef {
        code: codes::STUDENT_CREATE,
        name: "เพิ่มนักเรียน",
        module: "student",
        action: "create",
        scope: "all",
        description: "สร้างนักเรียนใหม่",
    },
    PermissionDef {
        code: codes::STUDENT_UPDATE_ALL,
        name: "แก้ไขนักเรียน",
        module: "student",
        action: "update",
        scope: "all",
        description: "แก้ไขข้อมูลนักเรียนทั้งหมด",
    },
    PermissionDef {
        code: codes::STUDENT_DELETE,
        name: "ลบนักเรียน",
        module: "student",
        action: "delete",
        scope: "all",
        description: "ลบนักเรียน",
    },
    // Achievement permissions
    PermissionDef {
        code: codes::ACHIEVEMENT_READ_OWN,
        name: "ดูผลงานตนเอง",
        module: "achievement",
        action: "read",
        scope: "own",
        description: "ดูรายการผลงานของตนเอง",
    },
    PermissionDef {
        code: codes::ACHIEVEMENT_READ_ALL,
        name: "ดูผลงานทั้งหมด",
        module: "achievement",
        action: "read",
        scope: "all",
        description: "ดูรายการผลงานของบุคคลากรทั้งหมด",
    },
    PermissionDef {
        code: codes::ACHIEVEMENT_CREATE_OWN,
        name: "เพิ่มผลงานตนเอง",
        module: "achievement",
        action: "create",
        scope: "own",
        description: "บันทึกผลงานของตนเอง",
    },
    PermissionDef {
        code: codes::ACHIEVEMENT_CREATE_ALL,
        name: "เพิ่มผลงานให้ผู้อื่น",
        module: "achievement",
        action: "create",
        scope: "all",
        description: "บันทึกผลงานแทนบุคคลากรอื่น",
    },
    PermissionDef {
        code: codes::ACHIEVEMENT_UPDATE_OWN,
        name: "แก้ไขผลงานตนเอง",
        module: "achievement",
        action: "update",
        scope: "own",
        description: "แก้ไขผลงานของตนเอง",
    },
    PermissionDef {
        code: codes::ACHIEVEMENT_UPDATE_ALL,
        name: "แก้ไขผลงานผู้อื่น",
        module: "achievement",
        action: "update",
        scope: "all",
        description: "แก้ไขผลงานของบุคคลากรอื่น",
    },
    PermissionDef {
        code: codes::ACHIEVEMENT_DELETE_OWN,
        name: "ลบผลงานตนเอง",
        module: "achievement",
        action: "delete",
        scope: "own",
        description: "ลบผลงานของตนเอง",
    },
    PermissionDef {
        code: codes::ACHIEVEMENT_DELETE_ALL,
        name: "ลบผลงานผู้อื่น",
        module: "achievement",
        action: "delete",
        scope: "all",
        description: "ลบผลงานของบุคคลากรอื่น",
    },
];
