import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';
type Schemas = components['schemas'];
export type BudgetCatalogs = Schemas['BudgetCatalogs'];
export type BudgetCatalogItem = Schemas['BudgetCatalogItem'];
export type BudgetFiscalYear = Schemas['BudgetFiscalYear'];
export type BudgetSettings = Schemas['BudgetSettings'];
export type SaveBudgetSettingsRequest = Schemas['SaveBudgetSettingsRequest'];
export type BudgetProjectInput = Schemas['BudgetProjectInput'];
export type CreateBudgetProjectRequest = Schemas['CreateBudgetProjectRequest'];
export type SaveBudgetProjectRequest = Schemas['SaveBudgetProjectRequest'];
export type BudgetActivityInput = Schemas['BudgetActivityInput'];
export type BudgetActivity = Schemas['BudgetActivity'];
export type BudgetPlanLineInput = Schemas['BudgetPlanLineInput'];
export type BudgetPlanLine = Schemas['BudgetPlanLine'];
export type BudgetProjectSummary = Schemas['BudgetProjectSummary'];
export type BudgetProjectWorkspace = Schemas['BudgetProjectWorkspace'];
export type BudgetProjectPage = Schemas['BudgetProjectPage'];
export type BudgetMutationRequest = Schemas['BudgetMutationRequest'];
export type BudgetActionRequest = Schemas['BudgetActionRequest'];
export type StartBudgetAmendmentRequest = Schemas['StartBudgetAmendmentRequest'];
export type BudgetRequestSummary = Schemas['BudgetRequestSummary'];
export type BudgetRequestPage = Schemas['BudgetRequestPage'];
export type BudgetSpendingRequestDetail = Schemas['BudgetSpendingRequestDetail'];
export type SaveBudgetSpendingRequestRequest = Schemas['SaveBudgetSpendingRequestRequest'];
export type BudgetPayment = Schemas['BudgetPayment'];
export type BudgetPaymentPage = Schemas['BudgetPaymentPage'];
export type BudgetPaymentAllocationInput = Schemas['BudgetPaymentAllocationInput'];
export type CreateBudgetPaymentRequest = Schemas['CreateBudgetPaymentRequest'];
export type RefundBudgetPaymentRequest = Schemas['RefundBudgetPaymentRequest'];
export type CloseBudgetRequestRequest = Schemas['CloseBudgetRequestRequest'];
export type BudgetAdjustmentDetail = Schemas['BudgetAdjustmentDetail'];
export type BudgetAdjustmentPage = Schemas['BudgetAdjustmentPage'];
export type BudgetAllocationRef = Schemas['BudgetAllocationRef'];
export type SaveBudgetAdjustmentRequest = Schemas['SaveBudgetAdjustmentRequest'];
export type BudgetDashboard = Schemas['BudgetDashboard'];
export type BudgetDashboardRow = Schemas['BudgetDashboardRow'];
export type BudgetBalance = Schemas['BudgetBalance'];
export type BudgetListQuery = Schemas['BudgetListQuery'];
export type BudgetReportQuery = Schemas['BudgetReportQuery'];
export type BudgetProcessDetail = Schemas['BudgetProcessDetail'];
export type BudgetHistoryPage = Schemas['BudgetHistoryPage'];
export type ProcessTemplateStep = Schemas['ProcessTemplateStep'];
export type BudgetStatus = Schemas['BudgetStatus'];
const base = '/api/budget';
const path = (kind: string, id: string) => `${base}/${kind}/${encodeURIComponent(id)}`;
const read = async <T>(url: string, options: ApiRequestOptions = {}): Promise<T> =>
	requireApiData(
		await apiClient.get<T>(url, {
			...options,
			query: options.query
				? Object.fromEntries(Object.entries(options.query).filter(([, value]) => value !== ''))
				: undefined
		}),
		'โหลดข้อมูลงบประมาณไม่สำเร็จ'
	);
const post = async <T>(url: string, input: object): Promise<T> =>
	requireApiData(await apiClient.post<T>(url, input), 'บันทึกข้อมูลงบประมาณไม่สำเร็จ');
export const getBudgetCatalogs = (options: ApiRequestOptions = {}) =>
	read<BudgetCatalogs>(`${base}/catalogs`, options);
export const getBudgetSettings = (options: ApiRequestOptions = {}) =>
	read<BudgetSettings>(`${base}/settings`, options);
export async function saveBudgetSettings(
	input: SaveBudgetSettingsRequest
): Promise<BudgetSettings> {
	return requireApiData(
		await apiClient.put<BudgetSettings>(`${base}/settings`, input),
		'บันทึกการตั้งค่าไม่สำเร็จ'
	);
}
export const listBudgetProjects = (options: ApiRequestOptions = {}) =>
	read<BudgetProjectPage>(`${base}/projects`, options);
