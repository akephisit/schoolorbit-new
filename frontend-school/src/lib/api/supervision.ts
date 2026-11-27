import {
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '#lib/api/client.js';
import type { components, operations } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];

export type SupervisionCycleStatus = Schemas['SupervisionCycleStatus'];
export type SupervisionTemplateStatus = Schemas['SupervisionTemplateStatus'];
export type SupervisionTargetType = Schemas['SupervisionTargetType'];
export type SupervisionTemplateItemType = Schemas['SupervisionTemplateItemType'];
export type SupervisionTemplateStepActorKind = Schemas['SupervisionTemplateStepActorKind'];
export type SupervisionTemplateStepActionKind = Schemas['SupervisionTemplateStepActionKind'];
export type SupervisionObservationStatus = Schemas['SupervisionObservationStatus'];
export type SupervisionEvaluatorStatus = Schemas['SupervisionEvaluatorStatus'];
export type LessonSnapshot = Schemas['LessonSnapshot'];
export type SupervisionCycleTarget = Schemas['SupervisionCycleTarget'];
export type SupervisionCycle = Schemas['SupervisionCycle'];
export type SupervisionTemplateItem = Schemas['SupervisionTemplateItem'];
export type SupervisionTemplateSection = Schemas['SupervisionTemplateSection'];
export type SupervisionTemplateStep = Schemas['SupervisionTemplateStep'];
export type SupervisionTemplate = Schemas['SupervisionTemplate'];
export type SupervisionTemplateSummary = Schemas['SupervisionTemplateSummary'];
export type SupervisionObservationPeriod = Schemas['SupervisionObservationPeriod'];
export type SupervisionPeriodManualEvidence = Schemas['SupervisionPeriodManualEvidence'];
export type SupervisionBookingOptions = Schemas['SupervisionBookingOptions'];
export type SupervisionCycleDeletionImpact = Schemas['SupervisionCycleDeletionImpact'];
export type SupervisionEvaluator = Schemas['SupervisionEvaluator'];
export type SupervisionEvaluatorConflict = Schemas['SupervisionEvaluatorConflict'];
export type SupervisionEvaluatorAvailability = Schemas['SupervisionEvaluatorAvailability'];
export type SupervisionEvaluatorAvailabilityResponse =
	Schemas['SupervisionEvaluatorAvailabilityResponse'];
export type SupervisionBookingAvailability = Schemas['SupervisionBookingAvailability'];
export type SupervisionAction = Schemas['SupervisionAction'];
export type SupervisionObservation = Schemas['SupervisionObservation'];
export type SupervisionCalendarEntry = Schemas['SupervisionCalendarEntry'];
export type SupervisionReviewResponse = Schemas['SupervisionReviewResponse'];
export type SupervisionReviewEvaluatorResult = Schemas['SupervisionReviewEvaluatorResult'];
export type SupervisionReviewItemSummary = Schemas['SupervisionReviewItemSummary'];
export type SupervisionObservationReview = Schemas['SupervisionObservationReview'];
export type SupervisionCycleProgress = Schemas['SupervisionCycleProgress'];
export type SupervisionTeacherStatusRow = Schemas['SupervisionTeacherStatusRow'];
export type SupervisionTimetableOption = Schemas['SupervisionTimetableOption'];
export type EvaluatorAssignmentInput = Schemas['EvaluatorAssignmentInput'];
export type EvaluationResponseInput = Schemas['EvaluationResponseInput'];

export type CreateSupervisionCycleTargetRequest = Schemas['CreateSupervisionCycleTargetRequest'];
export type CreateSupervisionTemplateItemRequest = Schemas['CreateSupervisionTemplateItemRequest'];
export type CreateSupervisionTemplateSectionRequest =
	Schemas['CreateSupervisionTemplateSectionRequest'];
export type CreateSupervisionTemplateStepRequest = Schemas['CreateSupervisionTemplateStepRequest'];
export type CreateSupervisionCycleRequest =
	operations['createSupervisionCycle']['requestBody']['content']['application/json'];
export type UpdateSupervisionCycleRequest =
	operations['updateSupervisionCycle']['requestBody']['content']['application/json'];
export type CreateSupervisionTemplateRequest =
	operations['createSupervisionTemplate']['requestBody']['content']['application/json'];
export type UpdateSupervisionTemplateRequest =
	operations['updateSupervisionTemplate']['requestBody']['content']['application/json'];
