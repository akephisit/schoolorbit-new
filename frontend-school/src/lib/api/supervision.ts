import { apiClient, requireApiData, type ApiResponse } from '$lib/api/client';

export type SupervisionCycleStatus = 'draft' | 'open' | 'closed' | 'archived';
export type SupervisionTemplateStatus = 'draft' | 'active' | 'archived';
export type SupervisionTargetType = 'school' | 'organization_unit' | 'subject_group' | 'staff';
export type SupervisionTemplateItemType = 'rating' | 'text';
export type SupervisionTemplateStepActorKind =
	| 'supervisor'
	| 'observed_teacher'
	| 'permission'
	| 'organization_position';
export type SupervisionTemplateStepActionKind =
	| 'submit'
	| 'approve'
	| 'return_for_revision'
	| 'publish'
	| 'acknowledge'
	| 'sign';
export type SupervisionObservationStatus =
	| 'requested'
	| 'planned'
	| 'in_progress'
	| 'evaluators_submitted'
	| 'under_review'
	| 'returned'
	| 'approved'
	| 'published'
	| 'acknowledged'
	| 'completed'
	| 'cancelled';
export type SupervisionEvaluatorStatus = 'assigned' | 'draft' | 'submitted';

export interface LessonSnapshot {
	source?: string | null;
	timetableEntryId?: string | null;
	subjectName?: string | null;
	classroomLabel?: string | null;
	roomLabel?: string | null;
	observedAt?: string | null;
	periodLabel?: string | null;
}

export interface SupervisionCycleTarget {
	id: string;
	cycleId: string;
	targetType: SupervisionTargetType;
	targetId?: string | null;
	requiredObservations: number;
	priority: number;
	createdAt: string;
	updatedAt: string;
}

export interface SupervisionCycle {
	id: string;
	academicYear: number;
	semester: string;
	academicSemesterId?: string | null;
	title: string;
	description?: string | null;
	templateId: string;
	bookingOpensAt?: string | null;
	bookingClosesAt?: string | null;
	startsAt: string;
	endsAt: string;
	status: SupervisionCycleStatus;
	createdBy?: string | null;
	createdAt: string;
	updatedAt: string;
	targets: SupervisionCycleTarget[];
}

export interface SupervisionTemplateItem {
	id: string;
	sectionId: string;
	label: string;
	description?: string | null;
	itemType: SupervisionTemplateItemType;
	required: boolean;
	sortOrder: number;
	createdAt: string;
	updatedAt: string;
}

export interface SupervisionTemplateSection {
	id: string;
	templateId: string;
	title: string;
	description?: string | null;
	sortOrder: number;
	createdAt: string;
	updatedAt: string;
	items: SupervisionTemplateItem[];
}

export interface SupervisionTemplateStep {
	id: string;
	templateId: string;
	stepOrder: number;
	stepCode: string;
	label: string;
	actorKind: SupervisionTemplateStepActorKind;
	actorPermission?: string | null;
	organizationPositionCode?: string | null;
	actionKind: SupervisionTemplateStepActionKind;
	required: boolean;
	createdAt: string;
	updatedAt: string;
}

export interface SupervisionTemplate {
	id: string;
	title: string;
	description?: string | null;
	status: SupervisionTemplateStatus;
	ratingMin: number;
	ratingMax: number;
	createdBy?: string | null;
	createdAt: string;
	updatedAt: string;
	sections: SupervisionTemplateSection[];
	steps: SupervisionTemplateStep[];
}

export interface ManualLesson {
	subjectName: string;
	classroomLabel: string;
	roomLabel?: string | null;
	observedAt: string;
	periodLabel: string;
	reason: string;
}

export interface SupervisionEvaluator {
	id: string;
	observationId: string;
	evaluatorUserId: string;
	evaluatorDisplayName?: string | null;
	roleLabel?: string | null;
	isRequired: boolean;
	status: SupervisionEvaluatorStatus;
	submittedAt?: string | null;
	createdAt: string;
	updatedAt: string;
}

export interface SupervisionAction {
	id: string;
	observationId: string;
	actorUserId?: string | null;
	actorDisplayName?: string | null;
	actionKind: string;
	fromStatus?: SupervisionObservationStatus | null;
	toStatus?: SupervisionObservationStatus | null;
	comment?: string | null;
	createdAt: string;
}

