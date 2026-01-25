// Lookup API Client
// API for fetching minimal data for dropdowns (only requires authentication)
// These endpoints return id + name only, safe for any authenticated user

import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

// ===================================================================
// Types
// ===================================================================

export interface LookupItem {
    id: string;
    name: string;
    code?: string;
}

export interface StaffLookupItem {
    id: string;
    name: string;
    title?: string;
    username?: string;
}

export interface RoleLookupItem {
    id: string;
    code: string;
    name: string;
    user_type: string;
}

export interface DepartmentLookupItem {
    id: string;
    code: string;
    name: string;
}

export interface GradeLevelLookupItem {
    id: string;
    code: string;
    name: string;
    short_name?: string;
    level_order: number;
}

export interface ClassroomLookupItem {
    id: string;
    name: string;
    grade_level?: string;
    grade_level_id?: string;
}

export interface AcademicYearLookupItem {
    id: string;
    name: string;
    is_current: boolean;
}

export interface StudentLookupItem {
    id: string;
    name: string;
    title?: string;
    student_id?: string;
    class_room?: string;
}

export interface LookupResponse<T> {
    success: boolean;
    data: T[];
}

export interface LookupOptions {
    /** Filter for active items only (default: true) */
    activeOnly?: boolean;
    /** Search term */
    search?: string;
    /** Maximum items to return (default: 100, max: 500) */
    limit?: number;
    subjectType?: string;
}

// ===================================================================
// Helper
// ===================================================================

function buildQueryString(options?: LookupOptions): string {
    const params = new URLSearchParams();
    if (options?.activeOnly !== undefined) {
        params.set('active_only', String(options.activeOnly));
    }
    if (options?.search) {
        params.set('search', options.search);
    }
    if (options?.limit) {
        params.set('limit', String(options.limit));
    }
    if (options?.subjectType) {
        params.set('subject_type', options.subjectType);
    }
    const queryString = params.toString();
    return queryString ? `?${queryString}` : '';
}

async function fetchLookup<T>(endpoint: string, options?: LookupOptions): Promise<T[]> {
    const query = buildQueryString(options);
    const response = await fetch(`${BACKEND_URL}/api/lookup/${endpoint}${query}`, {
        method: 'GET',
        credentials: 'include',
        headers: {
            'Content-Type': 'application/json'
        }
    });

    if (!response.ok) {
        throw new Error(`Failed to fetch ${endpoint}: ${response.statusText}`);
    }

    const result: LookupResponse<T> = await response.json();
    return result.data;
}

// ===================================================================
// API Functions
// ===================================================================

/**
 * Fetch staff list for dropdowns
 * Returns: id, name, title, username
 */
export async function lookupStaff(options?: LookupOptions): Promise<StaffLookupItem[]> {
    return fetchLookup<StaffLookupItem>('staff', options);
}

/**
 * Fetch students list for dropdowns
 * Returns: id, name, student_id, class_room
 */
export async function lookupStudents(options?: LookupOptions): Promise<StudentLookupItem[]> {
    return fetchLookup<StudentLookupItem>('students', options);
}

/**
 * Fetch roles list for dropdowns
 * Returns: id, code, name, user_type
 */
export async function lookupRoles(options?: LookupOptions): Promise<RoleLookupItem[]> {
    return fetchLookup<RoleLookupItem>('roles', options);
}

/**
 * Fetch departments list for dropdowns
 * Returns: id, code, name
 */
export async function lookupDepartments(options?: LookupOptions): Promise<DepartmentLookupItem[]> {
    return fetchLookup<DepartmentLookupItem>('departments', options);
}

/**
 * Fetch grade levels list for dropdowns
 * Returns: id, code, name, level_order
 */
export async function lookupGradeLevels(options?: LookupOptions): Promise<GradeLevelLookupItem[]> {
    return fetchLookup<GradeLevelLookupItem>('grade-levels', options);
}

/**
 * Fetch classrooms list for dropdowns
 * Returns: id, name, grade_level
 */
export async function lookupClassrooms(options?: LookupOptions): Promise<ClassroomLookupItem[]> {
    return fetchLookup<ClassroomLookupItem>('classrooms', options);
}

/**
 * Fetch academic years list for dropdowns
 * Returns: id, name, is_current
 */
export async function lookupAcademicYears(
    options?: LookupOptions
): Promise<AcademicYearLookupItem[]> {
    return fetchLookup<AcademicYearLookupItem>('academic-years', options);
}

export interface RoomLookupItem {
    id: string;
    name_th: string;
    name_en?: string;
    code?: string;
    room_type: string;
    building_name?: string;
}

/**
 * Fetch active rooms list for dropdowns
 * Returns active rooms with basic info
 */
export async function lookupRooms(options?: LookupOptions): Promise<RoomLookupItem[]> {
    return fetchLookup<RoomLookupItem>('rooms', options);
}

/**
 * Fetch subjects list for dropdowns
 * Returns: id, name, code
 */
export async function lookupSubjects(options?: LookupOptions): Promise<LookupItem[]> {
    return fetchLookup<LookupItem>('subjects', options);
}
