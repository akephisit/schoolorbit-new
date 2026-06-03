// Menu API Client
// API for fetching user's dynamic menu based on permissions

import { apiClient } from '$lib/api/client';

export interface MenuItem {
	id: string;
	code: string;
	name: string;
	path: string;
	icon?: string;
}

export interface MenuGroup {
	code: string;
	name: string;
	icon?: string;
	items: MenuItem[];
}

export interface UserMenuResponse {
	success: boolean;
	data?: {
		groups: MenuGroup[];
	};
	groups: MenuGroup[];
}

/**
 * Fetch user's menu items based on their permissions
 * Menu is dynamically generated from database
 */
export async function getUserMenu(): Promise<UserMenuResponse> {
	const response = await apiClient.get<{ groups: MenuGroup[] }>('/api/menu/user');
	if (!response.success) throw new Error(response.error || 'Failed to fetch menu');

	return {
		...response,
		groups: response.data?.groups ?? []
	};
}