export interface SupervisionObservation {
	id: string;
	cycleId: string;
	observedUserId: string;
	observedDisplayName?: string | null;
	requestedBy?: string | null;
	approvedBy?: string | null;
	templateId: string;
	timetableEntryId?: string | null;
	observedAt: string;
	manualLesson?: ManualLesson | null;
	lessonSnapshot: LessonSnapshot;
	status: SupervisionObservationStatus;
	requestedAt: string;
	approvedAt?: string | null;
	cancelledAt?: string | null;
	createdAt: string;
	updatedAt: string;
	evaluators: SupervisionEvaluator[];
	actions: SupervisionAction[];
	averageRating?: number | null;
}

export interface CreateSupervisionCycleTargetRequest {
	targetType: SupervisionTargetType;
	targetId?: string | null;
	requiredObservations?: number;
	priority?: number;
}

export interface CreateSupervisionCycleRequest {
	academicYear: number;
	semester: string;
	academicSemesterId?: string | null;
	title: string;
	description?: string | null;
	templateId: string;
	bookingOpensAt?: string | null;
	bookingClosesAt?: string | null;
	startsAt: string;
	endsAt: string;
	status?: SupervisionCycleStatus;
	targets?: CreateSupervisionCycleTargetRequest[];
}

export type UpdateSupervisionCycleRequest = Partial<CreateSupervisionCycleRequest>;

export interface CreateSupervisionTemplateItemRequest {
	label: string;
	description?: string | null;
	itemType: SupervisionTemplateItemType;
	required?: boolean;
	sortOrder?: number;
}

export interface CreateSupervisionTemplateSectionRequest {
	title: string;
	description?: string | null;
	sortOrder?: number;
	items?: CreateSupervisionTemplateItemRequest[];
}

export interface CreateSupervisionTemplateStepRequest {
	stepOrder: number;
	stepCode: string;
	label: string;
	actorKind: SupervisionTemplateStepActorKind;
	actorPermission?: string | null;
	organizationPositionCode?: string | null;
	actionKind: SupervisionTemplateStepActionKind;
	required?: boolean;
}

export interface CreateSupervisionTemplateRequest {
	title: string;
	description?: string | null;
	status?: SupervisionTemplateStatus;
	ratingMin?: number;
	ratingMax?: number;
	sections?: CreateSupervisionTemplateSectionRequest[];
	steps?: CreateSupervisionTemplateStepRequest[];
}

export type UpdateSupervisionTemplateRequest = Partial<CreateSupervisionTemplateRequest>;

export interface RequestSupervisionObservationRequest {
	cycleId: string;
	timetableEntryId?: string | null;
	observedAt?: string | null;
	manualLesson?: ManualLesson | null;
}

export type UpdateRequestedObservationRequest = Pick<
	RequestSupervisionObservationRequest,
	'timetableEntryId' | 'observedAt' | 'manualLesson'
>;

export type UpdateSupervisionObservationRequest = Partial<
	Pick<RequestSupervisionObservationRequest, 'timetableEntryId' | 'observedAt' | 'manualLesson'> & {
		templateId: string;
	}
>;

export interface EvaluatorAssignmentInput {
	evaluatorUserId: string;
	roleLabel?: string | null;
	isRequired?: boolean;
}

export interface ApproveObservationRequest {
	evaluators: EvaluatorAssignmentInput[];
}

export interface ReplaceObservationEvaluatorsRequest {
	evaluators: EvaluatorAssignmentInput[];
}

export interface ReturnObservationRequest {
	comment?: string | null;
}

export interface CancelObservationRequest {
	reason?: string | null;
}

export interface EvaluationResponseInput {
	templateItemId: string;
	ratingScore?: number | null;
	textResponse?: string | null;
}

export interface SaveEvaluationRequest {
	responses: EvaluationResponseInput[];
}

export interface AcknowledgeObservationRequest {
	comment?: string | null;
}

export interface ListSupervisionObservationsParams {
	cycleId?: string;
	status?: SupervisionObservationStatus;
}

export interface SupervisionCycleProgress {
	cycleId: string;
	totalObservations: number;
	requestedCount: number;
	plannedCount: number;
	underReviewCount: number;
	approvedCount: number;
	publishedCount: number;
	completedCount: number;
	cancelledCount: number;
	averageRating?: number | null;
}

function observationsQuery(params: ListSupervisionObservationsParams = {}): string {
	const search = new URLSearchParams();
	if (params.cycleId) search.set('cycleId', params.cycleId);
	if (params.status) search.set('status', params.status);
	const query = search.toString();
	return query ? `?${query}` : '';
}

export async function listSupervisionCycles(): Promise<SupervisionCycle[]> {
	const response = await apiClient.get<{ items: SupervisionCycle[] }>('/api/supervision/cycles');
	return requireApiData(response, 'ไม่สามารถโหลดรอบนิเทศได้').items;
}

