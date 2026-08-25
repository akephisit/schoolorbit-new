/**
 * Student API Client
 * Handles all student-related API calls (both admin and self-service)
 */

import { apiClient, requireApiData } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type StudentParent = Schemas['ParentDto'];
export type Student = Schemas['StudentProfile'];
export type StudentListItem = Schemas['StudentListItem'];
export type StudentListResponse = Schemas['StudentListResponse'];
export type ListStudentsQuery = NonNullable<
	operations['listStudents']['parameters']['query']
>;

export type CreateStudentRequest = Schemas['CreateStudentRequest'];
export type CreateParentRequest = Schemas['CreateParentRequest'];
export type UpdateStudentRequest = Schemas['UpdateStudentRequest'];
export type UpdateOwnProfileRequest = Schemas['UpdateOwnProfileRequest'];
type CreateStudentResponse = Schemas['CreateStudentResponse'];
type EmptyData = Schemas['EmptyData'];

/**
 * List all students (Admin)
 */
export async function listStudents(query: ListStudentsQuery): Promise<StudentListResponse> {
	return requireApiData(
		await apiClient.get<StudentListResponse>('/api/students', { query: { ...query } }),
		'Failed to list students'
	);
}

/**
 * Get student by ID (Admin)
 */
export async function getStudent(id: string, academicYearId: string): Promise<Student> {
	const query = { academicYearId } satisfies NonNullable<
		operations['getStudent']['parameters']['query']
	>;
	return requireApiData(
		await apiClient.get<Student>(`/api/students/${encodeURIComponent(id)}`, { query }),
		'Failed to get student'
	);
}

/**
 * Create new student (Admin)
 */
export async function createStudent(
	data: CreateStudentRequest
): Promise<{ success: boolean; id: string }> {
	const response = await apiClient.post<CreateStudentResponse>('/api/students', data);
	const result = requireApiData(response, 'Failed to create student');
	return { success: true, id: result.id };
}

/**
 * Update student (Admin)
 */
export async function updateStudent(
	id: string,
	data: UpdateStudentRequest
): Promise<{ success: boolean }> {
	const response = await apiClient.put<EmptyData>(`/api/students/${id}`, data);
	if (!response.success) throw new Error(response.error || 'Failed to update student');
	return { success: true };
}

/**
 * Delete student (Admin)
 */
export async function deleteStudent(id: string): Promise<{ success: boolean }> {
	const response = await apiClient.delete<EmptyData>(`/api/students/${id}`);
	if (!response.success) throw new Error(response.error || 'Failed to delete student');
	return { success: true };
}

/**
 * Get own profile (Student self-service)
 */
export async function getOwnProfile(): Promise<{ success: boolean; data: Student }> {
	const response = await apiClient.get<Student>('/api/student/profile');
	const data = requireApiData(response, 'Failed to get profile');
	return { success: true, data };
}

/**
 * Update own profile (Student self-service)
 */
export async function updateOwnProfile(
	data: UpdateOwnProfileRequest
): Promise<{ success: boolean }> {
	const response = await apiClient.put<EmptyData>('/api/student/profile', data);
	if (!response.success) throw new Error(response.error || 'Failed to update profile');
	return { success: true };
}

/**
 * Add parent to student
 */
export async function addParentToStudent(
	studentId: string,
	data: CreateParentRequest
): Promise<{ success: boolean; message: string }> {
	const response = await apiClient.post<EmptyData>(`/api/students/${studentId}/parents`, data);
	if (!response.success) throw new Error(response.error || 'Failed to add parent');
	return { success: true, message: response.message || 'เพิ่มผู้ปกครองสำเร็จ' };
}

/**
 * Remove parent from student
 */
export async function removeParentFromStudent(
	studentId: string,
	parentId: string
): Promise<{ success: boolean; message: string }> {
	const response = await apiClient.delete<EmptyData>(
		`/api/students/${studentId}/parents/${parentId}`
	);
	if (!response.success) throw new Error(response.error || 'Failed to remove parent');
	return { success: true, message: response.message || 'ลบผู้ปกครองสำเร็จ' };
}
