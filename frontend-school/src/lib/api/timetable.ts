import {
	ApiClientError,
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type TimetableBlock = Schemas['TimetableBlock'];
export type TimetableBlockKind = Schemas['TimetableBlockKind'];
export type TimetableBlockGroup = Schemas['TimetableBlockGroup'];
export type TimetableBlockHomeroom = Schemas['TimetableBlockHomeroom'];
export type TimetableBlockTeacher = Schemas['TimetableBlockTeacher'];
export type TimetableBlockInstructor = Schemas['TimetableBlockInstructor'];
export type TimetableBlockSyncState = Schemas['TimetableBlockSyncState'];
export type TimetableBlockSyncStatus = Schemas['TimetableBlockSyncStatus'];
export type TimetableStructuralKind = Schemas['TimetableStructuralKind'];
export type TimetableTargetKind = Schemas['TimetableTargetKind'];
export type TimetableVersion = Schemas['TimetableVersion'];
export type TimetableVersionStatus = Schemas['TimetableVersionStatus'];
export type TimetableVersionTarget = Schemas['TimetableVersionTarget'];
export type TimetableBlockWorkspace = Schemas['TimetableBlockWorkspace'];
export type TimetableBlockWorkspaceLearningGroup = Schemas['TimetableBlockWorkspaceLearningGroup'];
export type TimetableBlockWorkspaceHomeroom = Schemas['TimetableBlockWorkspaceHomeroom'];
export type TimetableBlockWorkspaceRoom = Schemas['TimetableBlockWorkspaceRoom'];
export type TimetableBlockWorkspaceStaff = Schemas['TimetableBlockWorkspaceStaff'];
export type TimetableOrdinaryDemand = Schemas['TimetableOrdinaryDemand'];
export type TimetableSynchronizedDemand = Schemas['TimetableSynchronizedDemand'];
export type TimetableBlockSummary = Schemas['TimetableBlockSummary'];
export type TimetableBlockPlacementState = Schemas['TimetableBlockPlacementState'];
export type TimetableBlockConflictType = Schemas['TimetableBlockConflictType'];
export type TimetableBlockMutationKind = Schemas['TimetableBlockMutationKind'];
export type TimetableBlockPlacementSource = Schemas['TimetableBlockPlacementSource'];
export type TimetableBlockPlacementCandidate = Schemas['TimetableBlockPlacementCandidate'];
export type TimetableBlockPlacementPreviewRequest =
	Schemas['TimetableBlockPlacementPreviewRequest'];
export type TimetableBlockPlacementPreview = Schemas['TimetableBlockPlacementPreview'];
export type CloneTimetableVersionRequest =
	operations['cloneTimetableVersion']['requestBody']['content']['application/json'];
export type CreateOrdinaryTimetableBlockRequest = Schemas['CreateOrdinaryTimetableBlockRequest'];
export type CreateSynchronizedTimetableBlockRequest =
	Schemas['CreateSynchronizedTimetableBlockRequest'];
export type CreateStructuralTimetableBlocksRequest =
	Schemas['CreateStructuralTimetableBlocksRequest'];
export type UpdateTimetableBlockRequest = Schemas['UpdateTimetableBlockRequest'];
export type RemoveTimetableBlockTargetRequest = Schemas['RemoveTimetableBlockTargetRequest'];
export type RetryTimetableBlockSyncRequest = Schemas['RetryTimetableBlockSyncRequest'];
export type RestoreTimetableBlockGroupRequest = Schemas['RestoreTimetableBlockGroupRequest'];
export type SwapTimetableBlocksRequest = Schemas['SwapTimetableBlocksRequest'];
export type SwapTimetableBlocksResponse = Schemas['SwapTimetableBlocksResponse'];
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
export type MyTimetableFilters = operations['getMyTimetable']['parameters']['query'];
export type DailyTeachingFilters = operations['getDailyTeachingOverview']['parameters']['query'];
type ListTimetableVersionsQuery = NonNullable<
	operations['listTimetableVersions']['parameters']['query']
>;
type ResolveTimetableVersionQuery = NonNullable<
	operations['resolveTimetableVersion']['parameters']['query']
>;
type DeleteTimetableBlockQuery = NonNullable<
	operations['deleteTimetableBlock']['parameters']['query']
>;
type DeleteTimetableBlockSeriesQuery = NonNullable<
	operations['deleteTimetableBlockSeries']['parameters']['query']
>;
export type TimetableBlockWorkspaceQuery = NonNullable<
	operations['getTimetableBlockWorkspace']['parameters']['query']
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

function personalTimetableQuery(filters: MyTimetableFilters): string {
	const params = new URLSearchParams({
		academicTermId: requiredTerm(filters.academicTermId),
		date: requiredDate(filters.date)
	});
	return `?${params.toString()}`;
}

export function periodsFromTimetableBlocks(blocks: TimetableBlock[]): TimetablePeriodSummary[] {
	const periods = new Map<string, TimetablePeriodSummary>();
	for (const block of blocks) {
		if (!periods.has(block.bellSchedulePeriodId)) {
			periods.set(block.bellSchedulePeriodId, {
				id: block.bellSchedulePeriodId,
				name: block.periodName,
				startTime: block.startTime,
				endTime: block.endTime
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

export const getTimetableBlockWorkspace = (
	query: TimetableBlockWorkspaceQuery,
	options: ApiRequestOptions = {}
) =>
	timetableData(
		apiClient.get<TimetableBlockWorkspace>('/api/academic/timetable-blocks/workspace', {
			...options,
			query
		}),
		'ไม่สามารถโหลดพื้นที่จัดตารางสอนได้'
	);

export const getMyTimetable = (filters: MyTimetableFilters, options: ApiRequestOptions = {}) =>
	timetableData(
		apiClient.get<TimetableBlock[]>(`/api/me/timetable${personalTimetableQuery(filters)}`, options),
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

export const createOrdinaryTimetableBlock = (body: CreateOrdinaryTimetableBlockRequest) =>
	timetableData(
		apiClient.post<TimetableBlock>('/api/academic/timetable-blocks/ordinary', body),
		'สร้างคาบในตารางไม่สำเร็จ'
	);

export const createSynchronizedTimetableBlock = (body: CreateSynchronizedTimetableBlockRequest) =>
	timetableData(
		apiClient.post<TimetableBlock>('/api/academic/timetable-blocks/synchronized', body),
		'สร้างช่วงกิจกรรมพร้อมกันไม่สำเร็จ'
	);

export const createStructuralTimetableBlocks = (body: CreateStructuralTimetableBlocksRequest) =>
	timetableData(
		apiClient.post<TimetableBlock[]>('/api/academic/timetable-blocks/structural', body),
		'เพิ่มคาบพิเศษไม่สำเร็จ'
	);

export const updateTimetableBlock = (id: string, body: UpdateTimetableBlockRequest) =>
	timetableData(
		apiClient.put<TimetableBlock>(`/api/academic/timetable-blocks/${encodeURIComponent(id)}`, body),
		'แก้ไขคาบในตารางไม่สำเร็จ'
	);

export const deleteTimetableBlock = (
	id: string,
	rowVersion: number,
	timetableVersionId: string
) => {
	const query = {
		rowVersion,
		timetableVersionId: requiredVersion(timetableVersionId)
	} satisfies DeleteTimetableBlockQuery;
	const params = new URLSearchParams({
		rowVersion: String(query.rowVersion),
		timetableVersionId: query.timetableVersionId
	});
	return timetableData(
		apiClient.delete<TimetableBlock>(
			`/api/academic/timetable-blocks/${encodeURIComponent(id)}?${params.toString()}`
		),
		'ลบคาบในตารางไม่สำเร็จ'
	);
};

export const deleteTimetableBlockSeries = (seriesId: string, timetableVersionId: string) => {
	const query = {
		timetableVersionId: requiredVersion(timetableVersionId)
	} satisfies DeleteTimetableBlockSeriesQuery;
	const params = new URLSearchParams(query);
	return timetableData(
		apiClient.delete<TimetableBlock[]>(
			`/api/academic/timetable-blocks/series/${encodeURIComponent(seriesId)}?${params.toString()}`
		),
		'ลบชุดคาบพิเศษไม่สำเร็จ'
	);
};

export const removeTimetableBlockTarget = (
	blockId: string,
	body: RemoveTimetableBlockTargetRequest
) =>
	timetableData(
		apiClient.deleteWithBody<TimetableBlock>(
			`/api/academic/timetable-blocks/${encodeURIComponent(blockId)}/targets`,
			body
		),
		'นำห้องหรือครูออกจากคาบไม่สำเร็จ'
	);

export const retryTimetableBlockSync = (blockId: string, body: RetryTimetableBlockSyncRequest) =>
	timetableData(
		apiClient.post<TimetableBlock>(
			`/api/academic/timetable-blocks/${encodeURIComponent(blockId)}/sync`,
			body
		),
		'ซิงค์กลุ่มกิจกรรมไม่สำเร็จ'
	);

export const restoreTimetableBlockGroup = (
	blockId: string,
	body: RestoreTimetableBlockGroupRequest
) =>
	timetableData(
		apiClient.post<TimetableBlock>(
			`/api/academic/timetable-blocks/${encodeURIComponent(blockId)}/restore`,
			body
		),
		'คืนกลุ่มกิจกรรมเข้าคาบไม่สำเร็จ'
	);

export const swapTimetableBlocks = (body: SwapTimetableBlocksRequest) =>
	timetableData(
		apiClient.post<SwapTimetableBlocksResponse>('/api/academic/timetable-blocks/swap', body),
		'สลับคาบไม่สำเร็จ'
	);

export const previewTimetableBlockPlacement = (
	body: TimetableBlockPlacementPreviewRequest,
	options: ApiRequestOptions = {}
) =>
	timetableData(
		apiClient.post<TimetableBlockPlacementPreview>(
			'/api/academic/timetable-blocks/placement-preview',
			body,
			options
		),
		'ตรวจสอบตำแหน่งวางคาบไม่สำเร็จ'
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
		apiClient.deleteWithBody<TimetableBlock[]>('/api/academic/timetable-blocks/clear', body),
		'ล้างตารางสอนไม่สำเร็จ'
	);
