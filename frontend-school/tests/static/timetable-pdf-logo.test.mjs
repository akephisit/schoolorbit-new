import assert from 'node:assert/strict';
import test from 'node:test';

test('loads a configured timetable PDF logo as a data URL', async () => {
	const module = await import('../../src/lib/utils/timetable-pdf-logo.ts').catch(() => ({}));
	assert.equal(typeof module.loadTimetablePdfLogoDataUrl, 'function');

	const result = await module.loadTimetablePdfLogoDataUrl({
		getLogoFileId: async () => 'logo-file-1',
		downloadLogo: async (fileId) => new Blob([`bytes:${fileId}`], { type: 'image/png' }),
		readLogo: async (blob) => `data:${blob.type};text,${await blob.text()}`
	});

	assert.equal(result, 'data:image/png;text,bytes:logo-file-1');
});

test('returns null only when no timetable PDF logo is configured', async () => {
	const { loadTimetablePdfLogoDataUrl } = await import('../../src/lib/utils/timetable-pdf-logo.ts');
	const result = await loadTimetablePdfLogoDataUrl({
		getLogoFileId: async () => undefined,
		downloadLogo: async () => {
			throw new Error('download must not run');
		},
		readLogo: async () => {
			throw new Error('conversion must not run');
		}
	});

	assert.equal(result, null);
});

test('propagates a configured timetable PDF logo delivery failure', async () => {
	const { loadTimetablePdfLogoDataUrl } = await import('../../src/lib/utils/timetable-pdf-logo.ts');
	const failure = new Error('public delivery failed');

	await assert.rejects(
		loadTimetablePdfLogoDataUrl({
			getLogoFileId: async () => 'logo-file-1',
			downloadLogo: async () => {
				throw failure;
			},
			readLogo: async () => 'unreachable'
		}),
		failure
	);
});
