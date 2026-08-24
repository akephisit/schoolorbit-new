// Lookup API Client
// API for fetching minimal reference data for dropdowns.
// Generic lookup responses must stay small; workflow-specific detail belongs in options endpoints.

import { apiClient, requireApiData } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

// ===================================================================
// Types
// ===================================================================

export type LookupItem = Schemas['LookupItem'];
export type StaffLookupItem = Schemas['StaffLookupItem'];
export type RoleLookupItem = Schemas['RoleLookupItem'];
export type OrganizationUnitLookupItem = Schemas['OrganizationUnitLookupItem'];
export type GradeLevelLookupItem = Schemas['GradeLevelLookupItem'];
export type HomeroomLookupItem = Schemas['HomeroomLookupItem'];
export type AcademicYearLookupItem = Schemas['AcademicYearLookupItem'];
export type StudentLookupItem = Schemas['StudentLookupItem'];
export type RoomLookupItem = Schemas['Room'];

export type LookupOptions = NonNullable<operations['lookupStaff']['parameters']['query']>;
export type AcademicLookupOptions = operations['lookupStudents']['parameters']['query'];

// ===================================================================
// Helper
// ===================================================================

function buildQueryString(options?: LookupOptions | AcademicLookupOptions): string {
	const params = new URLSearchParams();
	if (options?.activeOnly !== undefined) {
		params.set('activeOnly', String(options.activeOnly));
	}
	if (options?.search) {
		params.set('search', options.search);
	}
	if (options?.limit) {
		params.set('limit', String(options.limit));
	}
	if (options && 'subjectType' in options && options.subjectType) {
		params.set('subjectType', options.subjectType);
	}
	if (options && 'levelType' in options && options.levelType) {
		params.set('levelType', options.levelType);
	}
	if (options && 'academicYearId' in options && options.academicYearId) {
		params.set('academicYearId', options.academicYearId);
	}
	const queryString = params.toString();
	return queryString ? `?${queryString}` : '';
}

async function fetchLookup<T>(
	endpoint: string,
	options?: LookupOptions | AcademicLookupOptions
): Promise<T[]> {
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
export async function lookupStudents(options: AcademicLookupOptions): Promise<StudentLookupItem[]> {
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
export async function lookupGradeLevels(
	options: AcademicLookupOptions
): Promise<GradeLevelLookupItem[]> {
	return fetchLookup<GradeLevelLookupItem>('grade-levels', options);
}

/**
 * Fetch homerooms for the caller-selected academic year.
 * Returns: id, name, grade_level
 */
export async function lookupHomerooms(
	options: AcademicLookupOptions
): Promise<HomeroomLookupItem[]> {
	return fetchLookup<HomeroomLookupItem>('homerooms', options);
}

/**
 * Fetch academic years list for dropdowns
 * Returns: id, name, year, status
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
export async function lookupSubjects(options: AcademicLookupOptions): Promise<LookupItem[]> {
	return fetchLookup<LookupItem>('subjects', options);
}
