import type { RequestHandler } from './$types';
import { publicRequestFromOrigin } from '#lib/api/client.js';
import { getPublicSchoolAppearance } from '#lib/appearance/api.js';
import { appearanceStylesheet } from '#lib/appearance/stylesheet.js';

export const GET: RequestHandler = ({ fetch, url, request }) =>
	appearanceStylesheet(
		(signal) =>
			getPublicSchoolAppearance({
				...publicRequestFromOrigin(async (input, init) => {
					// Workers support manual redirects; "error" fails before the request is sent.
					const response = await fetch(input, {
						...init,
						redirect: 'manual',
						credentials: 'omit'
					});
					if (response.status >= 300 && response.status < 400) {
						await response.body?.cancel();
						throw new Error('Appearance endpoint redirected');
					}
					return response;
				}, url.origin),
				signal
			}),
		request.headers.get('if-none-match')
	);
