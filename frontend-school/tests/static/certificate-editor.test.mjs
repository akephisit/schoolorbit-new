import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const projectRoot = new URL('../../', import.meta.url);

function textElement(overrides = {}) {
	return {
		type: 'text',
		id: 'text-1',
		content: 'มอบให้ {ชื่อ} {นามสกุล}',
		frame: { x: 72, y: 72, width: 240, height: 60 },
		rotation: 0,
		fontSource: { type: 'built_in' },
		fontFamily: 'Sarabun',
		fontWeight: 400,
		fontSize: 32,
		minFontSize: 14,
		color: '#183153',
		alignment: 'center',
		lineHeight: 1.2,
		autoShrink: true,
		shadow: null,
		...overrides
	};
}

test('dragging converts screen pixels to page points', async () => {
	const { moveElement } = await import('../../src/lib/certificates/editor-state.ts');
	const moved = moveElement(textElement(), { dxPixels: 40, dyPixels: -20 }, 2);
	assert.deepEqual(moved.frame, { x: 92, y: 62, width: 240, height: 60 });
});

test('eight-direction resize keeps the opposite edge and minimum frame size', async () => {
	const { MIN_CERTIFICATE_FRAME_POINTS, resizeElement } =
		await import('../../src/lib/certificates/editor-state.ts');
	const resized = resizeElement(textElement(), { handle: 'nw', dxPixels: 300, dyPixels: 100 }, 1);
	assert.deepEqual(resized.frame, {
		x: 300,
		y: 120,
		width: MIN_CERTIFICATE_FRAME_POINTS,
		height: MIN_CERTIFICATE_FRAME_POINTS
	});
});

test('rotation and duplication are deterministic and never reuse element ids', async () => {
	const { duplicateElement, rotateElement } =
		await import('../../src/lib/certificates/editor-state.ts');
	assert.equal(rotateElement(textElement(), 450).rotation, 90);
	const duplicate = duplicateElement(textElement(), () => 'text-copy');
	assert.equal(duplicate.id, 'text-copy');
	assert.deepEqual(duplicate.frame, { x: 84, y: 84, width: 240, height: 60 });
});

test('rotated resize converts pointer movement into the element local axes', async () => {
	const { resizeElement, screenDeltaToElementAxes } =
		await import('../../src/lib/certificates/editor-state.ts');
	const localDelta = screenDeltaToElementAxes({ dxPixels: 0, dyPixels: 20 }, 90);
	assert.ok(Math.abs(localDelta.dxPixels - 20) < 1e-9);
	assert.ok(Math.abs(localDelta.dyPixels) < 1e-9);
	const resized = resizeElement(textElement({ rotation: 90 }), { handle: 'e', ...localDelta }, 1);
	assert.equal(resized.frame.width, 260);
	const westHandle = (element) => {
		const radians = (element.rotation * Math.PI) / 180;
		return {
			x: element.frame.x + element.frame.width / 2 - (Math.cos(radians) * element.frame.width) / 2,
			y: element.frame.y + element.frame.height / 2 - (Math.sin(radians) * element.frame.width) / 2
		};
	};
	const beforeWest = westHandle(textElement({ rotation: 90 }));
	const afterWest = westHandle(resized);
	assert.ok(Math.abs(afterWest.x - beforeWest.x) < 1e-9);
	assert.ok(Math.abs(afterWest.y - beforeWest.y) < 1e-9);
});

test('constraining a rotated frame keeps its rendered bounds inside the page', async () => {
	const { constrainElementToPage } = await import('../../src/lib/certificates/editor-state.ts');
	const constrained = constrainElementToPage(
		textElement({
			rotation: 45,
			frame: { x: 160, y: 65, width: 120, height: 80 }
		}),
		{ width: 220, height: 160 }
	);
	const radians = (constrained.rotation * Math.PI) / 180;
	const extentX =
		(Math.abs(Math.cos(radians)) * constrained.frame.width +
			Math.abs(Math.sin(radians)) * constrained.frame.height) /
		2;
	const extentY =
		(Math.abs(Math.sin(radians)) * constrained.frame.width +
			Math.abs(Math.cos(radians)) * constrained.frame.height) /
		2;
	const centerX = constrained.frame.x + constrained.frame.width / 2;
	const centerY = constrained.frame.y + constrained.frame.height / 2;
	assert.ok(centerX - extentX >= -1e-9);
	assert.ok(centerX + extentX <= 220 + 1e-9);
	assert.ok(centerY - extentY >= -1e-9);
	assert.ok(centerY + extentY <= 160 + 1e-9);
});

