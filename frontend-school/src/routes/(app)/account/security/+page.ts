import type { PageLoad } from './$types';
import { authAPI } from '#lib/api/auth.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
export const _meta = { access: { authenticated: true } } as const;
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const requestKey = url.pathname + url.search;
	const sessions = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => {
			const ownerKey = `${appIdentityKey()}|${requestKey}`;
			return {
				ownerKey,
				sessions: user ? await authAPI.listSessions({ requestFetch: fetch }) : []
			};
		}),
		'ไม่สามารถโหลดรายการอุปกรณ์ได้'
	);
	return { requestKey, sessions };
};
