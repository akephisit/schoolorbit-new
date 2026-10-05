import { ApiClientError, publicRequestFromOrigin } from '#lib/api/client.js';
import { getPublicFileDelivery } from '#lib/api/files.js';
import { getRequiredPublicSchoolInfo } from '#lib/api/school.js';
import type { RequestHandler } from './$types';

/** Resolve only this school's current public crest; callers cannot select a file. */
export const GET: RequestHandler = async ({ fetch, url }) => {
	const headers = { 'cache-control': 'no-store' };
	const controller = new AbortController();
	const timeout = setTimeout(() => controller.abort(), 3_000);
	const options = { ...publicRequestFromOrigin(fetch, url.origin), signal: controller.signal };
	try {
		const info = await getRequiredPublicSchoolInfo(options);
		if (!info.logoFileId) return new Response(null, { status: 404, headers });
		const delivery = await getPublicFileDelivery(info.logoFileId, options);
		const target = new URL(delivery.url);
		if (target.protocol !== 'https:') return new Response(null, { status: 503, headers });
		return new Response(null, { status: 307, headers: { ...headers, location: target.href } });
	} catch (error) {
		return new Response(null, {
			status: error instanceof ApiClientError && error.status === 404 ? 404 : 503,
			headers
		});
	} finally {
		clearTimeout(timeout);
	}
};
