import { ApiClientError, publicRequestFromOrigin } from '#lib/api/client.js';
import { getPublicSchoolFavicon } from '#lib/api/school.js';
import type { RequestHandler } from './$types';

/** A stable per-host favicon resolves only the current school's public crest. */
export const GET: RequestHandler = async ({ fetch, url }) => {
	const options = {
		...publicRequestFromOrigin(fetch, url.origin),
		signal: AbortSignal.timeout(15_000)
	};
	const fallback = () =>
		new Response(null, {
			status: 307,
			headers: { location: '/schoolorbit.png', 'cache-control': 'public, max-age=300' }
		});
	try {
		const png = await getPublicSchoolFavicon(options);
		return new Response(png, {
			headers: {
				'content-type': 'image/png',
				'cache-control': 'public, max-age=300',
				'x-content-type-options': 'nosniff'
			}
		});
	} catch (error) {
		if (error instanceof ApiClientError && error.status === 404) return fallback();
		return new Response(null, {
			status: 503,
			headers: { 'cache-control': 'no-store', 'retry-after': '60' }
		});
	}
};
