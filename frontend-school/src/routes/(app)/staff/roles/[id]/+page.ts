import { get } from 'svelte/store';
import { roleAPI, permissionAPI } from '#lib/api/roles.js';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { requireApiData } from '#lib/api/client.js';
import type { PageLoad } from './$types';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.ROLES }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	const isNew = params.id === 'new';
	const access = waitForAuthenticatedUser().then((user) =>
		Boolean(
			user?.user_type === 'staff' &&
			get(can).has(isNew ? PERMISSIONS.ROLES_CREATE_ALL : PERMISSIONS.ROLES_READ_ALL)
		)
	);
	const role = captureRouteLoad(
		access.then(async (readable) =>
			readable && !isNew
				? requireApiData(
						await roleAPI.getRole(params.id, { requestFetch: fetch }),
						'โหลดบทบาทไม่สำเร็จ'
					)
				: null
		),
		'โหลดบทบาทไม่สำเร็จ'
	);
	const catalog = captureRouteLoad(
		access.then(async (readable) =>
			readable && get(can).has(PERMISSIONS.SETTINGS_READ_ALL)
				? requireApiData(
						await permissionAPI.listPermissionsByModule({ requestFetch: fetch }),
						'โหลดรายการสิทธิ์ไม่สำเร็จ'
					)
				: null
		),
		'โหลดรายการสิทธิ์ไม่สำเร็จ'
	);
	return { roleId: params.id, role, catalog };
};
