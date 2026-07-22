// Role & Permission API Client
import { apiClient } from './client';
import type { components } from './generated/school-api';
import type { ApiResponse } from './types';

type Schemas = components['schemas'];
export type Role = Schemas['Role'];
export type Permission = Schemas['Permission'];
export type UserRoleAssignment = Schemas['UserRoleAssignmentResponse'];
export type PermissionsByModule = Schemas['ApiResponse_HashMap_String_Vec_Permission']['data'];
type CreateRoleRequest = Schemas['CreateRoleRequest'];
type UpdateRoleRequest = Schemas['UpdateRoleRequest'];
type AssignRoleRequest = Schemas['AssignRoleRequest'];
type UuidIdData = Schemas['UuidIdData'];
type EmptyData = Schemas['EmptyData'];

// Role Management API
export const roleAPI = {
	// List all roles
	async listRoles(): Promise<ApiResponse<Role[]>> {
		return apiClient.get<Role[]>('/api/roles');
	},

	// Get single role
	async getRole(roleId: string): Promise<ApiResponse<Role>> {
		return apiClient.get<Role>(`/api/roles/${roleId}`);
	},

	// Create role
	async createRole(data: CreateRoleRequest): Promise<ApiResponse<UuidIdData>> {
		return apiClient.post<UuidIdData>('/api/roles', data);
	},

	// Update role
	async updateRole(roleId: string, data: UpdateRoleRequest): Promise<ApiResponse<EmptyData>> {
		return apiClient.put<EmptyData>(`/api/roles/${roleId}`, data);
	},

	// Delete role
	async deleteRole(roleId: string): Promise<ApiResponse<Record<string, never>>> {
		return apiClient.delete<Record<string, never>>(`/api/roles/${roleId}`);
	}
};

// Permission API
export const permissionAPI = {
	// List all permissions
	async listPermissions(): Promise<ApiResponse<Permission[]>> {
		return apiClient.get<Permission[]>('/api/permissions');
	},

	// List permissions grouped by module
	async listPermissionsByModule(): Promise<ApiResponse<PermissionsByModule>> {
		return apiClient.get<PermissionsByModule>('/api/permissions/modules');
	}
};

// User Role Assignment API
export const userRoleAPI = {
	// Get user's roles
	async getUserRoles(userId: string): Promise<ApiResponse<UserRoleAssignment[]>> {
		return apiClient.get<UserRoleAssignment[]>(`/api/users/${userId}/roles`);
	},

	// Assign role to user
	async assignRole(userId: string, data: AssignRoleRequest): Promise<ApiResponse<UuidIdData>> {
		return apiClient.post<UuidIdData>(`/api/users/${userId}/roles`, data);
	},

	// Remove role from user
	async removeRole(userId: string, roleId: string): Promise<ApiResponse<EmptyData>> {
		return apiClient.delete<EmptyData>(`/api/users/${userId}/roles/${roleId}`);
	},

	// Get user's effective permissions
	async getUserPermissions(userId: string): Promise<ApiResponse<string[]>> {
		return apiClient.get<string[]>(`/api/users/${userId}/permissions`);
	}
};
