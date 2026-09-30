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
	const query = { search: search || undefined, page, page_size: 20 };
	const staff = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			user?.user_type === 'staff' &&
			get(can).hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_OWN,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			)
				? listStaff(query, { requestFetch: fetch })
				: null
		),
		'โหลดรายชื่อบุคลากรไม่สำเร็จ'
	);
	return {
		title: _meta.menu.title,
		search,
		page,
		query,
		listKey: JSON.stringify([search, page]),
		staff
	};
};
