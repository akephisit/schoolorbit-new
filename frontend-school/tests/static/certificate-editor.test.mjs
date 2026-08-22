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
		fontStyle: 'normal',
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

function imageElement(overrides = {}) {
	return {
		type: 'image',
		id: 'image-1',
		assetId: 'asset-image',
		frame: { x: 120, y: 90, width: 120, height: 80 },
		rotation: 0,
		lockAspectRatio: true,
		aspectRatio: 1.5,
		...overrides
	};
}

function fontAsset(id, family, weight, style) {
	return {
		id,
		fileId: `file-${id}`,
		kind: 'font',
		displayName: `${family} ${weight} ${style}`,
		fontFamily: family,
		fontWeight: weight,
		fontStyle: style,
		imageWidthPixels: null,
		imageHeightPixels: null,
		rightsConfirmed: true,
		createdAt: '2026-08-16T00:00:00Z'
	};
}

test('font controls resolve only real deterministic variants and patch every font field', async () => {
	const {
		certificateFontVariants,
		fontVariantPatch,
		selectFontFamily,
		selectFontWeight,
		toggleBoldVariant,
		toggleItalicVariant
	} = await import('../../src/lib/certificates/font-variants.ts');
	const variants = certificateFontVariants([
		fontAsset('font-regular', 'Uploaded Thai', 400, 'normal'),
		fontAsset('font-bold', 'Uploaded Thai', 700, 'normal'),
		fontAsset('font-italic', 'Uploaded Thai', 400, 'italic'),
		fontAsset('font-light', 'Fallback Thai', 300, 'normal'),
		fontAsset('font-medium', 'Fallback Thai', 500, 'normal'),
		fontAsset('font-italic-only', 'Italic Only', 400, 'italic')
	]);
	const regular = selectFontFamily(variants, 'asset:uploaded thai');
	assert.ok(regular);
	assert.deepEqual(fontVariantPatch(regular), {
		fontSource: { type: 'asset', asset_id: 'font-regular' },
		fontFamily: 'Uploaded Thai',
		fontWeight: 400,
		fontStyle: 'normal'
	});
	const bold = toggleBoldVariant(variants, regular);
	assert.equal(bold?.source.type === 'asset' ? bold.source.asset_id : null, 'font-bold');
	assert.equal(toggleBoldVariant(variants, bold)?.weight, 400);
	const italic = toggleItalicVariant(variants, regular);
	assert.deepEqual(fontVariantPatch(italic), {
		fontSource: { type: 'asset', asset_id: 'font-italic' },
		fontFamily: 'Uploaded Thai',
		fontWeight: 400,
		fontStyle: 'italic'
	});
	assert.equal(
		toggleItalicVariant(variants, bold),
		null,
		'missing exact 700 italic stays disabled'
	);
	assert.equal(
		selectFontWeight(variants, italic, 700),
		null,
		'weight selection must not switch italic text to a normal-only variant'
	);
	assert.equal(selectFontFamily(variants, 'asset:fallback thai')?.weight, 300);
	assert.equal(selectFontFamily(variants, 'asset:italic only')?.style, 'italic');
	assert.equal(toggleBoldVariant(variants, selectFontFamily(variants, 'asset:italic only')), null);
});

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

test('locked image resize preserves source ratio and opposite anchors for every handle and rotation', async () => {
	const { resizeElement } = await import('../../src/lib/certificates/editor-state.ts');
	const handleAxis = {
		n: [0, -1],
		ne: [1, -1],
		e: [1, 0],
		se: [1, 1],
		s: [0, 1],
		sw: [-1, 1],
		w: [-1, 0],
		nw: [-1, -1]
	};
	const handlePoint = (element, handle) => {
		const [axisX, axisY] = handleAxis[handle];
		const radians = (element.rotation * Math.PI) / 180;
		const localX = (axisX * element.frame.width) / 2;
		const localY = (axisY * element.frame.height) / 2;
		const centerX = element.frame.x + element.frame.width / 2;
		const centerY = element.frame.y + element.frame.height / 2;
		return {
			x: centerX + Math.cos(radians) * localX - Math.sin(radians) * localY,
			y: centerY + Math.sin(radians) * localX + Math.cos(radians) * localY
		};
	};
	for (const rotation of [0, 45, 90]) {
		for (const [handle, [axisX, axisY]] of Object.entries(handleAxis)) {
			const before = imageElement({ rotation });
			const opposite = Object.entries(handleAxis).find(
				([, candidate]) => candidate[0] === -axisX && candidate[1] === -axisY
			)?.[0];
			assert.ok(opposite);
			const anchoredBefore = handlePoint(before, opposite);
			const resized = resizeElement(
				before,
				{ handle, dxPixels: axisX * 30, dyPixels: axisY * 20 },
				1
			);
			const anchoredAfter = handlePoint(resized, opposite);
			assert.ok(Math.abs(resized.frame.width / resized.frame.height - 1.5) < 1e-9);
			assert.ok(Math.abs(anchoredAfter.x - anchoredBefore.x) < 1e-9);
			assert.ok(Math.abs(anchoredAfter.y - anchoredBefore.y) < 1e-9);
		}
	}
});

