// Lookup API Client
// API for fetching minimal reference data for dropdowns.
// Generic lookup responses must stay small; workflow-specific detail belongs in options endpoints.

import { apiClient, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

// ===================================================================
// Types
// ===================================================================

export type LookupItem = Schemas['LookupItem'];
export type StaffLookupItem = Schemas['StaffLookupItem'];
export type RoleLookupItem = Schemas['RoleLookupItem'];
export type OrganizationUnitLookupItem = Schemas['OrganizationUnitLookupItem'];
export type GradeLevelLookupItem = Schemas['GradeLevelLookupItem'];
export type ClassroomLookupItem = Schemas['ClassroomLookupItem'];
export type AcademicYearLookupItem = Schemas['AcademicYearLookupItem'];
export type StudentLookupItem = Schemas['StudentLookupItem'];
export type RoomLookupItem = Schemas['Room'];

export interface LookupOptions {
	/** Filter for active items only (default: true) */
	activeOnly?: boolean;
	/** Search term */
	search?: string;
	/** Maximum items to return (default: 100, max: 500) */
	limit?: number;
	subjectType?: string;
	/** Filter grade levels by academic year */
	academicYearId?: string;
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
	if (options?.academicYearId) {
		params.set('academic_year_id', options.academicYearId);
	}
	const queryString = params.toString();
	return queryString ? `?${queryString}` : '';
}

async function fetchLookup<T>(endpoint: string, options?: LookupOptions): Promise<T[]> {
	const query = buildQueryString(options);
	const response = await apiClient.get<T[]>(`/api/lookup/${endpoint}${query}`);
	return requireApiData(response, `Failed to fetch ${endpoint}`);
}

// ===================================================================
// API Functions
// ===================================================================

/**
 * Fetch staff list for dropdowns
 * Returns: id, name, title
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
 * Requires roles.read.all or roles.assign.all.
 */
export async function lookupRoles(options?: LookupOptions): Promise<RoleLookupItem[]> {
	return fetchLookup<RoleLookupItem>('roles', options);
}

/**
 * Fetch organization units list for dropdowns
 * Returns: id, code, name
 */
export async function lookupOrganizationUnits(
	options?: LookupOptions
): Promise<OrganizationUnitLookupItem[]> {
	return fetchLookup<OrganizationUnitLookupItem>('organization-units', options);
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
