import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';
type Schemas = components['schemas'];
export type SupplyCategory = Schemas['SupplyCategory'];
export type SupplyCategoryWrite = Schemas['SupplyCategoryWrite'];
export type SupplyItem = Schemas['SupplyItem'];
export type SupplyItemDetail = Schemas['SupplyItemDetail'];
export type SupplyItemPage = Schemas['SupplyItemPage'];
export type SupplyItemWrite = Schemas['SupplyItemWrite'];
export type SupplyItemUpdate = Schemas['SupplyItemUpdate'];
export type SupplyVersion = Schemas['SupplyVersion'];
export type SupplyVersionPage = Schemas['SupplyVersionPage'];
export type SupplyVersionWrite = Schemas['SupplyVersionWrite'];
export type SupplyVersionAppend = Schemas['SupplyVersionAppend'];
export type SupplyBundle = Schemas['SupplyBundle'];
export type SupplyBundlePage = Schemas['SupplyBundlePage'];
export type SupplyBundleWrite = Schemas['SupplyBundleWrite'];
export type SupplyProposal = Schemas['SupplyProposal'];
export type SupplyProposalPage = Schemas['SupplyProposalPage'];
export type SupplyProposalWrite = Schemas['SupplyProposalWrite'];
export type SupplyProposalApprove = Schemas['SupplyProposalApprove'];
export type SupplyProposalReject = Schemas['SupplyProposalReject'];
const base = '/api/supply-catalog';
const path = (kind: string, id: string) => `${base}/${kind}/${encodeURIComponent(id)}`;
export type ItemFilters = {
	search?: string;
	categoryId?: string;
	favoritesOnly?: boolean;
	page?: number;
	pageSize?: number;
};
export type PageFilters = { page?: number; pageSize?: number };
export type ProposalFilters = PageFilters & { review?: boolean };
export async function listCategories(options: ApiRequestOptions = {}): Promise<SupplyCategory[]> {
	return requireApiData(
		await apiClient.get<SupplyCategory[]>(`${base}/categories`, options),
		'โหลดหมวดหมู่ไม่สำเร็จ'
	);
}
export async function saveCategory(
	id: string | null,
	payload: SupplyCategoryWrite
): Promise<SupplyCategory> {
	return requireApiData(
		id
			? await apiClient.put<SupplyCategory>(path('categories', id), payload)
			: await apiClient.post<SupplyCategory>(`${base}/categories`, payload),
		'บันทึกหมวดหมู่ไม่สำเร็จ'
	);
}
export async function listItems(
	query: ItemFilters = {},
	options: ApiRequestOptions = {}
): Promise<SupplyItemPage> {
	return requireApiData(
		await apiClient.get<SupplyItemPage>(`${base}/items`, { ...options, query }),
		'โหลดคลังพัสดุไม่สำเร็จ'
	);
}
export async function getItem(
	id: string,
	options: ApiRequestOptions = {}
): Promise<SupplyItemDetail> {
	return requireApiData(
		await apiClient.get<SupplyItemDetail>(path('items', id), options),
		'โหลดรายละเอียดพัสดุไม่สำเร็จ'
	);
}
export async function createItem(payload: SupplyItemWrite): Promise<SupplyItem> {
	return requireApiData(
		await apiClient.post<SupplyItem>(`${base}/items`, payload),
		'เพิ่มพัสดุไม่สำเร็จ'
	);
}
export async function updateItem(id: string, payload: SupplyItemUpdate): Promise<SupplyItem> {
	return requireApiData(
		await apiClient.put<SupplyItem>(path('items', id), payload),
		'บันทึกพัสดุไม่สำเร็จ'
	);
}
export async function appendVersion(
	id: string,
	payload: SupplyVersionAppend
): Promise<SupplyItemDetail> {
	return requireApiData(
		await apiClient.post<SupplyItemDetail>(`${path('items', id)}/versions`, payload),
		'เพิ่มรุ่นราคาไม่สำเร็จ'
	);
}
export async function updateVersion(
	itemId: string,
	versionId: string,
	payload: SupplyVersionWrite
): Promise<SupplyItemDetail> {
	return requireApiData(
		await apiClient.put<SupplyItemDetail>(
			`${path('items', itemId)}/versions/${encodeURIComponent(versionId)}`,
			payload
		),
		'แก้ไขรุ่นราคาไม่สำเร็จ'
	);
}
export async function setFavorite(id: string, favorite: boolean): Promise<SupplyItem> {
	return requireApiData(
		await apiClient.put<SupplyItem>(`${path('items', id)}/favorite`, { favorite }),
		'บันทึกรายการโปรดไม่สำเร็จ'
	);
}
export async function listBundles(
	query: PageFilters = {},
	options: ApiRequestOptions = {}
): Promise<SupplyBundlePage> {
	return requireApiData(
		await apiClient.get<SupplyBundlePage>(`${base}/bundles`, { ...options, query }),
		'โหลดชุดพัสดุไม่สำเร็จ'
	);
}
export async function saveBundle(
	id: string | null,
	payload: SupplyBundleWrite
): Promise<SupplyBundle> {
	return requireApiData(
		id
			? await apiClient.put<SupplyBundle>(path('bundles', id), payload)
			: await apiClient.post<SupplyBundle>(`${base}/bundles`, payload),
		'บันทึกชุดพัสดุไม่สำเร็จ'
	);
}
export async function deleteBundle(id: string, revision: number): Promise<void> {
	requireApiData(
		await apiClient.deleteWithBody<Schemas['EmptyData']>(path('bundles', id), { revision }),
		'ลบชุดพัสดุไม่สำเร็จ'
	);
}
export async function listProposals(
	query: ProposalFilters = {},
	options: ApiRequestOptions = {}
): Promise<SupplyProposalPage> {
	return requireApiData(
		await apiClient.get<SupplyProposalPage>(`${base}/proposals`, { ...options, query }),
		'โหลดข้อเสนอไม่สำเร็จ'
	);
}
export async function getProposal(
	id: string,
	options: ApiRequestOptions = {}
): Promise<SupplyProposal> {
	return requireApiData(
		await apiClient.get<SupplyProposal>(path('proposals', id), options),
		'โหลดข้อเสนอไม่สำเร็จ'
	);
}
export async function createProposal(payload: SupplyProposalWrite): Promise<SupplyProposal> {
	return requireApiData(
		await apiClient.post<SupplyProposal>(`${base}/proposals`, payload),
		'ส่งข้อเสนอไม่สำเร็จ'
	);
}
export async function approveProposal(
	id: string,
	payload: SupplyProposalApprove
): Promise<SupplyItemDetail> {
	return requireApiData(
		await apiClient.post<SupplyItemDetail>(`${path('proposals', id)}/approve`, payload),
		'อนุมัติข้อเสนอไม่สำเร็จ'
	);
}
export async function rejectProposal(
	id: string,
	payload: SupplyProposalReject
): Promise<SupplyProposal> {
	return requireApiData(
		await apiClient.post<SupplyProposal>(`${path('proposals', id)}/reject`, payload),
		'ปฏิเสธข้อเสนอไม่สำเร็จ'
	);
}

export async function findDuplicates(
	name: string,
	unit: string,
	options: ApiRequestOptions = {}
): Promise<SupplyItem[]> {
	return requireApiData(
		await apiClient.get<SupplyItem[]>(`${base}/duplicates`, { ...options, query: { name, unit } }),
		'ตรวจพัสดุที่คล้ายกันไม่สำเร็จ'
	);
}

export async function listVersions(
	id: string,
	query: PageFilters = {},
	options: ApiRequestOptions = {}
): Promise<SupplyVersionPage> {
	return requireApiData(
		await apiClient.get<SupplyVersionPage>(`${path('items', id)}/versions`, { ...options, query }),
		'โหลดประวัติราคาไม่สำเร็จ'
	);
}
