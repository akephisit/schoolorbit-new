/**
 * Menu Admin API Client
 * Module-based permission control for managing menu structure
 */

import { apiClient, requireApiData } from '$lib/api/client';

export interface MenuGroup {
	id: string;
	code: string;
	name: string;
	name_en: string | null;
	icon: string | null;
	display_order: number;
	is_active: boolean;
}

export interface MenuItem {
	id: string;
	code: string;
	name: string;
	name_en: string | null;
	path: string;
	icon: string | null;
	required_permission: string | null; // Module name
	user_type: string; // 'staff' | 'student' | 'parent'
	group_id: string;
	parent_id: string | null;
	display_order: number;
	is_active: boolean;
}

export interface MenuGroupListResponse {
	success: boolean;
	data: MenuGroup[];
}

export interface MenuItemListResponse {
	success: boolean;
	data: MenuItem[];
}

export interface MenuGroupResponse {
	success: boolean;
	data: MenuGroup | null;
	message: string | null;
}

export interface MenuItemResponse {
	success: boolean;
	data: MenuItem | null;
	message: string | null;
}

export interface CreateMenuGroupRequest {
	code: string;
	name: string;
	name_en?: string;
	description?: string;
	icon?: string;
	display_order?: number;
}

export interface UpdateMenuGroupRequest {
	name?: string;
	name_en?: string;
	description?: string;
	icon?: string;
	display_order?: number;
	is_active?: boolean;
}

export interface CreateMenuItemRequest {
	code: string;
	name: string;
	name_en?: string;
	description?: string;
	path: string;
	icon?: string;
	group_id: string;
	parent_id?: string;
	required_permission?: string; // Module name
	display_order?: number;
}

export interface UpdateMenuItemRequest {
	name?: string;
	name_en?: string;
	description?: string;
	path?: string;
	icon?: string;
	group_id?: string;
	parent_id?: string;
	required_permission?: string;
	display_order?: number;
	is_active?: boolean;
}

export interface ReorderItem {
	id: string;
	display_order: number;
	group_id?: string;
}

// ==================== Menu Groups ====================

export async function listMenuGroups(): Promise<MenuGroup[]> {
	const response = await apiClient.get<MenuGroup[]>('/api/admin/menu/groups');
	return requireApiData(response, 'Failed to fetch menu groups');
}

export async function createMenuGroup(data: CreateMenuGroupRequest): Promise<MenuGroup> {
	const response = await apiClient.post<MenuGroup>('/api/admin/menu/groups', data);
	return requireApiData(response, 'Failed to create menu group');
}

export async function updateMenuGroup(
	id: string,
	data: UpdateMenuGroupRequest
): Promise<MenuGroup> {
	const response = await apiClient.put<MenuGroup>(`/api/admin/menu/groups/${id}`, data);
	return requireApiData(response, 'Failed to update menu group');
}

export async function deleteMenuGroup(id: string): Promise<void> {
	const response = await apiClient.delete<Record<string, never>>(`/api/admin/menu/groups/${id}`);
	if (!response.success) throw new Error(response.error || 'Failed to delete menu group');
}

export async function reorderMenuGroups(groups: ReorderItem[]): Promise<void> {
	const response = await apiClient.post<Record<string, never>>('/api/admin/menu/groups/reorder', {
		groups
	});
	if (!response.success) throw new Error(response.error || 'Failed to reorder menu groups');
}

// ==================== Menu Items ====================

export async function listMenuItems(groupId?: string): Promise<MenuItem[]> {
	const endpoint = groupId ? `/api/admin/menu/items?group_id=${groupId}` : '/api/admin/menu/items';
	const response = await apiClient.get<MenuItem[]>(endpoint);
	return requireApiData(response, 'Failed to fetch menu items');
}

export async function createMenuItem(data: CreateMenuItemRequest): Promise<MenuItem> {
	const response = await apiClient.post<MenuItem>('/api/admin/menu/items', data);
	return requireApiData(response, 'Failed to create menu item');
}

export async function updateMenuItem(id: string, data: UpdateMenuItemRequest): Promise<MenuItem> {
	const response = await apiClient.put<MenuItem>(`/api/admin/menu/items/${id}`, data);
	return requireApiData(response, 'Failed to update menu item');
}

export async function deleteMenuItem(id: string): Promise<void> {
	const response = await apiClient.delete<Record<string, never>>(`/api/admin/menu/items/${id}`);
	if (!response.success) throw new Error(response.error || 'Failed to delete menu item');
}

export async function reorderMenuItems(items: ReorderItem[]): Promise<void> {
	const response = await apiClient.post<Record<string, never>>('/api/admin/menu/items/reorder', {
		items
	});
	if (!response.success) throw new Error(response.error || 'Failed to reorder menu items');
}

export async function moveItemToGroup(itemId: string, groupId: string): Promise<MenuItem> {
	const response = await apiClient.put<MenuItem>(`/api/admin/menu/items/${itemId}/group`, {
		group_id: groupId
	});
	return requireApiData(response, 'Failed to move menu item');
}