export type RequestSupervisionObservationRequest =
	operations['requestSupervisionObservation']['requestBody']['content']['application/json'];
export type UpdateRequestedObservationRequest =
	operations['updateRequestedSupervisionObservation']['requestBody']['content']['application/json'];
export type UpdateSupervisionObservationRequest =
	operations['updateSupervisionObservation']['requestBody']['content']['application/json'];
export type ReplaceObservationEvaluatorsRequest =
	operations['replaceSupervisionObservationEvaluators']['requestBody']['content']['application/json'];
export type CancelObservationRequest =
	operations['cancelSupervisionObservation']['requestBody']['content']['application/json'];
export type ApproveObservationRequest =
	operations['approveSupervisionObservationRequest']['requestBody']['content']['application/json'];
export type ReturnObservationRequest =
	operations['returnSupervisionObservationRequest']['requestBody']['content']['application/json'];
export type SaveEvaluationRequest =
	operations['submitMySupervisionEvaluation']['requestBody']['content']['application/json'];
export type AcknowledgeObservationRequest =
	operations['acknowledgeSupervisionObservation']['requestBody']['content']['application/json'];

export type ListSupervisionObservationsParams = NonNullable<
	operations['listSupervisionObservations']['parameters']['query']
>;
export type ListSupervisionCalendarParams = NonNullable<
	operations['listSupervisionCalendar']['parameters']['query']
>;

type ListSupervisionCyclesQuery = NonNullable<
	operations['listSupervisionCycles']['parameters']['query']
>;
type ObservationId = operations['getSupervisionObservation']['parameters']['path']['id'];
type TemplateId = operations['getSupervisionTemplate']['parameters']['path']['id'];
type CycleId = operations['getSupervisionCycleProgress']['parameters']['path']['id'];
type SupervisionCycleItems = Schemas['ItemsData_SupervisionCycle'];
type SupervisionTemplateItems = Schemas['ItemsData_SupervisionTemplate'];
type SupervisionObservationItems = Schemas['ItemsData_SupervisionObservation'];
type SupervisionCalendarItems = Schemas['ItemsData_SupervisionCalendarEntry'];
type SupervisionTeacherStatusItems = Schemas['ItemsData_SupervisionTeacherStatusRow'];

export type SupervisionBookingOptionsQuery = NonNullable<
	operations['getSupervisionBookingOptions']['parameters']['query']
>;
export type DeleteSupervisionCycleRequest =
	operations['deleteSupervisionCycle']['requestBody']['content']['application/json'];

export async function getSupervisionBookingOptions(
	query: SupervisionBookingOptionsQuery,
	options: ApiRequestOptions = {}
): Promise<SupervisionBookingOptions> {
	const response = await apiClient.get<SupervisionBookingOptions>(
		'/api/supervision/booking-options',
		{ ...options, query }
	);
	return requireApiData(response, 'ไม่สามารถโหลดคาบสอนสำหรับจองได้');
}

export async function getSupervisionCycleDeletionImpact(
	id: CycleId,
	options: ApiRequestOptions = {}
): Promise<SupervisionCycleDeletionImpact> {
	const response = await apiClient.get<SupervisionCycleDeletionImpact>(
		`/api/supervision/cycles/${id}/deletion-impact`,
		options
	);
	return requireApiData(response, 'ไม่สามารถตรวจผลกระทบของการลบรอบได้');
}

export function deleteSupervisionCycle(
	id: CycleId,
	payload: DeleteSupervisionCycleRequest
): Promise<ApiResponse<SupervisionCycleDeletionImpact>> {
	return apiClient.deleteWithBody<SupervisionCycleDeletionImpact>(
		`/api/supervision/cycles/${id}`,
		payload
	);
}

function requiredAcademicYearId(value: string): string {
	const id = value.trim();
	if (!id) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	return id;
}

function cycleQuery(
	academicYearId: string,
	academicTermId?: string | null
): ListSupervisionCyclesQuery {
	return {
		academicYearId: requiredAcademicYearId(academicYearId),
		...(academicTermId ? { academicTermId } : {})
	};
}

export async function listSupervisionCycles(
	academicYearId: string,
	academicTermId?: string | null,
	options: ApiRequestOptions = {}
): Promise<SupervisionCycle[]> {
	const query = cycleQuery(academicYearId, academicTermId);
	const response = await apiClient.get<SupervisionCycleItems>('/api/supervision/cycles', {
		...options,
		query
	});
	return requireApiData(response, 'ไม่สามารถโหลดรอบนิเทศได้').items;
}

