// API Client for Staff Management
// ติดต่อกับ backend-school service

import {
	apiClient,
	requireApiData,
	type ApiResponse,
	type ApiRequestOptions
} from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type StaffListItem = Schemas['StaffListItem'];

export interface StaffListResponse {
	success: boolean;
	data: StaffListItem[];
	total: number;
	page: number;
	page_size: number;
	total_pages: number;
}

export type StaffDashboardOverview = Schemas['StaffDashboardOverview'];
export type RoleResponse = Schemas['RoleResponse'];
export type OrganizationUnitResponse = Schemas['OrganizationUnitResponse'];

/** กลุ่มการเรียนและรายวิชาที่ครูสอนใน Academic Delivery */
export type TeachingAssignmentItem = Schemas['TeachingAssignmentItem'];

/** ห้องประจำชั้นที่ครูเป็นที่ปรึกษาตามปีการศึกษา */
export type AdvisorHomeroomItem = Schemas['AdvisorHomeroomItem'];
export type StaffInfoResponse = Schemas['StaffInfoResponse'];
export type StaffProfileResponse = Schemas['StaffProfileResponse'];
export type PublicStaffRoleResponse = Schemas['PublicStaffRole'];
export type PublicStaffOrganizationUnitResponse = Schemas['PublicStaffOrganizationUnit'];
export type PublicStaffProfileResponse = Schemas['PublicStaffProfile'];

export type CreateStaffRequest = Schemas['CreateStaffRequest'];
export type UpdateStaffRequest = Schemas['UpdateStaffRequest'];

export type Role = Schemas['Role'];
export type OrganizationUnit = Schemas['OrganizationUnit'];
export type OrganizationUnitLookupItem = Schemas['OrganizationUnitLookupItem'];
type ManagedListOptions = { include_inactive?: boolean };

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

type StaffListData = Schemas['StaffListData'];

