import assert from 'node:assert/strict';
import test from 'node:test';
import {
	FILE_CHUNK_BYTES,
	FILE_READ_BYTES,
	BUFFER_HIGH_WATER,
	BUFFER_LOW_WATER,
	validateTransferFile,
	validateTransferBatch,
	validateBatchSummary,
	batchBytes,
	sameFiles,
	sameBatch,
	batchSummary,
	parseTransferControl,
	pairingCodeFromHash,
	pairingUrl
} from '../../src/lib/tools/file-transfer.ts';
import { TransferRate } from '../../src/lib/tools/file-transfer-rate.ts';

const metadata = { name: 'เอกสาร.pdf', size: 0, mimeType: 'application/pdf' };
test('safe browser metadata admits files and batches above former business caps', () => {
	for (const size of [0, FILE_CHUNK_BYTES, 65 * 1024 * 1024, Number.MAX_SAFE_INTEGER])
		assert.equal(validateTransferFile({ ...metadata, size }), null);
	for (const size of [-1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])
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
	assert.equal(validateTransferBatch(Array(1000).fill(metadata)), null);
	assert.ok(validateTransferBatch([]));
	const files = [
		{ ...metadata, size: 65 * 1024 * 1024 },
		{ ...metadata, size: 65 * 1024 * 1024 }
	];
	assert.equal(batchBytes(files), 130 * 1024 * 1024);
	assert.equal(validateTransferBatch(files), null);
	assert.ok(
		validateTransferBatch([
			{ ...metadata, size: Number.MAX_SAFE_INTEGER },
			{ ...metadata, size: 1 }
		])
	);
	assert.equal(
		sameFiles([metadata, { ...metadata, size: 1 }], [{ ...metadata, size: 1 }, metadata]),
		false
	);
	assert.deepEqual(batchSummary(files), { fileCount: 2, totalBytes: 130 * 1024 * 1024 });
	assert.equal(sameBatch(batchSummary(files), { fileCount: 2, totalBytes: 1 }), false);
	for (const batch of [
		{ fileCount: 0, totalBytes: 0 },
		{ fileCount: 1, totalBytes: -1 },
		{ fileCount: Number.MAX_SAFE_INTEGER + 1, totalBytes: 0 }
	])
		assert.ok(validateBatchSummary(batch));
	assert.equal(
		validateBatchSummary({
			fileCount: Number.MAX_SAFE_INTEGER,
			totalBytes: Number.MAX_SAFE_INTEGER
		}),
		null
	);
	assert.ok(FILE_READ_BYTES > FILE_CHUNK_BYTES);
	assert.ok(BUFFER_HIGH_WATER >= FILE_READ_BYTES);
	assert.ok(BUFFER_LOW_WATER < BUFFER_HIGH_WATER - FILE_CHUNK_BYTES);
});
test('v3 controls keep peer manifests bounded and permit safe large counts and byte credits', () => {
	for (const control of [
		{ type: 'manifest_start', count: 1000, totalBytes: 200 * 1024 * 1024 },
		{ type: 'manifest_file', index: 100, file: metadata },
		{ type: 'manifest_ack', count: 100 },
		{ type: 'manifest_ready', count: 1000, totalBytes: 0 },
		{ type: 'receive_ready', count: 1000, totalBytes: 0 },
		{ type: 'persisted', bytes: Number.MAX_SAFE_INTEGER },
		{ type: 'file_start', index: 100 },
		{ type: 'file_end', index: 999, bytes: 65 * 1024 * 1024 },
		{ type: 'file_received', index: 0, bytes: 0 },
		{ type: 'batch_end', count: 1000, totalBytes: 200 * 1024 * 1024 },
		{ type: 'batch_received', count: 1, totalBytes: 0 }
	])
		assert.deepEqual(parseTransferControl(JSON.stringify(control)), control);
	for (const value of [
		null,
		{},
		[],
		{ type: 'manifest_file', index: 0, file: { ...metadata, name: '../secret' } },
		{ type: 'manifest_file', index: 0, file: { ...metadata, size: '1' } },
		{ type: 'persisted', bytes: -1 },
		{ type: 'manifest_ack', count: 0.1 },
		{ type: 'file_start', index: -1 },
		{ type: 'file_start', index: Number.MAX_SAFE_INTEGER + 1 },
		{ type: 'file_end', index: 0, bytes: Number.MAX_SAFE_INTEGER + 1 },
		{ type: 'batch_end', count: 0, totalBytes: 0 },
		{ type: 'batch_received', count: 1, totalBytes: -1 },
		{ type: 'end', bytes: 1 }
	])
		assert.throws(() => parseTransferControl(JSON.stringify(value)));
	assert.throws(() => parseTransferControl(' '.repeat(4097)));
	assert.throws(() =>
		parseTransferControl(
			JSON.stringify({
				type: 'manifest_file',
				index: 0,
				file: { ...metadata, name: 'ก'.repeat(2000) }
			})
		)
	);
});
test('rates sample acknowledged decimal MB over monotonic time and become zero when idle', () => {
	const rate = new TransferRate();
	rate.reset(1000);
	assert.equal(rate.sample(1499, 1_000_000), 0);
	assert.equal(rate.sample(1500, 1_000_000), 2);
	assert.equal(rate.sample(2000, 1_500_000), 1);
	assert.equal(rate.sample(2500, 1_500_000), 0);
	assert.equal(rate.sample(3000, 0), 0);
	assert.equal(rate.sample(2500, 1_000_000), 0);
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
