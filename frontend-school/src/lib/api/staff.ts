// API Client for Staff Management
// ติดต่อกับ backend-school service

import { apiClient, requireApiData, type ApiResponse } from '$lib/api/client';

export interface StaffListItem {
	id: string;
	username: string;
	title: string;
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
	user_type: string; // Changed from category to user_type
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
	category?: string;
	org_type?: string;
	responsibilities?: string;
}

/** วิชาที่ครูสอน — ดึงจาก classroom_courses (Course Planning) */
export interface TeachingCourseItem {
	classroom_course_id: string;
	subject_code: string;
	subject_name: string;
	hours_per_semester?: number;
	classroom_name: string;
	classroom_code: string;
	academic_year: number;
	academic_year_label: string;
	term: string;
	role: 'primary' | 'secondary';
}

/** ห้องที่ครูเป็นครูที่ปรึกษา — ดึงจาก classroom_advisors */
export interface AdvisorClassroomItem {
	classroom_id: string;
	classroom_name: string;
	classroom_code: string;
	academic_year: number;
	academic_year_label: string;
	role: 'primary' | 'secondary';
}

export interface StaffInfoResponse {
	education_level?: string;
	major?: string;
	university?: string;
}

export interface StaffProfileResponse {
	id: string;
	username: string;
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
	profile_image_url?: string | null;
	staff_info?: StaffInfoResponse;
	roles: RoleResponse[];
	primary_role?: RoleResponse;
	departments: DepartmentResponse[];
	teaching_courses: TeachingCourseItem[];
	advisor_classrooms: AdvisorClassroomItem[];
	permissions: string[];
}

export interface PublicStaffRoleResponse {
	id: string;
	code: string;
	name: string;
	level?: number;
}

export interface PublicStaffDepartmentResponse {
	id: string;
	code: string;
	name: string;
	position: string;
}

export interface PublicStaffProfileResponse {
	id: string;
	username: string;
	email?: string;
	title?: string;
	first_name: string;
	last_name: string;
	nickname?: string;
	phone?: string;
	hired_date?: string;
	user_type: string;
	status: string;
	profile_image_url?: string | null;
	roles: PublicStaffRoleResponse[];
	departments: PublicStaffDepartmentResponse[];
}

export interface CreateStaffRequest {
	username?: string;
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
	};
	profile_image_url?: string;
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
	profile_image_url?: string;
	staff_info?: {
		education_level?: string;
		major?: string;
		university?: string;
		teaching_license_number?: string;
		teaching_license_expiry?: string;
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
	user_type: string; // Changed from category to user_type
	level: number;
	permissions: string[];
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
	category?: string; // administrative, academic
	org_type?: string; // group, unit
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

// ===================================================================
// Staff APIs
// ===================================================================

interface StaffListData {
	items: StaffListItem[];
	total: number;
	page: number;
	page_size: number;
	total_pages: number;
}

export async function listStaff(filter?: StaffFilter): Promise<StaffListResponse> {
	const params = new URLSearchParams();
	if (filter?.status) params.append('status', filter.status);
	if (filter?.search) params.append('search', filter.search);
	if (filter?.page) params.append('page', filter.page.toString());
	if (filter?.page_size) params.append('page_size', filter.page_size.toString());

	const response = await apiClient.get<StaffListData>(`/api/staff?${params.toString()}`);
	const data = requireApiData(response, 'Failed to fetch staff list');

	return {
		success: true,
		data: data.items,
		total: data.total,
		page: data.page,
		page_size: data.page_size,
		total_pages: data.total_pages
	};
}

export async function getStaffProfile(staffId: string): Promise<ApiResponse<StaffProfileResponse>> {
	return apiClient.get<StaffProfileResponse>(`/api/staff/${staffId}`);
}

export async function getPublicStaffProfile(
	staffId: string
): Promise<ApiResponse<PublicStaffProfileResponse>> {
	return apiClient.get<PublicStaffProfileResponse>(`/api/staff/${staffId}/public-profile`);
}

export async function createStaff(data: CreateStaffRequest): Promise<ApiResponse<{ id: string }>> {
	return apiClient.post<{ id: string }>('/api/staff', data);
}

export async function updateStaff(
	staffId: string,
	data: UpdateStaffRequest
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.put<Record<string, never>>(`/api/staff/${staffId}`, data);
}

export async function deleteStaff(staffId: string): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.delete<Record<string, never>>(`/api/staff/${staffId}`);
}

// ===================================================================
// Role APIs
// ===================================================================

export async function listRoles(): Promise<ApiResponse<Role[]>> {
	return apiClient.get<Role[]>('/api/roles');
}

export async function getRole(roleId: string): Promise<ApiResponse<Role>> {
	return apiClient.get<Role>(`/api/roles/${roleId}`);
}

// ===================================================================
// Department APIs
// ===================================================================

export async function listDepartments(): Promise<ApiResponse<Department[]>> {
	return apiClient.get<Department[]>('/api/departments');
}

