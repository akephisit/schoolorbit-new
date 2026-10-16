import { apiClient } from '#lib/api/client.js';
import type { PageLoad } from './$types';
export const load: PageLoad = async ({ parent }) => {
	await parent();
	return { schools: await apiClient.customerSchools() };
};
