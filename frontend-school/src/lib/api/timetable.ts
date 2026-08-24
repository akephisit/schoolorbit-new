import {
	ApiClientError,
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type TimetableEntry = Schemas['TimetableEntry'];
export type TimetableInstructor = Schemas['TimetableInstructor'];
export type CreateTimetableEntryRequest = Schemas['CreateTimetableEntryRequest'];
export type UpdateTimetableEntryRequest = Schemas['UpdateTimetableEntryRequest'];
export type CreateBatchTimetableEntriesRequest = Schemas['CreateBatchTimetableEntriesRequest'];
export type BatchTimetableResult = Schemas['BatchTimetableResult'];
export type SwapTimetableEntriesRequest = Schemas['SwapTimetableEntriesRequest'];
export type SwapTimetableEntriesResponse = Schemas['SwapTimetableEntriesResponse'];
export type MoveValidityCell = Schemas['MoveValidityCell'];
export type TimetableOccupancyCell = Schemas['TimetableOccupancyCell'];
export type TimetableTemplate = Schemas['TimetableTemplate'];
export type TimetableTemplateEntry = Schemas['TimetableTemplateEntry'];
export type TemplateWithEntries = Schemas['TemplateWithEntries'];
export type CreateTemplateRequest = Schemas['CreateTemplateRequest'];
export type UpdateTemplateRequest = Schemas['UpdateTemplateRequest'];
export type FromCurrentRequest = Schemas['FromCurrentRequest'];
export type ApplyTemplateRequest = Schemas['ApplyTemplateRequest'];
export type ClearTimetableRequest = Schemas['ClearTimetableRequest'];
export type TemplateApplyResult = Schemas['TemplateApplyResult'];
export type DailyTeachingPeriod = Schemas['DailyTeachingPeriod'];
export type DailyTeachingEntry = Schemas['DailyTeachingEntry'];
export type DailyTeachingPeriodCell = Schemas['DailyTeachingPeriodCell'];
export type DailyTeachingTeacher = Schemas['DailyTeachingTeacher'];
export type DailyTeachingSummary = Schemas['DailyTeachingSummary'];
export type DailyTeachingOverview = Schemas['DailyTeachingOverview'];
export type TimetableFilters = operations['listTimetableEntries']['parameters']['query'];
export type MyTimetableFilters = operations['getMyTimetable']['parameters']['query'];
export type DailyTeachingFilters = operations['getDailyTeachingOverview']['parameters']['query'];

export interface TimetablePeriodSummary {
	id: string;
	name: string | null;
	startTime: string;
	endTime: string;
}

async function timetableData<T>(request: Promise<ApiResponse<T>>, fallback: string): Promise<T> {
	const response = await request;
	if (response.status === 409) {
		throw new ApiClientError(
			`${response.error || fallback} กรุณาโหลดตารางล่าสุดก่อนดำเนินการอีกครั้ง`,
			409
		);
	}
	return requireApiData(response, fallback);
}

function requiredTerm(academicTermId: string): string {
	const selected = academicTermId.trim();
	if (!selected) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return selected;
}

function timetableQuery(filters: TimetableFilters): string {
	const params = new URLSearchParams({ academicTermId: requiredTerm(filters.academicTermId) });
	if (filters.learningGroupId) params.set('learningGroupId', filters.learningGroupId);
	if (filters.homeroomId) params.set('homeroomId', filters.homeroomId);
	if (filters.instructorId) params.set('instructorId', filters.instructorId);
	if (filters.roomId) params.set('roomId', filters.roomId);
	if (filters.dayOfWeek) params.set('dayOfWeek', filters.dayOfWeek);
	if (filters.entryType) params.set('entryType', filters.entryType);
	return `?${params.toString()}`;
}

export function periodsFromTimetableEntries(entries: TimetableEntry[]): TimetablePeriodSummary[] {
	const periods = new Map<string, TimetablePeriodSummary>();
	for (const entry of entries) {
		if (!periods.has(entry.bellSchedulePeriodId)) {
			periods.set(entry.bellSchedulePeriodId, {
				id: entry.bellSchedulePeriodId,
				name: entry.periodName ?? null,
				startTime: entry.startTime,
				endTime: entry.endTime
			});
		}
	}
	return [...periods.values()].sort(
		(a, b) =>
			a.startTime.localeCompare(b.startTime) ||
			a.endTime.localeCompare(b.endTime) ||
			(a.name ?? '').localeCompare(b.name ?? '', 'th')
	);
}

export const listTimetableEntries = (filters: TimetableFilters, options: ApiRequestOptions = {}) =>
	timetableData(
		apiClient.get<TimetableEntry[]>(`/api/academic/timetable${timetableQuery(filters)}`, options),
		'ไม่สามารถโหลดตารางสอนได้'
	);

export const getMyTimetable = (filters: MyTimetableFilters) =>
	timetableData(
		apiClient.get<TimetableEntry[]>(`/api/me/timetable${timetableQuery(filters)}`),
		'ไม่สามารถโหลดตารางสอนของฉันได้'
	);

export const createTimetableEntry = (body: CreateTimetableEntryRequest) =>
	timetableData(
		apiClient.post<TimetableEntry>('/api/academic/timetable', body),
		'สร้างคาบในตารางไม่สำเร็จ'
	);

export const createBatchTimetableEntries = (body: CreateBatchTimetableEntriesRequest) =>
	timetableData(
		apiClient.post<BatchTimetableResult>('/api/academic/timetable/batch', body),
		'สร้างคาบแบบกลุ่มไม่สำเร็จ'
	);

export const updateTimetableEntry = (id: string, body: UpdateTimetableEntryRequest) =>
	timetableData(
		apiClient.put<TimetableEntry>(`/api/academic/timetable/${encodeURIComponent(id)}`, body),
		'แก้ไขคาบในตารางไม่สำเร็จ'
	);

export const deleteTimetableEntry = (id: string, rowVersion: number) =>
	timetableData(
		apiClient.delete<TimetableEntry>(
			`/api/academic/timetable/${encodeURIComponent(id)}?rowVersion=${rowVersion}`
		),
		'ลบคาบในตารางไม่สำเร็จ'
	);

export const deleteTimetableBatch = (batchId: string) =>
	timetableData(
		apiClient.delete<TimetableEntry[]>(
			`/api/academic/timetable/batch-group/${encodeURIComponent(batchId)}`
		),
		'ลบชุดคาบในตารางไม่สำเร็จ'
	);

export const swapTimetableEntries = (body: SwapTimetableEntriesRequest) =>
	timetableData(
		apiClient.post<SwapTimetableEntriesResponse>('/api/academic/timetable/swap', body),
		'สลับคาบไม่สำเร็จ'
	);

export const validateTimetableMoves = (academicTermId: string, entryId: string) =>
	timetableData(
		apiClient.post<MoveValidityCell[]>('/api/academic/timetable/validate-moves', {
			academicTermId: requiredTerm(academicTermId),
			entryId
		}),
		'ตรวจสอบตำแหน่งย้ายคาบไม่สำเร็จ'
	);

export const getTimetableOccupancy = (academicTermId: string, options: ApiRequestOptions = {}) =>
	timetableData(
		apiClient.get<TimetableOccupancyCell[]>(
			`/api/academic/timetable/occupancy?academicTermId=${encodeURIComponent(requiredTerm(academicTermId))}`,
			options
		),
		'โหลดข้อมูลการใช้คาบไม่สำเร็จ'
	);

export const getDailyTeachingOverview = (filters: DailyTeachingFilters) => {
	const params = new URLSearchParams({ academicTermId: requiredTerm(filters.academicTermId) });
	if (filters.date) params.set('date', filters.date);
	if (filters.includeEmptyTeachers !== undefined) {
		params.set('includeEmptyTeachers', String(filters.includeEmptyTeachers));
	}
	return timetableData(
		apiClient.get<DailyTeachingOverview>(
			`/api/academic/timetable/daily-teaching?${params.toString()}`
		),
		'ไม่สามารถโหลดภาพรวมการสอนประจำวันได้'
	);
};

export const listTimetableTemplates = () =>
	timetableData(
		apiClient.get<TimetableTemplate[]>('/api/academic/timetable-templates'),
		'ไม่สามารถโหลดแม่แบบตารางสอนได้'
	);
export const getTimetableTemplate = (id: string) =>
	timetableData(
		apiClient.get<TemplateWithEntries>(
			`/api/academic/timetable-templates/${encodeURIComponent(id)}`
		),
		'ไม่สามารถโหลดแม่แบบตารางสอนได้'
	);
export const createTimetableTemplate = (body: CreateTemplateRequest) =>
	timetableData(
		apiClient.post<TimetableTemplate>('/api/academic/timetable-templates', body),
		'สร้างแม่แบบตารางสอนไม่สำเร็จ'
	);
export const updateTimetableTemplate = (id: string, body: UpdateTemplateRequest) =>
	timetableData(
		apiClient.put<TimetableTemplate>(
			`/api/academic/timetable-templates/${encodeURIComponent(id)}`,
			body
		),
		'แก้ไขแม่แบบตารางสอนไม่สำเร็จ'
	);
export const deleteTimetableTemplate = (id: string) =>
	timetableData(
		apiClient.delete<Schemas['EmptyData']>(
			`/api/academic/timetable-templates/${encodeURIComponent(id)}`
		),
		'ลบแม่แบบตารางสอนไม่สำเร็จ'
	);
export const createTimetableTemplateFromCurrent = (body: FromCurrentRequest) =>
	timetableData(
		apiClient.post<TimetableTemplate>('/api/academic/timetable-templates/from-current', body),
		'สร้างแม่แบบจากตารางปัจจุบันไม่สำเร็จ'
	);
export const applyTimetableTemplate = (id: string, body: ApplyTemplateRequest) =>
	timetableData(
		apiClient.post<TemplateApplyResult>(
			`/api/academic/timetable-templates/${encodeURIComponent(id)}/apply`,
			body
		),
		'นำแม่แบบไปใช้ไม่สำเร็จ'
	);
export const clearTimetable = (body: ClearTimetableRequest) =>
	timetableData(
		apiClient.deleteWithBody<TimetableEntry[]>('/api/academic/timetable/clear', body),
		'ล้างตารางสอนไม่สำเร็จ'
	);
