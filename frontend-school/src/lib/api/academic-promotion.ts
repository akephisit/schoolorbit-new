import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components, operations } from '#lib/api/generated/school-api.js';
export type PromotionRun = components['schemas']['PromotionRun'];
export type PromotionRunWorkspace = components['schemas']['PromotionRunWorkspace'];
export type PromotionRunList = components['schemas']['PromotionRunList'];
export type PromotionRunListQuery = NonNullable<
	operations['listPromotionRuns']['parameters']['query']
>;
export type PromotionImpactWorkspace = components['schemas']['PromotionImpactWorkspace'];
export type PromotionImpactQuery = NonNullable<
	operations['getPromotionRunImpacts']['parameters']['query']
>;
export async function listPromotionRuns(
	query: PromotionRunListQuery,
	options: ApiRequestOptions = {}
): Promise<PromotionRunList> {
	return requireApiData(
		await apiClient.get<PromotionRunList>('/api/academic/lifecycle/promotion-runs', {
			...options,
			query
		}),
		'โหลดประวัติขึ้นปีใหม่ไม่สำเร็จ'
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
		'โหลดประวัติขึ้นปีใหม่ไม่สำเร็จ'
	);
}
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
		'โหลดประวัติผลแก้ไขไม่สำเร็จ'
	);
}
