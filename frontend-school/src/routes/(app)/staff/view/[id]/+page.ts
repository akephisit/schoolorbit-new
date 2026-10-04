import { get } from 'svelte/store';
import { getPublicStaffProfile } from '#lib/api/staff.js';
import { getAchievements } from '#lib/api/achievement.js';
import { requireApiData } from '#lib/api/client.js';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import type { PageLoad } from './$types';

export const _meta = { academicContext: 'none' as const, access: { user_type: 'staff' } };
export const load: PageLoad = ({ fetch, params, depends }) => {
	depends('school:app-identity');
	const identity = waitForAuthenticatedUser();
	const profile = captureRouteLoad(
		identity.then(async (user) =>
			user?.user_type === 'staff'
				? requireApiData(
						await getPublicStaffProfile(params.id, { requestFetch: fetch }),
						'โหลดบุคลากรไม่สำเร็จ'
					)
				: null
		),
		'โหลดบุคลากรไม่สำเร็จ'
	);
	const achievements = captureRouteLoad(
		identity.then(async (user) => {
			const readable = Boolean(
				user?.user_type === 'staff' &&
				(get(can).has(PERMISSIONS.ACHIEVEMENT_READ_ALL) ||
					(user.id === params.id && get(can).has(PERMISSIONS.ACHIEVEMENT_READ_OWN)))
			);
			return {
				readable,
				items: readable
					? requireApiData(
							await getAchievements({ user_id: params.id }, { requestFetch: fetch }),
							'โหลดผลงานไม่สำเร็จ'
						)
					: []
			};
		}),
		'โหลดผลงานไม่สำเร็จ'
	);
	return { staffId: params.id, profile, achievements };
};
