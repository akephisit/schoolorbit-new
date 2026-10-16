import { PUBLIC_API_URL } from '$app/env/public';
import { getPublicSite, publicRobots } from '#lib/public-site.js';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = ({ url }) =>
	new Response(publicRobots(getPublicSite(new URL('/', url), PUBLIC_API_URL)), {
		headers: { 'content-type': 'text/plain; charset=utf-8', 'cache-control': 'no-store' }
	});
