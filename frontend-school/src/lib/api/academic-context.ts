import { apiClient, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type AcademicContextOptionsResponse = Schemas['AcademicContextOptions'];
export type AcademicYearOption = Schemas['AcademicYearOption'];
export type AcademicTermOption = Schemas['AcademicTermOption'];

export async function listAcademicContextOptions(
	signal?: AbortSignal
): Promise<AcademicContextOptionsResponse> {
	const response = await apiClient.get<AcademicContextOptionsResponse>(
		'/api/academic/context/options',
		{ signal }
	);
	return requireApiData(response, 'ไม่สามารถโหลดบริบทการศึกษาได้');
}
