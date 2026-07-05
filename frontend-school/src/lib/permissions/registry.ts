export const WILDCARD_PERMISSION = '*' as const;

export const PERMISSION_MODULES = {
	ACADEMIC_ASSESSMENT: 'academic_assessment',
	ACADEMIC_CLASSROOM: 'academic_classroom',
	ACADEMIC_COURSE_PLAN: 'academic_course_plan',
	ACADEMIC_CURRICULUM: 'academic_curriculum',
	ACADEMIC_ENROLLMENT: 'academic_enrollment',
	ACADEMIC_EXAM_SCHEDULE: 'academic_exam_schedule',
	ACADEMIC_PROMOTION: 'academic_promotion',
	ACADEMIC_STRUCTURE: 'academic_structure',
	ACADEMIC_TIMETABLE_TODAY: 'academic_timetable_today',
	ACTIVITY: 'activity',
	ACHIEVEMENT: 'achievement',
	ADMISSION: 'admission',
	CALENDAR: 'calendar',
	DASHBOARD: 'dashboard',
	FACILITY: 'facility',
	FEATURES: 'features',
	MENU: 'menu',
	ORGANIZATION_WORK: 'organization_work',
	ROLES: 'roles',
	SETTINGS: 'settings',
	STAFF: 'staff',
	STAFF_PII: 'staff_pii',
	STAFF_PROFILE: 'staff_profile',
	SUPERVISION: 'supervision',
	STUDENT: 'student',
	STUDENT_PII: 'student_pii',
	SYSTEM: 'system'
} as const;

