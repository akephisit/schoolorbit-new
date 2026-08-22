import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const projectRoot = new URL('../../', import.meta.url);

test('shared preview source files own fitting, rendering, dialog, and fullscreen behavior', async () => {
	const surface = await readFile(
		new URL('src/lib/components/certificates/CertificatePreviewSurface.svelte', projectRoot),
		'utf8'
	);
	const dialog = await readFile(
		new URL('src/lib/components/certificates/CertificatePreviewDialog.svelte', projectRoot),
		'utf8'
	);
	const fullscreen = await readFile(
		new URL(
			'src/lib/components/certificates/CertificatePreviewFullscreenDialog.svelte',
			projectRoot
		),
		'utf8'
	);

	assert.match(surface, /calculateCertificatePreviewFit/);
	assert.match(surface, /ResizeObserver/);
	assert.match(surface, /loadCertificateRenderer/);
	assert.match(dialog, /CertificatePreviewSurface/);
	assert.match(dialog, /CertificatePreviewFullscreenDialog/);
	assert.match(fullscreen, /CertificatePreviewSurface/);
});

test('preview fit uses the limiting dimension and preserves landscape ratio', async () => {
	const { calculateCertificatePreviewFit } =
		await import('../../src/lib/certificates/preview-fit.ts');
	const fit = calculateCertificatePreviewFit({
		availableWidth: 960,
		availableHeight: 540,
		pageWidthPoints: 842,
		pageHeightPoints: 595,
		devicePixelRatio: 2
	});
	assert.ok(fit);
	assert.ok(Math.abs(fit.cssHeight - 540) < 1e-9);
	assert.ok(Math.abs(fit.cssWidth / fit.cssHeight - 842 / 595) < 1e-9);
	assert.ok(fit.renderScale <= 2);
});

test('preview fit uses width for portrait paper and rejects an unmeasured stage', async () => {
	const { calculateCertificatePreviewFit } =
		await import('../../src/lib/certificates/preview-fit.ts');
	const portrait = calculateCertificatePreviewFit({
		availableWidth: 360,
		availableHeight: 700,
		pageWidthPoints: 595,
		pageHeightPoints: 842,
		devicePixelRatio: 1
	});
	assert.ok(portrait);
	assert.ok(Math.abs(portrait.cssWidth - 360) < 1e-9);
	assert.equal(
		calculateCertificatePreviewFit({
			availableWidth: 0,
			availableHeight: 700,
			pageWidthPoints: 595,
			pageHeightPoints: 842,
			devicePixelRatio: 1
		}),
		null
	);
});

test('preview fit caps high-DPI rendering without changing logical size', async () => {
	const { calculateCertificatePreviewFit } =
		await import('../../src/lib/certificates/preview-fit.ts');
	const fit = calculateCertificatePreviewFit({
		availableWidth: 842,
		availableHeight: 595,
		pageWidthPoints: 842,
		pageHeightPoints: 595,
		devicePixelRatio: 4
	});
	assert.deepEqual(fit, {
		logicalScale: 1,
		cssWidth: 842,
		cssHeight: 595,
		renderScale: 2
	});
});
