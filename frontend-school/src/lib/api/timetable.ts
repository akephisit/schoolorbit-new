import { apiClient, requireApiData, type ApiResponse } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type EmptyResponseData = Record<string, never>;
type LoadedApiResponse<T> = ApiResponse<T> & { success: true; data: T };
type Schemas = components['schemas'];

// Helper for authenticated requests
async function fetchApi<T = EmptyResponseData>(
	path: string,
	options: RequestInit = {}
): Promise<LoadedApiResponse<T>> {
	const method = (options.method || 'GET').toUpperCase();
	const body = options.body ? JSON.parse(options.body.toString()) : undefined;

	let response: ApiResponse<T>;
	if (method === 'POST') {
		response = await apiClient.post<T>(path, body);
	} else if (method === 'PUT') {
		response = await apiClient.put<T>(path, body);
	} else if (method === 'DELETE' && body !== undefined) {
		response = await apiClient.deleteWithBody<T>(path, body);
	} else if (method === 'DELETE') {
		response = await apiClient.delete<T>(path);
	} else {
		response = await apiClient.get<T>(path);
	}

	if (!response.success) throw new Error(response.error || 'Request failed');
	if (response.data === undefined) throw new Error('Response data missing');
	return { success: true, data: response.data, message: response.message };
}

// ============================================
// Types
// ============================================

export interface AcademicPeriod {
	id: string;
	academic_year_id: string;
	name: string | null;
	start_time: string; // "HH:MM:SS"
	end_time: string;
	order_index: number;
	applicable_days?: string;
	is_active: boolean;
	created_at?: string;
	updated_at?: string;
}

export type TimetableEntryDto = Schemas['TimetableEntry'];

type OptimisticTimetableNullableField =
	| 'classroom_course_id'
	| 'room_id'
	| 'note'
	| 'title'
	| 'classroom_id'
	| 'activity_slot_id'
	| 'created_by'
	| 'updated_by'
	| 'day_of_week'
	| 'entry_type';

type TimetableDay = 'MON' | 'TUE' | 'WED' | 'THU' | 'FRI' | 'SAT' | 'SUN';
type TimetableEntryType = 'COURSE' | 'BREAK' | 'ACTIVITY' | 'HOMEROOM' | 'ACADEMIC';

/**
 * UI view model for server entries and optimistic drag/drop previews.
 * The wire payload remains owned by TimetableEntryDto; only values unavailable
 * before server confirmation are relaxed here.
 */
export type TimetableEntry = Omit<TimetableEntryDto, OptimisticTimetableNullableField> & {
	classroom_course_id?: string;
	room_id?: string;
	note?: string;
	title?: string;
	classroom_id?: string;
	activity_slot_id?: string;
	created_by?: string;
	updated_by?: string;
	day_of_week: TimetableDay;
	entry_type: TimetableEntryType;
	subject_name_en?: string;
};

const timetableDays = new Set<TimetableDay>(['MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN']);
const timetableEntryTypes = new Set<TimetableEntryType>([
	'COURSE',
	'BREAK',
	'ACTIVITY',
	'HOMEROOM',
	'ACADEMIC'
]);

function isTimetableDay(value: string): value is TimetableDay {
	return timetableDays.has(value as TimetableDay);
}

function isTimetableEntryType(value: string): value is TimetableEntryType {
	return timetableEntryTypes.has(value as TimetableEntryType);
}

export function timetableEntryFromDto(dto: TimetableEntryDto): TimetableEntry {
	if (!isTimetableDay(dto.day_of_week)) {
		throw new Error(`Unsupported timetable day: ${dto.day_of_week}`);
	}
	if (!isTimetableEntryType(dto.entry_type)) {
		throw new Error(`Unsupported timetable entry type: ${dto.entry_type}`);
	}

	return {
		...dto,
		day_of_week: dto.day_of_week,
		entry_type: dto.entry_type,
		classroom_course_id: dto.classroom_course_id ?? undefined,
		room_id: dto.room_id ?? undefined,
		note: dto.note ?? undefined,
		title: dto.title ?? undefined,
		classroom_id: dto.classroom_id ?? undefined,
		activity_slot_id: dto.activity_slot_id ?? undefined,
		created_by: dto.created_by ?? undefined,
		updated_by: dto.updated_by ?? undefined
	};
}

export interface DailyTeachingPeriod {
	id: string;
	name: string | null;
	startTime: string;
	endTime: string;
	orderIndex: number;
}