export function createSupervisionCycle(
	payload: CreateSupervisionCycleRequest
): Promise<ApiResponse<SupervisionCycle>> {
	return apiClient.post<SupervisionCycle>('/api/supervision/cycles', payload);
}

export async function getSupervisionCycle(
	id: CycleId,
	options: ApiRequestOptions = {}
): Promise<SupervisionCycle> {
	const response = await apiClient.get<SupervisionCycle>(`/api/supervision/cycles/${id}`, options);
	return requireApiData(response, 'ไม่สามารถโหลดรอบนิเทศได้');
}

export function updateSupervisionCycle(
	id: CycleId,
	payload: UpdateSupervisionCycleRequest
): Promise<ApiResponse<SupervisionCycle>> {
	return apiClient.patch<SupervisionCycle>(`/api/supervision/cycles/${id}`, payload);
}

export async function listSupervisionTemplates(
	options: ApiRequestOptions = {}
): Promise<SupervisionTemplate[]> {
	const response = await apiClient.get<SupervisionTemplateItems>(
		'/api/supervision/templates',
		options
	);
	return requireApiData(response, 'ไม่สามารถโหลดแบบประเมินนิเทศได้').items;
}

export async function listSupervisionTemplateSummaries(
	options: ApiRequestOptions = {}
): Promise<SupervisionTemplateSummary[]> {
	const response = await apiClient.get<Schemas['ItemsData_SupervisionTemplateSummary']>(
		'/api/supervision/templates/summaries',
		options
	);
	return requireApiData(response, 'ไม่สามารถโหลดแบบประเมินนิเทศได้').items;
}

export async function getSupervisionTemplate(
	id: TemplateId,
	options: ApiRequestOptions = {}
): Promise<SupervisionTemplate> {
	const response = await apiClient.get<SupervisionTemplate>(
		`/api/supervision/templates/${id}`,
		options
	);
	return requireApiData(response, 'ไม่สามารถโหลดแบบประเมินนิเทศได้');
}

export async function getSupervisionTemplateSummary(
	id: TemplateId,
	options: ApiRequestOptions = {}
): Promise<SupervisionTemplateSummary> {
	const response = await apiClient.get<SupervisionTemplateSummary>(
		`/api/supervision/templates/${id}/summary`,
		options
	);
	return requireApiData(response, 'ไม่สามารถโหลดข้อมูลแบบประเมินนิเทศได้');
}

export function createSupervisionTemplate(
	payload: CreateSupervisionTemplateRequest
): Promise<ApiResponse<SupervisionTemplate>> {
	return apiClient.post<SupervisionTemplate>('/api/supervision/templates', payload);
}

export function updateSupervisionTemplate(
	id: TemplateId,
	payload: UpdateSupervisionTemplateRequest
): Promise<ApiResponse<SupervisionTemplate>> {
	return apiClient.patch<SupervisionTemplate>(`/api/supervision/templates/${id}`, payload);
}

