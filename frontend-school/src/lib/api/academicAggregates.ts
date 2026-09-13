import {
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';
import type { AcademicResultContext } from '$lib/api/academicResults';

type Schemas = components['schemas'];
type ContextQuery = NonNullable<operations['listTermAggregateRevisions']['parameters']['query']>;
type PreviewQuery = NonNullable<operations['previewTermAggregate']['parameters']['query']>;
type StudentPath = operations['listTermAggregateRevisions']['parameters']['path'];
export type AggregatePolicyInput = Schemas['AggregatePolicyInput'];
export type AggregatePolicyVersion = Schemas['AggregatePolicyVersion'];
export type TermAggregatePreview = Schemas['TermAggregatePreview'];
export type AggregateLockInput = Schemas['AggregateLockInput'];
export type TermAggregateRevision = Schemas['TermAggregateRevision'];
export type AnnualResultPreview = Schemas['AnnualResultPreview'];
export type AnnualResultStudent = Schemas['AnnualResultStudent'];
export type AnnualResultRevision = Schemas['AnnualResultRevision'];
export type AnnualLockInput = Schemas['AnnualLockInput'];
export type AnnualResultContext = NonNullable<
	operations['previewAnnualResult']['parameters']['query']
>;

function annualContextQuery(context: AnnualResultContext): AnnualResultContext {
	const academicYearId = context.academicYearId.trim();
	if (!academicYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	return { academicYearId };
}

export function listAnnualResultStudents(
	context: AnnualResultContext,
	options: ApiRequestOptions = {}
): Promise<AnnualResultStudent[]> {
	return data(
		apiClient.get<AnnualResultStudent[]>('/api/academic/results/annual-students', {
			...options,
			query: annualContextQuery(context)
		}),
		'ไม่สามารถโหลดรายชื่อนักเรียนสำหรับสรุปผลรายปีได้'
	);
}

export function previewAnnualResult(
	studentYearId: string,
	context: AnnualResultContext,
	options: ApiRequestOptions = {}
): Promise<AnnualResultPreview> {
	return data(
		apiClient.get<AnnualResultPreview>(`${studentEndpoint(studentYearId)}/annual-preview`, {
			...options,
			query: annualContextQuery(context)
		}),
		'ไม่สามารถคำนวณผลรวมรายปีได้'
	);
}

export function listAnnualResultRevisions(
	studentYearId: string,
	context: AnnualResultContext,
	options: ApiRequestOptions = {}
): Promise<AnnualResultRevision[]> {
	return data(
		apiClient.get<AnnualResultRevision[]>(`${studentEndpoint(studentYearId)}/annual-revisions`, {
			...options,
			query: annualContextQuery(context)
		}),
		'ไม่สามารถโหลดประวัติผลรายปีได้'
	);
}

export function lockAnnualResult(
	studentYearId: string,
	context: AnnualResultContext,
	input: AnnualLockInput,
	options: ApiRequestOptions = {}
): Promise<AnnualResultRevision> {
	return data(
		apiClient.post<AnnualResultRevision>(
			`${studentEndpoint(studentYearId)}/annual-revisions`,
			input,
			{
				...options,
				query: annualContextQuery(context)
			}
		),
		'ไม่สามารถยืนยันผลรายปีได้'
	);
}

function contextQuery(context: AcademicResultContext): ContextQuery {
	const query = {
		academicYearId: context.academicYearId.trim(),
		academicTermId: context.academicTermId.trim()
	} satisfies ContextQuery;
	if (!query.academicYearId || !query.academicTermId)
		throw new Error('กรุณาเลือกปีและภาคเรียนก่อน');
	return query;
}

function studentEndpoint(studentYearId: string): string {
	const path = { student_year_id: studentYearId } satisfies StudentPath;
	return `/api/academic/results/students/${encodeURIComponent(path.student_year_id)}`;
}

function data<T>(request: Promise<ApiResponse<T>>, fallback: string): Promise<T> {
	return request.then((response) => requireApiData(response, fallback));
}

export function listAggregatePolicies(
	options: ApiRequestOptions = {}
): Promise<AggregatePolicyVersion[]> {
	return data(
		apiClient.get<AggregatePolicyVersion[]>('/api/academic/results/aggregate-policies', options),
		'ไม่สามารถโหลดนโยบายผลรวมได้'
	);
}

/** Approval creates an immutable policy version; it never activates or closes a term. */
export function createAggregatePolicy(
	input: AggregatePolicyInput,
	options: ApiRequestOptions = {}
): Promise<AggregatePolicyVersion> {
	return data(
		apiClient.post<AggregatePolicyVersion>(
			'/api/academic/results/aggregate-policies',
			input,
			options
		),
		'ไม่สามารถบันทึกนโยบายผลรวมได้'
	);
}

export function previewTermAggregate(
	studentYearId: string,
	context: AcademicResultContext,
	policyId: string,
	options: ApiRequestOptions = {}
): Promise<TermAggregatePreview> {
	const query = { ...contextQuery(context), policyId } satisfies PreviewQuery;
	return data(
		apiClient.get<TermAggregatePreview>(`${studentEndpoint(studentYearId)}/aggregate-preview`, {
			...options,
			query
		}),
		'ไม่สามารถคำนวณผลรวมรายภาคได้'
	);
}

/** Preserve requestId when retrying an uncertain response; do not create a new revision. */
export function lockTermAggregate(
	studentYearId: string,
	context: AcademicResultContext,
	input: AggregateLockInput,
	options: ApiRequestOptions = {}
): Promise<TermAggregateRevision> {
	return data(
		apiClient.post<TermAggregateRevision>(
			`${studentEndpoint(studentYearId)}/aggregate-revisions`,
			input,
			{ ...options, query: contextQuery(context) }
		),
		'ไม่สามารถล็อกผลรวมรายภาคได้'
	);
}

export function listTermAggregateRevisions(
	studentYearId: string,
	context: AcademicResultContext,
	options: ApiRequestOptions = {}
): Promise<TermAggregateRevision[]> {
	return data(
		apiClient.get<TermAggregateRevision[]>(
			`${studentEndpoint(studentYearId)}/aggregate-revisions`,
			{ ...options, query: contextQuery(context) }
		),
		'ไม่สามารถโหลดประวัติผลรวมรายภาคได้'
	);
}

export type AggregateStudent = Schemas['AggregateStudent'];
export function listAggregateStudents(
	context: AcademicResultContext,
	options: ApiRequestOptions = {}
): Promise<AggregateStudent[]> {
	const query = contextQuery(context) satisfies NonNullable<
		operations['listAggregateStudents']['parameters']['query']
	>;
	return data(
		apiClient.get<AggregateStudent[]>('/api/academic/results/aggregate-students', {
			...options,
			query
		}),
		'โหลดรายการผลสรุปไม่สำเร็จ'
	);
}
