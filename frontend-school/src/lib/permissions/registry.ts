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
	ACTIVITY_MANAGE_OWN: 'activity.manage.own',
	ACTIVITY_MEMBERS_MANAGE: 'activity.members.manage',
	ADMISSION_READ_ALL: 'admission.read.all',
	ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT: 'organization_work.approve.organization_unit',
	FACILITY_READ_ALL: 'facility.read.all',
	ROLES_ASSIGN_ALL: 'roles.assign.all',
	SETTINGS_UPDATE: 'settings.update',
	STAFF_PII_READ_OWN: 'staff_pii.read.own',
	STAFF_PII_READ_SCHOOL: 'staff_pii.read.school',
	STAFF_PROFILE_READ_OWN: 'staff_profile.read.own',
	STAFF_PROFILE_READ_ORGANIZATION_TREE: 'staff_profile.read.organization_tree',
	STAFF_PROFILE_READ_ORGANIZATION_UNIT: 'staff_profile.read.organization_unit',
	STAFF_PROFILE_READ_SCHOOL: 'staff_profile.read.school',
	STUDENT_PII_READ_ASSIGNED: 'student_pii.read.assigned',
	STUDENT_PII_READ_OWN: 'student_pii.read.own',
	STUDENT_PII_READ_SCHOOL: 'student_pii.read.school',
	STUDENT_READ_ASSIGNED: 'student.read.assigned',
	STUDENT_READ_OWN: 'student.read.own',
	STUDENT_READ_SCHOOL: 'student.read.school'
} as const;

export type PermissionCode = (typeof PERMISSIONS)[keyof typeof PERMISSIONS];
export type PermissionModule = (typeof PERMISSION_MODULES)[keyof typeof PERMISSION_MODULES];
export type RoutePermission = PermissionCode | PermissionModule;
