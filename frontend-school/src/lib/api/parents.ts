import { apiClient } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';
import { timetableEntryFromDto, type TimetableEntry, type TimetableEntryDto } from './timetable';
import type { Student } from './students';

type LoadedApiResponse<T> = { success: true; data: T };
type Schemas = components['schemas'];

export type ChildDto = Schemas['ChildDto'];
export type ParentProfile = Schemas['ParentProfile'];

/**
 * Get own parent profile (Parent self-service)
 */
export async function getOwnParentProfile(): Promise<LoadedApiResponse<ParentProfile>> {
	const response = await apiClient.get<ParentProfile>('/api/parent/profile');
	if (!response.success || !response.data) {
		throw new Error(response.error || 'Failed to get parent profile');
	}
	return { success: true, data: response.data };
}

/**
 * Get detailed profile of a child linked to the current parent
 */
export async function getChildProfile(studentId: string): Promise<LoadedApiResponse<Student>> {
	const response = await apiClient.get<Student>(`/api/parent/students/${studentId}`);
	if (!response.success || response.data === undefined) {
		throw new Error(response.error || 'Failed to get student profile');
	}
	return { success: true, data: response.data };
}

/**
 * Get child's timetable (parent self-service)
 */
export async function getChildTimetable(
	studentId: string,
	academicSemesterId?: string
): Promise<LoadedApiResponse<TimetableEntry[]>> {
	const params = new URLSearchParams();
	if (academicSemesterId) params.append('academic_semester_id', academicSemesterId);
	const qs = params.toString() ? `?${params.toString()}` : '';

	const response = await apiClient.get<TimetableEntryDto[]>(
		`/api/parent/students/${studentId}/timetable${qs}`
	);
	if (!response.success || !response.data) {
		throw new Error(response.error || 'Failed to get child timetable');
	}
	return { success: true, data: response.data.map(timetableEntryFromDto) };
}
