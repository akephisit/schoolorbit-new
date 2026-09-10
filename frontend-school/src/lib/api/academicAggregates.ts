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
