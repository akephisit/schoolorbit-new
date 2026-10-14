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
export type ItemField = 'name' | 'description' | 'categoryId' | 'unit' | 'price' | 'shop';
export type ItemFieldErrors = Partial<Record<ItemField, string>>;
export type PriceFieldErrors = Partial<Record<'price' | 'shop', string>>;
export function trimSupplyText(value: string): string {
	return value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, '');
}
export function validateSupplyText(
	value: string,
	label: string,
	max: number,
	required = true
): string | null {
	const trimmed = trimSupplyText(value);
	if (required && !trimmed) return `กรอก${label}`;
	if (Array.from(trimmed).length > max)
		return `${label}ต้องไม่เกิน ${max.toLocaleString('th-TH')} ตัวอักษร`;
	if (/\p{Cc}/u.test(trimmed.replace(/[\n\t]/g, ''))) return `${label}มีอักขระควบคุมที่ใช้ไม่ได้`;
	return null;
}
export function validatePriceFields(draft: { price: string; shop: string }): PriceFieldErrors {
	const errors: PriceFieldErrors = {};
	if (!draft.price) errors.price = 'กรอกราคาต่อหน่วย';
	else if (
		!/^[0-9]{1,10}(?:\.[0-9]{1,2})?$/.test(draft.price) ||
		satang(draft.price) > 999999999999n
	)
		errors.price = 'กรอกราคา 0–9,999,999,999.99 และมีทศนิยมไม่เกิน 2 ตำแหน่ง';
	const shop = validateSupplyText(draft.shop, 'ชื่อร้านค้า', 300, true);
	if (shop) errors.shop = shop;
	return errors;
}
export function validateItemFields(
	draft: ItemDraft,
	{ metadataOnly = false }: { metadataOnly?: boolean } = {}
): ItemFieldErrors {
	const errors: ItemFieldErrors = {};
	const name = validateSupplyText(draft.name, 'ชื่อพัสดุ', 200, true);
	const description = validateSupplyText(draft.description, 'รายละเอียด', 5000, false);
	const unit = validateSupplyText(draft.unit, 'หน่วยนับ', 80, true);
	if (name) errors.name = name;
	if (description) errors.description = description;
	if (!draft.categoryId) errors.categoryId = 'เลือกหมวดหมู่';
	if (unit) errors.unit = unit;
	return metadataOnly ? errors : { ...errors, ...validatePriceFields(draft) };
}
export function validateSourceUrl(value: string): string | null {
	const trimmed = trimSupplyText(value);
	if (!trimmed) return null;
	const error = validateSupplyText(trimmed, 'ลิงก์แหล่งที่มา', 2048, false);
	if (error) return error;
	const invalid = 'กรอกลิงก์ http หรือ https ที่ไม่มีชื่อผู้ใช้หรือรหัสผ่าน';
	try {
		const url = new URL(trimmed);
		if (
			['https:', 'http:'].includes(url.protocol) &&
			url.hostname &&
			!url.username &&
			!url.password
		)
			return null;
	} catch {
		return invalid;
	}
	return invalid;
}
export function validateItem(draft: ItemDraft): string | null {
	return Object.values(validateItemFields(draft))[0] ?? null;
}
export function cleanItem(draft: ItemDraft): ItemDraft {
	return {
		...draft,
		name: trimSupplyText(draft.name),
		description: trimSupplyText(draft.description),
		unit: trimSupplyText(draft.unit),
		shop: trimSupplyText(draft.shop)
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

export function normalizeSearch(value: string): string {
	return value.normalize('NFKC').trim().replace(/\s+/gu, ' ').toLowerCase();
}
export function matchesItem(
	item: SupplyItem,
	query: { search: string; categoryId: string; favoritesOnly: boolean }
): boolean {
	const search = normalizeSearch(query.search);
	return (
		(!query.categoryId || item.categoryId === query.categoryId) &&
		(!query.favoritesOnly || item.favorite) &&
		(!search ||
			normalizeSearch(item.name).includes(search) ||
			item.description.normalize('NFKC').toLowerCase().includes(search))
	);
}
