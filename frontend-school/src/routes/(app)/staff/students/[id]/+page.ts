import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
import { loadStudentProfileHistory } from '#lib/academic-core/student-profile-history.js';
import { studentRegisterContext } from '#lib/navigation/student-register.js';
import { getStudent } from '#lib/api/students.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff' as const, permission: PERMISSION_MODULES.STUDENT }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	depends(`school:student-register-profile:${params.id}`);
	const studentId = params.id;
	const context = studentRegisterContext(url);
	const academicYearId = context.academicYearId;
	const student = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			user?.user_type === 'staff' &&
			get(can).hasAny(
				PERMISSIONS.STUDENT_READ_SCHOOL,
				PERMISSIONS.STUDENT_READ_ASSIGNED,
				PERMISSIONS.STUDENT_READ_OWN
			)
				? getStudent(studentId, academicYearId, { requestFetch: fetch })
				: null
		),
		'โหลดข้อมูลนักเรียนไม่สำเร็จ'
	);
	const history = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			academicYearId &&
			user?.user_type === 'staff' &&
			get(can).has(PERMISSIONS.STUDENT_ACADEMIC_YEAR_READ_SCHOOL) &&
			get(can).hasAny(PERMISSIONS.HOMEROOM_READ_SCHOOL, PERMISSIONS.HOMEROOM_MANAGE_SCHOOL) &&
			get(can).hasAny(
				PERMISSIONS.STUDENT_READ_SCHOOL,
				PERMISSIONS.STUDENT_READ_ASSIGNED,
				PERMISSIONS.STUDENT_READ_OWN
			)
				? loadStudentProfileHistory(studentId, academicYearId, { requestFetch: fetch })
				: null
		),
		'โหลดประวัติการจัดห้องไม่สำเร็จ'
	);
	return {
		history,
		returnTo: context.returnTo,
		context,
		title: 'ข้อมูลนักเรียน',
		studentId,
		academicYearId,
		profileKey: `${studentId}:${academicYearId}`,
		student
	};
};
