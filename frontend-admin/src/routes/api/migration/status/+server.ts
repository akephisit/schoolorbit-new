import type { RequestHandler } from './$types';
// The browser now calls the authenticated operator API directly.
export const GET: RequestHandler = async () =>
	Response.json(
		{
			success: false,
			error: 'Use the authenticated operator migration API'
		},
		{ status: 410 }
	);