export async function getStaffDashboard(
	academicYearId: string,
	options: ApiRequestOptions = {}
): Promise<ApiResponse<StaffDashboardOverview>> {
	const selectedYearId = academicYearId.trim();
	if (!selectedYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	const query = new URLSearchParams({ academicYearId: selectedYearId });
	return apiClient.get<StaffDashboardOverview>(`/api/staff/dashboard?${query}`, options);
}

export async function listStaff(
	filter?: StaffFilter,
	options: ApiRequestOptions = {}
): Promise<StaffListResponse> {
	const params = new URLSearchParams();
	if (filter?.status) params.append('status', filter.status);
	if (filter?.role_id) params.append('role_id', filter.role_id);
	if (filter?.organization_unit_id)
		params.append('organization_unit_id', filter.organization_unit_id);
	if (filter?.search) params.append('search', filter.search);
	if (filter?.page) params.append('page', filter.page.toString());
	if (filter?.page_size) params.append('page_size', filter.page_size.toString());

	const response = await apiClient.get<StaffListData>(`/api/staff?${params.toString()}`, options);
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

export async function getStaffProfile(
	staffId: string,
	options: ApiRequestOptions = {}
): Promise<ApiResponse<StaffProfileResponse>> {
	return apiClient.get<StaffProfileResponse>(`/api/staff/${encodeURIComponent(staffId)}`, options);
}

export async function getPublicStaffProfile(
	staffId: string,
	options: ApiRequestOptions = {}
): Promise<ApiResponse<PublicStaffProfileResponse>> {
	return apiClient.get<PublicStaffProfileResponse>(`/api/staff/${staffId}/public-profile`, options);
}

export async function createStaff(data: CreateStaffRequest): Promise<ApiResponse<UuidIdData>> {
	return apiClient.post<UuidIdData>('/api/staff', data);
}

export async function updateStaff(
	staffId: string,
	data: UpdateStaffRequest
): Promise<ApiResponse<EmptyData>> {
	return apiClient.put<EmptyData>(`/api/staff/${staffId}`, data);
}

export async function deleteStaff(staffId: string): Promise<ApiResponse<EmptyData>> {
	return apiClient.delete<EmptyData>(`/api/staff/${encodeURIComponent(staffId)}`);
}

// ===================================================================
// Role APIs
// ===================================================================

export async function listRoles(options: ApiRequestOptions = {}): Promise<ApiResponse<Role[]>> {
	return apiClient.get<Role[]>('/api/roles', options);
}

export async function getRole(roleId: string): Promise<ApiResponse<Role>> {
	return apiClient.get<Role>(`/api/roles/${roleId}`);
}

// ===================================================================
// Organization Unit APIs
// ===================================================================

export async function listOrganizationUnits(
	options?: ManagedListOptions,
	requestOptions: ApiRequestOptions = {}
): Promise<ApiResponse<OrganizationUnit[]>> {
	const params = new URLSearchParams();
	if (options?.include_inactive) params.set('include_inactive', 'true');
	const qs = params.toString() ? `?${params}` : '';
	return apiClient.get<OrganizationUnit[]>(`/api/organization/units${qs}`, requestOptions);
}

// Auth-only version (no roles.read.all required) — for non-admin pages
export async function listOrganizationUnitsLookup(options?: {
	member_only?: boolean;
}): Promise<ApiResponse<OrganizationUnitLookupItem[]>> {
	const params = new URLSearchParams();
	if (options?.member_only) params.set('member_only', 'true');
	const qs = params.toString() ? `?${params}` : '';
	return apiClient.get<OrganizationUnitLookupItem[]>(`/api/lookup/organization-units${qs}`);
}

// Get single organization unit (auth only, no roles.read.all required)
export async function getOrganizationUnitLookup(
	id: string
): Promise<ApiResponse<OrganizationUnitLookupItem>> {
	return apiClient.get<OrganizationUnitLookupItem>(`/api/lookup/organization-units/${id}`);
}

export async function getOrganizationUnit(
	unitId: string,
	options: ApiRequestOptions = {}
): Promise<ApiResponse<OrganizationUnit>> {
	return apiClient.get<OrganizationUnit>(
		`/api/organization/units/${encodeURIComponent(unitId)}`,
		options
	);
}

export type CreateOrganizationUnitRequest = Schemas['CreateOrganizationUnitRequest'];
export type UpdateOrganizationUnitRequest = Schemas['UpdateOrganizationUnitRequest'];
type UuidIdData = Schemas['UuidIdData'];
type EmptyData = Schemas['EmptyData'];

export async function createOrganizationUnit(
	data: CreateOrganizationUnitRequest
): Promise<ApiResponse<UuidIdData>> {
	return apiClient.post<UuidIdData>('/api/organization/units', data);
}

export async function updateOrganizationUnit(
	unitId: string,
	data: UpdateOrganizationUnitRequest
): Promise<ApiResponse<EmptyData>> {
	return apiClient.put<EmptyData>(`/api/organization/units/${unitId}`, data);
}

export async function deleteOrganizationUnit(unitId: string): Promise<ApiResponse<EmptyData>> {
	return apiClient.delete<EmptyData>(`/api/organization/units/${unitId}`);
}

// ===================================================================
// Organization Permission APIs
// ===================================================================

export type OrganizationPermissionGrant = Schemas['OrganizationPermissionGrant'];
type OrganizationPermissionGrantInput = Schemas['OrganizationPermissionGrantInput'];

export async function getOrganizationPermissions(
	unitId: string,
	options: ApiRequestOptions = {}
): Promise<OrganizationPermissionGrant[]> {
	const response = await apiClient.get<OrganizationPermissionGrant[]>(
		`/api/organization/units/${encodeURIComponent(unitId)}/permissions`,
		options
	);
	return requireApiData(response, 'Failed to fetch organization permissions');
}

export async function updateOrganizationPermissions(
	unitId: string,
	grants: OrganizationPermissionGrantInput[]
): Promise<ApiResponse<EmptyData>> {
	return apiClient.put<EmptyData>(`/api/organization/units/${unitId}/permissions`, {
		grants
	});
}

// ===================================================================
// Delegation APIs
// ===================================================================

export type DelegationItem = Schemas['DelegationItem'];
export type CreateDelegationBody = Schemas['CreateDelegationRequest'];
export type DelegatablePermission = Schemas['DelegatablePermission'];

export async function listDelegatablePermissions(
	organizationUnitId: string,
	options: ApiRequestOptions = {}
): Promise<ApiResponse<DelegatablePermission[]>> {
	return apiClient.get<DelegatablePermission[]>(
		`/api/organization/units/${encodeURIComponent(organizationUnitId)}/delegatable-permissions`,
		options
	);
}

export async function listDelegations(
	organizationUnitId: string,
	options: ApiRequestOptions = {}
): Promise<ApiResponse<DelegationItem[]>> {
	return apiClient.get<DelegationItem[]>(
		`/api/organization/units/${encodeURIComponent(organizationUnitId)}/delegations`,
		options
	);
}

export async function createDelegation(
	organizationUnitId: string,
	body: CreateDelegationBody
): Promise<ApiResponse<Schemas['DelegationIdData']>> {
	return apiClient.post<Schemas['DelegationIdData']>(
		`/api/organization/units/${organizationUnitId}/delegations`,
		body
	);
}

export async function revokeDelegation(delegationId: string): Promise<ApiResponse<EmptyData>> {
	return apiClient.delete<EmptyData>(`/api/organization/delegations/${delegationId}`);
}

// ===================================================================
// Organization Member Management APIs
// ===================================================================

export type OrganizationMemberItem = Schemas['OrganizationMemberItem'];
export type AddMemberBody = Schemas['AddMemberRequest'];
export type UpdateMemberBody = Schemas['UpdateMemberRequest'];

export async function listOrganizationMembers(
	unitId: string,
	options?: Schemas['ListMembersQuery'],
	requestOptions: ApiRequestOptions = {}
): Promise<ApiResponse<OrganizationMemberItem[]>> {
	const params = options?.include_children ? '?include_children=true' : '';
	return apiClient.get<OrganizationMemberItem[]>(
		`/api/organization/units/${encodeURIComponent(unitId)}/members${params}`,
		requestOptions
	);
}

export async function addOrganizationMember(
	unitId: string,
	body: AddMemberBody
): Promise<ApiResponse<EmptyData>> {
	return apiClient.post<EmptyData>(`/api/organization/units/${unitId}/members`, body);
}

export async function updateOrganizationMember(
	unitId: string,
	userId: string,
	body: UpdateMemberBody
): Promise<ApiResponse<EmptyData>> {
	return apiClient.put<EmptyData>(`/api/organization/units/${unitId}/members/${userId}`, body);
}

export async function removeOrganizationMember(
	unitId: string,
	userId: string
): Promise<ApiResponse<EmptyData>> {
	return apiClient.delete<EmptyData>(`/api/organization/units/${unitId}/members/${userId}`);
}
