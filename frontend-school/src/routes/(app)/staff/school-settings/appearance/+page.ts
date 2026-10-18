import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { getSchoolAppearance } from '#lib/appearance/api.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { can } from '#lib/stores/permissions.js';

export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.SETTINGS_READ_ALL }
};
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: 'รูปแบบและตัวอักษร',
		appearance: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const identityKey = appIdentityKey();
				return {
					identityKey,
					settings:
						user?.user_type === 'staff' && get(can).has(PERMISSIONS.SETTINGS_READ_ALL)
							? await getSchoolAppearance({ requestFetch: fetch })
							: null
				};
			}),
			'โหลดรูปแบบโรงเรียนไม่สำเร็จ'
		)
	};
};
