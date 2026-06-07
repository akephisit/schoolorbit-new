import { apiClient } from '$lib/api/client';
import type { TimetableEntry } from './timetable';
import type { Student } from './students';

type LoadedApiResponse<T> = { success: true; data: T };

export interface ChildDto {
	id: string;
	first_name: string;
	last_name: string;
	student_id?: string;
	grade_level?: string;
	class_room?: string;
	profile_image_url?: string;
	relationship: string;
}

export interface ParentProfile {
	id: string;
	username: string;
	first_name: string;
	last_name: string;
	title?: string;
	phone?: string;
	email?: string;
	national_id?: string;
	children: ChildDto[];
}

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

	const response = await apiClient.get<TimetableEntry[]>(
		`/api/parent/students/${studentId}/timetable${qs}`
	);
	if (!response.success || !response.data) {
		throw new Error(response.error || 'Failed to get child timetable');
	}
	return { success: true, data: response.data };
}
