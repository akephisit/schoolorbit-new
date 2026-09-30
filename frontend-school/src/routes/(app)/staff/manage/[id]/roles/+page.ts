import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';
import type { PageLoad } from './$types';
import { userRoleAPI } from '$lib/api/roles';
import { requireApiData } from '$lib/api/client';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.ROLES }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	const userId = params.id;
	const userRead = waitForAuthenticatedUser();
	const roles = captureRouteLoad(
		userRead.then(async (user) =>
			user?.user_type === 'staff' && get(can).has(PERMISSIONS.ROLES_READ_ALL)
				? requireApiData(
						await userRoleAPI.getUserRoles(userId, { requestFetch: fetch }),
						'โหลดบทบาทที่ได้รับไม่สำเร็จ'
					)
				: []
		),
		'โหลดบทบาทที่ได้รับไม่สำเร็จ'
	);
	const permissions = captureRouteLoad(
		userRead.then(async (user) =>
			user?.user_type === 'staff' && get(can).has(PERMISSIONS.ROLES_READ_ALL)
				? requireApiData(
						await userRoleAPI.getUserPermissions(userId, { requestFetch: fetch }),
						'โหลดสิทธิ์ที่มีผลไม่สำเร็จ'
					)
				: []
		),
		'โหลดสิทธิ์ที่มีผลไม่สำเร็จ'
	);
	return { title: 'จัดการสิทธิ์ผู้ใช้งาน', userId, roles, permissions };
};
