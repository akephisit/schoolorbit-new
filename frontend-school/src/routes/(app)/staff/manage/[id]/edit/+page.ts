import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
import { getStaffProfile } from '#lib/api/staff.js';
import { requireApiData } from '#lib/api/client.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.STAFF_UPDATE_ALL }
};
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	const staffId = params.id;
	const userRead = waitForAuthenticatedUser();
	const staff = captureRouteLoad(
		userRead.then(async (user) =>
			user?.user_type === 'staff' &&
			(get(can).hasAny(
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
				PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE,
				PERMISSIONS.STAFF_PROFILE_READ_SCHOOL
			) ||
				(user.id === staffId && get(can).has(PERMISSIONS.STAFF_PROFILE_READ_OWN)))
				? requireApiData(
						await getStaffProfile(staffId, { requestFetch: fetch }),
						'โหลดข้อมูลบุคลากรไม่สำเร็จ'
					)
				: null
		),
		'โหลดข้อมูลบุคลากรไม่สำเร็จ'
	);
	return { title: 'แก้ไขข้อมูลบุคลากร', staffId, staff };
};
