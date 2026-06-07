export const WILDCARD_PERMISSION = '*' as const;

export const PERMISSION_MODULES = {
	ACADEMIC_CURRICULUM: 'academic_curriculum',
	ACTIVITY: 'activity',
	ACHIEVEMENT: 'achievement',
	ROLES: 'roles',
	SETTINGS: 'settings',
	STAFF: 'staff',
	STAFF_PII: 'staff_pii',
	STAFF_PROFILE: 'staff_profile',
	STUDENT: 'student',
	STUDENT_PII: 'student_pii'
} as const;

export const PERMISSIONS = {
	ACADEMIC_CLASSROOM_READ_ALL: 'academic_classroom.read.all',
	ACADEMIC_COURSE_PLAN_MANAGE_ALL: 'academic_course_plan.manage.all',
	ACADEMIC_COURSE_PLAN_READ_ALL: 'academic_course_plan.read.all',
	ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT: 'academic_curriculum.manage.organization_unit',
	ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE: 'academic_curriculum.manage.organization_tree',
	ACADEMIC_CURRICULUM_READ_ALL: 'academic_curriculum.read.all',
	ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE: 'academic_curriculum.read.organization_tree',
	ACADEMIC_ENROLLMENT_READ_ALL: 'academic_enrollment.read.all',
	ACADEMIC_STRUCTURE_MANAGE_ALL: 'academic_structure.manage.all',
	ACADEMIC_STRUCTURE_READ_ALL: 'academic_structure.read.all',
	ACHIEVEMENT_CREATE_ALL: 'achievement.create.all',
	ACHIEVEMENT_CREATE_OWN: 'achievement.create.own',
	ACHIEVEMENT_DELETE_ALL: 'achievement.delete.all',
	ACHIEVEMENT_DELETE_OWN: 'achievement.delete.own',
	ACHIEVEMENT_READ_ALL: 'achievement.read.all',
	ACHIEVEMENT_READ_OWN: 'achievement.read.own',
	ACHIEVEMENT_UPDATE_ALL: 'achievement.update.all',
	ACHIEVEMENT_UPDATE_OWN: 'achievement.update.own',
	ACTIVITY_MANAGE_ALL: 'activity.manage.all',
	ACTIVITY_MANAGE_MEMBERS_ALL: 'activity.manage_members.all',
	ACTIVITY_MANAGE_OWN: 'activity.manage.own',
	ADMISSION_ENROLL_ALL: 'admission.enroll.all',
	ADMISSION_READ_ALL: 'admission.read.all',
	ADMISSION_SCORES_ALL: 'admission.scores.all',
	ADMISSION_VERIFY_ALL: 'admission.verify.all',
	DASHBOARD_READ_OWN: 'dashboard.read.own',
	FACILITY_READ_ALL: 'facility.read.all',
	ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT: 'organization_work.approve.organization_unit',
	ORGANIZATION_WORK_CREATE_OWN: 'organization_work.create.own',
	ROLES_ASSIGN_ALL: 'roles.assign.all',
	ROLES_UPDATE_ALL: 'roles.update.all',
	SETTINGS_READ_ALL: 'settings.read.all',
	SETTINGS_UPDATE_ALL: 'settings.update.all',
	STAFF_PII_READ_OWN: 'staff_pii.read.own',
	STAFF_PII_READ_SCHOOL: 'staff_pii.read.school',
	STAFF_PROFILE_READ_OWN: 'staff_profile.read.own',
	STAFF_PROFILE_READ_ORGANIZATION_TREE: 'staff_profile.read.organization_tree',
	STAFF_PROFILE_READ_ORGANIZATION_UNIT: 'staff_profile.read.organization_unit',
	STAFF_PROFILE_READ_SCHOOL: 'staff_profile.read.school',
	STUDENT_PII_READ_ASSIGNED: 'student_pii.read.assigned',
	STUDENT_PII_READ_OWN: 'student_pii.read.own',
	STUDENT_PII_READ_SCHOOL: 'student_pii.read.school',
	STUDENT_CREATE_ALL: 'student.create.all',
	STUDENT_DELETE_ALL: 'student.delete.all',
	STUDENT_READ_ASSIGNED: 'student.read.assigned',
	STUDENT_READ_OWN: 'student.read.own',
	STUDENT_READ_SCHOOL: 'student.read.school'
} as const;

