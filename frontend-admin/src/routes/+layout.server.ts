import { PUBLIC_API_URL } from '$app/env/public';
import { getPublicSite } from '#lib/public-site.js';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = ({ url }) => ({
	publicSite: getPublicSite(url, PUBLIC_API_URL)
});
