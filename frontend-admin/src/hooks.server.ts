import type { Handle } from '@sveltejs/kit/hooks';
import { PUBLIC_API_URL } from '$app/env/public';
import {
	deploymentIsInMaintenance,
	maintenanceResponse
} from '#lib/server/deployment-maintenance.js';

export const handle: Handle = async ({ event, resolve }) => {
	const api = PUBLIC_API_URL;
	if (api) {
		const host = new URL(api).hostname;
		if (host.startsWith('admin-api.')) {
			const base = host.slice('admin-api.'.length);
			if (event.url.hostname === `admin.${base}`) {
				return new Response(null, { status: 410, headers: { 'Cache-Control': 'no-store' } });
			}
		}
	}
	if (!api || event.url.pathname.startsWith('/_app/')) return resolve(event);
	if (await deploymentIsInMaintenance(api)) return maintenanceResponse(api);
	return resolve(event);
};