test('layer ordering and multi-element alignment preserve every element', async () => {
	const { alignElements, reorderElement } =
		await import('../../src/lib/certificates/editor-state.ts');
	const elements = [
		textElement({ id: 'a', frame: { x: 10, y: 20, width: 40, height: 20 } }),
		textElement({ id: 'b', frame: { x: 30, y: 60, width: 20, height: 40 } }),
		textElement({ id: 'c', frame: { x: 70, y: 30, width: 30, height: 30 } })
	];
	assert.deepEqual(
		reorderElement(elements, 'b', 'forward').map((element) => element.id),
		['a', 'c', 'b']
	);
	assert.deepEqual(
		reorderElement(elements, 'b', 'backward').map((element) => element.id),
		['b', 'a', 'c']
	);
	const aligned = alignElements(elements, ['a', 'b', 'c'], 'middle');
	assert.deepEqual(
		aligned.map((element) => element.frame.y + element.frame.height / 2),
		[60, 60, 60]
	);
});

test('background scale/reset and fit zoom produce deterministic page-point results', async () => {
	const { fitEditorZoom, resetCertificateLayout, scaleCertificateLayout } =
		await import('../../src/lib/certificates/editor-state.ts');
	const layout = {
		schemaVersion: 1,
		elements: [
			textElement(),
			{
				type: 'image',
				id: 'image-1',
				assetId: 'asset-1',
				frame: { x: 20, y: 40, width: 100, height: 80 },
				rotation: 0
			},
			{
				type: 'qr',
				id: 'qr-1',
				frame: { x: 300, y: 300, width: 60, height: 60 },
				rotation: 0
			}
		]
	};
	const scaled = scaleCertificateLayout(
		layout,
		{ width: 400, height: 400 },
		{ width: 800, height: 200 }
	);
	assert.deepEqual(scaled.elements[0].frame, { x: 144, y: 36, width: 480, height: 30 });
	assert.equal(scaled.elements[0].fontSize, 16);
	assert.deepEqual(scaled.elements[1].frame, { x: 40, y: 20, width: 50, height: 40 });
	assert.deepEqual(scaled.elements[2].frame, { x: 600, y: 150, width: 30, height: 30 });
	assert.deepEqual(resetCertificateLayout(), { schemaVersion: 1, elements: [] });
	assert.equal(fitEditorZoom(1000, 600, 800, 400, 100), 1.125);
});

test('editor workspace exposes the approved focused controls and safe save contract', async () => {
	const files = [
		'src/lib/components/certificates/editor/CertificateEditor.svelte',
		'src/lib/components/certificates/editor/CertificateCanvas.svelte',
		'src/lib/components/certificates/editor/CertificateToolbar.svelte',
		'src/lib/components/certificates/editor/CertificateElementPanel.svelte',
		'src/lib/components/certificates/editor/CertificateLayersPanel.svelte',
		'src/lib/components/certificates/editor/CertificateVariablePicker.svelte',
		'src/lib/components/certificates/editor/CertificateBackgroundReplaceDialog.svelte',
		'src/routes/(app)/staff/certificates/[campaignId]/templates/[templateId]/editor/+page.svelte'
	];
	const source = (
		await Promise.all(files.map((file) => readFile(new URL(file, projectRoot), 'utf8')))
	).join('\n');

	for (const label of [
		'เพิ่มข้อความ',
		'เพิ่ม QR Code',
		'เพิ่มรูปภาพ',
		'ขนาดตัวอักษร',
		'สีข้อความ',
		'จัดแนว',
		'ระยะบรรทัด',
		'ย่ออัตโนมัติ',
		'เงา',
		'ทำสำเนา',
		'ลบองค์ประกอบ',
		'ลำดับชั้น',
		'พื้นที่ปลอดภัย',
		'ชื่อสั้น',
		'ชื่อปกติ',
		'ชื่อยาว',
		'ผู้รับจริง',
		'ปรับตามสัดส่วน',
		'เริ่มจัดวางใหม่'
	]) {
		assert.match(source, new RegExp(label), `missing editor control: ${label}`);
	}
	assert.match(source, /expectedUpdatedAt/);
	assert.match(source, /ApiClientError/);
	assert.match(source, /status\s*===\s*409/);
	assert.match(source, /loadCertificateRenderer/);
	assert.doesNotMatch(source, /renderer\.browser/);
});

