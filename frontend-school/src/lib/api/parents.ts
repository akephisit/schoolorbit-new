import { apiClient } from '$lib/api/client';
import type { TimetableEntry } from './timetable';

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
export async function getOwnParentProfile(): Promise<{ success: boolean; data: ParentProfile }> {
	const response = await apiClient.get<ParentProfile>('/api/parent/profile');
	if (!response.success || !response.data) {
		throw new Error(response.error || 'Failed to get parent profile');
	}
	return response as { success: boolean; data: ParentProfile };
}

/**
 * Get detailed profile of a child linked to the current parent
 */
export async function getChildProfile(
	studentId: string
): Promise<{ success: boolean; data: unknown }> {
	const response = await apiClient.get<unknown>(`/api/parent/students/${studentId}`);
	if (!response.success || response.data === undefined) {
		throw new Error(response.error || 'Failed to get student profile');
	}
	return response as { success: boolean; data: unknown };
}

/**
 * Get child's timetable (parent self-service)
 */
export async function getChildTimetable(
	studentId: string,
	academicSemesterId?: string
): Promise<{ success: boolean; data: TimetableEntry[] }> {
	const params = new URLSearchParams();
	if (academicSemesterId) params.append('academic_semester_id', academicSemesterId);
	const qs = params.toString() ? `?${params.toString()}` : '';

	const response = await apiClient.get<TimetableEntry[]>(
		`/api/parent/students/${studentId}/timetable${qs}`
	);
	if (!response.success || !response.data) {
		throw new Error(response.error || 'Failed to get child timetable');
	}
	return response as { success: boolean; data: TimetableEntry[] };
}
