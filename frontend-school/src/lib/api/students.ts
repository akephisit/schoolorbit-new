/**
 * Student API Client
 * Handles all student-related API calls (both admin and self-service)
 */

import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

export interface Student {
	id: string;
	national_id?: string;
	email?: string;
	first_name: string;
	last_name: string;
	title?: string;
	nickname?: string;
	phone?: string;
	date_of_birth?: string;
	gender?: string;
	address?: string;
	profile_image_url?: string;
	student_id?: string;
	grade_level?: string;
	class_room?: string;
	student_number?: number;
	blood_type?: string;
	allergies?: string;
	medical_conditions?: string;
	status: string;
}

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
	grade_level_id?: string;
	class_room_id?: string;
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
	grade_level?: string;
	class_room?: string;
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

	const response = await fetch(`${BACKEND_URL}/api/students?${queryParams.toString()}`, {
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to list students');
	}

	return await response.json();
}

/**
 * Get student by ID (Admin)
 */
export async function getStudent(id: string): Promise<{ success: boolean; data: Student }> {
	const response = await fetch(`${BACKEND_URL}/api/students/${id}`, {
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to get student');
	}

	return await response.json();
}

/**
 * Create new student (Admin)
 */
export async function createStudent(
	data: CreateStudentRequest
): Promise<{ success: boolean; id: string }> {
	const response = await fetch(`${BACKEND_URL}/api/students`, {
		method: 'POST',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		// Try to parse as JSON first
		const contentType = response.headers.get('content-type');
		if (contentType && contentType.includes('application/json')) {
			const error = await response.json();
			throw new Error(error.error || 'Failed to create student');
		} else {
			// If not JSON, use the text response
			const errorText = await response.text();
			throw new Error(errorText || `Failed to create student (${response.status})`);
		}
	}

	return await response.json();
}

/**
 * Update student (Admin)
 */
export async function updateStudent(
	id: string,
	data: UpdateStudentRequest
): Promise<{ success: boolean }> {
	const response = await fetch(`${BACKEND_URL}/api/students/${id}`, {
		method: 'PUT',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to update student');
	}

	return await response.json();
}

/**
 * Delete student (Admin)
 */
export async function deleteStudent(id: string): Promise<{ success: boolean }> {
	const response = await fetch(`${BACKEND_URL}/api/students/${id}`, {
		method: 'DELETE',
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to delete student');
	}

	return await response.json();
}

/**
 * Get own profile (Student self-service)
 */
export async function getOwnProfile(): Promise<{ success: boolean; data: Student }> {
	const response = await fetch(`${BACKEND_URL}/api/student/profile`, {
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to get profile');
	}

	return await response.json();
}

/**
 * Update own profile (Student self-service)
 */
export async function updateOwnProfile(
	data: UpdateOwnProfileRequest
): Promise<{ success: boolean }> {
	const response = await fetch(`${BACKEND_URL}/api/student/profile`, {
		method: 'PUT',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to update profile');
	}

	return await response.json();
}

/**
 * Add parent to student
 */
export async function addParentToStudent(
	studentId: string,
	data: CreateParentRequest
): Promise<{ success: boolean; message: string }> {
	const response = await fetch(`${BACKEND_URL}/api/students/${studentId}/parents`, {
		method: 'POST',
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify(data)
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to add parent');
	}

	return await response.json();
}

/**
 * Remove parent from student
 */
export async function removeParentFromStudent(
	studentId: string,
	parentId: string
): Promise<{ success: boolean; message: string }> {
	const response = await fetch(`${BACKEND_URL}/api/students/${studentId}/parents/${parentId}`, {
		method: 'DELETE',
		credentials: 'include'
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(error.error || 'Failed to remove parent');
	}

	return await response.json();
}
