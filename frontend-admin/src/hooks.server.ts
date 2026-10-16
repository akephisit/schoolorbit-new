import { canonicalRedirect, getPublicSite } from '#lib/public-site.js';
import type { Handle } from '@sveltejs/kit/hooks';
import { PUBLIC_API_URL } from '$app/env/public';
import {
	deploymentIsInMaintenance,
	maintenanceResponse
} from '#lib/server/deployment-maintenance.js';

export const handle: Handle = async ({ event, resolve }) => {
	const api = PUBLIC_API_URL;
	const target = canonicalRedirect(event.url, api);
	if (target)
		return new Response(null, {
			status: 301,
			headers: { Location: target.href, 'Cache-Control': 'no-store' }
		});
	if (api) {
		const host = new URL(api).hostname;
		if (host.startsWith('admin-api.')) {
			const base = host.slice('admin-api.'.length);
			if (event.url.hostname === `admin.${base}`) {
				return new Response(null, { status: 410, headers: { 'Cache-Control': 'no-store' } });
			}
		}
	}
	if (api && !event.url.pathname.startsWith('/_app/') && (await deploymentIsInMaintenance(api)))
		return maintenanceResponse(api);
	const response = await resolve(event);
	if (
		response.headers.get('content-type')?.includes('text/html') &&
		(response.status >= 400 || !getPublicSite(event.url, api).indexable)
	)
		response.headers.set('X-Robots-Tag', 'noindex');
	return response;
};
