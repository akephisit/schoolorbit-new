import assert from 'node:assert/strict';
import test from 'node:test';
import {
	money,
	satang,
	subtotal,
	sumPrices,
	validateItem,
	emptyItem,
	itemFilters
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
