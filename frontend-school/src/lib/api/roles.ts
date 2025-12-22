// Role & Permission API Client
import { apiClient } from './client';
import type { ApiResponse } from './types';

// Types
export interface Role {
	id: string;
	code: string;
	name: string;
	name_en?: string;
	description?: string;
	category?: string;
	level: number;
	permissions: string[];
	is_active: boolean;
	created_at: string;
	updated_at?: string;
}

export interface Permission {
	id: string;
	code: string;
	name: string;
	module: string;
	action: string;
	description?: string;
	created_at: string;
}

export interface UserRole {
	id: string;
	user_id: string;
	role_id: string;
	role?: Role;
	is_primary: boolean;
	started_at: string;
	ended_at?: string;
	created_at: string;
}

export interface PermissionsByModule {
	[module: string]: Permission[];
}

// Role Management API
export const roleAPI = {
	// List all roles
	async listRoles(): Promise<ApiResponse<Role[]>> {
		return apiClient.get('/api/roles');
	},

	// Get single role
	async getRole(roleId: string): Promise<ApiResponse<Role>> {
		return apiClient.get(`/api/roles/${roleId}`);
	},

	// Create role
	async createRole(data: {
		code: string;
		name: string;
		name_en?: string;
		description?: string;
		category?: string;
		level?: number;
		permissions?: string[];
	}): Promise<ApiResponse<{ id: string }>> {
		return apiClient.post('/api/roles', data);
	},

	// Update role
	async updateRole(
		roleId: string,
		data: {
			name?: string;
			name_en?: string;
			description?: string;
			category?: string;
			level?: number;
			permissions?: string[];
			is_active?: boolean;
		}
	): Promise<ApiResponse<void>> {
		return apiClient.put(`/api/roles/${roleId}`, data);
	},

	// Delete role
	async deleteRole(roleId: string): Promise<ApiResponse<void>> {
		return apiClient.delete(`/api/roles/${roleId}`);
	}
};

// Permission API
export const permissionAPI = {
	// List all permissions
	async listPermissions(): Promise<ApiResponse<Permission[]>> {
		return apiClient.get('/api/permissions');
	},

	// List permissions grouped by module
	async listPermissionsByModule(): Promise<ApiResponse<PermissionsByModule>> {
		return apiClient.get('/api/permissions/modules');
	}
};

// User Role Assignment API
export const userRoleAPI = {
	// Get user's roles
	async getUserRoles(userId: string): Promise<ApiResponse<UserRole[]>> {
		return apiClient.get(`/api/users/${userId}/roles`);
	},

	// Assign role to user
	async assignRole(
		userId: string,
		data: {
			role_id: string;
			is_primary?: boolean;
			started_at?: string;
		}
	): Promise<ApiResponse<{ id: string }>> {
		return apiClient.post(`/api/users/${userId}/roles`, data);
	},

	// Remove role from user
	async removeRole(userId: string, roleId: string): Promise<ApiResponse<void>> {
		return apiClient.delete(`/api/users/${userId}/roles/${roleId}`);
	},

	// Get user's effective permissions
	async getUserPermissions(userId: string): Promise<ApiResponse<string[]>> {
		return apiClient.get(`/api/users/${userId}/permissions`);
	}
};
