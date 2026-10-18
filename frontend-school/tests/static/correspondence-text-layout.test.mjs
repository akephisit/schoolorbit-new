import test from 'node:test';
import assert from 'node:assert/strict';
import { fitAnnotationText } from '../../src/lib/correspondence/text-layout.ts';
test('annotation font fit uses PDF point geometry and fixed baseline spacing', () => {
	const result = fitAnnotationText('abcd efgh', 40, 40, (text, size) => (text.length * size) / 2);
	assert.equal(result.size, 14);
	assert.deepEqual(result.lines, ['abcd ', 'efgh']);
	assert.equal(result.overflow, false);
});
test('Thai marks stay attached when wrapping and oversized text is refused', () => {
	const result = fitAnnotationText('กิข', 8, 100, (text) => Array.from(text).length * 8);
	assert.deepEqual(result.lines, ['กิ', 'ข']);
	assert.equal(
		fitAnnotationText('a'.repeat(100), 1, 1, (text, size) => text.length * size).overflow,
		true
	);
});