export async function listSupervisionObservations(
	params: ListSupervisionObservationsParams,
	options: ApiRequestOptions = {}
): Promise<SupervisionObservation[]> {
	const query = {
		...params,
		academicYearId: requiredAcademicYearId(params.academicYearId)
	} satisfies ListSupervisionObservationsParams;
	const response = await apiClient.get<SupervisionObservationItems>(
		'/api/supervision/observations',
		{ ...options, query }
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายการนิเทศได้').items;
}

export async function listSupervisionCalendar(
	params: ListSupervisionCalendarParams,
	options: ApiRequestOptions = {}
): Promise<SupervisionCalendarEntry[]> {
	const query = {
		...params,
		academicYearId: requiredAcademicYearId(params.academicYearId)
	} satisfies ListSupervisionCalendarParams;
	const response = await apiClient.get<SupervisionCalendarItems>('/api/supervision/calendar', {
		...options,
		query
	});
	return requireApiData(response, 'โหลดปฏิทินนิเทศไม่สำเร็จ').items;
}

export async function getSupervisionObservation(
	id: ObservationId,
	options: ApiRequestOptions = {}
): Promise<SupervisionObservation> {
	const response = await apiClient.get<SupervisionObservation>(
		`/api/supervision/observations/${id}`,
		options
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายการนิเทศได้');
}

export async function getSupervisionObservationReview(
	id: ObservationId,
	options: ApiRequestOptions = {}
): Promise<SupervisionObservationReview> {
	const response = await apiClient.get<SupervisionObservationReview>(
		`/api/supervision/observations/${id}/review`,
		options
	);
	return requireApiData(response, 'ไม่สามารถโหลดผลประเมินนิเทศได้');
}

export function requestSupervisionObservation(
	payload: RequestSupervisionObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>('/api/supervision/observations/requests', payload);
}

export function updateRequestedSupervisionObservation(
	id: ObservationId,
	payload: UpdateRequestedObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.patch<SupervisionObservation>(
		`/api/supervision/observations/${id}/request`,
		payload
	);
}

export function cancelRequestedSupervisionObservation(
	id: ObservationId
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.delete<SupervisionObservation>(`/api/supervision/observations/${id}/request`);
}

export function updateSupervisionObservation(
	id: ObservationId,
	payload: UpdateSupervisionObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.patch<SupervisionObservation>(`/api/supervision/observations/${id}`, payload);
}

export function replaceSupervisionObservationEvaluators(
	id: ObservationId,
	payload: ReplaceObservationEvaluatorsRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.put<SupervisionObservation>(
		`/api/supervision/observations/${id}/evaluators`,
		payload
	);
}

export async function getSupervisionEvaluatorAvailability(
	id: ObservationId,
	options: ApiRequestOptions = {}
): Promise<SupervisionEvaluatorAvailabilityResponse> {
	const response = await apiClient.get<SupervisionEvaluatorAvailabilityResponse>(
		`/api/supervision/observations/${id}/evaluator-availability`,
		options
	);
	return requireApiData(response, 'ไม่สามารถตรวจสอบผู้ประเมินที่ว่างได้');
}

export async function getSupervisionObservationTimetableOptions(
	id: ObservationId,
	weekStartDate: string,
	options: ApiRequestOptions = {}
): Promise<SupervisionBookingOptions> {
	const query: NonNullable<
		operations['getSupervisionObservationTimetableOptions']['parameters']['query']
	> = { weekStartDate };
	const response = await apiClient.get<SupervisionBookingOptions>(
		`/api/supervision/observations/${id}/timetable-options`,
		{ ...options, query }
	);
	return requireApiData(response, 'ไม่สามารถโหลดคาบสอนสำหรับแก้ไขได้');
}

export function cancelSupervisionObservation(
	id: ObservationId,
	payload: CancelObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/cancel`,
		payload
	);
}

export function approveSupervisionObservationRequest(
	id: ObservationId,
	payload: ApproveObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/approve-request`,
		payload
	);
}

export function returnSupervisionObservationRequest(
	id: ObservationId,
	payload: ReturnObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/return-request`,
		payload
	);
}

export function submitMySupervisionEvaluation(
	id: ObservationId,
	payload: SaveEvaluationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/evaluations/me/submit`,
		payload
	);
}

export function acknowledgeSupervisionObservation(
	id: ObservationId,
	payload: AcknowledgeObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/acknowledge`,
		payload
	);
}

export async function getSupervisionCycleProgress(
	cycleId: CycleId,
	options: ApiRequestOptions = {}
): Promise<SupervisionCycleProgress> {
	const response = await apiClient.get<SupervisionCycleProgress>(
		`/api/supervision/reports/cycles/${cycleId}/progress`,
		options
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายงานรอบนิเทศได้');
}

export async function getSupervisionTeacherStatusOverview(
	cycleId: CycleId,
	options: ApiRequestOptions = {}
): Promise<SupervisionTeacherStatusRow[]> {
	const response = await apiClient.get<SupervisionTeacherStatusItems>(
		`/api/supervision/reports/cycles/${cycleId}/teacher-status`,
		options
	);
	return requireApiData(response, 'ไม่สามารถโหลดภาพรวมสถานะครูได้').items;
}

export type ResubmitSupervisionObservationRequest =
	operations['resubmitSupervisionObservationRequest']['requestBody']['content']['application/json'];

export function resubmitSupervisionObservationRequest(
	id: ObservationId
): Promise<ApiResponse<SupervisionObservation>> {
	const payload: ResubmitSupervisionObservationRequest = {};
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/resubmit-request`,
		payload
	);
}