export interface DailyTeachingEntry {
	entryId: string;
	entryType: 'COURSE' | 'BREAK' | 'ACTIVITY' | 'HOMEROOM' | 'ACADEMIC';
	subjectCode: string | null;
	subjectName: string | null;
	subjectGroupName: string | null;
	classroomName: string | null;
	roomCode: string | null;
	title: string | null;
	note: string | null;
	isTeamTeaching: boolean;
}

export interface DailyTeachingPeriodCell {
	periodId: string;
	entries: DailyTeachingEntry[];
}

export interface DailyTeachingTeacher {
	id: string;
	displayName: string;
	subjectGroupNames: string[];
	periods: DailyTeachingPeriodCell[];
}

export interface DailyTeachingSummary {
	totalTeacherCount: number;
	displayedTeacherCount: number;
	teachersTeachingCount: number;
	lessonCount: number;
	emptyTeacherCount: number;
}

export interface DailyTeachingOverview {
	date: string;
	dayOfWeek: string;
	academicSemesterId: string;
	periods: DailyTeachingPeriod[];
	teachers: DailyTeachingTeacher[];
	summary: DailyTeachingSummary;
}

export interface TimetablePeriodSummary {
	id: string;
	name: string | null;
	start_time?: string;
	end_time?: string;
}

export function periodsFromTimetableEntries(entries: TimetableEntry[]): TimetablePeriodSummary[] {
	const periods = new Map<string, TimetablePeriodSummary>();

	for (const entry of entries) {
		if (!periods.has(entry.period_id)) {
			periods.set(entry.period_id, {
				id: entry.period_id,
				name: entry.period_name ?? null,
				start_time: entry.start_time,
				end_time: entry.end_time
			});
		}
	}

	return Array.from(periods.values()).sort((a, b) => {
		const timeCompare =
			(a.start_time ?? '').localeCompare(b.start_time ?? '') ||
			(a.end_time ?? '').localeCompare(b.end_time ?? '');
		if (timeCompare !== 0) return timeCompare;
		return (a.name ?? '').localeCompare(b.name ?? '', 'th') || a.id.localeCompare(b.id);
	});
}

export interface CreatePeriodRequest {
	academic_year_id: string;
	/** ตั้งได้ตามต้องการ (เช่น "พักเที่ยง", "โฮมรูม"); ปล่อยว่างก็ได้ */
	name?: string | null;
	start_time: string; // "HH:MM"
	end_time: string;

	/** ถ้าไม่ส่ง backend จะ auto-assign เป็น MAX+1 ของปีการศึกษา */
	order_index?: number;
	applicable_days?: string;
}

export interface CreateTimetableEntryRequest {
	classroom_course_id?: string;
	day_of_week: string;
	period_id: string;
	room_id?: string;
	note?: string;
	// Activity entry support
	activity_slot_id?: string;
	entry_type?: string;
	title?: string;
	classroom_id?: string;
	academic_semester_id?: string;
	/** Phase 2: client-generated temp UUID — backend echo ใน EntryCreated broadcast
	 *  เพื่อให้ทุก client correlate temp → real entry */
	client_temp_id?: string;
}

export interface CreateBatchTimetableEntriesRequest {
	classroom_ids: string[];
	days_of_week: string[];
	period_ids: string[];
	academic_semester_id: string;
	entry_type: 'ACTIVITY' | 'BREAK' | 'HOMEROOM' | 'ACADEMIC';
	title: string;
	room_id?: string;
	note?: string;
	subject_id?: string;
	force?: boolean;
	activity_slot_id?: string;
	/** ครูที่ติดคาบด้วย event นี้ (attach ไป tei).
	 * classroom_ids ว่าง + instructor_ids มี → teacher-only event (classroom_id = NULL)
	 */
	instructor_ids?: string[];
}

export interface ConflictInfo {
	conflict_type: string;
	message: string;
	existing_entry?: TimetableEntry;
}

export interface TimetableValidationResponse {
	is_valid: boolean;
	conflicts: ConflictInfo[];
}

type TimetableItemsData = Schemas['TimetableItemsData'];

function normalizeTimetableListResponse(
	response: ApiResponse<TimetableEntryDto[] | TimetableItemsData>
): { data: TimetableEntry[]; current_seq?: number } {
	const payload = requireApiData(response, 'ไม่สามารถโหลดตารางสอนได้');
	if (Array.isArray(payload)) {
		return { data: payload.map(timetableEntryFromDto) };
	}
	return {
		data: payload.items.map(timetableEntryFromDto),
		current_seq: payload.current_seq
	};
}

interface ConflictPayload {
	conflicts?: ConflictInfo[];
}

