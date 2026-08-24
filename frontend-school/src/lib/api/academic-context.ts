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

export async function listPublicAcademicContextOptions(
	signal?: AbortSignal
): Promise<AcademicContextOptionsResponse> {
	const response = await apiClient.get<AcademicContextOptionsResponse>(
		'/api/public/academic-context/options',
		{ signal }
	);
	return requireApiData(response, 'ไม่สามารถโหลดปีและภาคเรียนของปฏิทินได้');
}

export async function listMyAcademicContextOptions(
	signal?: AbortSignal
): Promise<AcademicContextOptionsResponse> {
	const response = await apiClient.get<AcademicContextOptionsResponse>(
		'/api/me/academic-context/options',
		{ signal }
	);
	return requireApiData(response, 'ไม่สามารถโหลดประวัติปีและภาคเรียนได้');
}

export async function listChildAcademicContextOptions(
	studentId: string,
	signal?: AbortSignal
): Promise<AcademicContextOptionsResponse> {
	const response = await apiClient.get<AcademicContextOptionsResponse>(
		`/api/parent/students/${encodeURIComponent(studentId)}/academic-context/options`,
		{ signal }
	);
	return requireApiData(response, 'ไม่สามารถโหลดประวัติปีและภาคเรียนของนักเรียนได้');
}
