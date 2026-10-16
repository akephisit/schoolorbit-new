import { ApiClientError, publicRequestFromOrigin } from '#lib/api/client.js';
import { getPublicSchoolFavicon, type PublicSchoolIconSize } from '#lib/api/school.js';

/** One current-school resolver for tab, installation and notification icons. */
export async function schoolIconResponse(
	requestFetch: typeof globalThis.fetch,
	origin: string,
	size: PublicSchoolIconSize = '256'
): Promise<Response> {
	const options = {
		...publicRequestFromOrigin(requestFetch, origin),
		signal: AbortSignal.timeout(15_000)
	};
	try {
		const png = await getPublicSchoolFavicon(options, size);
		return new Response(png, {
			headers: {
				'content-type': 'image/png',
				'cache-control': 'public, max-age=300',
				'x-content-type-options': 'nosniff'
			}
		});
	} catch (error) {
		if (error instanceof ApiClientError && error.status === 404) {
			return new Response(null, {
				status: 307,
				headers: {
					location: size === '256' ? '/schoolorbit.png' : `/schoolorbit-${size}.png`,
					'cache-control': 'public, max-age=300'
				}
			});
		}
		return new Response(null, {
			status: 503,
			headers: {
				'cache-control': 'no-store',
				'retry-after': '60'
			}
		});
	}
}