test('image creation, unlock, relock, and reset use inspected source dimensions', async () => {
	const {
		createImageElement,
		imageAssetAspectRatio,
		resetImageAspectRatio,
		resizeElement,
		setImageAspectRatioLock
	} = await import('../../src/lib/certificates/editor-state.ts');
	const sourceRatio = imageAssetAspectRatio({
		kind: 'image',
		imageWidthPixels: 1200,
		imageHeightPixels: 800
	});
	assert.equal(sourceRatio, 1.5);
	const created = createImageElement(
		{ width: 600, height: 400 },
		'asset-image',
		sourceRatio,
		() => 'new-image'
	);
	assert.equal(created.lockAspectRatio, true);
	assert.equal(created.aspectRatio, 1.5);
	assert.ok(Math.abs(created.frame.width / created.frame.height - 1.5) < 1e-9);
	const unlocked = setImageAspectRatioLock(created, false, { width: 600, height: 400 });
	const freelyResized = resizeElement(unlocked, { handle: 'se', dxPixels: 30, dyPixels: 5 }, 1);
	assert.notEqual(freelyResized.frame.width / freelyResized.frame.height, 1.5);
	const relocked = setImageAspectRatioLock(freelyResized, true, { width: 600, height: 400 });
	assert.ok(Math.abs(relocked.frame.width / relocked.frame.height - 1.5) < 1e-9);
	const reset = resetImageAspectRatio(relocked, 2, { width: 600, height: 400 });
	assert.equal(reset.lockAspectRatio, true);
	assert.equal(reset.aspectRatio, 2);
	assert.ok(Math.abs(reset.frame.width / reset.frame.height - 2) < 1e-9);
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
				rotation: 0,
				lockAspectRatio: true,
				aspectRatio: 1.25
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
		'src/lib/components/certificates/CertificatePreviewSurface.svelte',
		'src/lib/components/certificates/CertificatePreviewDialog.svelte',
		'src/lib/components/certificates/CertificatePreviewFullscreenDialog.svelte',
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
		'ตระกูลฟอนต์',
		'น้ำหนักฟอนต์',
		'ตัวหนา',
		'ตัวเอียง',
		'ล็อกสัดส่วน',
		'รีเซ็ตสัดส่วนต้นฉบับ',
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
		'กำลังโหลดฟอนต์และสร้างตัวอย่าง…',
		'ลองใหม่',
		'ปิด',
		'ปรับตามสัดส่วน',
		'เริ่มจัดวางใหม่'
	]) {
		assert.match(source, new RegExp(label), `missing editor control: ${label}`);
	}
	assert.match(source, /expectedUpdatedAt/);
	assert.match(source, /ApiClientError/);
	assert.match(source, /status\s*===\s*409/);
	assert.match(source, /loadCertificateRenderer/);
	assert.match(source, /certificateFontVariants/);
	assert.match(source, /fontVariantPatch/);
	assert.match(source, /setImageAspectRatioLock/);
	assert.match(source, /resetImageAspectRatio/);
	assert.match(source, /prepareFontAliases/);
	assert.match(source, /fontAlias/);
	assert.match(source, /style:font-style/);
	assert.match(source, /aria-busy/);
	assert.match(source, /AbortController/);
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
						rotation: 0,
						lockAspectRatio: true,
						aspectRatio: 1
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
						rotation: 0,
						lockAspectRatio: true,
						aspectRatio: 1
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

test('editor real PDF preview delegates rendering UI to the shared preview dialog', async () => {
	const source = await readFile(
		new URL('src/lib/components/certificates/editor/CertificateEditor.svelte', projectRoot),
		'utf8'
	);
	assert.match(source, /CertificatePreviewDialog/);
	assert.doesNotMatch(source, /window\.innerWidth[\s\S]*freshManifest\.pageGeometry/);
	assert.doesNotMatch(source, /<canvas[\s\S]*ผลพรีวิว PDF จริง/);
});
