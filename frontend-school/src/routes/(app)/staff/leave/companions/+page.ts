import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { listLeaveCompanionInvitations } from '#lib/api/staff-leave.js';
import { captureLeaveRead } from '#lib/staff-leave/route-data.js';
import { companionMaxOffset } from '#lib/staff-leave/filters.js';

export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STAFF_LEAVE_READ_ASSIGNED }
};

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity', 'school:leave-companions');
	const rawOffset = Number(url.searchParams.get('offset') ?? '0');
	const offset =
		Number.isSafeInteger(rawOffset) && rawOffset >= 0 && rawOffset <= companionMaxOffset
			? rawOffset
			: 0;
	return {
		title: 'คำเชิญร่วมเดินทาง',
		offset,
		invitations: captureLeaveRead(
			() => get(can).has(PERMISSIONS.STAFF_LEAVE_READ_ASSIGNED),
			() => listLeaveCompanionInvitations({ requestFetch: fetch, query: { offset } }),
			[]
		)
	};
};
