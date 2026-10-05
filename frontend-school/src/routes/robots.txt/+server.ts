import { BACKEND_URL } from '#lib/api/client.js';
import { getSchoolPublicIndexing, schoolRobots } from '#lib/school-public/seo.js';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = ({ url }) =>
	new Response(schoolRobots(getSchoolPublicIndexing(url, BACKEND_URL)), {
		headers: { 'content-type': 'text/plain; charset=utf-8', 'cache-control': 'no-store' }
	});
