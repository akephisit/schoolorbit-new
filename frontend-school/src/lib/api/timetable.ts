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
export type TimetableVersion = Schemas['TimetableVersion'];
export type TimetableVersionStatus = Schemas['TimetableVersionStatus'];
export type TimetableVersionTarget = Schemas['TimetableVersionTarget'];
export type CloneTimetableVersionRequest =
	operations['cloneTimetableVersion']['requestBody']['content']['application/json'];
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
type ListTimetableVersionsQuery = NonNullable<
	operations['listTimetableVersions']['parameters']['query']
>;
type ResolveTimetableVersionQuery = NonNullable<
	operations['resolveTimetableVersion']['parameters']['query']
>;
type DeleteTimetableEntryQuery = NonNullable<
	operations['deleteTimetableEntry']['parameters']['query']
>;
type DeleteTimetableBatchQuery = NonNullable<
	operations['deleteTimetableBatch']['parameters']['query']
>;
type TimetableOccupancyQuery = NonNullable<
	operations['getTimetableOccupancy']['parameters']['query']
>;

export interface TimetablePeriodSummary {
	id: string;
	name: string | null;
	startTime: string;
	endTime: string;
}

export function currentLocalDate(date = new Date()): string {
	const year = date.getFullYear();
	const month = String(date.getMonth() + 1).padStart(2, '0');
	const day = String(date.getDate()).padStart(2, '0');
	return `${year}-${month}-${day}`;
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

function requiredVersion(timetableVersionId: string): string {
	const selected = timetableVersionId.trim();
	if (!selected) throw new Error('กรุณาเลือกรุ่นตารางสอนก่อน');
	return selected;
}

function requiredDate(date: string): string {
	const selected = date.trim();
	if (!selected) throw new Error('กรุณาเลือกวันที่ก่อน');
	return selected;
}

function timetableQuery(filters: TimetableFilters): string {
	const params = new URLSearchParams({
		academicTermId: requiredTerm(filters.academicTermId),
		timetableVersionId: requiredVersion(filters.timetableVersionId)
	});
	if (filters.learningGroupId) params.set('learningGroupId', filters.learningGroupId);
	if (filters.homeroomId) params.set('homeroomId', filters.homeroomId);
	if (filters.instructorId) params.set('instructorId', filters.instructorId);
	if (filters.roomId) params.set('roomId', filters.roomId);
	if (filters.dayOfWeek) params.set('dayOfWeek', filters.dayOfWeek);
	if (filters.entryType) params.set('entryType', filters.entryType);
	return `?${params.toString()}`;
}

function personalTimetableQuery(filters: MyTimetableFilters): string {
	const params = new URLSearchParams({
		academicTermId: requiredTerm(filters.academicTermId),
		date: requiredDate(filters.date)
	});
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

export const getMyTimetable = (filters: MyTimetableFilters, options: ApiRequestOptions = {}) =>
	timetableData(
		apiClient.get<TimetableEntry[]>(`/api/me/timetable${personalTimetableQuery(filters)}`, options),
		'ไม่สามารถโหลดตารางสอนของฉันได้'
	);

export const listTimetableVersions = (academicTermId: string, options: ApiRequestOptions = {}) => {
	const query = {
		academicTermId: requiredTerm(academicTermId)
	} satisfies ListTimetableVersionsQuery;
	return timetableData(
		apiClient.get<TimetableVersion[]>('/api/academic/timetable-versions', { ...options, query }),
		'ไม่สามารถโหลดรุ่นตารางสอนได้'
	);
};

export const resolveTimetableVersion = (
	academicTermId: string,
	date: string,
	options: ApiRequestOptions = {}
) => {
	const query = {
		academicTermId: requiredTerm(academicTermId),
		date: requiredDate(date)
	} satisfies ResolveTimetableVersionQuery;
	return timetableData(
		apiClient.get<TimetableVersion>('/api/academic/timetable-versions/resolve', {
			...options,
			query
		}),
		'ไม่พบรุ่นตารางสอนที่ใช้ในวันที่เลือก'
	);
};

export const cloneTimetableVersion = (sourceId: string, body: CloneTimetableVersionRequest) =>
	timetableData(
		apiClient.post<TimetableVersion>(
			`/api/academic/timetable-versions/${encodeURIComponent(sourceId)}/clone`,
			body
		),
		'สร้างแบบร่างตารางสอนไม่สำเร็จ'
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

export const deleteTimetableEntry = (
	id: string,
	rowVersion: number,
	timetableVersionId: string
) => {
	const query = {
		rowVersion,
		timetableVersionId: requiredVersion(timetableVersionId)
	} satisfies DeleteTimetableEntryQuery;
	const params = new URLSearchParams({
		rowVersion: String(query.rowVersion),
		timetableVersionId: query.timetableVersionId
	});
	return timetableData(
		apiClient.delete<TimetableEntry>(
			`/api/academic/timetable/${encodeURIComponent(id)}?${params.toString()}`
		),
		'ลบคาบในตารางไม่สำเร็จ'
	);
};

export const deleteTimetableBatch = (batchId: string, timetableVersionId: string) => {
	const query = {
		timetableVersionId: requiredVersion(timetableVersionId)
	} satisfies DeleteTimetableBatchQuery;
	const params = new URLSearchParams(query);
	return timetableData(
		apiClient.delete<TimetableEntry[]>(
			`/api/academic/timetable/batch-group/${encodeURIComponent(batchId)}?${params.toString()}`
		),
		'ลบชุดคาบในตารางไม่สำเร็จ'
	);
};

export const swapTimetableEntries = (body: SwapTimetableEntriesRequest) =>
	timetableData(
		apiClient.post<SwapTimetableEntriesResponse>('/api/academic/timetable/swap', body),
		'สลับคาบไม่สำเร็จ'
	);

export const validateTimetableMoves = (
	academicTermId: string,
	timetableVersionId: string,
	entryId: string
) =>
	timetableData(
		apiClient.post<MoveValidityCell[]>('/api/academic/timetable/validate-moves', {
			academicTermId: requiredTerm(academicTermId),
			timetableVersionId: requiredVersion(timetableVersionId),
			entryId
		}),
		'ตรวจสอบตำแหน่งย้ายคาบไม่สำเร็จ'
	);

export const getTimetableOccupancy = (
	academicTermId: string,
	timetableVersionId: string,
	options: ApiRequestOptions = {}
) => {
	const query = {
		academicTermId: requiredTerm(academicTermId),
		timetableVersionId: requiredVersion(timetableVersionId)
	} satisfies TimetableOccupancyQuery;
	const params = new URLSearchParams(query);
	return timetableData(
		apiClient.get<TimetableOccupancyCell[]>(
			`/api/academic/timetable/occupancy?${params.toString()}`,
			options
		),
		'โหลดข้อมูลการใช้คาบไม่สำเร็จ'
	);
};

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
