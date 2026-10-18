import type { RequestHandler } from './$types';
import { publicRequestFromOrigin } from '#lib/api/client.js';
import { getPublicSchoolAppearance } from '#lib/appearance/api.js';
import { appearanceStylesheet } from '#lib/appearance/stylesheet.js';

export const GET: RequestHandler = ({ fetch, url, request }) =>
	appearanceStylesheet(
		(signal) =>
			getPublicSchoolAppearance({
				...publicRequestFromOrigin(
					(input, init) => fetch(input, { ...init, redirect: 'error', credentials: 'omit' }),
					url.origin
				),
				signal
			}),
		request.headers.get('if-none-match')
	);
