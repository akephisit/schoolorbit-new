import {
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];
export type HomeroomRoster = Schemas['HomeroomRoster'];
export type HomeroomRosterStudent = Schemas['HomeroomRosterStudent'];
export type HomeroomRosterCandidate = Schemas['HomeroomRosterCandidate'];
export type HomeroomNumberingMethod = Schemas['HomeroomNumberingMethod'];
export type HomeroomNumberingPreview = Schemas['HomeroomNumberingPreview'];
export type HomeroomNumberInput = Schemas['HomeroomNumberInput'];
export type HomeroomRosterSelection = Schemas['HomeroomRosterSelection'];
export type MutateHomeroomRosterRequest = Schemas['MutateHomeroomRosterRequest'];

async function result<T>(promise: Promise<ApiResponse<T>>): Promise<T> {
	return requireApiData(await promise, 'ดำเนินการไม่สำเร็จ');
}
const base = (id: string) => `/api/academic/homerooms/${encodeURIComponent(id)}`;
export const getHomeroomRoster = (id: string, options: ApiRequestOptions = {}) =>
	result(apiClient.get<HomeroomRoster>(`${base(id)}/students`, options));
export const listHomeroomRosterCandidates = (
	id: string,
	search: string,
	options: ApiRequestOptions = {}
) =>
	result(
		apiClient.get<HomeroomRosterCandidate[]>(`${base(id)}/student-candidates`, {
			...options,
			query: { search }
		})
	);
export const previewHomeroomNumbers = (
	id: string,
	method: HomeroomNumberingMethod,
	startNumber: number,
	options: ApiRequestOptions = {}
) =>
	result(
		apiClient.get<HomeroomNumberingPreview>(`${base(id)}/numbering-preview`, {
			...options,
			query: { method, startNumber }
		})
	);
export const updateHomeroomNumbers = (
	id: string,
	revision: string,
	numbers: HomeroomNumberInput[]
) =>
	result(
		apiClient.patch<HomeroomRoster>(`${base(id)}/numbers`, {
			revision,
			numbers
		} satisfies Schemas['UpdateHomeroomNumbersRequest'])
	);
export const mutateHomeroomRoster = (id: string, request: MutateHomeroomRosterRequest) =>
	result(apiClient.post<HomeroomRoster>(`${base(id)}/students`, request));

export const listHomeroomTransferTargets = (id: string, options: ApiRequestOptions = {}) =>
	result(apiClient.get<Schemas['Homeroom'][]>(`${base(id)}/transfer-targets`, options));
