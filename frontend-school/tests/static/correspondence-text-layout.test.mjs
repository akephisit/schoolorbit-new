import test from 'node:test';
import assert from 'node:assert/strict';
import { fitAnnotationText, fitAnnotationBox } from '../../src/lib/correspondence/text-layout.ts';
test('annotation font fit uses PDF point geometry and fixed baseline spacing', () => {
	const result = fitAnnotationText('abcd efgh', 40, 40, (text, size) => (text.length * size) / 2);
	assert.equal(result.size, 14);
	assert.deepEqual(result.lines, ['abcd ', 'efgh']);
	assert.equal(result.overflow, false);
});

test('bordered boxes share the compositor four-point padding and refuse overflowing content', () => {
	const measure = (text, size) => (text.length * size) / 2;
	const text = 'abcdef';
	assert.deepEqual(fitAnnotationBox(text, 40, 40, measure, true), {
		...fitAnnotationText(text, 32, 32, measure),
		padding: 4
	});
	assert.deepEqual(fitAnnotationBox(text, 40, 40, measure), {
		...fitAnnotationText(text, 40, 40, measure),
		padding: 0
	});
	assert.equal(fitAnnotationBox('ก'.repeat(5000), 30, 30, measure, true).overflow, true);
});
test('Thai marks stay attached when wrapping and oversized text is refused', () => {
	const result = fitAnnotationText('กิข', 8, 100, (text) => Array.from(text).length * 8);
	assert.deepEqual(result.lines, ['กิ', 'ข']);
	assert.equal(
		fitAnnotationText('a'.repeat(100), 1, 1, (text, size) => text.length * size).overflow,
		true
	);
});

test('a wide single glyph and a border without inner area cannot escape the frame', () => {
	const measure = (text, size) => [...text].length * size;
	assert.equal(fitAnnotationText('ก', 4, 100, measure).overflow, true);
	const narrow = fitAnnotationText('ก', 5, 100, measure);
	assert.equal(narrow.overflow, false);
	assert.equal(narrow.size, 5);
	assert.equal(fitAnnotationBox('ก', 8, 40, measure, true).overflow, true);
	assert.equal(fitAnnotationBox('ก', 7, 40, measure, true).overflow, true);
	assert.equal(fitAnnotationBox('ก', 12, 40, measure, true).overflow, true);
	assert.equal(fitAnnotationText('กิ', 8, 100, measure).overflow, true);
	assert.deepEqual(fitAnnotationText('กิ', 8, 100, measure).lines, ['กิ']);
});

test('CRLF, lone CR and tabs use the compositor normalization before wrapping and measuring', () => {
	const measure = (text, size) => (text.length * size) / 2;
	const input = 'กิ\tข\r\nค\rง';
	const normalized = 'กิ    ข\nค\nง';
	assert.deepEqual(
		fitAnnotationText(input, 60, 100, measure),
		fitAnnotationText(normalized, 60, 100, measure)
	);
	const fitted = fitAnnotationText(input, 60, 100, measure);
	assert.equal(fitted.overflow, false);
	assert.ok(fitted.lines.every((line) => !/[\r\t]/u.test(line)));
	assert.ok(fitted.lines.every((line) => measure(line, fitted.size) <= 60));
});

test('unversioned accepted text retains height-only font layout while version one obeys width', () => {
	const measure = (text, size) => [...text].length * size;
	const legacy = fitAnnotationBox('W', 6, 40, measure, false, 'legacy-height-only');
	assert.equal(legacy.size, 14);
	assert.equal(legacy.overflow, false);
	assert.deepEqual(legacy.lines, ['W']);
	const strict = fitAnnotationBox('W', 6, 40, measure);
	assert.equal(strict.size, 6);
	assert.equal(strict.overflow, false);
	assert.equal(fitAnnotationBox('W', 4, 40, measure).overflow, true);
	assert.equal(fitAnnotationBox('W', 4, 40, measure, false, 'legacy-height-only').size, 14);
	assert.deepEqual(
		fitAnnotationBox('ก\tข\r\nค\rง', 60, 100, measure, false, 'legacy-height-only'),
		fitAnnotationBox('ก    ข\nค\nง', 60, 100, measure, false, 'legacy-height-only')
	);
});
