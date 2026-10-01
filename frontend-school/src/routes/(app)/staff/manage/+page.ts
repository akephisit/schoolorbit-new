/**
 * Staff Management Page
 */

import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';
import type { PageLoad } from './$types';
import { listStaff } from '$lib/api/staff';
import { ACADEMIC_RANK_LABELS, EDUCATION_LEVEL_LABELS } from '$lib/forms/staff-personnel';
import { STAFF_STATUS_OPTIONS } from '$lib/forms/staff-status';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'บุคลากร',
		icon: 'Users',
		group: 'personnel',
		workspace: 'personnel',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STAFF_PROFILE
	}
};

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const search = url.searchParams.get('search') ?? '';
	const requestedPage = Number(url.searchParams.get('page'));
	const page = Number.isSafeInteger(requestedPage) && requestedPage > 0 ? requestedPage : 1;
	const requestedStatus = url.searchParams.get('status') ?? 'active';
	const status =
		requestedStatus === 'all' ||
		STAFF_STATUS_OPTIONS.some((option) => option.value === requestedStatus)
			? requestedStatus
			: 'active';
	const uuid = /^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i;
	const requestedRole = url.searchParams.get('role_id') ?? '';
	const requestedOrganization = url.searchParams.get('organization_unit_id') ?? '';
	const roleId = uuid.test(requestedRole) ? requestedRole : '';
	const organizationId = uuid.test(requestedOrganization) ? requestedOrganization : '';
	const jobPositionId = url.searchParams.get('job_position_id') ?? '';
	const academicRank = url.searchParams.get('academic_rank') ?? '';
	const educationLevel = url.searchParams.get('education_level') ?? '';
	const subjectGroupId = url.searchParams.get('subject_group_id') ?? '';
	const hr = {
		job_position_id: jobPositionId,
		academic_rank: academicRank,
		education_level: educationLevel,
		subject_group_id: subjectGroupId
	};
	const malformed =
		(jobPositionId && jobPositionId !== 'unspecified' && !uuid.test(jobPositionId)) ||
		(subjectGroupId && subjectGroupId !== 'unassigned' && !uuid.test(subjectGroupId)) ||
		(academicRank &&
			academicRank !== 'unspecified' &&
			!Object.hasOwn(ACADEMIC_RANK_LABELS, academicRank)) ||
		(educationLevel &&
			educationLevel !== 'unspecified' &&
			!Object.hasOwn(EDUCATION_LEVEL_LABELS, educationLevel));
	const query = {
		search: search || undefined,
		status,
		role_id: roleId || undefined,
		organization_unit_id: organizationId || undefined,
		page,
		page_size: 20,
		job_position_id: jobPositionId || undefined,
		academic_rank: academicRank || undefined,
		education_level: educationLevel || undefined,
		subject_group_id: subjectGroupId || undefined
	};
	const staff = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			user?.user_type === 'staff' &&
			get(can).hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_OWN,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			)
				? malformed
					? Promise.reject(new Error('ตัวกรองบุคลากรไม่ถูกต้อง กรุณาล้างตัวกรอง'))
					: listStaff(query, { requestFetch: fetch })
				: null
		),
		'โหลดรายชื่อบุคลากรไม่สำเร็จ'
	);
	return {
		title: _meta.menu.title,
		search,
		page,
		status,
		roleId,
		organizationId,
		query,
		hr,
		listKey: JSON.stringify([search, page, status, roleId, organizationId, hr]),
		staff
	};
};
