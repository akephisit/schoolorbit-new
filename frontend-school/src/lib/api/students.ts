/**
 * Student API Client
 * Handles all student-related API calls (both admin and self-service)
 */

import { apiClient, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type StudentParent = Schemas['ParentDto'];
export type Student = Schemas['StudentProfile'];

export interface StudentListItem {
	id: string;
	title?: string;
	first_name: string;
	last_name: string;
	student_id?: string;
	grade_level?: string;
	class_room?: string;
	status: string;
}

export interface ListStudentsParams {
	page?: number;
	page_size?: number;
	grade_level?: string;
	class_room?: string;
	search?: string;
	status?: string;
}

export interface ListStudentsResponse {
	success: boolean;
	data: StudentListItem[];
	page: number;
	page_size: number;
	total?: number;
	total_pages?: number;
}

export interface CreateStudentRequest {
	username?: string;
	national_id?: string;
	email?: string;
	password: string;
	first_name: string;
	last_name: string;
	title?: string;
	student_id: string;
	student_number?: number;
	date_of_birth?: string;
	gender?: string;
	parent?: CreateParentRequest;
}

export interface CreateParentRequest {
	title?: string;
	first_name: string;
	last_name: string;
	phone: string;
	relationship: string;
	national_id?: string;
	email?: string;
}

export interface UpdateStudentRequest {
	email?: string;
	first_name?: string;
	last_name?: string;
	phone?: string;
	address?: string;
	student_number?: number;
}

export interface UpdateOwnProfileRequest {
	phone?: string;
	address?: string;
	nickname?: string;
}

/**
 * List all students (Admin)
 */
export async function listStudents(params?: ListStudentsParams): Promise<ListStudentsResponse> {
	const queryParams = new URLSearchParams();
	if (params?.page) queryParams.append('page', params.page.toString());
	if (params?.page_size) queryParams.append('page_size', params.page_size.toString());
	if (params?.grade_level) queryParams.append('grade_level', params.grade_level);
	if (params?.class_room) queryParams.append('class_room', params.class_room);
	if (params?.search) queryParams.append('search', params.search);
	if (params?.status) queryParams.append('status', params.status);

	const response = await apiClient.get<{
		items: StudentListItem[];
		page: number;
		page_size: number;
		total?: number;
		total_pages?: number;
	}>(`/api/students?${queryParams.toString()}`);
	const data = requireApiData(response, 'Failed to list students');
	return {
		success: true,
		data: data.items,
		page: data.page,
		page_size: data.page_size,
		total: data.total,
		total_pages: data.total_pages
	};
}

/**
 * Get student by ID (Admin)
 */
export async function getStudent(id: string): Promise<{ success: boolean; data: Student }> {
	const response = await apiClient.get<Student>(`/api/students/${id}`);
	const data = requireApiData(response, 'Failed to get student');
	return { success: true, data };
}

/**
 * Create new student (Admin)
 */
export async function createStudent(
	data: CreateStudentRequest
): Promise<{ success: boolean; id: string }> {
	const response = await apiClient.post<{ id: string; username?: string }>('/api/students', data);
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
	const response = await apiClient.put<Record<string, never>>(`/api/students/${id}`, data);
	if (!response.success) throw new Error(response.error || 'Failed to update student');
	return { success: true };
}

/**
 * Delete student (Admin)
 */
export async function deleteStudent(id: string): Promise<{ success: boolean }> {
	const response = await apiClient.delete<Record<string, never>>(`/api/students/${id}`);
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
	const response = await apiClient.put<Record<string, never>>('/api/student/profile', data);
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
	const response = await apiClient.post<Record<string, never>>(
		`/api/students/${studentId}/parents`,
		data
	);
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
	const response = await apiClient.delete<Record<string, never>>(
		`/api/students/${studentId}/parents/${parentId}`
	);
	if (!response.success) throw new Error(response.error || 'Failed to remove parent');
	return { success: true, message: response.message || 'ลบผู้ปกครองสำเร็จ' };
}