export const getBudgetProject = (id: string, options: ApiRequestOptions = {}) =>
	read<BudgetProjectWorkspace>(path('projects', id), options);
export const createBudgetProject = (input: CreateBudgetProjectRequest) =>
	post<BudgetProjectWorkspace>(`${base}/projects`, input);
export async function saveBudgetProject(
	id: string,
	input: SaveBudgetProjectRequest
): Promise<BudgetProjectWorkspace> {
	return requireApiData(
		await apiClient.put<BudgetProjectWorkspace>(path('projects', id), input),
		'บันทึกโครงการไม่สำเร็จ'
	);
}
export const submitBudgetProject = (id: string, input: BudgetMutationRequest) =>
	post<BudgetProjectWorkspace>(`${path('projects', id)}/submit`, input);
export const amendBudgetProject = (id: string, input: StartBudgetAmendmentRequest) =>
	post<BudgetProjectWorkspace>(`${path('projects', id)}/amendments`, input);
export const actOnBudgetProject = (id: string, input: BudgetActionRequest) =>
	post<BudgetProjectWorkspace>(`${path('projects', id)}/actions`, input);
export const listBudgetRequests = (options: ApiRequestOptions = {}) =>
	read<BudgetRequestPage>(`${base}/requests`, options);
export const getBudgetRequest = (id: string, options: ApiRequestOptions = {}) =>
	read<BudgetSpendingRequestDetail>(path('requests', id), options);
export const createBudgetRequest = (projectId: string, input: SaveBudgetSpendingRequestRequest) =>
	post<BudgetSpendingRequestDetail>(`${path('projects', projectId)}/requests`, input);
export async function saveBudgetRequest(
	id: string,
	input: SaveBudgetSpendingRequestRequest
): Promise<BudgetSpendingRequestDetail> {
	return requireApiData(
		await apiClient.put<BudgetSpendingRequestDetail>(path('requests', id), input),
		'บันทึกคำขอใช้เงินไม่สำเร็จ'
	);
}
export const submitBudgetRequest = (id: string, input: BudgetMutationRequest) =>
	post<BudgetSpendingRequestDetail>(`${path('requests', id)}/submit`, input);
export const actOnBudgetRequest = (id: string, input: BudgetActionRequest) =>
	post<BudgetSpendingRequestDetail>(`${path('requests', id)}/actions`, input);
export const closeBudgetRequest = (id: string, input: CloseBudgetRequestRequest) =>
	post<BudgetSpendingRequestDetail>(`${path('requests', id)}/close`, input);
export const listBudgetPayments = (id: string, options: ApiRequestOptions = {}) =>
	read<BudgetPaymentPage>(`${path('requests', id)}/payments`, options);
export const createBudgetPayment = (id: string, input: CreateBudgetPaymentRequest) =>
	post<BudgetSpendingRequestDetail>(`${path('requests', id)}/payments`, input);
export const refundBudgetPayment = (
	requestId: string,
	paymentId: string,
	input: RefundBudgetPaymentRequest
) =>
	post<BudgetSpendingRequestDetail>(
		`${path('requests', requestId)}/payments/${encodeURIComponent(paymentId)}/refunds`,
		input
	);
export const listBudgetAdjustments = (options: ApiRequestOptions = {}) =>
	read<BudgetAdjustmentPage>(`${base}/adjustments`, options);
export const getBudgetAdjustment = (id: string, options: ApiRequestOptions = {}) =>
	read<BudgetAdjustmentDetail>(path('adjustments', id), options);
export const createBudgetAdjustment = (input: SaveBudgetAdjustmentRequest) =>
	post<BudgetAdjustmentDetail>(`${base}/adjustments`, input);
export const submitBudgetAdjustment = (id: string, input: BudgetMutationRequest) =>
	post<BudgetAdjustmentDetail>(`${path('adjustments', id)}/submit`, input);
export const actOnBudgetAdjustment = (id: string, input: BudgetActionRequest) =>
	post<BudgetAdjustmentDetail>(`${path('adjustments', id)}/actions`, input);
export const getBudgetDashboard = (options: ApiRequestOptions = {}) =>
	read<BudgetDashboard>(`${base}/dashboard`, options);
export const getBudgetProcess = (
	kind: 'projects' | 'requests' | 'adjustments',
	id: string,
	options: ApiRequestOptions = {}
) => read<BudgetProcessDetail>(`${path(kind, id)}/process`, options);
export const getBudgetHistory = (
	kind: 'projects' | 'requests' | 'adjustments',
	id: string,
	options: ApiRequestOptions = {}
) => read<BudgetHistoryPage>(`${path(kind, id)}/history`, options);
export async function exportBudgetReport(
	query: BudgetReportQuery,
	signal?: AbortSignal
): Promise<Blob> {
	return requireApiData(
		await apiClient.getBlob(`${base}/reports/export`, { query, signal }),
		'ส่งออกรายงานไม่สำเร็จ'
	);
}
