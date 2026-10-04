/**
 * Roles Management Page
 */

import { roleAPI } from '#lib/api/roles.js';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { requireApiData } from '#lib/api/client.js';
import type { PageLoad } from './$types';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'จัดการบทบาท',
		icon: 'Shield',
		group: 'settings',
		workspace: 'settings',
		order: 1000,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ROLES
	}
};

export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const roles = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) =>
			user?.user_type === 'staff' && get(can).has(PERMISSIONS.ROLES_READ_ALL)
				? requireApiData(
						await roleAPI.listRoles({ include_inactive: true }, { requestFetch: fetch }),
						'โหลดบทบาทไม่สำเร็จ'
					)
				: []
		),
		'โหลดบทบาทไม่สำเร็จ'
	);
	return { title: _meta.menu.title, roles };
};
