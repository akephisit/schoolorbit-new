import { apiClient } from '#lib/api/client.js';
import { authStore } from '#lib/stores/auth.svelte.js';
import type { PageLoad } from './$types';
export const load: PageLoad = async ({ parent }) => {
	await parent();
	const [requests, schools, managed] = await Promise.all([
		apiClient.serviceRequests(),
		apiClient.customerSchools(),
		authStore.user?.role === 'operator' ? apiClient.listSchools(1, 100) : Promise.resolve(null)
	]);
	return { requests, schools, managed };
};