export async function createSupervisionCycle(
	payload: CreateSupervisionCycleRequest
): Promise<ApiResponse<SupervisionCycle>> {
	return apiClient.post<SupervisionCycle>('/api/supervision/cycles', payload);
}

export async function updateSupervisionCycle(
	id: string,
	payload: UpdateSupervisionCycleRequest
): Promise<ApiResponse<SupervisionCycle>> {
	return apiClient.patch<SupervisionCycle>(`/api/supervision/cycles/${id}`, payload);
}

export async function listSupervisionTemplates(): Promise<SupervisionTemplate[]> {
	const response = await apiClient.get<{ items: SupervisionTemplate[] }>(
		'/api/supervision/templates'
	);
	return requireApiData(response, 'ไม่สามารถโหลดแบบประเมินนิเทศได้').items;
}

export async function getSupervisionTemplate(id: string): Promise<SupervisionTemplate> {
	const response = await apiClient.get<SupervisionTemplate>(`/api/supervision/templates/${id}`);
	return requireApiData(response, 'ไม่สามารถโหลดแบบประเมินนิเทศได้');
}

export async function createSupervisionTemplate(
	payload: CreateSupervisionTemplateRequest
): Promise<ApiResponse<SupervisionTemplate>> {
	return apiClient.post<SupervisionTemplate>('/api/supervision/templates', payload);
}

export async function updateSupervisionTemplate(
	id: string,
	payload: UpdateSupervisionTemplateRequest
): Promise<ApiResponse<SupervisionTemplate>> {
	return apiClient.patch<SupervisionTemplate>(`/api/supervision/templates/${id}`, payload);
}

export async function listSupervisionObservations(
	params: ListSupervisionObservationsParams = {}
): Promise<SupervisionObservation[]> {
	const response = await apiClient.get<{ items: SupervisionObservation[] }>(
		`/api/supervision/observations${observationsQuery(params)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายการนิเทศได้').items;
}

export async function getSupervisionObservation(id: string): Promise<SupervisionObservation> {
	const response = await apiClient.get<SupervisionObservation>(
		`/api/supervision/observations/${id}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายการนิเทศได้');
}

export async function requestSupervisionObservation(
	payload: RequestSupervisionObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>('/api/supervision/observations/requests', payload);
}

export async function updateRequestedSupervisionObservation(
	id: string,
	payload: UpdateRequestedObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.patch<SupervisionObservation>(
		`/api/supervision/observations/${id}/request`,
		payload
	);
}

export async function cancelRequestedSupervisionObservation(
	id: string
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.delete<SupervisionObservation>(`/api/supervision/observations/${id}/request`);
}

export async function updateSupervisionObservation(
	id: string,
	payload: UpdateSupervisionObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.patch<SupervisionObservation>(`/api/supervision/observations/${id}`, payload);
}

export async function replaceSupervisionObservationEvaluators(
	id: string,
	payload: ReplaceObservationEvaluatorsRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.put<SupervisionObservation>(
		`/api/supervision/observations/${id}/evaluators`,
		payload
	);
}

export async function cancelSupervisionObservation(
	id: string,
	payload: CancelObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/cancel`,
		payload
	);
}

export async function approveSupervisionObservationRequest(
	id: string,
	payload: ApproveObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/approve-request`,
		payload
	);
}

export async function returnSupervisionObservationRequest(
	id: string,
	payload: ReturnObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/return-request`,
		payload
	);
}

export async function saveMySupervisionEvaluation(
	id: string,
	payload: SaveEvaluationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.put<SupervisionObservation>(
		`/api/supervision/observations/${id}/evaluations/me`,
		payload
	);
}

export async function submitMySupervisionEvaluation(
	id: string,
	payload: SaveEvaluationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/evaluations/me/submit`,
		payload
	);
}

export async function certifySupervisionObservation(
	id: string
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(`/api/supervision/observations/${id}/certify`);
}

export async function approveSupervisionObservation(
	id: string
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(`/api/supervision/observations/${id}/approve`);
}

export async function acknowledgeSupervisionObservation(
	id: string,
	payload: AcknowledgeObservationRequest
): Promise<ApiResponse<SupervisionObservation>> {
	return apiClient.post<SupervisionObservation>(
		`/api/supervision/observations/${id}/acknowledge`,
		payload
	);
}

export async function getSupervisionCycleProgress(
	cycleId: string
): Promise<SupervisionCycleProgress> {
	const response = await apiClient.get<SupervisionCycleProgress>(
		`/api/supervision/reports/cycles/${cycleId}/progress`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายงานรอบนิเทศได้');
}
