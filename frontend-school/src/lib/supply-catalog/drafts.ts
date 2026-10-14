import type { SupplyItem, SupplyProposal, SupplyItemWrite } from '#lib/api/supply-catalog.js';
export type ItemDraft = Omit<SupplyItemWrite, 'imageFileId'> & { imageFileId: string | null };
export function emptyItem(categoryId = ''): ItemDraft {
	return {
		name: '',
		description: '',
		categoryId,
		unit: '',
		imageFileId: null,
		price: '',
		shop: ''
	};
}
export function itemDraft(item: SupplyItem | SupplyProposal): ItemDraft {
	return {
		name: item.name,
		description: item.description,
		categoryId: item.categoryId,
		unit: item.unit,
		imageFileId: item.imageFileId ?? null,
		price: 'latestVersion' in item ? item.latestVersion.price : item.price,
		shop: 'latestVersion' in item ? item.latestVersion.shop : item.shop
	};
}
export function validateItem(draft: ItemDraft): string | null {
	if (!draft.name.trim()) return 'กรอกชื่อพัสดุ';
	if (!draft.categoryId) return 'เลือกหมวดหมู่';
	if (!draft.unit.trim()) return 'กรอกหน่วยนับ';
	if (!/^\d{1,10}(?:\.\d{1,2})?$/.test(draft.price) || Number(draft.price) > 9999999999.99)
		return 'กรอกราคา 0–9,999,999,999.99 และมีทศนิยมไม่เกิน 2 ตำแหน่ง';
	if (!draft.shop.trim()) return 'กรอกชื่อร้านค้า';
	return null;
}
export function cleanItem(draft: ItemDraft): ItemDraft {
	return {
		...draft,
		name: draft.name.trim(),
		description: draft.description.trim(),
		unit: draft.unit.trim(),
		shop: draft.shop.trim()
	};
}
export function satang(price: string): bigint {
	if (!/^\d+(?:\.\d{1,2})?$/.test(price)) throw new Error('ราคาไม่ถูกต้อง');
	const [whole, fraction = ''] = price.split('.');
	return BigInt(whole) * 100n + BigInt(fraction.padEnd(2, '0'));
}
export function decimalPrice(value: bigint): string {
	return `${value / 100n}.${(value % 100n).toString().padStart(2, '0')}`;
}
export function subtotal(price: string, quantity: number): string {
	return decimalPrice(
		satang(price) * BigInt(Number.isSafeInteger(quantity) && quantity >= 0 ? quantity : 0)
	);
}
export function sumPrices(prices: string[]): string {
	return decimalPrice(prices.reduce((total, price) => total + satang(price), 0n));
}
export function money(price: string): string {
	const value = satang(price);
	return `${new Intl.NumberFormat('th-TH').format(value / 100n)}.${(value % 100n).toString().padStart(2, '0')}`;
}
export function message(error: unknown, fallback: string): string {
	return error instanceof Error && error.message ? error.message : fallback;
}
export function itemFilters(url: URL) {
	const page = Number(url.searchParams.get('page') ?? '1');
	return {
		search: url.searchParams.get('search')?.trim() ?? '',
		categoryId: url.searchParams.get('categoryId') ?? '',
		favoritesOnly: url.searchParams.get('favoritesOnly') === 'true',
		page: Number.isSafeInteger(page) && page > 0 && page <= 100000 ? page : 1,
		pageSize: 24
	};
}
