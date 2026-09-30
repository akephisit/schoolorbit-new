import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';
import type { PageLoad } from './$types';
import { getStudent } from '$lib/api/students';
export const _meta = {
	academicContext: 'year_required' as const,
	access: { user_type: 'staff' as const, permission: PERMISSION_MODULES.STUDENT }
};
export const load: PageLoad = ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	const studentId = params.id;
	const academicYearId = url.searchParams.get('academicYearId') ?? '';
	const student = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			academicYearId &&
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
		title: 'ข้อมูลนักเรียน',
		studentId,
		academicYearId,
		profileKey: `${studentId}:${academicYearId}`,
		student
	};
};
