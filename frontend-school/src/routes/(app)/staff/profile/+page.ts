import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { PERMISSIONS } from '$lib/permissions/registry';
import { listStaffCareerHistory } from '$lib/api/staff-career';
import { authAPI } from '$lib/api/auth';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageLoad } from './$types';
export const _meta = { academicContext: 'none' as const, access: { user_type: 'staff' } };
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const userRead = waitForAuthenticatedUser();
	return {
		careerHistory: captureRouteLoad(
			userRead.then((user) =>
				user?.user_type === 'staff' &&
				get(can).hasAny(
					PERMISSIONS.STAFF_PROFILE_READ_OWN,
					PERMISSIONS.STAFF_PROFILE_READ_SCHOOL,
					PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_UNIT,
					PERMISSIONS.STAFF_PROFILE_READ_ORGANIZATION_TREE
				)
					? listStaffCareerHistory(user.id, {}, { requestFetch: fetch })
					: null
			),
			'โหลดประวัติไม่สำเร็จ'
		),
		profile: captureRouteLoad(
			userRead.then((user) =>
				user?.user_type === 'staff' ? authAPI.getFullProfile({ requestFetch: fetch }) : null
			),
			'โหลดโปรไฟล์ไม่สำเร็จ'
		)
	};
};
