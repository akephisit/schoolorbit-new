import {
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];
type EvaluationQuery = NonNullable<
	operations['listLearnerEvaluationSubjects']['parameters']['query']
>;
type PolicyPath = NonNullable<operations['activateLearnerEvaluationPolicy']['parameters']['path']>;
type CatalogPath = NonNullable<operations['updateLearnerEvaluationCatalog']['parameters']['path']>;
type DomainPath = NonNullable<
	operations['getLearnerEvaluationConfiguration']['parameters']['path']
>;
type CriterionPath = NonNullable<
	operations['updateSubjectEvaluationCriterion']['parameters']['path']
>;
type GroupDomainPath = NonNullable<
	operations['getLearnerEvaluationWorkspace']['parameters']['path']
>;
type StudentSummaryPath = NonNullable<
	operations['getStudentLearnerEvaluationSummary']['parameters']['path']
>;

export type LearnerEvaluationContext = EvaluationQuery;
export type LearnerEvaluationDomain = Schemas['LearnerEvaluationDomain'];
export type LearnerEvaluationPolicyInput = Schemas['AggregationPolicyInput'];
export type LearnerEvaluationPolicyVersion = Schemas['AggregationPolicyVersion'];
export type LearnerEvaluationVersionInput = Schemas['VersionInput'];
export type LearnerEvaluationSubject = Schemas['EvaluationSubject'];
export type LearnerEvaluationCatalogCriterion = Schemas['CatalogCriterion'];
export type LearnerEvaluationCatalogInput = Schemas['CatalogInput'];
export type LearnerEvaluationControl = Schemas['EvaluationControl'];
export type LearnerEvaluationControlInput = Schemas['ControlInput'];
export type LearnerEvaluationConfiguration = Schemas['EvaluationConfiguration'];
export type LearnerEvaluationCriterion = Schemas['EvaluationCriterion'];
export type LearnerEvaluationCriterionInput = Schemas['CriterionInput'];
export type LearnerEvaluationCriterionRemoval = Schemas['CriterionRemoval'];
export type LearnerEvaluationWorkspace = Schemas['EvaluationWorkspace'];
export type LearnerEvaluationResponseBatchInput = Schemas['ResponseBatchInput'];
export type LearnerEvaluationConfirmationInput = Schemas['ConfirmationInput'];
export type LearnerEvaluationConfirmationOutcome = Schemas['ConfirmationOutcome'];
export type LearnerEvaluationLockOutcome = Schemas['LockOutcome'];
export type StudentLearnerEvaluationSummary = Schemas['StudentEvaluationSummary'];

function evaluationData<T>(request: Promise<ApiResponse<T>>, fallback: string): Promise<T> {
	return request.then((response) => requireApiData(response, fallback));
}

