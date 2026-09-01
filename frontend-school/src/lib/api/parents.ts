import { apiClient, requireApiData } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';
import type { TimetableBlock } from './timetable';
import type { Student } from './students';

type Schemas = components['schemas'];

export type ChildDto = Schemas['ChildDto'];
export type ParentProfile = Schemas['ParentProfile'];

/**
 * Get own parent profile (Parent self-service)
 */
export async function getOwnParentProfile(academicYearId: string): Promise<ParentProfile> {
	const query = { academicYearId } satisfies NonNullable<
		operations['getParentProfile']['parameters']['query']
	>;
	return requireApiData(
		await apiClient.get<ParentProfile>('/api/parent/profile', { query }),
		'Failed to get parent profile'
	);
}

/**
 * Get detailed profile of a child linked to the current parent
 */
export async function getChildProfile(studentId: string, academicYearId: string): Promise<Student> {
	const query = { academicYearId } satisfies NonNullable<
		operations['getParentChildProfile']['parameters']['query']
	>;
	return requireApiData(
		await apiClient.get<Student>(`/api/parent/students/${encodeURIComponent(studentId)}`, {
			query
		}),
		'Failed to get student profile'
	);
}

/**
 * Get child's timetable (parent self-service)
 */
export async function getChildTimetable(
	studentId: string,
	academicTermId: string,
	date: string
): Promise<TimetableBlock[]> {
	const trimmedAcademicTermId = academicTermId.trim();
	if (!trimmedAcademicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	const requiredDate = date.trim();
	if (!requiredDate) throw new Error('กรุณาเลือกวันที่ก่อน');
	const query = {
		academicTermId: trimmedAcademicTermId,
		date: requiredDate
	} satisfies NonNullable<operations['getParentChildTimetable']['parameters']['query']>;
	return requireApiData(
		await apiClient.get<TimetableBlock[]>(
			`/api/parent/students/${encodeURIComponent(studentId)}/timetable`,
			{ query }
		),
		'ไม่สามารถโหลดตารางเรียนของนักเรียนได้'
	);
}
