import assert from 'node:assert/strict';
import test from 'node:test';
import {
	money,
	satang,
	subtotal,
	sumPrices,
	validateItem,
	validateItemFields,
	validatePriceFields,
	validateSourceUrl,
	validateSupplyText,
	cleanItem,
	trimSupplyText,
	emptyItem,
	itemFilters,
	matchesItem
} from '../../src/lib/supply-catalog/drafts.ts';

test('catalog decimal money preserves cents at the largest allowed bundle total', () => {
	assert.equal(satang('0.10'), 10n);
	assert.equal(subtotal('0.10', 3), '0.30');
	assert.equal(subtotal('9999999999.99', 1000000), '9999999999990000.00');
	const total = sumPrices(Array.from({ length: 100 }, () => subtotal('9999999999.99', 1000000)));
	assert.equal(total, '999999999999000000.00');
	assert.equal(money(total), '999,999,999,999,000,000.00');
	assert.equal(sumPrices(['12.34', '0.10', '0.20']), '12.64');
});
test('item validation accepts free items and refuses malformed or overflowing prices', () => {
	const draft = { ...emptyItem('category'), name: 'กระดาษ', unit: 'รีม', shop: 'ร้านทดสอบ' };
	for (const price of ['0', '0.00', '0001.2', '9999999999.99'])
		assert.equal(validateItem({ ...draft, price }), null);
	for (const price of ['-1', 'NaN', '1e3', '1.234', '1.', '.2', '10000000000.00', ' 1'])
		assert.match(validateItem({ ...draft, price }), /ราคา/);
	assert.throws(() => satang('1.234'));
});
test('catalog URL paging remains bounded and favorite filter is explicit', () => {
	assert.equal(itemFilters(new URL('https://example.invalid/?page=100001')).page, 1);
	assert.equal(itemFilters(new URL('https://example.invalid/?page=-1')).page, 1);
	assert.equal(
		itemFilters(new URL('https://example.invalid/?page=2&favoritesOnly=true')).favoritesOnly,
		true
	);
	assert.equal(
		itemFilters(new URL('https://example.invalid/?favoritesOnly=false')).favoritesOnly,
		false
	);
});

test('active catalog filter membership follows normalized name and description', () => {
	const item = {
		name: '  ＰＡＰＥＲ  A4 ',
		description: 'กระดาษสำหรับสำนักงาน',
		categoryId: 'office',
		favorite: true
	};
	const query = { search: 'paper a4', categoryId: 'office', favoritesOnly: true };
	assert.equal(matchesItem(item, query), true);
	assert.equal(matchesItem({ ...item, name: 'Pen' }, query), false);
	assert.equal(matchesItem(item, { ...query, search: 'สำนักงาน' }), true);
	assert.equal(matchesItem(item, { ...query, categoryId: 'other' }), false);
	assert.equal(matchesItem({ ...item, favorite: false }, query), false);
});

test('metadata-only validation never requires hidden price or shop fields', () => {
	const draft = {
		...emptyItem('category'),
		name: 'กระดาษ',
		unit: 'รีม',
		price: 'invalid',
		shop: ''
	};
	assert.deepEqual(validateItemFields(draft, { metadataOnly: true }), {});
	assert.deepEqual(Object.keys(validateItemFields(draft)).sort(), ['price', 'shop']);
	assert.deepEqual(validatePriceFields({ price: '0.00', shop: 'ร้านค้า' }), {});
	const allMissing = validateItemFields(emptyItem());
	assert.deepEqual(Object.keys(allMissing).sort(), ['categoryId', 'name', 'price', 'shop', 'unit']);
});
test('field text limits count Unicode characters and refuse backend-prohibited controls', () => {
	const draft = {
		...emptyItem('category'),
		name: '😀'.repeat(200),
		unit: '📦'.repeat(80),
		description: 'ก'.repeat(5000),
		price: '9999999999.99',
		shop: '🛒'.repeat(300)
	};
	assert.deepEqual(validateItemFields(draft), {});
	assert.match(validateItemFields({ ...draft, name: draft.name + 'ก' }).name, /200/);
	assert.match(validateItemFields({ ...draft, unit: draft.unit + 'ก' }).unit, /80/);
	assert.match(validateItemFields({ ...draft, shop: draft.shop + 'ก' }).shop, /300/);
	assert.match(
		validateItemFields({ ...draft, description: draft.description + 'ก' }).description,
		/5,000/
	);
	assert.match(validateItemFields({ ...draft, name: 'กระดาษ\u0000' }).name, /อักขระควบคุม/);
	assert.match(
		validateItemFields({ ...draft, description: 'รายละเอียด\u0080' }).description,
		/อักขระควบคุม/
	);
	assert.equal(
		validateItemFields({ ...draft, description: 'รายละเอียด\nบรรทัดใหม่\tข้อความ' }).description,
		undefined
	);
	assert.equal(trimSupplyText('\u0085 กระดาษ \u0085'), 'กระดาษ');
	assert.equal(trimSupplyText('\uFEFFกระดาษ'), '\uFEFFกระดาษ');
	assert.equal(cleanItem({ ...draft, name: ' \tกระดาษ\n ' }).name, 'กระดาษ');
});
test('price field validation retains exact satang boundary and rejects backend-invalid formats', () => {
	for (const price of ['0', '00.00', '9999999999.98', '9999999999.99'])
		assert.equal(validatePriceFields({ price, shop: 'ร้านค้า' }).price, undefined);
	for (const price of [
		'10000000000',
		'10000000000.00',
		'00000000001',
		'9999999999.999',
		'-0.01',
		'1e2',
		'1.',
		' 1',
		'١'
	])
		assert.match(validatePriceFields({ price, shop: 'ร้านค้า' }).price, /ราคา/);
	assert.equal(satang('9999999999.99'), 999999999999n);
});
test('optional proposal URLs align length, host and credential rules', () => {
	for (const url of [
		'',
		'  ',
		'https://shop.example.invalid/source',
		'http://shop.example.invalid'
	])
		assert.equal(validateSourceUrl(url), null);
	for (const url of [
		'https://user:pass@shop.example.invalid',
		'ftp://shop.example.invalid',
		'https://',
		'not-a-url'
	])
		assert.match(validateSourceUrl(url), /http/);
	const prefix = 'https://shop.example.invalid/';
	assert.equal(validateSourceUrl(prefix + 'ก'.repeat(2048 - prefix.length)), null);
	assert.match(validateSourceUrl(prefix + 'ก'.repeat(2048)), /2,048/);
});

test('other catalog text fields reuse the same backend character rules', () => {
	assert.equal(validateSupplyText('🛒'.repeat(120), 'ชื่อหมวดหมู่', 120), null);
	assert.match(validateSupplyText('ก'.repeat(121), 'ชื่อหมวดหมู่', 120), /120/);
	assert.match(validateSupplyText(' ', 'ชื่อชุดพัสดุ', 200), /กรอกชื่อชุดพัสดุ/);
	assert.equal(validateSupplyText('ก'.repeat(2000), 'เหตุผล', 2000), null);
	assert.match(validateSupplyText('ก'.repeat(2001), 'เหตุผล', 2000), /2,000/);
});
