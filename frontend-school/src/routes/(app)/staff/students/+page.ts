/**
 * Student Management Page (Staff)
 */

import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
import { registrationStatus, studentAccountStatus } from '#lib/forms/student-register.js';
import { listStudents } from '#lib/api/students.js';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'ทะเบียนนักเรียน',
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
	const search = url.searchParams.get('search') ?? '';
	const selectedRegistrationStatus = registrationStatus(url.searchParams.get('registrationStatus'));
	const accountStatus = studentAccountStatus(url.searchParams.get('accountStatus'));
	const requestedPage = Number(url.searchParams.get('page'));
	const page = Number.isSafeInteger(requestedPage) && requestedPage > 0 ? requestedPage : 1;
	const query = {
		registrationStatus: selectedRegistrationStatus,
		accountStatus,
		search: search || undefined,
		page,
		pageSize: 20
	};
	const students = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
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
		registrationStatus: selectedRegistrationStatus ?? 'all',
		accountStatus: accountStatus ?? 'all',
		search,
		page,
		query,
		listKey: JSON.stringify([search, selectedRegistrationStatus, accountStatus, page]),
		students
	};
};
