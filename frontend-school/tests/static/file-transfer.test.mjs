import assert from 'node:assert/strict';
import test from 'node:test';
import {
	MAX_FILE_BYTES,
	MAX_BATCH_BYTES,
	MAX_BATCH_FILES,
	FILE_CHUNK_BYTES,
	FILE_READ_BYTES,
	BUFFER_HIGH_WATER,
	BUFFER_LOW_WATER,
	validateTransferFile,
	validateTransferBatch,
	batchBytes,
	sameFiles,
	parseTransferControl,
	pairingCodeFromHash,
	pairingUrl
} from '../../src/lib/tools/file-transfer.ts';

const metadata = { name: 'เอกสาร.pdf', size: 0, mimeType: 'application/pdf' };
test('file and ordered batch bounds include zero files and duplicate names but reject unsafe metadata and aggregate overflow', () => {
	for (const size of [0, FILE_CHUNK_BYTES, MAX_FILE_BYTES])
		assert.equal(validateTransferFile({ ...metadata, size }), null);
	for (const size of [-1, 0.5, NaN, Infinity, MAX_FILE_BYTES + 1])
		assert.ok(validateTransferFile({ ...metadata, size }));
	for (const name of [
		'',
		'  ',
		'.',
		'..',
		'../secret',
		'folder\\file',
		'a\u0000b',
		'a\nb',
		'x'.repeat(256)
	])
		assert.ok(validateTransferFile({ ...metadata, name }));
	assert.equal(validateTransferFile({ ...metadata, name: '😀'.repeat(255) }), null);
	assert.ok(validateTransferFile({ ...metadata, mimeType: 'x'.repeat(129) }));
	assert.equal(validateTransferBatch([metadata, metadata]), null);
	assert.equal(validateTransferBatch(Array(MAX_BATCH_FILES).fill(metadata)), null);
	assert.ok(validateTransferBatch([]));
	assert.ok(validateTransferBatch(Array(MAX_BATCH_FILES + 1).fill(metadata)));
	const maximum = [
		{ ...metadata, size: MAX_FILE_BYTES },
		{ ...metadata, size: MAX_FILE_BYTES }
	];
	assert.equal(batchBytes(maximum), MAX_BATCH_BYTES);
	assert.equal(validateTransferBatch(maximum), null);
	assert.ok(validateTransferBatch([...maximum, { ...metadata, size: 1 }]));
	assert.equal(
		sameFiles([metadata, { ...metadata, size: 1 }], [{ ...metadata, size: 1 }, metadata]),
		false
	);
	assert.ok(FILE_READ_BYTES > FILE_CHUNK_BYTES);
	assert.ok(BUFFER_HIGH_WATER >= FILE_READ_BYTES);
	assert.ok(BUFFER_LOW_WATER < BUFFER_HIGH_WATER - FILE_CHUNK_BYTES);
});
test('v2 controls require bounded indices, exact file acknowledgments and nonempty bounded batch totals', () => {
	for (const control of [
		{ type: 'file_start', index: 0 },
		{ type: 'file_end', index: 19, bytes: MAX_FILE_BYTES },
		{ type: 'file_received', index: 0, bytes: 0 },
		{ type: 'batch_end', count: 20, totalBytes: MAX_BATCH_BYTES },
		{ type: 'batch_received', count: 1, totalBytes: 0 }
	])
		assert.deepEqual(parseTransferControl(JSON.stringify(control)), control);
	for (const value of [
		null,
		{},
		[],
		{ type: 'file_start', index: -1 },
		{ type: 'file_start', index: 20 },
		{ type: 'file_end', index: 0, bytes: MAX_FILE_BYTES + 1 },
		{ type: 'batch_end', count: 0, totalBytes: 0 },
		{ type: 'batch_received', count: 1, totalBytes: MAX_BATCH_BYTES + 1 },
		{ type: 'end', bytes: 1 },
		{ type: 'received', bytes: 1 }
	])
		assert.throws(() => parseTransferControl(JSON.stringify(value)));
	assert.throws(() => parseTransferControl(' '.repeat(257)));
});
test('pairing links keep the code in a fragment and reject arbitrary URL payloads', () => {
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
