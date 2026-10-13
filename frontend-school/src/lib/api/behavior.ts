import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components, operations } from '#lib/api/generated/school-api.js';
type Schemas = components['schemas'];
export type BehaviorSide = Schemas['BehaviorSide'];
export type BehaviorSource = Schemas['BehaviorSource'];
export type BehaviorScoreAction = Schemas['BehaviorScoreAction'];
export type BehaviorResetSchedule = Schemas['BehaviorResetSchedule'];
export type BehaviorCleanupMode = Schemas['BehaviorCleanupMode'];
export type BehaviorItem = Schemas['BehaviorItem'];
export type SaveBehaviorItem = Schemas['SaveBehaviorItem'];
export type BehaviorAttendanceRule = Schemas['BehaviorAttendanceRule'];
export type SaveBehaviorAttendanceRule = Schemas['SaveBehaviorAttendanceRule'];
export type BehaviorSettings = Schemas['BehaviorSettings'];
export type BehaviorCycle = Schemas['BehaviorCycle'];
export type BehaviorStudent = Schemas['BehaviorStudent'];
export type BehaviorStudentPage = Schemas['BehaviorStudentPage'];
export type BehaviorSelectorOptions = Schemas['BehaviorSelectorOptions'];
export type CreateBehaviorBatch = Schemas['CreateBehaviorBatch'];
export type BehaviorBatchReceipt = Schemas['BehaviorBatchReceipt'];
export type CancelBehaviorEntries = Schemas['CancelBehaviorEntries'];
export type BehaviorCancellation = Schemas['BehaviorCancellation'];
export type BehaviorReportEntry = Schemas['BehaviorReportEntry'];
export type BehaviorReportPage = Schemas['BehaviorReportPage'];
export type BehaviorStudentSummary = Schemas['BehaviorStudentSummary'];
export type BehaviorDailyTotal = Schemas['BehaviorDailyTotal'];
export type BehaviorCleanupScope = Schemas['BehaviorCleanupScope'];
export type BehaviorCleanupPreview = Schemas['BehaviorCleanupPreview'];
export type PreviewBehaviorCleanup = Schemas['PreviewBehaviorCleanup'];
export type ExecuteBehaviorCleanup = Schemas['ExecuteBehaviorCleanup'];
export type BehaviorCleanupOutcome = Schemas['BehaviorCleanupOutcome'];
export type AdjustBehaviorHistory = Schemas['AdjustBehaviorHistory'];
export type BehaviorHistoryAdjustment = Schemas['BehaviorHistoryAdjustment'];
export type PreviewBehaviorReset = Schemas['PreviewBehaviorReset'];
export type ResetBehaviorScores = Schemas['ResetBehaviorScores'];
export type BehaviorOverview = Schemas['BehaviorOverview'];
export type BehaviorStudentsQuery = NonNullable<
	operations['behavior_students']['parameters']['query']
>;
export type BehaviorReportQuery = NonNullable<operations['behavior_report']['parameters']['query']>;
export type BehaviorSummaryQuery = NonNullable<
	operations['behavior_student_summary']['parameters']['query']
>;
export type BehaviorItemsQuery = NonNullable<
	operations['behavior_list_items']['parameters']['query']