// Auth-only version (no roles.read.all required) — for non-admin pages
export async function listDepartmentsLookup(options?: {
	member_only?: boolean;
}): Promise<ApiResponse<Department[]>> {
	const params = new URLSearchParams();
	if (options?.member_only) params.set('member_only', 'true');
	const qs = params.toString() ? `?${params}` : '';
	return apiClient.get<Department[]>(`/api/lookup/departments${qs}`);
}

// Get single department (auth only, no roles.read.all required)
export async function getDepartmentLookup(id: string): Promise<ApiResponse<Department>> {
	return apiClient.get<Department>(`/api/lookup/departments/${id}`);
}

export async function getDepartment(deptId: string): Promise<ApiResponse<Department>> {
	return apiClient.get<Department>(`/api/departments/${deptId}`);
}

export interface CreateDepartmentRequest {
	code: string;
	name: string;
	name_en?: string;
	description?: string;
	parent_department_id?: string;
	category?: string;
	org_type?: string;
	phone?: string;
	email?: string;
	location?: string;
	is_active?: boolean;
	display_order?: number;
}

export interface UpdateDepartmentRequest {
	name?: string;
	name_en?: string;
	description?: string;
	parent_department_id?: string;
	category?: string;
	org_type?: string;
	phone?: string;
	email?: string;
	location?: string;
	is_active?: boolean;
	display_order?: number;
}

export async function createDepartment(
	data: CreateDepartmentRequest
): Promise<ApiResponse<{ id: string }>> {
	return apiClient.post<{ id: string }>('/api/departments', data);
}

export async function updateDepartment(
	deptId: string,
	data: UpdateDepartmentRequest
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.put<Record<string, never>>(`/api/departments/${deptId}`, data);
}

export async function deleteDepartment(
	deptId: string
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.delete<Record<string, never>>(`/api/departments/${deptId}`);
}

// ===================================================================
// Department Permissions APIs
// ===================================================================

export async function getDepartmentPermissions(deptId: string): Promise<string[]> {
	const response = await apiClient.get<string[]>(`/api/departments/${deptId}/permissions`);
	return requireApiData(response, 'Failed to fetch department permissions');
}

export async function updateDepartmentPermissions(
	deptId: string,
	permission_ids: string[]
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.put<Record<string, never>>(`/api/departments/${deptId}/permissions`, {
		permission_ids
	});
}

// ===================================================================
// Delegation APIs
// ===================================================================

export interface DelegationItem {
	id: string;
	from_user_id: string;
	from_user_name: string;
	to_user_id: string;
	to_user_name: string;
	permission_id: string;
	permission_code: string;
	permission_name: string;
	reason: string | null;
	started_at: string;
	expires_at: string | null;
}

export interface CreateDelegationBody {
	to_user_id: string;
	permission_id: string;
	reason?: string;
	expires_at?: string;
}

export interface DelegatablePermission {
	id: string;
	code: string;
	name: string;
}

export async function listDelegatablePermissions(
	departmentId: string
): Promise<ApiResponse<DelegatablePermission[]>> {
	return apiClient.get<DelegatablePermission[]>(
		`/api/departments/${departmentId}/delegatable-permissions`
	);
}

export async function listDelegations(
	departmentId: string
): Promise<ApiResponse<DelegationItem[]>> {
	return apiClient.get<DelegationItem[]>(`/api/departments/${departmentId}/delegations`);
}

export async function createDelegation(
	departmentId: string,
	body: CreateDelegationBody
): Promise<ApiResponse<{ delegation_id: string }>> {
	return apiClient.post<{ delegation_id: string }>(
		`/api/departments/${departmentId}/delegations`,
		body
	);
}

export async function revokeDelegation(
	delegationId: string
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.delete<Record<string, never>>(`/api/delegations/${delegationId}`);
}

// ===================================================================
// Department Member Management APIs
// ===================================================================

export interface DeptMemberItem {
	user_id: string;
	department_id: string;
	department_name: string;
	name: string;
	title: string;
	position: string;
	is_primary: boolean;
	responsibilities: string | null;
	started_at: string;
}

export interface AddMemberBody {
	user_id: string;
	position: string;
	is_primary?: boolean;
	responsibilities?: string;
}

export interface UpdateMemberBody {
	position: string;
	is_primary?: boolean;
	responsibilities?: string;
	new_department_id?: string;
}

export async function listDeptMembers(
	deptId: string,
	options?: { include_children?: boolean }
): Promise<ApiResponse<DeptMemberItem[]>> {
	const params = options?.include_children ? '?include_children=true' : '';
	return apiClient.get<DeptMemberItem[]>(`/api/departments/${deptId}/members${params}`);
}

export async function addDeptMember(
	deptId: string,
	body: AddMemberBody
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.post<Record<string, never>>(`/api/departments/${deptId}/members`, body);
}

export async function updateDeptMember(
	deptId: string,
	userId: string,
	body: UpdateMemberBody
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.put<Record<string, never>>(
		`/api/departments/${deptId}/members/${userId}`,
		body
	);
}

export async function removeDeptMember(
	deptId: string,
	userId: string
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.delete<Record<string, never>>(`/api/departments/${deptId}/members/${userId}`);
}
