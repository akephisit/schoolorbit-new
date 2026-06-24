import type { Handle } from '@sveltejs/kit';
import { getRoutePreviewMeta, injectRoutePreviewMeta } from '$lib/server/route-preview-meta';

export const handle: Handle = async ({ event, resolve }) => {
	const routePreviewMeta = getRoutePreviewMeta(event.url.pathname);

	if (!routePreviewMeta) {
		return resolve(event);
	}

	let injected = false;

	return resolve(event, {
		transformPageChunk: ({ html }) => {
			if (injected) return html;

			const transformedHtml = injectRoutePreviewMeta(html, routePreviewMeta, event.url);
			injected = transformedHtml !== html;

			return transformedHtml;
		}
	});
};