>;
export async function behaviorOverview(options: ApiRequestOptions = {}): Promise<BehaviorOverview> {
	return requireApiData(
		await apiClient.get<BehaviorOverview>('/api/behavior/overview', options),
		'โหลดข้อมูลพฤติกรรมไม่สำเร็จ'
	);
}
export async function behaviorItems(
	query: BehaviorItemsQuery,
	options: ApiRequestOptions = {}
): Promise<BehaviorItem[]> {
	return requireApiData(
		await apiClient.get<BehaviorItem[]>('/api/behavior/items', { ...options, query: { ...query } }),
		'โหลดข้อมูลพฤติกรรมไม่สำเร็จ'
	);
}
export async function behaviorRules(
	options: ApiRequestOptions = {}
): Promise<BehaviorAttendanceRule[]> {
	return requireApiData(
		await apiClient.get<BehaviorAttendanceRule[]>('/api/behavior/rules', options),
		'โหลดข้อมูลพฤติกรรมไม่สำเร็จ'
	);
}
export async function behaviorSettings(options: ApiRequestOptions = {}): Promise<BehaviorSettings> {
	return requireApiData(
		await apiClient.get<BehaviorSettings>('/api/behavior/settings', options),
		'โหลดข้อมูลพฤติกรรมไม่สำเร็จ'
	);
}
export async function behaviorStudents(
	query: BehaviorStudentsQuery,
	options: ApiRequestOptions = {}
): Promise<BehaviorStudentPage> {
	return requireApiData(
		await apiClient.get<BehaviorStudentPage>('/api/behavior/students', {
			...options,
			query: { ...query }
		}),
		'โหลดข้อมูลพฤติกรรมไม่สำเร็จ'
	);
}
export async function behaviorOptions(
	query: BehaviorStudentsQuery,
	options: ApiRequestOptions = {}
): Promise<BehaviorSelectorOptions> {
	return requireApiData(
		await apiClient.get<BehaviorSelectorOptions>('/api/behavior/options', {
			...options,
			query: { ...query }
		}),
		'โหลดข้อมูลพฤติกรรมไม่สำเร็จ'
	);
}
export async function behaviorEntries(
	query: BehaviorReportQuery,
	options: ApiRequestOptions = {}
): Promise<BehaviorReportPage> {
	return requireApiData(
		await apiClient.get<BehaviorReportPage>('/api/behavior/entries', {
			...options,
			query: { ...query }
		}),
		'โหลดข้อมูลพฤติกรรมไม่สำเร็จ'
	);
}
export async function behaviorCycles(options: ApiRequestOptions = {}): Promise<BehaviorCycle[]> {
	return requireApiData(
		await apiClient.get<BehaviorCycle[]>('/api/behavior/cycles', options),
		'โหลดข้อมูลพฤติกรรมไม่สำเร็จ'
	);
}
export async function behaviorSummary(
	studentId: string,
	query: BehaviorSummaryQuery = {},
	options: ApiRequestOptions = {}
): Promise<BehaviorStudentSummary> {
	return requireApiData(
		await apiClient.get<BehaviorStudentSummary>(
			`/api/behavior/students/${encodeURIComponent(studentId)}/summary`,
			{ ...options, query: { ...query } }
		),
		'โหลดคะแนนไม่สำเร็จ'
	);
}
export async function saveBehaviorItem(payload: SaveBehaviorItem): Promise<BehaviorItem> {
	return requireApiData(
		await apiClient.post<BehaviorItem>('/api/behavior/items', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function saveBehaviorRule(
	payload: SaveBehaviorAttendanceRule
): Promise<BehaviorAttendanceRule> {
	return requireApiData(
		await apiClient.post<BehaviorAttendanceRule>('/api/behavior/rules', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function createBehaviorBatch(
	payload: CreateBehaviorBatch
): Promise<BehaviorBatchReceipt> {
	return requireApiData(
		await apiClient.post<BehaviorBatchReceipt>('/api/behavior/batches', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function cancelBehaviorEntries(
	payload: CancelBehaviorEntries
): Promise<BehaviorCancellation> {
	return requireApiData(
		await apiClient.post<BehaviorCancellation>('/api/behavior/entries/cancel', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function previewBehaviorCleanup(
	payload: PreviewBehaviorCleanup
): Promise<BehaviorCleanupPreview> {
	return requireApiData(
		await apiClient.post<BehaviorCleanupPreview>('/api/behavior/cleanup/preview', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function executeBehaviorCleanup(
	payload: ExecuteBehaviorCleanup
): Promise<BehaviorCleanupOutcome> {
	return requireApiData(
		await apiClient.post<BehaviorCleanupOutcome>('/api/behavior/cleanup', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function adjustBehaviorHistory(
	payload: AdjustBehaviorHistory
): Promise<BehaviorHistoryAdjustment> {
	return requireApiData(
		await apiClient.post<BehaviorHistoryAdjustment>('/api/behavior/history/adjustments', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function previewBehaviorReset(
	payload: PreviewBehaviorReset
): Promise<BehaviorCleanupPreview> {
	return requireApiData(
		await apiClient.post<BehaviorCleanupPreview>('/api/behavior/reset/preview', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function resetBehaviorScores(
	payload: ResetBehaviorScores
): Promise<BehaviorCleanupOutcome> {
	return requireApiData(
		await apiClient.post<BehaviorCleanupOutcome>('/api/behavior/reset', payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function saveBehaviorSettings(settings: BehaviorSettings): Promise<BehaviorSettings> {
	return requireApiData(
		await apiClient.put<BehaviorSettings>('/api/behavior/settings', { settings }),
		'บันทึกการตั้งค่าไม่สำเร็จ'
	);
}

export async function behaviorCleanupStatus(
	id: string,
	options: ApiRequestOptions = {}
): Promise<BehaviorCleanupOutcome> {
	return requireApiData(
		await apiClient.get<BehaviorCleanupOutcome>(
			`/api/behavior/cleanup/${encodeURIComponent(id)}`,
			options
		),
		'โหลดสถานะการล้างไม่ได้'
	);
}
