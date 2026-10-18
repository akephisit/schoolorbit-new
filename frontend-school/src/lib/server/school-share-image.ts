import { publicRequestFromOrigin } from '#lib/api/client.js';
import { getPublicSchoolShareImage } from '#lib/api/school.js';

/** Anonymous and host-specific; generation never blocks the homepage's HTML. */
export async function schoolShareImageResponse(
	requestFetch: typeof globalThis.fetch,
	origin: string
): Promise<Response> {
	try {
		const png = await getPublicSchoolShareImage({
			...publicRequestFromOrigin(requestFetch, origin),
			signal: AbortSignal.timeout(15_000)
		});
		return new Response(png, {
			headers: {
				'content-type': 'image/png',
				'cache-control': 'public, max-age=300',
				'x-content-type-options': 'nosniff'
			}
		});
	} catch {
		return new Response(null, {
			status: 503,
			headers: { 'cache-control': 'no-store', 'retry-after': '60' }
		});
	}
}
