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
	organization_units: string[];
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

export interface OrganizationUnitResponse {
	id: string;
	code: string;
	name: string;
	position_code?: string;
	position_title?: string;
	is_primary?: boolean;
	category?: string;
	unit_type?: string;
	subject_group_id?: string;
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
	organization_units: OrganizationUnitResponse[];
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

export interface PublicStaffOrganizationUnitResponse {
	id: string;
	code: string;
	name: string;
	position_code: string;
	position_title?: string;
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
	organization_units: PublicStaffOrganizationUnitResponse[];
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
	organization_assignments?: Array<{
		organization_unit_id: string;
		position_code: string;
		position_title?: string;
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
	organization_assignments?: Array<{
		organization_unit_id: string;
		position_code: string;
		position_title?: string;
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

export interface OrganizationUnit {
	id: string;
	code: string;
	name: string;
	name_en?: string;
	description?: string;
	parent_unit_id?: string;
	category?: string;
	unit_type?: string;
	subject_group_id?: string;
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
	organization_unit_id?: string;
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
// Organization Unit APIs
// ===================================================================

export async function listOrganizationUnits(): Promise<ApiResponse<OrganizationUnit[]>> {
	return apiClient.get<OrganizationUnit[]>('/api/organization/units');
}

// Auth-only version (no roles.read.all required) — for non-admin pages
export async function listOrganizationUnitsLookup(options?: {
	member_only?: boolean;
}): Promise<ApiResponse<OrganizationUnit[]>> {
	const params = new URLSearchParams();
	if (options?.member_only) params.set('member_only', 'true');
	const qs = params.toString() ? `?${params}` : '';
	return apiClient.get<OrganizationUnit[]>(`/api/lookup/organization-units${qs}`);
}

// Get single organization unit (auth only, no roles.read.all required)
export async function getOrganizationUnitLookup(
	id: string
): Promise<ApiResponse<OrganizationUnit>> {
	return apiClient.get<OrganizationUnit>(`/api/lookup/organization-units/${id}`);
}

export async function getOrganizationUnit(
	unitId: string
): Promise<ApiResponse<OrganizationUnit>> {
	return apiClient.get<OrganizationUnit>(`/api/organization/units/${unitId}`);
}

export interface CreateOrganizationUnitRequest {
	code: string;
	name: string;
	name_en?: string;
	description?: string;
	parent_unit_id?: string;
	category?: string;
	unit_type?: string;
	subject_group_id?: string;
	phone?: string;
	email?: string;
	location?: string;
	is_active?: boolean;
	display_order?: number;
}

export interface UpdateOrganizationUnitRequest {
	name?: string;
	name_en?: string;
	description?: string;
	parent_unit_id?: string;
	category?: string;
	unit_type?: string;
	subject_group_id?: string;
	phone?: string;
	email?: string;
	location?: string;
	is_active?: boolean;
	display_order?: number;
}

export async function createOrganizationUnit(
	data: CreateOrganizationUnitRequest
): Promise<ApiResponse<{ id: string }>> {
	return apiClient.post<{ id: string }>('/api/organization/units', data);
}

export async function updateOrganizationUnit(
	unitId: string,
	data: UpdateOrganizationUnitRequest
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.put<Record<string, never>>(`/api/organization/units/${unitId}`, data);
}

export async function deleteOrganizationUnit(
	unitId: string
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.delete<Record<string, never>>(`/api/organization/units/${unitId}`);
}

// ===================================================================
// Organization Permission APIs
// ===================================================================

export interface OrganizationPermissionGrant {
	permission_id: string;
	position_code?: string | null;
}

export async function getOrganizationPermissions(
	unitId: string
): Promise<OrganizationPermissionGrant[]> {
	const response = await apiClient.get<OrganizationPermissionGrant[]>(
		`/api/organization/units/${unitId}/permissions`
	);
	return requireApiData(response, 'Failed to fetch organization permissions');
}

export async function updateOrganizationPermissions(
	unitId: string,
	grants: OrganizationPermissionGrant[]
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.put<Record<string, never>>(`/api/organization/units/${unitId}/permissions`, {
		grants
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
	organizationUnitId: string
): Promise<ApiResponse<DelegatablePermission[]>> {
	return apiClient.get<DelegatablePermission[]>(
		`/api/organization/units/${organizationUnitId}/delegatable-permissions`
	);
}

export async function listDelegations(
	organizationUnitId: string
): Promise<ApiResponse<DelegationItem[]>> {
	return apiClient.get<DelegationItem[]>(
		`/api/organization/units/${organizationUnitId}/delegations`
	);
}

export async function createDelegation(
	organizationUnitId: string,
	body: CreateDelegationBody
): Promise<ApiResponse<{ delegation_id: string }>> {
	return apiClient.post<{ delegation_id: string }>(
		`/api/organization/units/${organizationUnitId}/delegations`,
		body
	);
}

export async function revokeDelegation(
	delegationId: string
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.delete<Record<string, never>>(`/api/organization/delegations/${delegationId}`);
}

// ===================================================================
// Organization Member Management APIs
// ===================================================================

export interface OrganizationMemberItem {
	user_id: string;
	organization_unit_id: string;
	organization_unit_name: string;
	name: string;
	title: string;
	position_code: string;
	position_title?: string | null;
	is_primary: boolean;
	responsibilities: string | null;
	started_at: string;
}

export interface AddMemberBody {
	user_id: string;
	position_code: string;
	position_title?: string;
	is_primary?: boolean;
	responsibilities?: string;
}

export interface UpdateMemberBody {
	position_code: string;
	position_title?: string;
	is_primary?: boolean;
	responsibilities?: string;
	new_organization_unit_id?: string;
}

export async function listOrganizationMembers(
	unitId: string,
	options?: { include_children?: boolean }
): Promise<ApiResponse<OrganizationMemberItem[]>> {
	const params = options?.include_children ? '?include_children=true' : '';
	return apiClient.get<OrganizationMemberItem[]>(
		`/api/organization/units/${unitId}/members${params}`
	);
}

export async function addOrganizationMember(
	unitId: string,
	body: AddMemberBody
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.post<Record<string, never>>(`/api/organization/units/${unitId}/members`, body);
}

export async function updateOrganizationMember(
	unitId: string,
	userId: string,
	body: UpdateMemberBody
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.put<Record<string, never>>(
		`/api/organization/units/${unitId}/members/${userId}`,
		body
	);
}

export async function removeOrganizationMember(
	unitId: string,
	userId: string
): Promise<ApiResponse<Record<string, never>>> {
	return apiClient.delete<Record<string, never>>(
		`/api/organization/units/${unitId}/members/${userId}`
	);
}
