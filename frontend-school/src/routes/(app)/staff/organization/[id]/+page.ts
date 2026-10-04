import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { requireApiData } from '#lib/api/client.js';
import {
	getOrganizationUnit,
	listOrganizationUnits,
	listOrganizationMembers
} from '#lib/api/staff.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.ROLES }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	const unitId = params.id;
	const allowed = waitForAuthenticatedUser().then(
		(user) => user?.user_type === 'staff' && get(can).has(PERMISSIONS.ROLES_READ_ALL)
	);
	const unit = captureRouteLoad(
		allowed.then(async (read) =>
			read
				? requireApiData(
						await getOrganizationUnit(unitId, { requestFetch: fetch }),
						'โหลดหน่วยงานไม่สำเร็จ'
					)
				: null
		),
		'โหลดหน่วยงานไม่สำเร็จ'
	);
	const structure = captureRouteLoad(
		allowed.then(async (read) =>
			read
				? requireApiData(
						await listOrganizationUnits(undefined, { requestFetch: fetch }),
						'โหลดโครงสร้างไม่สำเร็จ'
					)
				: []
		),
		'โหลดโครงสร้างไม่สำเร็จ'
	);
	const members = captureRouteLoad(
		allowed.then(async (read) =>
			read
				? requireApiData(
						await listOrganizationMembers(
							unitId,
							{ include_children: true },
							{ requestFetch: fetch }
						),
						'โหลดสมาชิกไม่สำเร็จ'
					)
				: []
		),
		'โหลดสมาชิกไม่สำเร็จ'
	);
	return { title: 'รายละเอียดหน่วยงาน', unitId, unit, structure, members };
};
