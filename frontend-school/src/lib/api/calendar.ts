import { apiClient, requireApiData } from '$lib/api/client';

export type CalendarAudienceType = 'all' | 'staff' | 'student' | 'parent';

export interface CalendarCategory {
	id: string;
	name: string;
	color: string;
	orderIndex: number;
	isActive: boolean;
	createdAt: string;
	updatedAt: string;
}

export interface CalendarEventTarget {
	id: string;
	audienceType: CalendarAudienceType;
	gradeLevelId?: string | null;
	classRoomId?: string | null;
}

export interface CalendarEventTargetInput {
	audienceType: CalendarAudienceType;
	gradeLevelId?: string | null;
	classRoomId?: string | null;
}

export interface CalendarEventReminder {
	id: string;
	daysBefore: number;
	remindOn: string;
	sentAt?: string | null;
}

export interface CalendarEvent {
	id: string;
	categoryId?: string | null;
	categoryName?: string | null;
	categoryColor?: string | null;
	title: string;
	description?: string | null;
	location?: string | null;
	startDate: string;
	endDate: string;
	allDay: boolean;
	startTime?: string | null;
	endTime?: string | null;
	isPublic: boolean;
	targets: CalendarEventTarget[];
	reminders: CalendarEventReminder[];
	createdBy?: string | null;
	updatedBy?: string | null;
	createdAt: string;
	updatedAt: string;
}

export interface CalendarPublicEvent {
	id: string;
	categoryId?: string | null;
	categoryName?: string | null;
	categoryColor?: string | null;
	title: string;
	description?: string | null;
	location?: string | null;
	startDate: string;
	endDate: string;
	allDay: boolean;
	startTime?: string | null;
	endTime?: string | null;
	isPublic: boolean;
	createdAt: string;
	updatedAt: string;
}

interface CalendarEventBaseFilters {
	from?: string;
	to?: string;
	categoryId?: string;
	q?: string;
}

export interface CalendarEventFilters extends CalendarEventBaseFilters {
	audience?: CalendarAudienceType;
	visibility?: 'public' | 'private';
}

export interface CalendarPublicEventFilters {
	from?: string;
	to?: string;
	categoryId?: string;
	q?: string;
}

export interface CreateCalendarEventRequest {
	title: string;
	description?: string | null;
	location?: string | null;
	categoryId?: string | null;
	startDate: string;
	endDate: string;
	allDay: boolean;
	startTime?: string | null;
	endTime?: string | null;
	isPublic: boolean;
	targets: CalendarEventTargetInput[];
	reminderOffsetsDays: number[];
	notifyAudience: boolean;
}

export type UpdateCalendarEventRequest = CreateCalendarEventRequest;

export interface UpsertCalendarCategoryRequest {
	name: string;
	color: string;
	orderIndex?: number;
	isActive?: boolean;
}

function calendarQuery(filters: CalendarEventFilters = {}) {
	const params = new URLSearchParams();
	if (filters.from) params.set('from', filters.from);
	if (filters.to) params.set('to', filters.to);
	if (filters.categoryId) params.set('category_id', filters.categoryId);
	if (filters.audience) params.set('audience', filters.audience);
	if (filters.visibility) params.set('visibility', filters.visibility);
	if (filters.q) params.set('q', filters.q);
	const query = params.toString();
	return query ? `?${query}` : '';
}

function publicCalendarQuery(filters: CalendarPublicEventFilters = {}) {
	const params = new URLSearchParams();
	if (filters.from) params.set('from', filters.from);
	if (filters.to) params.set('to', filters.to);
	if (filters.categoryId) params.set('category_id', filters.categoryId);
	if (filters.q) params.set('q', filters.q);
	const query = params.toString();
	return query ? `?${query}` : '';
}

export async function listCalendarEvents(
	filters: CalendarEventFilters = {}
): Promise<CalendarEvent[]> {
	const response = await apiClient.get<CalendarEvent[]>(
		`/api/calendar/events${calendarQuery(filters)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดกิจกรรมปฏิทินได้');
}

export async function listMyCalendarEvents(
	filters: CalendarEventFilters = {}
): Promise<CalendarEvent[]> {
	const response = await apiClient.get<CalendarEvent[]>(
		`/api/me/calendar/events${calendarQuery(filters)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินของฉันได้');
}

export async function listChildCalendarEvents(
	studentId: string,
	filters: CalendarEventFilters = {}
): Promise<CalendarEvent[]> {
	const response = await apiClient.get<CalendarEvent[]>(
		`/api/parent/students/${encodeURIComponent(studentId)}/calendar/events${calendarQuery(filters)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินนักเรียนได้');
}

export async function listPublicCalendarEvents(
	filters: CalendarPublicEventFilters = {}
): Promise<CalendarPublicEvent[]> {
	const response = await apiClient.get<CalendarPublicEvent[]>(
		`/api/public/calendar/events${publicCalendarQuery(filters)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินสาธารณะได้');
}

export async function createCalendarEvent(
	payload: CreateCalendarEventRequest
): Promise<CalendarEvent> {
	const response = await apiClient.post<CalendarEvent>('/api/calendar/events', payload);
	return requireApiData(response, 'ไม่สามารถสร้างกิจกรรมปฏิทินได้');
}

export async function updateCalendarEvent(
	id: string,
	payload: UpdateCalendarEventRequest
): Promise<CalendarEvent> {
	const response = await apiClient.put<CalendarEvent>(
		`/api/calendar/events/${encodeURIComponent(id)}`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถบันทึกกิจกรรมปฏิทินได้');
}

export async function deleteCalendarEvent(id: string): Promise<Record<string, never>> {
	const response = await apiClient.delete<Record<string, never>>(
		`/api/calendar/events/${encodeURIComponent(id)}`
	);
	return requireApiData(response, 'ไม่สามารถลบกิจกรรมปฏิทินได้');
}

export async function listCalendarCategories(): Promise<CalendarCategory[]> {
	const response = await apiClient.get<CalendarCategory[]>('/api/calendar/categories');
	return requireApiData(response, 'ไม่สามารถโหลดหมวดหมู่ปฏิทินได้');
}

export async function createCalendarCategory(
	payload: UpsertCalendarCategoryRequest
): Promise<CalendarCategory> {
	const response = await apiClient.post<CalendarCategory>('/api/calendar/categories', payload);
	return requireApiData(response, 'ไม่สามารถสร้างหมวดหมู่ปฏิทินได้');
}

export async function updateCalendarCategory(
	id: string,
	payload: UpsertCalendarCategoryRequest
): Promise<CalendarCategory> {
	const response = await apiClient.put<CalendarCategory>(
		`/api/calendar/categories/${encodeURIComponent(id)}`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถบันทึกหมวดหมู่ปฏิทินได้');
}

export async function deleteCalendarCategory(id: string): Promise<Record<string, never>> {
	const response = await apiClient.delete<Record<string, never>>(
		`/api/calendar/categories/${encodeURIComponent(id)}`
	);
	return requireApiData(response, 'ไม่สามารถลบหมวดหมู่ปฏิทินได้');
}
