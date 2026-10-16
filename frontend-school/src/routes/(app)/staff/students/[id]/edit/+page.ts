import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
import {
	studentRegisterContext,
	withStudentRegisterContext
} from '#lib/navigation/student-register.js';
import { getStudent } from '#lib/api/students.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff' as const, permission: PERMISSION_MODULES.STUDENT }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	const studentId = params.id;
	const context = studentRegisterContext(url);
	const academicYearId = null;
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
	return {
		title: 'แก้ไขทะเบียนนักเรียน',
		context,
		detailHref: withStudentRegisterContext(
			`/staff/students/${encodeURIComponent(studentId)}`,
			context
		),
		studentId,
		academicYearId,
		profileKey: studentId,
		student
	};
};