export const PERMISSIONS = {
	ACADEMIC_ASSESSMENT_MANAGE_ASSIGNED: 'academic_assessment.manage.assigned',
	ACADEMIC_ASSESSMENT_MANAGE_SCHOOL: 'academic_assessment.manage.school',
	ACADEMIC_ASSESSMENT_READ_ASSIGNED: 'academic_assessment.read.assigned',
	ACADEMIC_ASSESSMENT_READ_ORGANIZATION_UNIT: 'academic_assessment.read.organization_unit',
	ACADEMIC_ASSESSMENT_READ_SCHOOL: 'academic_assessment.read.school',
	ACADEMIC_CLASSROOM_CREATE_ALL: 'academic_classroom.create.all',
	ACADEMIC_CLASSROOM_DELETE_ALL: 'academic_classroom.delete.all',
	ACADEMIC_CLASSROOM_READ_ALL: 'academic_classroom.read.all',
	ACADEMIC_CLASSROOM_UPDATE_ALL: 'academic_classroom.update.all',
	ACADEMIC_COURSE_PLAN_MANAGE_ALL: 'academic_course_plan.manage.all',
	ACADEMIC_COURSE_PLAN_READ_ALL: 'academic_course_plan.read.all',
	ACADEMIC_CURRICULUM_CREATE_ALL: 'academic_curriculum.create.all',
	ACADEMIC_CURRICULUM_DELETE_ALL: 'academic_curriculum.delete.all',
	ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT: 'academic_curriculum.manage.organization_unit',
	ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_TREE: 'academic_curriculum.manage.organization_tree',
	ACADEMIC_CURRICULUM_READ_ALL: 'academic_curriculum.read.all',
	ACADEMIC_CURRICULUM_READ_ORGANIZATION_TREE: 'academic_curriculum.read.organization_tree',
	ACADEMIC_CURRICULUM_UPDATE_ALL: 'academic_curriculum.update.all',
	ACADEMIC_ENROLLMENT_READ_ALL: 'academic_enrollment.read.all',
	ACADEMIC_ENROLLMENT_UPDATE_ALL: 'academic_enrollment.update.all',
	ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL: 'academic_exam_schedule.manage.school',
	ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL: 'academic_exam_schedule.publish.school',
	ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL: 'academic_exam_schedule.read.school',
	ACADEMIC_PROMOTION_EXECUTE_ALL: 'academic_promotion.execute.all',
	ACADEMIC_PROMOTION_READ_ALL: 'academic_promotion.read.all',
	ACADEMIC_STRUCTURE_MANAGE_ALL: 'academic_structure.manage.all',
	ACADEMIC_STRUCTURE_READ_ALL: 'academic_structure.read.all',
	ACADEMIC_TIMETABLE_TODAY_READ_SCHOOL: 'academic_timetable_today.read.school',
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
	ACTIVITY_READ_ALL: 'activity.read.all',
	ADMISSION_ENROLL_ALL: 'admission.enroll.all',
	ADMISSION_MANAGE_ALL: 'admission.manage.all',
	ADMISSION_READ_ALL: 'admission.read.all',
	ADMISSION_SCORES_ALL: 'admission.scores.all',
	ADMISSION_VERIFY_ALL: 'admission.verify.all',
	CALENDAR_MANAGE_SCHOOL: 'calendar.manage.school',
	CALENDAR_READ_SCHOOL: 'calendar.read.school',
	DASHBOARD_READ_OWN: 'dashboard.read.own',
	FACILITY_CREATE_ALL: 'facility.create.all',
	FACILITY_DELETE_ALL: 'facility.delete.all',
	FACILITY_READ_ALL: 'facility.read.all',
	FACILITY_UPDATE_ALL: 'facility.update.all',
	FEATURES_READ_ALL: 'features.read.all',
	FEATURES_UPDATE_ALL: 'features.update.all',
	MENU_CREATE_ALL: 'menu.create.all',
	MENU_DELETE_ALL: 'menu.delete.all',
	MENU_READ_ALL: 'menu.read.all',
	MENU_UPDATE_ALL: 'menu.update.all',
	ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT: 'organization_work.approve.organization_unit',
	ORGANIZATION_WORK_CREATE_OWN: 'organization_work.create.own',
	ORGANIZATION_WORK_READ_ORGANIZATION_UNIT: 'organization_work.read.organization_unit',
	ORGANIZATION_WORK_READ_OWN: 'organization_work.read.own',
	ORGANIZATION_WORK_UPDATE_OWN: 'organization_work.update.own',
	ROLES_ASSIGN_ALL: 'roles.assign.all',
	ROLES_CREATE_ALL: 'roles.create.all',
	ROLES_DELETE_ALL: 'roles.delete.all',
	ROLES_READ_ALL: 'roles.read.all',
	ROLES_REMOVE_ALL: 'roles.remove.all',
	ROLES_UPDATE_ALL: 'roles.update.all',
	SETTINGS_READ_ALL: 'settings.read.all',
	SETTINGS_UPDATE_ALL: 'settings.update.all',
	STAFF_CREATE_ALL: 'staff.create.all',
	STAFF_DELETE_ALL: 'staff.delete.all',
	STAFF_PII_READ_OWN: 'staff_pii.read.own',
	STAFF_PII_READ_SCHOOL: 'staff_pii.read.school',
	STAFF_PROFILE_READ_OWN: 'staff_profile.read.own',
	STAFF_PROFILE_READ_ORGANIZATION_TREE: 'staff_profile.read.organization_tree',
	STAFF_PROFILE_READ_ORGANIZATION_UNIT: 'staff_profile.read.organization_unit',
	STAFF_PROFILE_READ_SCHOOL: 'staff_profile.read.school',
	STAFF_READ_ALL: 'staff.read.all',
	STAFF_UPDATE_ALL: 'staff.update.all',
	SUPERVISION_APPROVE_SCHOOL: 'supervision.approve.school',
	SUPERVISION_EVALUATE_ASSIGNED: 'supervision.evaluate.assigned',
	SUPERVISION_MANAGE_ORGANIZATION_TREE: 'supervision.manage.organization_tree',
	SUPERVISION_MANAGE_ORGANIZATION_UNIT: 'supervision.manage.organization_unit',
	SUPERVISION_MANAGE_SCHOOL: 'supervision.manage.school',
	SUPERVISION_READ_ASSIGNED: 'supervision.read.assigned',
	SUPERVISION_READ_ORGANIZATION_TREE: 'supervision.read.organization_tree',
	SUPERVISION_READ_ORGANIZATION_UNIT: 'supervision.read.organization_unit',
	SUPERVISION_READ_OWN: 'supervision.read.own',
	SUPERVISION_READ_SCHOOL: 'supervision.read.school',
	SUPERVISION_REQUEST_OWN: 'supervision.request.own',
	STUDENT_PII_READ_ASSIGNED: 'student_pii.read.assigned',
	STUDENT_PII_READ_OWN: 'student_pii.read.own',
	STUDENT_PII_READ_SCHOOL: 'student_pii.read.school',
	STUDENT_CREATE_ALL: 'student.create.all',
	STUDENT_DELETE_ALL: 'student.delete.all',
	STUDENT_READ_ASSIGNED: 'student.read.assigned',
	STUDENT_READ_OWN: 'student.read.own',
	STUDENT_READ_SCHOOL: 'student.read.school',
	STUDENT_UPDATE_ALL: 'student.update.all',
	STUDENT_UPDATE_OWN: 'student.update.own'
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
	request: 'ส่งคำขอ',
	scores: 'คะแนน',
	update: 'แก้ไข',
	evaluate: 'ประเมิน',
	verify: 'ตรวจสอบ'
};

export function permissionScopeMeta(scope: string | undefined): PermissionMeta {
	return (
		SCOPE_META[scope ?? ''] ?? {
			label: scope || 'ไม่ระบุขอบเขต',
			description: 'ขอบเขตนี้ยังไม่มีคำอธิบายในระบบ',
			tone: 'warning'
		}
	);
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
