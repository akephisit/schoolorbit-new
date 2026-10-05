import { BACKEND_URL } from '#lib/api/client.js';
import { getSchoolPublicIndexing, schoolSitemap } from '#lib/school-public/seo.js';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = ({ url }) =>
	new Response(schoolSitemap(getSchoolPublicIndexing(url, BACKEND_URL)), {
		headers: { 'content-type': 'application/xml; charset=utf-8', 'cache-control': 'no-store' }
	});
