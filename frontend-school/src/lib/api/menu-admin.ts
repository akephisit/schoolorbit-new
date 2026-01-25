/**
 * Menu Admin API Client
 * Module-based permission control for managing menu structure
 */

import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

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
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/groups`, {
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to fetch menu groups' }));
		throw new Error(error.error || 'Failed to fetch menu groups');
	}

	const result: MenuGroupListResponse = await response.json();
	return result.data;
}

export async function createMenuGroup(data: CreateMenuGroupRequest): Promise<MenuGroup> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/groups`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		credentials: 'include',
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to create menu group' }));
		throw new Error(error.error || 'Failed to create menu group');
	}

	const result: MenuGroupResponse = await response.json();
	if (!result.data) {
		throw new Error('Failed to create menu group');
	}
	return result.data;
}

export async function updateMenuGroup(
	id: string,
	data: UpdateMenuGroupRequest
): Promise<MenuGroup> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/groups/${id}`, {
		method: 'PUT',
		headers: {
			'Content-Type': 'application/json'
		},
		credentials: 'include',
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to update menu group' }));
		throw new Error(error.error || 'Failed to update menu group');
	}

	const result: MenuGroupResponse = await response.json();
	if (!result.data) {
		throw new Error('Failed to update menu group');
	}
	return result.data;
}

export async function deleteMenuGroup(id: string): Promise<void> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/groups/${id}`, {
		method: 'DELETE',
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to delete menu group' }));
		throw new Error(error.error || 'Failed to delete menu group');
	}
}

export async function reorderMenuGroups(groups: ReorderItem[]): Promise<void> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/groups/reorder`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		credentials: 'include',
		body: JSON.stringify({ groups })
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to reorder menu groups' }));
		throw new Error(error.error || 'Failed to reorder menu groups');
	}
}

// ==================== Menu Items ====================

export async function listMenuItems(groupId?: string): Promise<MenuItem[]> {
	const url = groupId
		? `${BACKEND_URL}/api/admin/menu/items?group_id=${groupId}`
		: `${BACKEND_URL}/api/admin/menu/items`;
	const response = await fetch(url, {
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to fetch menu items' }));
		throw new Error(error.error || 'Failed to fetch menu items');
	}

	const result: MenuItemListResponse = await response.json();
	return result.data;
}

export async function createMenuItem(data: CreateMenuItemRequest): Promise<MenuItem> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/items`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		credentials: 'include',
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to create menu item' }));
		throw new Error(error.error || 'Failed to create menu item');
	}

	const result: MenuItemResponse = await response.json();
	if (!result.data) {
		throw new Error('Failed to create menu item');
	}
	return result.data;
}

export async function updateMenuItem(id: string, data: UpdateMenuItemRequest): Promise<MenuItem> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/items/${id}`, {
		method: 'PUT',
		headers: {
			'Content-Type': 'application/json'
		},
		credentials: 'include',
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to update menu item' }));
		throw new Error(error.error || 'Failed to update menu item');
	}

	const result: MenuItemResponse = await response.json();
	if (!result.data) {
		throw new Error('Failed to update menu item');
	}
	return result.data;
}

export async function deleteMenuItem(id: string): Promise<void> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/items/${id}`, {
		method: 'DELETE',
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to delete menu item' }));
		throw new Error(error.error || 'Failed to delete menu item');
	}
}

export async function reorderMenuItems(items: ReorderItem[]): Promise<void> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/items/reorder`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		credentials: 'include',
		body: JSON.stringify({ items })
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to reorder menu items' }));
		throw new Error(error.error || 'Failed to reorder menu items');
	}
}

export async function moveItemToGroup(itemId: string, groupId: string): Promise<MenuItem> {
	const response = await fetch(`${BACKEND_URL}/api/admin/menu/items/${itemId}/group`, {
		method: 'PUT',
		headers: {
			'Content-Type': 'application/json'
		},
		credentials: 'include',
		body: JSON.stringify({ group_id: groupId })
	});

	if (!response.ok) {
		const error = await response.json().catch(() => ({ error: 'Failed to move menu item' }));
		throw new Error(error.error || 'Failed to move menu item');
	}

	const result = await response.json();
	if (!result.data) {
		throw new Error('Failed to move menu item');
	}
	return result.data;
}