export type PermissionCode = (typeof PERMISSIONS)[keyof typeof PERMISSIONS];
export type PermissionModule = (typeof PERMISSION_MODULES)[keyof typeof PERMISSION_MODULES];
export type RoutePermission = PermissionCode | PermissionModule;

type PermissionMeta = {
	label: string;
	description: string;
	tone: 'default' | 'muted' | 'warning' | 'danger';
};

const SCOPE_META: Record<string, PermissionMeta> = {
	own: {
		label: 'เฉพาะตนเอง',
		description: 'ใช้กับข้อมูลหรือรายการที่เป็นของผู้ใช้นั้นเอง',
		tone: 'muted'
	},
	assigned: {
		label: 'ที่รับผิดชอบ',
		description: 'ใช้กับข้อมูลที่ผู้ใช้นั้นได้รับมอบหมายโดยตรง',
		tone: 'default'
	},
	organization_unit: {
		label: 'หน่วยงานเดียวกัน',
		description: 'ใช้กับข้อมูลในหน่วยงานที่ผู้ใช้สังกัดเท่านั้น',
		tone: 'default'
	},
	organization_tree: {
		label: 'สายงาน/หน่วยงานย่อย',
		description: 'ใช้กับหน่วยงานที่สังกัดและหน่วยงานย่อยในสายงาน',
		tone: 'warning'
	},
	school: {
		label: 'ทั้งโรงเรียน',
		description: 'ใช้กับข้อมูลทั้งโรงเรียน ต้องให้เฉพาะบทบาทที่ดูแลภาพรวม',
		tone: 'danger'
	},
	all: {
		label: 'ทั้งหมด',
		description: 'สิทธิ์ระดับระบบหรืองาน admin ที่ไม่ผูกกับ resource รายบุคคล',
		tone: 'danger'
	},
	global: {
		label: 'ระบบทั้งหมด',
		description: 'สิทธิ์สูงสุดสำหรับผู้ดูแลระบบ',
		tone: 'danger'
	}
};

const ACTION_LABELS: Record<string, string> = {
	all: 'ทั้งหมด',
	approve: 'อนุมัติ',
	assign: 'มอบหมาย',
	create: 'สร้าง',
	delete: 'ลบ',
	enroll: 'มอบตัว',
	execute: 'ดำเนินการ',
	manage: 'จัดการ',
	manage_members: 'จัดการสมาชิก',
	read: 'ดู',
	remove: 'ถอดออก',
	scores: 'คะแนน',
	update: 'แก้ไข',
	verify: 'ตรวจสอบ'
};

export function permissionScopeMeta(scope: string | undefined): PermissionMeta {
	return SCOPE_META[scope ?? ''] ?? {
		label: scope || 'ไม่ระบุขอบเขต',
		description: 'ขอบเขตนี้ยังไม่มีคำอธิบายในระบบ',
		tone: 'warning'
	};
}

export function permissionActionLabel(action: string | undefined): string {
	return ACTION_LABELS[action ?? ''] ?? action ?? 'ไม่ระบุการทำงาน';
}

export function permissionScopeToneClass(tone: PermissionMeta['tone']): string {
	switch (tone) {
		case 'danger':
			return 'border-red-200 bg-red-50 text-red-700';
		case 'warning':
			return 'border-amber-200 bg-amber-50 text-amber-700';
		case 'default':
			return 'border-blue-200 bg-blue-50 text-blue-700';
		default:
			return 'border-muted bg-muted/40 text-muted-foreground';
	}
}
