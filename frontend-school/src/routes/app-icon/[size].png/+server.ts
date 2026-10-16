import { schoolIconResponse } from '#lib/server/school-icon.js';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = ({ fetch, url, params }) => {
	if (params.size !== '192' && params.size !== '512')
		return new Response(null, { status: 404, headers: { 'cache-control': 'no-store' } });
	return schoolIconResponse(fetch, url.origin, params.size);
};
