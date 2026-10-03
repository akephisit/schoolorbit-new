import { apiClient, requireApiData, type ApiRequestOptions } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';
type Schemas = components['schemas'];
export type PersonnelOverview = Schemas['PersonnelOverview'];
export type PersonnelBucket = Schemas['PersonnelBucket'];
export type PersonnelDimension = Schemas['PersonnelDimension'];
export type PersonnelStatusFilter = Schemas['PersonnelStatusFilter'];
export type StaffJobPositionSummary = Schemas['StaffJobPositionSummary'];
export type JobPositionPage = Schemas['JobPositionPage'];
export type JobPositionListQuery = NonNullable<
	operations['listStaffJobPositions']['parameters']['query']
>;
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
export async function listStaffJobPositions(
	query: JobPositionListQuery = {},
	options: ApiRequestOptions = {}
): Promise<JobPositionPage> {
	return requireApiData(
		await apiClient.get<JobPositionPage>(`/api/staff/job-positions?${queryString(query)}`, options),
		'โหลดรายการตำแหน่งไม่สำเร็จ'
	);
}
