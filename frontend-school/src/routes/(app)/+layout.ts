import { getUserMenu } from '#lib/api/menu.js';
import { getMyWorkCounts } from '#lib/api/work.js';
import { waitForAuthenticatedUser, appIdentityKey } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import type { LayoutLoad } from './$types';

export const ssr = false;
export const load: LayoutLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const identity = waitForAuthenticatedUser().then((user) => ({
		user,
		identityKey: appIdentityKey()
	}));
	// Independent shared regions; children inherit these same in-flight reads.
	const userMenu = captureRouteLoad(
		identity.then(async ({ user, identityKey }) => ({
			identityKey,
			groups: user ? (await getUserMenu({ requestFetch: fetch })).groups : []
		})),
		'โหลดเมนูบริการไม่สำเร็จ'
	);
	const workCounts = captureRouteLoad(
		identity.then(async ({ user, identityKey }) => ({
			identityKey,
			counts: user
				? await getMyWorkCounts({ requestFetch: fetch })
				: { open: 0, dueSoon: 0, overdue: 0, submitted: 0, closed: 0, total: 0 }
		})),
		'โหลดจำนวนงานไม่สำเร็จ'
	);
	return { userMenu, workCounts };
};
