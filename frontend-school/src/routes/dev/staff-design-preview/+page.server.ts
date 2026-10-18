import { appearanceFromQuery } from '#lib/design-preview/appearance.js';
import { dev } from '$app/env';
import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ url }) => {
	if (!dev) error(404, 'Not found');
	return {
		appearance: appearanceFromQuery(url.searchParams),
		mode: url.searchParams.get('mode') === 'dark' ? 'dark' : 'light'
	};
};
