import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { requireApiData } from '$lib/api/client';
import { listOrganizationUnits, listOrganizationMembers } from '$lib/api/staff';
import type { PageLoad } from './$types';
/**
 * School Organization Management Page
 */

import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'โครงสร้างโรงเรียน',
		icon: 'Building2',
		group: 'personnel',
		workspace: 'personnel',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ROLES
	}
};

export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const units = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) =>
			user?.user_type === 'staff' && get(can).has(PERMISSIONS.ROLES_READ_ALL)
				? requireApiData(
						await listOrganizationUnits({ include_inactive: true }, { requestFetch: fetch }),
						'โหลดหน่วยงานไม่สำเร็จ'
					)
				: []
		),
		'โหลดหน่วยงานไม่สำเร็จ'
	);
	const members = captureRouteLoad(
		units.then(async (result) => {
			if (!result.ok) throw new Error(result.error);
			const unit =
				result.data.find((unit) => unit.code === 'SCHOOL' || unit.unit_type === 'school') ??
				result.data[0];
			return {
				unitId: unit?.id ?? '',
				members: unit
					? requireApiData(
							await listOrganizationMembers(unit.id, undefined, { requestFetch: fetch }),
							'โหลดสมาชิกไม่สำเร็จ'
						)
					: []
			};
		}),
		'โหลดสมาชิกไม่สำเร็จ'
	);
	return { title: _meta.menu.title, units, members };
};
