import { PUBLIC_API_URL } from '$app/env/public';
import { getPublicSite, publicSitemap } from '#lib/public-site.js';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = ({ url }) =>
	new Response(publicSitemap(getPublicSite(new URL('/', url), PUBLIC_API_URL)), {
		headers: { 'content-type': 'application/xml; charset=utf-8', 'cache-control': 'no-store' }
	});
