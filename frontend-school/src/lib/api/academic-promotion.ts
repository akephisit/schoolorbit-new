import { apiClient, requireApiData, type ApiRequestOptions } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

export type PromotionPolicy = components['schemas']['PromotionPolicyVersion'];
export type PromotionRule = components['schemas']['PromotionRuleInput'];
export type PromotionPolicyOptions = components['schemas']['PromotionPolicyOptions'];
export type PromotionPolicyInput =
	operations['createPromotionPolicy']['requestBody']['content']['application/json'];
export type GradeProgressionSet = components['schemas']['GradeProgressionSet'];
export type GradeProgressionInput = components['schemas']['GradeProgressionInput'];
export type PromotionRun = components['schemas']['PromotionRun'];
export type PromotionRunWorkspace = components['schemas']['PromotionRunWorkspace'];
export type PromotionRunStudent = components['schemas']['PromotionRunStudent'];
export type PromotionRunList = components['schemas']['PromotionRunList'];
export type PromotionRunListQuery = NonNullable<
	operations['listPromotionRuns']['parameters']['query']
>;
export type PromotionRunCreateInput = components['schemas']['CreatePromotionRunInput'];
export type PromotionRunCalculateInput = components['schemas']['CalculatePromotionRunInput'];
export type PromotionRunApproveInput = components['schemas']['ApprovePromotionRunInput'];
export type PromotionRunExecuteInput = components['schemas']['ExecutePromotionRunInput'];
export type PromotionItemReviewInput = components['schemas']['ReviewPromotionItemInput'];
export type PromotionItemReview = components['schemas']['PromotionItemReview'];
export type PromotionRunCalculation = components['schemas']['PromotionRunCalculation'];
export type PromotionExecutionResult = components['schemas']['PromotionExecutionResult'];
export type PromotionImpactWorkspace = components['schemas']['PromotionImpactWorkspace'];
export type PromotionCorrectionImpact = components['schemas']['PromotionCorrectionImpact'];
export type PromotionImpactResolution = components['schemas']['PromotionImpactResolution'];
export type ResolvePromotionImpactInput = components['schemas']['ResolvePromotionImpactInput'];
export type PromotionImpactQuery = NonNullable<
	operations['getPromotionRunImpacts']['parameters']['query']
>;

export async function getPromotionRunImpacts(
	id: string,
	query: PromotionImpactQuery = {},
	options: ApiRequestOptions = {}
): Promise<PromotionImpactWorkspace> {
	return requireApiData(
		await apiClient.get<PromotionImpactWorkspace>(
			`/api/academic/lifecycle/promotion-runs/${encodeURIComponent(id)}/impacts`,
			{ ...options, query }
		),
		'โหลดผลแก้ไขหลังเลื่อนชั้นไม่สำเร็จ'
	);
}

export async function resolvePromotionRunImpact(
	runId: string,
	impactId: string,
	input: ResolvePromotionImpactInput
): Promise<PromotionImpactResolution> {
	return requireApiData(
		await apiClient.post<PromotionImpactResolution>(
			`/api/academic/lifecycle/promotion-runs/${encodeURIComponent(runId)}/impacts/${encodeURIComponent(impactId)}/resolve`,
			input
		),
		'จัดการผลกระทบหลังแก้ผลการเรียนไม่สำเร็จ'
	);
}

export async function listPromotionRuns(
	query: PromotionRunListQuery,
	options: ApiRequestOptions = {}
): Promise<PromotionRunList> {
	return requireApiData(
		await apiClient.get<PromotionRunList>('/api/academic/lifecycle/promotion-runs', {
			...options,
			query
		}),
		'โหลดรอบเลื่อนชั้นไม่สำเร็จ'
	);
}
export async function getPromotionRun(
	id: string,
	options: ApiRequestOptions = {}
): Promise<PromotionRunWorkspace> {
	return requireApiData(
		await apiClient.get<PromotionRunWorkspace>(
			`/api/academic/lifecycle/promotion-runs/${encodeURIComponent(id)}`,
			options
		),
		'โหลดรายละเอียดรอบไม่สำเร็จ'
	);
}
export async function createPromotionRun(input: PromotionRunCreateInput): Promise<PromotionRun> {
	return requireApiData(
		await apiClient.post<PromotionRun>('/api/academic/lifecycle/promotion-runs', input),
		'สร้างรอบเลื่อนชั้นไม่สำเร็จ'
	);
}
export async function calculatePromotionRun(
	id: string,
	input: PromotionRunCalculateInput
): Promise<PromotionRunCalculation> {
	return requireApiData(
		await apiClient.post<PromotionRunCalculation>(
			`/api/academic/lifecycle/promotion-runs/${encodeURIComponent(id)}/calculate`,
			input
		),
		'คำนวณข้อเสนอไม่สำเร็จ'
	);
}
export async function reviewPromotionItem(
	runId: string,
	itemId: string,
	input: PromotionItemReviewInput
): Promise<PromotionItemReview> {
	return requireApiData(
		await apiClient.put<PromotionItemReview>(
			`/api/academic/lifecycle/promotion-runs/${encodeURIComponent(runId)}/items/${encodeURIComponent(itemId)}`,
			input
		),
		'บันทึกผลพิจารณาไม่สำเร็จ'
	);
}
export async function approvePromotionRun(
	id: string,
	input: PromotionRunApproveInput
): Promise<PromotionRun> {
	return requireApiData(
		await apiClient.post<PromotionRun>(
			`/api/academic/lifecycle/promotion-runs/${encodeURIComponent(id)}/approve`,
			input
		),
		'อนุมัติรอบไม่สำเร็จ'
	);
}
export async function executePromotionRun(
	id: string,
	input: PromotionRunExecuteInput
): Promise<PromotionExecutionResult> {
	return requireApiData(
		await apiClient.post<PromotionExecutionResult>(
			`/api/academic/lifecycle/promotion-runs/${encodeURIComponent(id)}/execute`,
			input
		),
		'ดำเนินการเลื่อนชั้นไม่สำเร็จ'
	);
}

export async function replacePromotionProgressions(
	input: components['schemas']['ReplaceGradeProgressionsRequest']
): Promise<GradeProgressionSet> {
	return requireApiData(
		await apiClient.put<GradeProgressionSet>('/api/academic/grade-progressions', input),
		'บันทึกลำดับชั้นไม่สำเร็จ'
	);
}

export async function listPromotionPolicies(
	options: ApiRequestOptions = {}
): Promise<PromotionPolicy[]> {
	return requireApiData(
		await apiClient.get<PromotionPolicy[]>('/api/academic/lifecycle/promotion-policies', options),
		'โหลดเกณฑ์การเลื่อนชั้นไม่สำเร็จ'
	);
}
export async function getPromotionPolicyOptions(
	options: ApiRequestOptions = {}
): Promise<PromotionPolicyOptions> {
	return requireApiData(
		await apiClient.get<PromotionPolicyOptions>(
			'/api/academic/lifecycle/promotion-policies/options',
			options
		),
		'โหลดชั้นและแผนการเรียนไม่สำเร็จ'
	);
}
export async function createPromotionPolicy(input: PromotionPolicyInput): Promise<PromotionPolicy> {
	return requireApiData(
		await apiClient.post<PromotionPolicy>('/api/academic/lifecycle/promotion-policies', input),
		'ยืนยันเกณฑ์ไม่สำเร็จ'
	);
}
