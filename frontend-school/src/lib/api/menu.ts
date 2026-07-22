// Menu API Client
// API for fetching user's dynamic menu based on permissions

import { apiClient } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type MenuItem = Schemas['MenuItemResponse'];
export type MenuGroup = Schemas['MenuGroupResponse'];
export type UserMenuData = Schemas['UserMenuData'];

export interface UserMenuResponse {
	success: boolean;
	data?: UserMenuData;
	groups: MenuGroup[];
}

/**
 * Fetch user's menu items based on their permissions
 * Menu is dynamically generated from database
 */
export async function getUserMenu(): Promise<UserMenuResponse> {
	const response = await apiClient.get<UserMenuData>('/api/menu/user');
	if (!response.success) throw new Error(response.error || 'Failed to fetch menu');

	return {
		...response,
		groups: response.data?.groups ?? []
	};
}
