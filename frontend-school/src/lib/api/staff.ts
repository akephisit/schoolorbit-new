// API Client for Staff Management
// ติดต่อกับ backend-school service

import { PUBLIC_BACKEND_URL } from '$env/static/public';

const API_BASE_URL = PUBLIC_BACKEND_URL || 'http://localhost:8081';

export interface StaffListItem {
	id: string;
	first_name: string;
	last_name: string;
	roles: string[];
	departments: string[];
	status: string;
}

export interface StaffListResponse {
	success: boolean;
	data: StaffListItem[];
	total: number;
	page: number;
	page_size: number;
	total_pages: number;
}

export interface RoleResponse {
	id: string;
	code: string;
	name: string;
	name_en?: string;
	category: string;
	level: number;
	is_primary?: boolean;
}

export interface DepartmentResponse {
	id: string;
	code: string;
	name: string;
	position?: string;
	is_primary_department?: boolean;
	is_primary?: boolean;
	responsibilities?: string;
}

export interface TeachingAssignmentResponse {
	id: string;
	subject: string;
	grade_level?: string;
	class_code?: string;
	class_name?: string;
	is_homeroom_teacher: boolean;
	hours_per_week?: number;
	academic_year: string;
	semester: string;
}

export interface StaffInfoResponse {
	education_level?: string;
	major?: string;
	university?: string;
}

export interface StaffProfileResponse {
	id: string;
	national_id?: string;
	email?: string;
	title?: string;
	first_name: string;
	last_name: string;
	nickname?: string;
	phone?: string;
	emergency_contact?: string;
	line_id?: string;
	date_of_birth?: string;
	gender?: string;
	address?: string;
	hired_date?: string;
	user_type: string;
	status: string;
	staff_info?: StaffInfoResponse;
	roles: RoleResponse[];
	primary_role?: RoleResponse;
	departments: DepartmentResponse[];
	teaching_assignments: TeachingAssignmentResponse[];
	permissions: string[];
}

export interface CreateStaffRequest {
	national_id?: string;
	email?: string;
	password: string;
	title?: string;
	first_name: string;
	last_name: string;
	nickname?: string;
	phone?: string;
	emergency_contact?: string;
	line_id?: string;
	date_of_birth?: string;
	gender?: string;
	address?: string;
	hired_date?: string;
	staff_info?: {
		education_level?: string;
		major?: string;
		university?: string;
		teaching_license_number?: string;
		teaching_license_expiry?: string;
		work_days?: string[];
	};
	role_ids: string[];
	primary_role_id?: string;
	department_assignments?: Array<{
		department_id: string;
		position: string;
		is_primary?: boolean;
		responsibilities?: string;
	}>;
}

export interface UpdateStaffRequest {
	title?: string;
	first_name?: string;
	last_name?: string;
	nickname?: string;
	email?: string;
	phone?: string;
	emergency_contact?: string;
	line_id?: string;
	date_of_birth?: string;
	gender?: string;
	address?: string;
	hired_date?: string;
	status?: string;
	staff_info?: {
		education_level?: string;
		major?: string;
		university?: string;
		teaching_license_number?: string;
		teaching_license_expiry?: string;
		work_days?: string[];
	};
	role_ids?: string[];
	primary_role_id?: string;
	department_assignments?: Array<{
		department_id: string;
		position: string;
		is_primary?: boolean;
		responsibilities?: string;
	}>;
}

export interface Role {
	id: string;
	code: string;
	name: string;
	name_en?: string;
	description?: string;
	category: string;
	level: number;
	permissions: Record<string, unknown>;
	is_active: boolean;
	created_at: string;
	updated_at: string;
}

export interface Department {
	id: string;
	code: string;
	name: string;
	name_en?: string;
	description?: string;
	parent_department_id?: string;
	phone?: string;
	email?: string;
	location?: string;
	is_active: boolean;
	display_order: number;
	created_at: string;
	updated_at: string;
}

interface StaffFilter {
	user_type?: string;
	role_id?: string;
	department_id?: string;
	status?: string;
	search?: string;
	page?: number;
	page_size?: number;
}

export interface ApiResponse<T> {
	success: boolean;
	data?: T;
	error?: string;
	message?: string;
}

// ===================================================================
// Staff APIs
// ===================================================================

export async function listStaff(filter?: StaffFilter): Promise<StaffListResponse> {
	const params = new URLSearchParams();
	if (filter?.status) params.append('status', filter.status);
	if (filter?.search) params.append('search', filter.search);
	if (filter?.page) params.append('page', filter.page.toString());
	if (filter?.page_size) params.append('page_size', filter.page_size.toString());

	const response = await fetch(`${API_BASE_URL}/api/staff?${params.toString()}`, {
		method: 'GET',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		}
	});

	if (!response.ok) {
		throw new Error('Failed to fetch staff list');
	}

	return response.json();
}

export async function getStaffProfile(staffId: string): Promise<ApiResponse<StaffProfileResponse>> {
	const response = await fetch(`${API_BASE_URL}/api/staff/${staffId}`, {
		method: 'GET',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		}
	});

	if (!response.ok) {
		throw new Error('Failed to fetch staff profile');
	}

	return response.json();
}

export async function createStaff(data: CreateStaffRequest): Promise<ApiResponse<{ id: string }>> {
	const response = await fetch(`${API_BASE_URL}/api/staff`, {
		method: 'POST',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to create staff');
	}

	return response.json();
}

export async function updateStaff(
	staffId: string,
	data: UpdateStaffRequest
): Promise<ApiResponse<void>> {
	const response = await fetch(`${API_BASE_URL}/api/staff/${staffId}`, {
		method: 'PUT',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to update staff');
	}

	return response.json();
}

export async function deleteStaff(staffId: string): Promise<ApiResponse<void>> {
	const response = await fetch(`${API_BASE_URL}/api/staff/${staffId}`, {
		method: 'DELETE',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		}
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to delete staff');
	}

	return response.json();
}

// ===================================================================
// Role APIs
// ===================================================================

export async function listRoles(): Promise<ApiResponse<Role[]>> {
	const response = await fetch(`${API_BASE_URL}/api/roles`, {
		method: 'GET',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		}
	});

	if (!response.ok) {
		throw new Error('Failed to fetch roles');
	}

	return response.json();
}

export async function getRole(roleId: string): Promise<ApiResponse<Role>> {
	const response = await fetch(`${API_BASE_URL}/api/roles/${roleId}`, {
		method: 'GET',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		}
	});

	if (!response.ok) {
		throw new Error('Failed to fetch role');
	}

	return response.json();
}

// ===================================================================
// Department APIs
// ===================================================================

export async function listDepartments(): Promise<ApiResponse<Department[]>> {
	const response = await fetch(`${API_BASE_URL}/api/departments`, {
		method: 'GET',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		}
	});

	if (!response.ok) {
		throw new Error('Failed to fetch departments');
	}

	return response.json();
}

export async function getDepartment(deptId: string): Promise<ApiResponse<Department>> {
	const response = await fetch(`${API_BASE_URL}/api/departments/${deptId}`, {
		method: 'GET',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		}
	});

	if (!response.ok) {
		throw new Error('Failed to fetch department');
	}

	return response.json();
}