test('safe-area guide labels the adjustable current margin instead of the ten-millimetre default', async () => {
	const canvas = await readFile(
		new URL('src/lib/components/certificates/editor/CertificateCanvas.svelte', projectRoot),
		'utf8'
	);
	assert.match(canvas, /pointsToMillimetres/);
	assert.match(canvas, /safeMarginMillimetres/);
	assert.doesNotMatch(canvas, /พื้นที่ปลอดภัย 10 มม\./);
});

test('manifest refresh detects expiring background, font, and image grants', async () => {
	const { certificateManifestExpiresSoon, certificateManifestNeedsLayoutGrants } =
		await import('../../src/lib/certificates/editor-state.ts');
	const now = Date.parse('2026-08-14T00:00:00Z');
	const manifest = {
		backgroundGrant: { expiresAt: '2026-08-14T00:02:00Z' },
		fontGrants: [{ expiresAt: '2026-08-14T00:03:00Z' }],
		imageGrants: [{ expiresAt: '2026-08-14T00:04:00Z' }]
	};
	assert.equal(certificateManifestExpiresSoon(manifest, now, 30_000), false);
	manifest.fontGrants[0].expiresAt = '2026-08-14T00:00:20Z';
	assert.equal(certificateManifestExpiresSoon(manifest, now, 30_000), true);
	manifest.fontGrants[0].expiresAt = 'not-a-date';
	assert.equal(certificateManifestExpiresSoon(manifest, now, 30_000), true);
	assert.equal(
		certificateManifestNeedsLayoutGrants(
			{ fontGrants: [], imageGrants: [] },
			{
				schemaVersion: 1,
				elements: [
					{
						type: 'image',
						id: 'image-1',
						assetId: 'asset-image',
						frame: { x: 0, y: 0, width: 20, height: 20 },
						rotation: 0
					},
					textElement({
						fontSource: { type: 'asset', asset_id: 'asset-font' },
						fontFamily: 'Uploaded',
						fontWeight: 400
					})
				]
			}
		),
		true
	);
	assert.equal(
		certificateManifestNeedsLayoutGrants(
			{
				fontGrants: [{ assetId: 'asset-font' }],
				imageGrants: [{ assetId: 'asset-image' }]
			},
			{
				schemaVersion: 1,
				elements: [
					{
						type: 'image',
						id: 'image-1',
						assetId: 'asset-image',
						frame: { x: 0, y: 0, width: 20, height: 20 },
						rotation: 0
					}
				]
			}
		),
		false
	);
});

test('background geometry comparison includes crop size and normalized rotation', async () => {
	const { certificatePageGeometryMatches } =
		await import('../../src/lib/certificates/editor-state.ts');
	const current = {
		cropBox: { xPoints: 4, yPoints: 5, widthPoints: 600, heightPoints: 400 },
		rotation: 90
	};
	assert.equal(
		certificatePageGeometryMatches(current, {
			cropBox: { xPoints: 99, yPoints: 88, widthPoints: 600.02, heightPoints: 399.99 },
			rotation: -270
		}),
		true
	);
	assert.equal(
		certificatePageGeometryMatches(current, {
			cropBox: { xPoints: 4, yPoints: 5, widthPoints: 601, heightPoints: 400 },
			rotation: 90
		}),
		false
	);
	assert.equal(
		certificatePageGeometryMatches(current, {
			cropBox: { xPoints: 4, yPoints: 5, widthPoints: 600, heightPoints: 400 },
			rotation: 270
		}),
		false
	);
});

test('background replacement renders the selected scale or reset result before confirmation', async () => {
	const source = await readFile(
		new URL('src/lib/components/certificates/CertificateBackgroundUpload.svelte', projectRoot),
		'utf8'
	);
	assert.match(source, /inspectBackgroundPdf/);
	assert.match(source, /scaleCertificateLayout/);
	assert.match(source, /resetCertificateLayout/);
	assert.match(source, /renderer\.renderPreview/);
	assert.match(source, /exactPreviewReady/);
	assert.match(source, /previewConfirmed\s*=\s*false/);
	assert.match(source, /!replacementReady/);
	assert.match(source, /certificateManifestExpiresSoon/);
	assert.match(source, /onmanifestrefresh/);
});
