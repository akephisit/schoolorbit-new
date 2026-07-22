import { apiClient, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type CalendarAudienceType = 'all' | 'staff' | 'student' | 'parent';

export type CalendarCategory = Schemas['CalendarCategory'];
export type CalendarTag = Schemas['CalendarTag'];
export type CalendarEventTag = Schemas['CalendarEventTag'];
export type CalendarEventTargetDto = Schemas['CalendarEventTarget'];
export type CalendarEventReminderDto = Schemas['CalendarEventReminder'];
export type CalendarEventDto = Schemas['CalendarEvent'];
export type CalendarPublicEvent = Schemas['CalendarPublicEvent'];

export type CalendarEventTarget = Omit<CalendarEventTargetDto, 'audienceType'> & {
	audienceType: CalendarAudienceType;
};
export type CalendarEventReminder = CalendarEventReminderDto;
export type CalendarEvent = Omit<CalendarEventDto, 'targets'> & {
	targets: CalendarEventTarget[];
};

export interface CalendarEventTargetInput {
	audienceType: CalendarAudienceType;
	gradeLevelId?: string | null;
	classRoomId?: string | null;
}

export type CalendarViewerEvent = Schemas['CalendarViewerEvent'];

const CALENDAR_AUDIENCES = new Set<CalendarAudienceType>(['all', 'staff', 'student', 'parent']);

function calendarEventFromDto(dto: CalendarEventDto): CalendarEvent {
	const targets = dto.targets.map((target): CalendarEventTarget => {
		if (!CALENDAR_AUDIENCES.has(target.audienceType as CalendarAudienceType)) {
			throw new Error(`Unsupported calendar audience: ${target.audienceType}`);
		}

		return {
			...target,
			audienceType: target.audienceType as CalendarAudienceType
		};
	});

	return { ...dto, targets };
}

interface CalendarEventBaseFilters {
	from?: string;
	to?: string;
	categoryId?: string;
	tagId?: string;
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
	tagId?: string;
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
	tagIds: string[];
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

export interface UpsertCalendarTagRequest {
	name: string;
}

function calendarQuery(filters: CalendarEventFilters = {}) {
	const params = new URLSearchParams();
	if (filters.from) params.set('from', filters.from);
	if (filters.to) params.set('to', filters.to);
	if (filters.categoryId) params.set('category_id', filters.categoryId);
	if (filters.tagId) params.set('tag_id', filters.tagId);
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
	if (filters.tagId) params.set('tag_id', filters.tagId);
	if (filters.q) params.set('q', filters.q);
	const query = params.toString();
	return query ? `?${query}` : '';
}

export async function listCalendarEvents(
	filters: CalendarEventFilters = {}
): Promise<CalendarEvent[]> {
	const response = await apiClient.get<CalendarEventDto[]>(
		`/api/calendar/events${calendarQuery(filters)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดกิจกรรมปฏิทินได้').map(calendarEventFromDto);
}

export async function listMyCalendarEvents(
	filters: CalendarEventFilters = {}
): Promise<CalendarViewerEvent[]> {
	const response = await apiClient.get<CalendarViewerEvent[]>(
		`/api/me/calendar/events${calendarQuery(filters)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินของฉันได้');
}

export async function listChildCalendarEvents(
	studentId: string,
	filters: CalendarEventFilters = {}
): Promise<CalendarViewerEvent[]> {
	const response = await apiClient.get<CalendarViewerEvent[]>(
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
	const response = await apiClient.post<CalendarEventDto>('/api/calendar/events', payload);
	return calendarEventFromDto(requireApiData(response, 'ไม่สามารถสร้างกิจกรรมปฏิทินได้'));
}

export async function updateCalendarEvent(
	id: string,
	payload: UpdateCalendarEventRequest
): Promise<CalendarEvent> {
	const response = await apiClient.put<CalendarEventDto>(
		`/api/calendar/events/${encodeURIComponent(id)}`,
		payload
	);
	return calendarEventFromDto(requireApiData(response, 'ไม่สามารถบันทึกกิจกรรมปฏิทินได้'));
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

export async function listCalendarTags(): Promise<CalendarTag[]> {
	const response = await apiClient.get<CalendarTag[]>('/api/calendar/tags');
	return requireApiData(response, 'ไม่สามารถโหลดแท็กปฏิทินได้');
}

export async function createCalendarTag(payload: UpsertCalendarTagRequest): Promise<CalendarTag> {
	const response = await apiClient.post<CalendarTag>('/api/calendar/tags', payload);
	return requireApiData(response, 'ไม่สามารถสร้างแท็กปฏิทินได้');
}

export async function updateCalendarTag(
	id: string,
	payload: UpsertCalendarTagRequest
): Promise<CalendarTag> {
	const response = await apiClient.put<CalendarTag>(
		`/api/calendar/tags/${encodeURIComponent(id)}`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถบันทึกแท็กปฏิทินได้');
}

export async function deleteCalendarTag(id: string): Promise<Record<string, never>> {
	const response = await apiClient.delete<Record<string, never>>(
		`/api/calendar/tags/${encodeURIComponent(id)}`
	);
	return requireApiData(response, 'ไม่สามารถลบแท็กปฏิทินได้');
}
