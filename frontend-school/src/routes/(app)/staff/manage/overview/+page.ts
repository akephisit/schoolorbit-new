import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	getPersonnelOverview,
	getRankMilestoneOverview,
	type PersonnelStatusFilter
} from '#lib/api/personnel.js';
import type { PageLoad } from './$types';
export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'ภาพรวมงานบุคคล',
		icon: 'ChartNoAxesCombined',
		group: 'personnel',
		workspace: 'personnel',
		order: 5,
		user_type: 'staff',
		permission: PERMISSION_MODULES.STAFF_PROFILE
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	depends('school:personnel-overview');
	const requested = url.searchParams.get('status');
	const status: PersonnelStatusFilter =
		requested === 'all' ||
		requested === 'inactive' ||
		requested === 'suspended' ||
		requested === 'resigned' ||
		requested === 'retired'
			? requested
			: 'active';
	const overview = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			user?.user_type === 'staff' &&
			get(can).hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_OWN,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			)
				? getPersonnelOverview({ status }, { requestFetch: fetch })
				: null
		),
		'โหลดภาพรวมงานบุคคลไม่สำเร็จ'
	);
	const rankMilestones = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			user?.user_type === 'staff' &&
			get(can).hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_OWN,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			)
				? getRankMilestoneOverview({ status }, { requestFetch: fetch })
				: null
		),
		'โหลดกำหนดเวลาวิทยฐานะไม่สำเร็จ'
	);
	return { title: _meta.menu.title, status, overview, rankMilestones };
};
