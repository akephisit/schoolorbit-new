import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';

export const ssr = false;

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACHIEVEMENT
	}
};

import type { PageLoad } from './$types';
import { getAchievements } from '$lib/api/achievement';
import { requireApiData } from '$lib/api/client';
import { appIdentityKey, waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { can } from '$lib/stores/permissions';
import { get } from 'svelte/store';
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: 'ผลงานที่บันทึกเอง',
		achievements: captureRouteLoad(
			waitForAuthenticatedUser().then(async (user) => {
				const identityKey = appIdentityKey(),
					allowed =
						user?.user_type === 'staff' &&
						(get(can).has(PERMISSIONS.ACHIEVEMENT_READ_OWN) ||
							get(can).has(PERMISSIONS.ACHIEVEMENT_READ_ALL));
				return {
					identityKey,
					records: allowed
						? requireApiData(
								await getAchievements({ user_id: user.id }, { requestFetch: fetch }),
								'โหลดผลงานไม่สำเร็จ'
							)
						: null
				};
			}),
			'โหลดผลงานไม่สำเร็จ'
		)
	};
};
