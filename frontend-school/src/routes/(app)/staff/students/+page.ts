/**
 * Student Management Page (Staff)
 */

import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
import { listStudents } from '#lib/api/students.js';

export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'รายชื่อนักเรียน',
		icon: 'GraduationCap',
		group: 'academic_registry',
		workspace: 'academic',
		order: 30,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STUDENT
	}
};

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const academicYearId = url.searchParams.get('academicYearId') ?? '';
	const search = url.searchParams.get('search') ?? '';
	const requestedStatus = url.searchParams.get('status');
	const status =
		requestedStatus === 'all' || requestedStatus === 'inactive' ? requestedStatus : 'active';
	const requestedPage = Number(url.searchParams.get('page'));
	const page = Number.isSafeInteger(requestedPage) && requestedPage > 0 ? requestedPage : 1;
	const query = {
		academicYearId,
		search: search || undefined,
		status: status === 'all' ? undefined : status,
		page,
		pageSize: 20
	};
	const students = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			academicYearId &&
			user?.user_type === 'staff' &&
			get(can).hasAny(
				PERMISSIONS.STUDENT_READ_SCHOOL,
				PERMISSIONS.STUDENT_READ_ASSIGNED,
				PERMISSIONS.STUDENT_READ_OWN
			)
				? listStudents(query, { requestFetch: fetch })
				: null
		),
		'โหลดรายชื่อนักเรียนไม่สำเร็จ'
	);
	return {
		title: _meta.menu.title,
		academicYearId,
		search,
		status,
		page,
		query,
		listKey: JSON.stringify([academicYearId, search, status, page]),
		students
	};
};
