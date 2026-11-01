import assert from 'node:assert/strict';
import test from 'node:test';
import {
	MAX_FILE_BYTES,
	FILE_CHUNK_BYTES,
	BUFFER_HIGH_WATER,
	BUFFER_LOW_WATER,
	validateTransferFile,
	parseTransferControl,
	pairingCodeFromHash,
	pairingUrl
} from '../../src/lib/tools/file-transfer.ts';

const metadata = { name: 'เอกสาร.pdf', size: 0, mimeType: 'application/pdf' };
test('transfer bounds include empty and maximum size files and reject unsafe metadata', () => {
	for (const size of [0, FILE_CHUNK_BYTES, MAX_FILE_BYTES])
		assert.equal(validateTransferFile({ ...metadata, size }), null);
	for (const size of [-1, 0.5, NaN, Infinity, MAX_FILE_BYTES + 1])
		assert.ok(validateTransferFile({ ...metadata, size }));
	for (const name of ['', '  ', '../secret', 'folder\\file', 'a\u0000b', 'a\nb', 'x'.repeat(256)])
		assert.ok(validateTransferFile({ ...metadata, name }));
	assert.equal(validateTransferFile({ ...metadata, name: 'ก'.repeat(255) }), null);
	assert.equal(validateTransferFile({ ...metadata, name: '😀'.repeat(255) }), null);
	assert.ok(validateTransferFile({ ...metadata, mimeType: 'x'.repeat(129) }));
	assert.ok(BUFFER_HIGH_WATER >= FILE_CHUNK_BYTES);
	assert.ok(BUFFER_LOW_WATER < BUFFER_HIGH_WATER - FILE_CHUNK_BYTES);
});
test('only bounded exact-length end and receiver acknowledgments are valid controls', () => {
	for (const type of ['end', 'received'])
		for (const bytes of [0, MAX_FILE_BYTES])
			assert.deepEqual(parseTransferControl(JSON.stringify({ type, bytes })), { type, bytes });
	for (const value of [
		'null',
		'{}',
		'[]',
		'{',
		JSON.stringify({ type: 'end', bytes: -1 }),
		JSON.stringify({ type: 'received', bytes: MAX_FILE_BYTES + 1 }),
		JSON.stringify({ type: 'offer', bytes: 1 }),
		' '.repeat(129)
	])
		assert.throws(() => parseTransferControl(value));
});
test('pairing links keep code in the fragment and do not accept arbitrary URL payloads', () => {
	const url = pairingUrl('https://school.example', '/staff/tools/file-transfer', 'ABCD1234EF');
	assert.equal(new URL(url).search, '');
	assert.equal(pairingCodeFromHash(new URL(url).hash), 'ABCD1234EF');
	assert.equal(pairingCodeFromHash('#code=abcd1234ef'), 'ABCD1234EF');
	for (const value of [
		'#code=https%3A%2F%2Fexample.com',
		'#code=%00',
		'#code=' + 'A'.repeat(33),
		'#unrelated=123'
	])
		assert.equal(pairingCodeFromHash(value), '');
});
