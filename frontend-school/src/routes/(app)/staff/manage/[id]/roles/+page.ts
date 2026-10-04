import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
import { userRoleAPI } from '#lib/api/roles.js';
import { requireApiData } from '#lib/api/client.js';
import { getStaffProfile } from '#lib/api/staff.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.ROLES }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	const userId = params.id;
	const userRead = waitForAuthenticatedUser();
	const staff = captureRouteLoad(
		userRead.then(async (user) =>
			user?.user_type === 'staff' &&
			(get(can).hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			) ||
				(user.id === userId && get(can).has(PERMISSIONS.STAFF_PROFILE_READ_OWN)))
				? requireApiData(
						await getStaffProfile(userId, { requestFetch: fetch }),
						'โหลดชื่อบุคลากรไม่สำเร็จ'
					)
				: null
		),
		'โหลดชื่อบุคลากรไม่สำเร็จ'
	);
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
	return { title: 'บทบาทและสิทธิ์บุคลากร', userId, staff, roles, permissions };
};
