import type { PageLoad } from './$types';
import { consentApi } from '#lib/api/consent.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
export const _meta = { access: { authenticated: true } } as const;
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const requestKey = url.pathname + url.search;
	const consent = captureRouteLoad(
		waitForAuthenticatedUser().then(async (user) => {
			const ownerKey = `${appIdentityKey()}|${requestKey}`;
			return {
				ownerKey,
				status: user ? await consentApi.getMyConsentStatus({ requestFetch: fetch }) : null
			};
		}),
		'เกิดข้อผิดพลาดในการโหลดข้อมูล'
	);
	return { requestKey, consent };
};
