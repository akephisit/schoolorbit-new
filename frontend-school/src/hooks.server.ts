import type { Handle } from '@sveltejs/kit/hooks';
import { getRoutePreviewMeta, injectRoutePreviewMeta } from '#lib/server/route-preview-meta.js';
import { BACKEND_URL } from '#lib/api/client.js';
import { getSchoolPublicIndexing } from '#lib/school-public/seo.js';

export const handle: Handle = async ({ event, resolve }) => {
	const routePreviewMeta = getRoutePreviewMeta(event.url.pathname);

	let injected = false;

	const response = await resolve(event, {
		transformPageChunk: ({ html }) => {
			if (!routePreviewMeta || injected) return html;

			const transformedHtml = injectRoutePreviewMeta(html, routePreviewMeta, event.url);
			injected = transformedHtml !== html;

			return transformedHtml;
		}
	});
	const indexable =
		event.url.pathname === '/' && getSchoolPublicIndexing(event.url, BACKEND_URL).indexable;
	if (!indexable && response.headers.get('content-type')?.includes('text/html')) {
		response.headers.set('X-Robots-Tag', 'noindex');
	}
	return response;
};
