/**
 * Menu Admin API Client
 * Module-based permission control for managing menu structure
 */

import { apiClient, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type MenuGroup = Schemas['MenuGroup'];
export type MenuItem = Schemas['MenuItem'];
export type MenuWorkspace = Schemas['MenuWorkspace'];
export type CreateMenuWorkspaceRequest = Schemas['CreateMenuWorkspaceRequest'];
export type UpdateMenuWorkspaceRequest = Schemas['UpdateMenuWorkspaceRequest'];
export type CreateMenuGroupRequest = Schemas['CreateMenuGroupRequest'];
export type UpdateMenuGroupRequest = Schemas['UpdateMenuGroupRequest'];
export type CreateMenuItemRequest = Schemas['CreateMenuItemRequest'];
export type UpdateMenuItemRequest = Schemas['UpdateMenuItemRequest'];
export type ReorderItem = Schemas['ReorderItem'];

// ==================== Management workspaces ====================

export async function listMenuWorkspaces(): Promise<MenuWorkspace[]> {
	const response = await apiClient.get<MenuWorkspace[]>('/api/admin/menu/workspaces');
	return requireApiData(response, 'Failed to fetch menu workspaces');
}

export async function createMenuWorkspace(
	data: CreateMenuWorkspaceRequest
): Promise<MenuWorkspace> {
	const response = await apiClient.post<MenuWorkspace>('/api/admin/menu/workspaces', data);
	return requireApiData(response, 'Failed to create menu workspace');
}

export async function updateMenuWorkspace(
	id: string,
	data: UpdateMenuWorkspaceRequest
): Promise<MenuWorkspace> {
	const response = await apiClient.put<MenuWorkspace>(`/api/admin/menu/workspaces/${id}`, data);
	return requireApiData(response, 'Failed to update menu workspace');
}

export async function deleteMenuWorkspace(id: string): Promise<Schemas['MovedCountData']> {
	const response = await apiClient.delete<Schemas['MovedCountData']>(
		`/api/admin/menu/workspaces/${id}`
	);
	return requireApiData(response, 'Failed to delete menu workspace');
}

export async function reorderMenuWorkspaces(workspaces: ReorderItem[]): Promise<void> {
	const response = await apiClient.post<Schemas['EmptyData']>(
		'/api/admin/menu/workspaces/reorder',
		{ workspaces }
	);
	if (!response.success) throw new Error(response.error || 'Failed to reorder menu workspaces');
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

export async function deleteMenuGroup(id: string): Promise<Schemas['MovedCountData']> {
	const response = await apiClient.delete<Schemas['MovedCountData']>(
		`/api/admin/menu/groups/${id}`
	);
	return requireApiData(response, 'Failed to delete menu group');
}

export async function reorderMenuGroups(groups: ReorderItem[]): Promise<void> {
	const response = await apiClient.post<Schemas['EmptyData']>('/api/admin/menu/groups/reorder', {
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
	const response = await apiClient.delete<Schemas['EmptyData']>(`/api/admin/menu/items/${id}`);
	if (!response.success) throw new Error(response.error || 'Failed to delete menu item');
}

export async function reorderMenuItems(items: ReorderItem[]): Promise<void> {
	const response = await apiClient.post<Schemas['EmptyData']>('/api/admin/menu/items/reorder', {
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
