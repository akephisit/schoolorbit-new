export const WILDCARD_PERMISSION = '*' as const;

export const PERMISSION_MODULES = {
	ACADEMIC_CURRICULUM: 'academic_curriculum',
	ACTIVITY: 'activity',
	ROLES: 'roles',
	SETTINGS: 'settings',
	STAFF: 'staff'
} as const;

export const PERMISSIONS = {
	ACADEMIC_CLASSROOM_READ_ALL: 'academic_classroom.read.all',
	ACADEMIC_COURSE_PLAN_MANAGE_ALL: 'academic_course_plan.manage.all',
	ACADEMIC_COURSE_PLAN_READ_ALL: 'academic_course_plan.read.all',
	ACADEMIC_CURRICULUM_MANAGE_DEPARTMENT: 'academic_curriculum.manage.department',
	ACADEMIC_CURRICULUM_READ_ALL: 'academic_curriculum.read.all',
	ACADEMIC_ENROLLMENT_READ_ALL: 'academic_enrollment.read.all',
	ACADEMIC_STRUCTURE_MANAGE_ALL: 'academic_structure.manage.all',
	ACADEMIC_STRUCTURE_READ_ALL: 'academic_structure.read.all',
	ACHIEVEMENT_READ_ALL: 'achievement.read.all',
	ACTIVITY_MANAGE_ALL: 'activity.manage.all',
	ACTIVITY_MANAGE_OWN: 'activity.manage.own',
	ACTIVITY_MEMBERS_MANAGE: 'activity.members.manage',
	ADMISSION_READ_ALL: 'admission.read.all',
	DEPT_WORK_APPROVE_DEPARTMENT: 'dept_work.approve.department',
	FACILITY_READ_ALL: 'facility.read.all',
	ROLES_ASSIGN_ALL: 'roles.assign.all',
	SETTINGS_UPDATE: 'settings.update',
	STUDENT_READ_ALL: 'student.read.all'
} as const;

export type PermissionCode = (typeof PERMISSIONS)[keyof typeof PERMISSIONS];
export type PermissionModule = (typeof PERMISSION_MODULES)[keyof typeof PERMISSION_MODULES];
export type RoutePermission = PermissionCode | PermissionModule;