function query(context: LearnerEvaluationContext): EvaluationQuery {
	const value = {
		academicYearId: context.academicYearId.trim(),
		academicTermId: context.academicTermId.trim()
	} satisfies EvaluationQuery;
	if (!value.academicYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	if (!value.academicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return value;
}

function domainPath(subjectId: string, domain: LearnerEvaluationDomain): DomainPath {
	return { subject_id: subjectId, domain } satisfies DomainPath;
}

function groupDomainPath(groupId: string, domain: LearnerEvaluationDomain): GroupDomainPath {
	return { group_id: groupId, domain } satisfies GroupDomainPath;
}

export function listLearnerEvaluationPolicies(
	context: LearnerEvaluationContext,
	options: ApiRequestOptions = {}
): Promise<LearnerEvaluationPolicyVersion[]> {
	return evaluationData(
		apiClient.get<LearnerEvaluationPolicyVersion[]>('/api/academic/learner-evaluations/policies', {
			...options,
			query: query(context)
		}),
		'ไม่สามารถโหลดเกณฑ์สรุปผลการประเมินได้'
	);
}

export function createLearnerEvaluationPolicy(
	context: LearnerEvaluationContext,
	body: LearnerEvaluationPolicyInput
): Promise<LearnerEvaluationPolicyVersion> {
	const input = body satisfies LearnerEvaluationPolicyInput;
	return evaluationData(
		apiClient.post<LearnerEvaluationPolicyVersion>(
			'/api/academic/learner-evaluations/policies',
			input,
			{ query: query(context) }
		),
		'ไม่สามารถสร้างเกณฑ์สรุปผลการประเมินได้'
	);
}

export function activateLearnerEvaluationPolicy(
	policyId: string,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationVersionInput
): Promise<LearnerEvaluationPolicyVersion> {
	const path = { policy_id: policyId } satisfies PolicyPath;
	const input = body satisfies LearnerEvaluationVersionInput;
	return evaluationData(
		apiClient.post<LearnerEvaluationPolicyVersion>(
			`/api/academic/learner-evaluations/policies/${encodeURIComponent(path.policy_id)}/activate`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถเปิดใช้เกณฑ์สรุปผลการประเมินได้'
	);
}

export function listLearnerEvaluationSubjects(
	context: LearnerEvaluationContext,
	options: ApiRequestOptions = {}
): Promise<LearnerEvaluationSubject[]> {
	return evaluationData(
		apiClient.get<LearnerEvaluationSubject[]>('/api/academic/learner-evaluations/subjects', {
			...options,
			query: query(context)
		}),
		'ไม่สามารถโหลดรายวิชาสำหรับประเมินผู้เรียนได้'
	);
}

export function listLearnerEvaluationCatalog(
	context: LearnerEvaluationContext,
	options: ApiRequestOptions = {}
): Promise<LearnerEvaluationCatalogCriterion[]> {
	return evaluationData(
		apiClient.get<LearnerEvaluationCatalogCriterion[]>(
			'/api/academic/learner-evaluations/catalog',
			{
				...options,
				query: query(context)
			}
		),
		'ไม่สามารถโหลดหัวข้อประเมินของโรงเรียนได้'
	);
}

export function createLearnerEvaluationCatalog(
	context: LearnerEvaluationContext,
	body: LearnerEvaluationCatalogInput
): Promise<LearnerEvaluationCatalogCriterion> {
	const input = body satisfies LearnerEvaluationCatalogInput;
	return evaluationData(
		apiClient.post<LearnerEvaluationCatalogCriterion>(
			'/api/academic/learner-evaluations/catalog',
			input,
			{ query: query(context) }
		),
		'ไม่สามารถเพิ่มหัวข้อประเมินของโรงเรียนได้'
	);
}

export function updateLearnerEvaluationCatalog(
	criterionId: string,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationCatalogInput
): Promise<LearnerEvaluationCatalogCriterion> {
	const path = { criterion_id: criterionId } satisfies CatalogPath;
	const input = body satisfies LearnerEvaluationCatalogInput;
	return evaluationData(
		apiClient.put<LearnerEvaluationCatalogCriterion>(
			`/api/academic/learner-evaluations/catalog/${encodeURIComponent(path.criterion_id)}`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถแก้ไขหัวข้อประเมินของโรงเรียนได้'
	);
}

export function removeLearnerEvaluationCatalog(
	criterionId: string,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationVersionInput
): Promise<Schemas['EmptyData']> {
	const path = { criterion_id: criterionId } satisfies CatalogPath;
	const input = body satisfies LearnerEvaluationVersionInput;
	return evaluationData(
		apiClient.deleteWithBody<Schemas['EmptyData']>(
			`/api/academic/learner-evaluations/catalog/${encodeURIComponent(path.criterion_id)}`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถนำหัวข้อประเมินของโรงเรียนออกได้'
	);
}

export function listLearnerEvaluationControls(
	context: LearnerEvaluationContext,
	options: ApiRequestOptions = {}
): Promise<LearnerEvaluationControl[]> {
	return evaluationData(
		apiClient.get<LearnerEvaluationControl[]>('/api/academic/learner-evaluations/controls', {
			...options,
			query: query(context)
		}),
		'ไม่สามารถโหลดสถานะเปิดกรอกผลประเมินได้'
	);
}

export function updateLearnerEvaluationControl(
	domain: LearnerEvaluationDomain,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationControlInput
): Promise<LearnerEvaluationControl> {
	const path = { domain } satisfies NonNullable<
		operations['updateLearnerEvaluationControl']['parameters']['path']
	>;
	const input = body satisfies LearnerEvaluationControlInput;
	return evaluationData(
		apiClient.put<LearnerEvaluationControl>(
			`/api/academic/learner-evaluations/controls/${encodeURIComponent(path.domain)}`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถบันทึกสถานะเปิดกรอกผลประเมินได้'
	);
}

export function getLearnerEvaluationConfiguration(
	subjectId: string,
	domain: LearnerEvaluationDomain,
	context: LearnerEvaluationContext,
	options: ApiRequestOptions = {}
): Promise<LearnerEvaluationConfiguration> {
	const path = domainPath(subjectId, domain);
	return evaluationData(
		apiClient.get<LearnerEvaluationConfiguration>(
			`/api/academic/learner-evaluations/subjects/${encodeURIComponent(path.subject_id)}/domains/${encodeURIComponent(path.domain)}/configuration`,
			{ ...options, query: query(context) }
		),
		'ไม่สามารถโหลดการตั้งค่าหัวข้อประเมินรายวิชาได้'
	);
}

export function createSubjectEvaluationCriterion(
	subjectId: string,
	domain: LearnerEvaluationDomain,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationCriterionInput
): Promise<LearnerEvaluationCriterion> {
	const path = domainPath(subjectId, domain);
	const input = body satisfies LearnerEvaluationCriterionInput;
	return evaluationData(
		apiClient.post<LearnerEvaluationCriterion>(
			`/api/academic/learner-evaluations/subjects/${encodeURIComponent(path.subject_id)}/domains/${encodeURIComponent(path.domain)}/criteria`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถเพิ่มหัวข้อประเมินรายวิชาได้'
	);
}

export function updateSubjectEvaluationCriterion(
	subjectId: string,
	domain: LearnerEvaluationDomain,
	criterionId: string,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationCriterionInput
): Promise<LearnerEvaluationCriterion> {
	const path = {
		...domainPath(subjectId, domain),
		criterion_id: criterionId
	} satisfies CriterionPath;
	const input = body satisfies LearnerEvaluationCriterionInput;
	return evaluationData(
		apiClient.put<LearnerEvaluationCriterion>(
			`/api/academic/learner-evaluations/subjects/${encodeURIComponent(path.subject_id)}/domains/${encodeURIComponent(path.domain)}/criteria/${encodeURIComponent(path.criterion_id)}`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถแก้ไขหัวข้อประเมินรายวิชาได้'
	);
}

export function removeSubjectEvaluationCriterion(
	subjectId: string,
	domain: LearnerEvaluationDomain,
	criterionId: string,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationVersionInput
): Promise<LearnerEvaluationCriterionRemoval> {
	const path = {
		...domainPath(subjectId, domain),
		criterion_id: criterionId
	} satisfies CriterionPath;
	const input = body satisfies LearnerEvaluationVersionInput;
	return evaluationData(
		apiClient.deleteWithBody<LearnerEvaluationCriterionRemoval>(
			`/api/academic/learner-evaluations/subjects/${encodeURIComponent(path.subject_id)}/domains/${encodeURIComponent(path.domain)}/criteria/${encodeURIComponent(path.criterion_id)}`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถนำหัวข้อประเมินรายวิชาออกได้'
	);
}

export function getLearnerEvaluationWorkspace(
	groupId: string,
	domain: LearnerEvaluationDomain,
	context: LearnerEvaluationContext,
	options: ApiRequestOptions = {}
): Promise<LearnerEvaluationWorkspace> {
	const path = groupDomainPath(groupId, domain);
	return evaluationData(
		apiClient.get<LearnerEvaluationWorkspace>(
			`/api/academic/learner-evaluations/groups/${encodeURIComponent(path.group_id)}/domains/${encodeURIComponent(path.domain)}`,
			{ ...options, query: query(context) }
		),
		'ไม่สามารถโหลดแบบประเมินผู้เรียนได้'
	);
}

export function saveLearnerEvaluationResponses(
	groupId: string,
	domain: LearnerEvaluationDomain,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationResponseBatchInput
): Promise<LearnerEvaluationWorkspace> {
	const path = groupDomainPath(groupId, domain);
	const input = body satisfies LearnerEvaluationResponseBatchInput;
	return evaluationData(
		apiClient.put<LearnerEvaluationWorkspace>(
			`/api/academic/learner-evaluations/groups/${encodeURIComponent(path.group_id)}/domains/${encodeURIComponent(path.domain)}/responses`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถบันทึกผลประเมินผู้เรียนได้'
	);
}

export function confirmLearnerEvaluationGroup(
	groupId: string,
	domain: LearnerEvaluationDomain,
	context: LearnerEvaluationContext,
	body: LearnerEvaluationConfirmationInput
): Promise<LearnerEvaluationConfirmationOutcome> {
	const path = groupDomainPath(groupId, domain);
	const input = body satisfies LearnerEvaluationConfirmationInput;
	return evaluationData(
		apiClient.post<LearnerEvaluationConfirmationOutcome>(
			`/api/academic/learner-evaluations/groups/${encodeURIComponent(path.group_id)}/domains/${encodeURIComponent(path.domain)}/confirm`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถยืนยันผลประเมินผู้เรียนได้'
	);
}

export function lockLearnerEvaluationSubject(
	subjectId: string,
	domain: LearnerEvaluationDomain,
	context: LearnerEvaluationContext
): Promise<LearnerEvaluationLockOutcome> {
	const path = domainPath(subjectId, domain);
	return evaluationData(
		apiClient.post<LearnerEvaluationLockOutcome>(
			`/api/academic/learner-evaluations/subjects/${encodeURIComponent(path.subject_id)}/domains/${encodeURIComponent(path.domain)}/lock`,
			undefined,
			{ query: query(context) }
		),
		'ไม่สามารถล็อกผลประเมินรายวิชาได้'
	);
}

export function getStudentLearnerEvaluationSummary(
	studentAcademicYearId: string,
	context: LearnerEvaluationContext,
	options: ApiRequestOptions = {}
): Promise<StudentLearnerEvaluationSummary> {
	const path = {
		student_academic_year_id: studentAcademicYearId
	} satisfies StudentSummaryPath;
	return evaluationData(
		apiClient.get<StudentLearnerEvaluationSummary>(
			`/api/academic/learner-evaluations/students/${encodeURIComponent(path.student_academic_year_id)}/summary`,
			{ ...options, query: query(context) }
		),
		'ไม่สามารถโหลดผลประเมินสรุปของนักเรียนได้'
	);
}
