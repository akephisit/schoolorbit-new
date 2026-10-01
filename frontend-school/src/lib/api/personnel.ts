import { apiClient, requireApiData, type ApiRequestOptions } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';
type Schemas = components['schemas'];
export type PersonnelOverview = Schemas['PersonnelOverview'];
export type PersonnelBucket = Schemas['PersonnelBucket'];
export type PersonnelDimension = Schemas['PersonnelDimension'];
export type PersonnelStatusFilter = Schemas['PersonnelStatusFilter'];
export type StaffReferenceKind = Schemas['StaffReferenceKind'];
export type StaffReferenceSummary = Schemas['StaffReferenceSummary'];
export type StaffReferenceItem = Schemas['StaffReferenceItem'];
export type ReferencePage = Schemas['ReferencePage'];
export type ReferenceListQuery = NonNullable<
	operations['listStaffReferenceItems']['parameters']['query']
>;
export type CreateReferenceRequest = Schemas['CreateReferenceRequest'];
export type UpdateReferenceRequest = Schemas['UpdateReferenceRequest'];
export type PersonnelOverviewQuery = NonNullable<
	operations['getPersonnelOverview']['parameters']['query']
>;
function queryString(query: object): string {
	const params = new URLSearchParams();
	for (const [key, value] of Object.entries(query))
		if (value !== undefined && value !== null && value !== '') params.set(key, String(value));
	return params.toString();
}
export async function getPersonnelOverview(
	query: PersonnelOverviewQuery = {},
	options: ApiRequestOptions = {}
): Promise<PersonnelOverview> {
	return requireApiData(
		await apiClient.get<PersonnelOverview>(
			`/api/staff/personnel-overview?${queryString(query)}`,
			options
		),
		'โหลดภาพรวมงานบุคคลไม่สำเร็จ'
	);
}
export async function listStaffReferenceItems(
	query: ReferenceListQuery,
	options: ApiRequestOptions = {}
): Promise<ReferencePage> {
	return requireApiData(
		await apiClient.get<ReferencePage>(`/api/staff/reference-items?${queryString(query)}`, options),
		'โหลดรายการกลางไม่สำเร็จ'
	);
}
export async function createStaffReferenceItem(
	input: CreateReferenceRequest
): Promise<StaffReferenceItem> {
	return requireApiData(
		await apiClient.post<StaffReferenceItem>('/api/staff/reference-items', input),
		'เพิ่มรายการไม่สำเร็จ'
	);
}
export async function updateStaffReferenceItem(
	id: string,
	input: UpdateReferenceRequest
): Promise<StaffReferenceItem> {
	return requireApiData(
		await apiClient.patch<StaffReferenceItem>(`/api/staff/reference-items/${id}`, input),
		'แก้ไขรายการไม่สำเร็จ'
	);
}
