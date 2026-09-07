import {
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];
type GradebookQuery = NonNullable<operations['listGradebookSubjects']['parameters']['query']>;
type GradebookGroupPath = NonNullable<
	operations['getGradebookGroupPhaseWorkspace']['parameters']['path']
>;
type GradebookItemPath = NonNullable<operations['updateGradebookItem']['parameters']['path']>;
type GradebookControlPath = NonNullable<operations['updateGradebookControl']['parameters']['path']>;

export type GradebookContext = GradebookQuery;
export type GradebookSubject = Schemas['GradebookSubject'];
export type GradebookControl = Schemas['GradebookControl'];
export type GroupPhaseWorkspace = Schemas['GroupPhaseWorkspace'];
export type GradebookPhaseCode = Schemas['AssessmentPhaseCode'];
export type GradebookItemInput = Schemas['ItemInput'];
export type GradebookScoreItem = Schemas['ScoreItem'];
export type GradebookRemoveItemInput = Schemas['RemoveItemInput'];
export type GradebookItemRemovalOutcome = Schemas['ScoreItemRemovalOutcome'];
export type GradebookScoreBatchInput = Schemas['ScoreBatchInput'];
export type GradebookScoreBatchOutcome = Schemas['ScoreBatchOutcome'];
export type GradebookConfirmInput = Schemas['ConfirmInput'];
export type GradebookPhaseConfirmation = Schemas['PhaseConfirmation'];
export type UpdateGradebookControlInput = Schemas['UpdateControlInput'];

function gradebookData<T>(request: Promise<ApiResponse<T>>, fallback: string): Promise<T> {
	return request.then((response) => requireApiData(response, fallback));
}

function query(context: GradebookContext): GradebookQuery {
	const value = {
		academicYearId: context.academicYearId.trim(),
		academicTermId: context.academicTermId.trim()
	} satisfies GradebookQuery;
	if (!value.academicYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	if (!value.academicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return value;
}

function groupPhasePath(groupId: string, phaseCode: GradebookPhaseCode): GradebookGroupPath {
	return {
		group_id: groupId,
		phase_code: phaseCode
	} satisfies GradebookGroupPath;
}

export function listGradebookSubjects(
	context: GradebookContext,
	options: ApiRequestOptions = {}
): Promise<GradebookSubject[]> {
	return gradebookData(
		apiClient.get<GradebookSubject[]>('/api/academic/gradebook/subjects', {
			...options,
			query: query(context)
		}),
		'ไม่สามารถโหลดรายวิชาสำหรับกรอกคะแนนได้'
	);
}

export function listGradebookControls(
	context: GradebookContext,
	options: ApiRequestOptions = {}
): Promise<GradebookControl[]> {
	return gradebookData(
		apiClient.get<GradebookControl[]>('/api/academic/gradebook/controls', {
			...options,
			query: query(context)
		}),
		'ไม่สามารถโหลดสถานะเปิดกรอกคะแนนได้'
	);
}

export function updateGradebookControl(
	controlId: string,
	context: GradebookContext,
	body: UpdateGradebookControlInput
): Promise<GradebookControl> {
	const path = { control_id: controlId } satisfies GradebookControlPath;
	const input = body satisfies UpdateGradebookControlInput;
	return gradebookData(
		apiClient.put<GradebookControl>(
			`/api/academic/gradebook/controls/${encodeURIComponent(path.control_id)}`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถบันทึกสถานะเปิดกรอกคะแนนได้'
	);
}

export function getGradebookGroupPhaseWorkspace(
	groupId: string,
	phaseCode: GradebookPhaseCode,
	context: GradebookContext,
	options: ApiRequestOptions = {}
): Promise<GroupPhaseWorkspace> {
	const path = groupPhasePath(groupId, phaseCode);
	return gradebookData(
		apiClient.get<GroupPhaseWorkspace>(
			`/api/academic/gradebook/groups/${encodeURIComponent(path.group_id)}/phases/${encodeURIComponent(path.phase_code)}`,
			{ ...options, query: query(context) }
		),
		'ไม่สามารถโหลดสมุดคะแนนได้'
	);
}

export function createGradebookItem(
	groupId: string,
	phaseCode: GradebookPhaseCode,
	context: GradebookContext,
	body: GradebookItemInput
): Promise<GradebookScoreItem> {
	const path = groupPhasePath(groupId, phaseCode);
	const input = body satisfies GradebookItemInput;
	return gradebookData(
		apiClient.post<GradebookScoreItem>(
			`/api/academic/gradebook/groups/${encodeURIComponent(path.group_id)}/phases/${encodeURIComponent(path.phase_code)}/items`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถเพิ่มรายการคะแนนได้'
	);
}

export function updateGradebookItem(
	groupId: string,
	phaseCode: GradebookPhaseCode,
	itemId: string,
	context: GradebookContext,
	body: GradebookItemInput
): Promise<GradebookScoreItem> {
	const path = {
		...groupPhasePath(groupId, phaseCode),
		item_id: itemId
	} satisfies GradebookItemPath;
	const input = body satisfies GradebookItemInput;
	return gradebookData(
		apiClient.put<GradebookScoreItem>(
			`/api/academic/gradebook/groups/${encodeURIComponent(path.group_id)}/phases/${encodeURIComponent(path.phase_code)}/items/${encodeURIComponent(path.item_id)}`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถแก้ไขรายการคะแนนได้'
	);
}

export function removeGradebookItem(
	groupId: string,
	phaseCode: GradebookPhaseCode,
	itemId: string,
	context: GradebookContext,
	body: GradebookRemoveItemInput
): Promise<GradebookItemRemovalOutcome> {
	const path = {
		...groupPhasePath(groupId, phaseCode),
		item_id: itemId
	} satisfies GradebookItemPath;
	const input = body satisfies GradebookRemoveItemInput;
	return gradebookData(
		apiClient.deleteWithBody<GradebookItemRemovalOutcome>(
			`/api/academic/gradebook/groups/${encodeURIComponent(path.group_id)}/phases/${encodeURIComponent(path.phase_code)}/items/${encodeURIComponent(path.item_id)}`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถนำรายการคะแนนออกได้'
	);
}

export function saveGradebookScoresBatch(
	groupId: string,
	phaseCode: GradebookPhaseCode,
	context: GradebookContext,
	body: GradebookScoreBatchInput
): Promise<GradebookScoreBatchOutcome> {
	const path = groupPhasePath(groupId, phaseCode);
	const input = body satisfies GradebookScoreBatchInput;
	return gradebookData(
		apiClient.put<GradebookScoreBatchOutcome>(
			`/api/academic/gradebook/groups/${encodeURIComponent(path.group_id)}/phases/${encodeURIComponent(path.phase_code)}/scores`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถบันทึกคะแนนได้'
	);
}

export function confirmGradebookPhase(
	groupId: string,
	phaseCode: GradebookPhaseCode,
	context: GradebookContext,
	body: GradebookConfirmInput
): Promise<GradebookPhaseConfirmation> {
	const path = groupPhasePath(groupId, phaseCode);
	const input = body satisfies GradebookConfirmInput;
	return gradebookData(
		apiClient.post<GradebookPhaseConfirmation>(
			`/api/academic/gradebook/groups/${encodeURIComponent(path.group_id)}/phases/${encodeURIComponent(path.phase_code)}/confirm`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถยืนยันคะแนนช่วงนี้ได้'
	);
}
