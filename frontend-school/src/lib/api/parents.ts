import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

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
    phone?: string;
    email?: string;
    national_id?: string;
    children: ChildDto[];
}

/**
 * Get own parent profile (Parent self-service)
 */
export async function getOwnParentProfile(): Promise<{ success: boolean; data: ParentProfile }> {
    const response = await fetch(`${BACKEND_URL}/api/parent/profile`, {
        credentials: 'include'
    });

    if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || 'Failed to get parent profile');
    }

    return await response.json();
}

/**
 * Get detailed profile of a child linked to the current parent
 */
export async function getChildProfile(studentId: string): Promise<{ success: boolean; data: any }> { // Using any for StudentProfile matching backend
    const response = await fetch(`${BACKEND_URL}/api/parent/students/${studentId}`, {
        credentials: 'include'
    });

    if (!response.ok) {
        const error = await response.json();
        throw new Error(error.error || 'Failed to get student profile');
    }

    return await response.json();
}