export interface TimetableConflictResponse {
	success: false;
	conflicts: ConflictInfo[];
	message: string;
}

type TimetableMutationResponse = LoadedApiResponse<TimetableEntry> | TimetableConflictResponse;

function isConflictPayload(
	data: TimetableEntryDto | ConflictPayload | undefined
): data is ConflictPayload {
	return !!data && typeof data === 'object' && 'conflicts' in data;
}

function normalizeConflictResponse(
	response: ApiResponse<TimetableEntryDto | ConflictPayload>
): TimetableConflictResponse {
	const payload = response.data;
	return {
		success: false,
		conflicts: isConflictPayload(payload) ? (payload.conflicts ?? []) : [],
		message: response.message || response.error || 'พบข้อขัดแย้งในตาราง'
	};
}

// ============================================
// Period API
// ============================================

export const listPeriods = async (
	filters: {
		academic_year_id?: string;

		active_only?: boolean;
	} = {}
): Promise<{ data: AcademicPeriod[] }> => {
	const params = new URLSearchParams();
	if (filters.academic_year_id) params.append('academic_year_id', filters.academic_year_id);

	if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));

	const queryString = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<AcademicPeriod[]>(`/api/academic/periods${queryString}`);
};

export const createPeriod = async (
	data: CreatePeriodRequest
): Promise<LoadedApiResponse<AcademicPeriod>> => {
	return await fetchApi<AcademicPeriod>('/api/academic/periods', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updatePeriod = async (
	id: string,
	data: Partial<CreatePeriodRequest>
): Promise<LoadedApiResponse<AcademicPeriod>> => {
	return await fetchApi<AcademicPeriod>(`/api/academic/periods/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deletePeriod = async (
	id: string
): Promise<LoadedApiResponse<Record<string, never>>> => {
	return await fetchApi<Record<string, never>>(`/api/academic/periods/${id}`, { method: 'DELETE' });
};

export interface ReorderPeriodItem {
	id: string;
	order_index: number;
}

export const reorderPeriods = async (
	academic_year_id: string,
	items: ReorderPeriodItem[]
): Promise<LoadedApiResponse<{ updated: number }>> => {
	return await fetchApi<{ updated: number }>('/api/academic/periods/reorder', {
		method: 'POST',
		body: JSON.stringify({ academic_year_id, items })
	});
};

// ============================================
// Timetable API
// ============================================

export const listTimetableEntries = async (
	filters: {
		classroom_id?: string;
		student_id?: string;
		instructor_id?: string;
		room_id?: string;
		academic_semester_id?: string;
		day_of_week?: string;
		entry_type?: string;
		/** คู่กับ instructor_id: รวม entries ของ course ที่ instructor อยู่ในทีม (รวม ghost cells) */
		include_team_ghosts?: boolean;
	} = {}
): Promise<{ data: TimetableEntry[]; current_seq?: number }> => {
	const params = new URLSearchParams();
	if (filters.classroom_id) params.append('classroom_id', filters.classroom_id);
	if (filters.student_id) params.append('student_id', filters.student_id);
	if (filters.instructor_id) params.append('instructor_id', filters.instructor_id);
	if (filters.room_id) params.append('room_id', filters.room_id);
	if (filters.academic_semester_id)
		params.append('academic_semester_id', filters.academic_semester_id);
	if (filters.day_of_week) params.append('day_of_week', filters.day_of_week);
	if (filters.entry_type) params.append('entry_type', filters.entry_type);
	if (filters.include_team_ghosts) params.append('include_team_ghosts', 'true');

	const queryString = params.toString() ? `?${params.toString()}` : '';
	const response = await apiClient.get<TimetableEntryDto[] | TimetableItemsData>(
		`/api/academic/timetable${queryString}`
	);
	return normalizeTimetableListResponse(response);
};

export const listTimetableEntriesWithSeq = listTimetableEntries;

export const getDailyTeachingOverview = async (
	filters: {
		date?: string;
		academicSemesterId?: string;
		includeEmptyTeachers?: boolean;
	} = {}
): Promise<LoadedApiResponse<DailyTeachingOverview>> => {
	const params = new URLSearchParams();
	if (filters.date) params.append('date', filters.date);
	if (filters.academicSemesterId) params.append('academic_semester_id', filters.academicSemesterId);
	if (filters.includeEmptyTeachers) params.append('include_empty_teachers', 'true');

	const queryString = params.toString() ? `?${params.toString()}` : '';
	const response = await apiClient.get<DailyTeachingOverview>(
		`/api/academic/timetable/daily-teaching${queryString}`
	);
	const data = requireApiData(response, 'ไม่สามารถโหลดตารางสอนวันนี้ได้');
	return { success: true, data, message: response.message };
};

/**
 * ตารางของผู้ใช้ปัจจุบัน (student/staff)
 * Backend resolves user_type จาก JWT แล้วเลือก filter ให้อัตโนมัติ
 * - student → filter ตาม student_class_enrollments
 * - staff → filter ตาม timetable_entry_instructors (+ team ghosts ถ้าระบุ)
 */
export const getMyTimetable = async (
	filters: {
		academic_semester_id?: string;
		day_of_week?: string;
		include_team_ghosts?: boolean;
	} = {}
): Promise<{ data: TimetableEntry[]; current_seq?: number }> => {
	const params = new URLSearchParams();
	if (filters.academic_semester_id)
		params.append('academic_semester_id', filters.academic_semester_id);
	if (filters.day_of_week) params.append('day_of_week', filters.day_of_week);
	if (filters.include_team_ghosts) params.append('include_team_ghosts', 'true');

	const queryString = params.toString() ? `?${params.toString()}` : '';
	const response = await apiClient.get<TimetableItemsData>(`/api/me/timetable${queryString}`);
	const payload = requireApiData(response, 'ไม่สามารถโหลดตารางสอนได้');
	return {
		data: payload.items.map(timetableEntryFromDto),
		current_seq: payload.current_seq
	};
};

export interface BatchSkippedCell {
	classroom_id: string | null;
	classroom_name: string | null;
	day_of_week: string;
	period_id: string;
	period_name: string | null;
	reason: string;
	message: string;
}

export interface BatchBlockedCell {
	classroom_id: string;
	classroom_name: string | null;
	day_of_week: string;
	period_id: string;
	period_name: string | null;
	reason: string;
	message: string;
}

export interface BatchDeletedEntry {
	id: string;
	classroom_name: string | null;
	day_of_week: string;
	period_id: string;
	period_name: string | null;
	title: string;
	entry_type: string;
	instructor_names: string[];
}

export interface BatchExcludedInstructor {
	instructor_id: string;
	instructor_name: string;
	conflicting_at: Array<{
		day_of_week: string;
		period_id: string;
		period_name: string | null;
		existing_title: string;
	}>;
}

export interface BatchSummary {
	inserted_count: number;
	skipped: BatchSkippedCell[];
	blocked: BatchBlockedCell[];
	deleted: BatchDeletedEntry[];
	excluded_instructors: BatchExcludedInstructor[];
}

export const createBatchTimetableEntries = async (
	data: CreateBatchTimetableEntriesRequest
): Promise<{ success: boolean; summary?: BatchSummary; message?: string }> => {
	const response = await apiClient.post<{ summary: BatchSummary }>(
		'/api/academic/timetable/batch',
		data
	);
	const payload = requireApiData(response, 'Request failed');
	return { success: true, summary: payload.summary, message: response.message };
};

export interface UpdateTimetableEntryRequest {
	day_of_week?: string;
	period_id?: string;
	room_id?: string;
	note?: string;
	/** Replace entry's content (drag-from-sidebar-onto-occupied flow). null = clear the field. */
	classroom_course_id?: string | null;
	activity_slot_id?: string | null;
	/** Move entry to different classroom (instructor-view replace ข้ามห้อง) */
	classroom_id?: string;
}

export interface MoveValidityCell {
	day_of_week: string;
	period_id: string;
	/** "empty" | "occupied" | "source" */
	state: 'empty' | 'occupied' | 'source';
	/** If occupied: id of entry that will be the swap partner. null otherwise. */
	target_entry_id: string | null;
	valid: boolean;
	reason: string;
}

export const swapTimetableEntries = async (entryAId: string, entryBId: string) => {
	return await fetchApi<Record<string, never>>('/api/academic/timetable/swap', {
		method: 'POST',
		body: JSON.stringify({ entry_a_id: entryAId, entry_b_id: entryBId })
	});
};

export const validateTimetableMoves = async (
	entryId: string
): Promise<{ data: MoveValidityCell[] }> => {
	return await fetchApi<MoveValidityCell[]>('/api/academic/timetable/validate-moves', {
		method: 'POST',
		body: JSON.stringify({ entry_id: entryId })
	});
};

/** Lightweight entry summary — used for client-side conflict checks (drop validity). */
export interface OccupancyEntry {
	id: string;
	classroom_id: string | null;
	day_of_week: string;
	period_id: string;
	room_id: string | null;
	instructor_ids: string[];
}

export const getTimetableOccupancy = async (
	semesterId: string
): Promise<{ data: OccupancyEntry[] }> => {
	return await fetchApi<OccupancyEntry[]>(
		`/api/academic/timetable/occupancy?semester_id=${encodeURIComponent(semesterId)}`
	);
};

export const createTimetableEntry = async (
	data: CreateTimetableEntryRequest
): Promise<TimetableMutationResponse> => {
	const response = await apiClient.post<TimetableEntryDto | ConflictPayload>(
		'/api/academic/timetable',
		data
	);
	if (!response.success) return normalizeConflictResponse(response);
	const entry = requireApiData(response, 'Request failed');
	if (isConflictPayload(entry)) return normalizeConflictResponse({ ...response, success: false });
	return { success: true, data: timetableEntryFromDto(entry), message: response.message };
};

export const updateTimetableEntry = async (
	id: string,
	data: UpdateTimetableEntryRequest
): Promise<TimetableMutationResponse> => {
	const response = await apiClient.put<TimetableEntryDto | ConflictPayload>(
		`/api/academic/timetable/${id}`,
		data
	);
	if (!response.success) return normalizeConflictResponse(response);
	const entry = requireApiData(response, 'Request failed');
	if (isConflictPayload(entry)) return normalizeConflictResponse({ ...response, success: false });
	return { success: true, data: timetableEntryFromDto(entry), message: response.message };
};

export const deleteTimetableEntry = async (id: string) => {
	return await fetchApi<Record<string, never>>(`/api/academic/timetable/${id}`, {
		method: 'DELETE'
	});
};

export const deleteBatchGroup = async (
	batchId: string
): Promise<{ success: boolean; deleted_count: number }> => {
	const response = await apiClient.delete<{ deleted_count: number }>(
		`/api/academic/timetable/batch-group/${batchId}`
	);
	const data = requireApiData(response, 'Request failed');
	return { success: true, deleted_count: data.deleted_count };
};

export const deleteBatchTimetableEntries = async (data: {
	activity_slot_id: string;
	day_of_week: string;
	academic_semester_id: string;
}): Promise<{ deleted_count: number }> => {
	const response = await apiClient.deleteWithBody<{ deleted_count: number }>(
		'/api/academic/timetable/batch',
		data
	);
	return requireApiData(response, 'Request failed');
};

export interface MyActivityForEntry {
	enrolled: boolean;
	slot_id: string;
	group_id?: string;
	group_name?: string;
	max_capacity?: number;
	member_count?: number;
	instructor_name?: string;
	instructors?: { id: string; name: string }[];
}

export const getMyActivityForEntry = async (
	entryId: string
): Promise<{ data: MyActivityForEntry | null }> => {
	return await fetchApi<MyActivityForEntry | null>(
		`/api/academic/timetable/${entryId}/my-activity`
	);
};

export const addEntryInstructor = async (
	entryId: string,
	instructorId: string,
	role: 'primary' | 'secondary' = 'secondary'
) => {
	return await fetchApi<Record<string, never>>(`/api/academic/timetable/${entryId}/instructors`, {
		method: 'POST',
		body: JSON.stringify({ instructor_id: instructorId, role })
	});
};

export const removeEntryInstructor = async (entryId: string, instructorId: string) => {
	return await fetchApi<Record<string, never>>(
		`/api/academic/timetable/${entryId}/instructors/${instructorId}`,
		{
			method: 'DELETE'
		}
	);
};

export const restoreInstructorToSlot = async (slotId: string, instructorId: string) => {
	return await fetchApi<{ inserted: number }>(
		`/api/academic/timetable/slots/${slotId}/instructors/${instructorId}/restore`,
		{
			method: 'POST'
		}
	);
};

export const hideInstructorFromSlot = async (slotId: string, instructorId: string) => {
	return await fetchApi<{ deleted: number }>(
		`/api/academic/timetable/slots/${slotId}/instructors/${instructorId}`,
		{
			method: 'DELETE'
		}
	);
};

export const hideInstructorFromSlotPeriod = async (
	slotId: string,
	instructorId: string,
	dayOfWeek: string,
	periodId: string
) => {
	const params = new URLSearchParams({ day_of_week: dayOfWeek, period_id: periodId });
	return await fetchApi<{ deleted: number }>(
		`/api/academic/timetable/slots/${slotId}/instructors/${instructorId}/period?${params.toString()}`,
		{ method: 'DELETE' }
	);
};
