import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components, operations } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];

export type CalendarAudienceType = Schemas['CalendarAudienceType'];

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

export type CalendarEventTargetInput = Schemas['CalendarEventTargetInput'];

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

export type CalendarEventFilters = NonNullable<
	operations['listCalendarEvents']['parameters']['query']
>;
type MyCalendarQuery = NonNullable<operations['listMyCalendarEvents']['parameters']['query']>;
type ChildCalendarQuery = NonNullable<
	operations['getParentChildCalendarEvents']['parameters']['query']
>;
type GeneratedPublicCalendarQuery = NonNullable<
	operations['listPublicCalendarEvents']['parameters']['query']
>;
export type CalendarPublicEventFilters = Omit<
	GeneratedPublicCalendarQuery,
	'audience' | 'visibility'
>;

export type CreateCalendarEventRequest = Schemas['UpsertCalendarEventRequest'];

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

export async function listCalendarEvents(
	filters: CalendarEventFilters,
	options: ApiRequestOptions = {}
): Promise<CalendarEvent[]> {
	const response = await apiClient.get<CalendarEventDto[]>('/api/calendar/events', {
		...options,
		query: { ...filters }
	});
	return requireApiData(response, 'ไม่สามารถโหลดกิจกรรมปฏิทินได้').map(calendarEventFromDto);
}

export async function listMyCalendarEvents(
	filters: MyCalendarQuery,
	options: ApiRequestOptions = {}
): Promise<CalendarViewerEvent[]> {
	const response = await apiClient.get<CalendarViewerEvent[]>('/api/me/calendar/events', {
		...options,
		query: { ...filters } satisfies MyCalendarQuery
	});
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินของฉันได้');
}

export async function listChildCalendarEvents(
	studentId: string,
	filters: ChildCalendarQuery,
	options: ApiRequestOptions = {}
): Promise<CalendarViewerEvent[]> {
	const response = await apiClient.get<CalendarViewerEvent[]>(
		`/api/parent/students/${encodeURIComponent(studentId)}/calendar/events`,
		{ ...options, query: { ...filters } satisfies ChildCalendarQuery }
	);
	return requireApiData(response, 'ไม่สามารถโหลดปฏิทินนักเรียนได้');
}

export async function listPublicCalendarEvents(
	filters: CalendarPublicEventFilters
): Promise<CalendarPublicEvent[]> {
	const response = await apiClient.get<CalendarPublicEvent[]>('/api/public/calendar/events', {
		query: { ...filters } satisfies GeneratedPublicCalendarQuery
	});
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

export async function listCalendarCategories(
	options: ApiRequestOptions = {}
): Promise<CalendarCategory[]> {
	const response = await apiClient.get<CalendarCategory[]>('/api/calendar/categories', options);
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

export async function listCalendarTags(options: ApiRequestOptions = {}): Promise<CalendarTag[]> {
	const response = await apiClient.get<CalendarTag[]>('/api/calendar/tags', options);
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

export type CreateCalendarRequest = Schemas['CreateCalendarRequest'];
export type CalendarEventRequest = Schemas['CalendarEventRequest'];
export type CalendarRequestStatus = Schemas['CalendarRequestStatus'];
export type CalendarRequestPage = Schemas['CalendarRequestPage'];
export type CalendarRequestQuery = NonNullable<
	operations['listCalendarRequests']['parameters']['query']
>;
export type CalendarTargetOptions = Schemas['CalendarTargetOptions'];
export type PendingCalendarRequest = Schemas['PendingCalendarRequest'];
export type PendingCalendarPage = Schemas['PendingCalendarPage'];
export type PendingCalendarQuery = NonNullable<
	operations['listPendingCalendarRequests']['parameters']['query']
>;

export async function listPendingCalendarRequests(
	query: PendingCalendarQuery,
	options: ApiRequestOptions = {}
): Promise<PendingCalendarPage> {
	return requireApiData(
		await apiClient.get<PendingCalendarPage>('/api/calendar/requests/calendar', {
			...options,
			query
		}),
		'โหลดคำร้องรออนุมัติไม่สำเร็จ'
	);
}

export async function createCalendarRequest(
	payload: CreateCalendarRequest
): Promise<CalendarEventRequest> {
	return requireApiData(
		await apiClient.post<CalendarEventRequest>('/api/calendar/requests', payload),
		'ส่งคำร้องไม่สำเร็จ'
	);
}
export async function listCalendarRequests(
	query: CalendarRequestQuery,
	options: ApiRequestOptions = {}
): Promise<CalendarRequestPage> {
	return requireApiData(
		await apiClient.get<CalendarRequestPage>('/api/calendar/requests', { ...options, query }),
		'โหลดคำร้องไม่สำเร็จ'
	);
}
export async function approveCalendarRequest(
	id: string,
	payload: CreateCalendarEventRequest
): Promise<Schemas['CalendarRequestApproval']> {
	return requireApiData(
		await apiClient.post<Schemas['CalendarRequestApproval']>(
			`/api/calendar/requests/${encodeURIComponent(id)}/approve`,
			payload
		),
		'อนุมัติคำร้องไม่สำเร็จ'
	);
}
export async function rejectCalendarRequest(
	id: string,
	reason: string
): Promise<CalendarEventRequest> {
	return requireApiData(
		await apiClient.post<CalendarEventRequest>(
			`/api/calendar/requests/${encodeURIComponent(id)}/reject`,
			{ reason } satisfies Schemas['RejectCalendarRequest']
		),
		'ไม่สามารถบันทึกผลคำร้องได้'
	);
}
export async function listCalendarTargetOptions(
	date: string,
	options: ApiRequestOptions = {}
): Promise<CalendarTargetOptions> {
	return requireApiData(
		await apiClient.get<CalendarTargetOptions>('/api/calendar/target-options', {
			...options,
			query: { date }
		}),
		'โหลดตัวเลือกชั้นเรียนไม่สำเร็จ'
	);
}
