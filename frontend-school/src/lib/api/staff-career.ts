import { apiClient, requireApiData, type ApiRequestOptions } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';
type Schemas = components['schemas'];
export type StaffCareerHistoryPage = Schemas['StaffCareerHistoryPage'];
export type StaffCareerMutationAck = Schemas['StaffCareerMutationAck'];
export type CreateStaffCareerHistoryRequest = Schemas['CreateStaffCareerHistoryRequest'];
export type CorrectStaffCareerHistoryRequest = Schemas['CorrectStaffCareerHistoryRequest'];
export type StaffCareerHistoryQuery = NonNullable<
	operations['listStaffCareerHistory']['parameters']['query']
>;
export async function listStaffCareerHistory(
	staffId: string,
	query: StaffCareerHistoryQuery = {},
	options: ApiRequestOptions = {}
): Promise<StaffCareerHistoryPage> {
	return requireApiData(
		await apiClient.get<StaffCareerHistoryPage>(`/api/staff/${staffId}/career-history`, {
			...options,
			query
		}),
		'โหลดประวัติบุคลากรไม่สำเร็จ'
	);
}
export async function appendStaffCareerHistory(
	staffId: string,
	input: CreateStaffCareerHistoryRequest,
	options: ApiRequestOptions = {}
): Promise<StaffCareerMutationAck> {
	return requireApiData(
		await apiClient.post<StaffCareerMutationAck>(
			`/api/staff/${staffId}/career-history`,
			input,
			options
		),
		'บันทึกประวัติไม่สำเร็จ'
	);
}
export async function correctStaffCareerHistory(
	staffId: string,
	entryId: string,
	input: CorrectStaffCareerHistoryRequest,
	options: ApiRequestOptions = {}
): Promise<StaffCareerMutationAck> {
	return requireApiData(
		await apiClient.patch<StaffCareerMutationAck>(
			`/api/staff/${staffId}/career-history/${entryId}`,
			input,
			options
		),
		'แก้ประวัติไม่สำเร็จ'
	);
}
