import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { PERMISSIONS } from '$lib/permissions/registry';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { listStaffReferenceItems, type ReferenceListQuery } from '$lib/api/personnel';
import type { PageLoad } from './$types';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STAFF_UPDATE_ALL }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	depends('school:staff-reference-items');
	const kind = url.searchParams.get('kind');
	const status = url.searchParams.get('status');
	const requestedPage = Number(url.searchParams.get('page'));
	const query: ReferenceListQuery = {
		kind: kind === 'major' || kind === 'university' ? kind : 'job_position',
		search: url.searchParams.get('search') || undefined,
		status: status === 'inactive' || status === 'all' ? status : 'active',
		page: Number.isSafeInteger(requestedPage) && requestedPage > 0 ? requestedPage : 1,
		pageSize: 25
	};
	const catalog = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			user?.user_type === 'staff' && get(can).has(PERMISSIONS.STAFF_UPDATE_ALL)
				? listStaffReferenceItems(query, { requestFetch: fetch })
				: null
		),
		'โหลดรายการกลางไม่สำเร็จ'
	);
	return { title: 'รายการกลางงานบุคคล', query, contextKey: JSON.stringify(query), catalog };
};
