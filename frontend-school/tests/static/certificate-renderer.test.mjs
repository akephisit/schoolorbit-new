import assert from 'node:assert/strict';
import test from 'node:test';

test('interpolates normalized values and supports escaped braces', async () => {
	const { interpolateCertificateText } =
		await import('../../src/lib/certificates/interpolation.ts');

	assert.equal(
		interpolateCertificateText('มอบ {{ตัวอย่าง}} ให้ {  ชื่อ  }', {
			' ชื่อ ': 'กมลชนก'
		}),
		'มอบ {ตัวอย่าง} ให้ กมลชนก'
	);
	assert.equal(interpolateCertificateText('วงเล็บปิดซ้อน }}', {}), 'วงเล็บปิดซ้อน }');
});

test('reports every missing interpolation value once and rejects malformed braces', async () => {
	const { CertificateInterpolationError, interpolateCertificateText } =
		await import('../../src/lib/certificates/interpolation.ts');

	assert.throws(
		() => interpolateCertificateText('{ชื่อ} {นามสกุล} ({ชื่อ}) {รางวัล}', { ชื่อ: 'กมล' }),
		(error) => {
			assert.ok(error instanceof CertificateInterpolationError);
			assert.equal(error.kind, 'missing_values');
			assert.deepEqual(error.missingVariables, ['นามสกุล', 'รางวัล']);
			return true;
		}
	);
	assert.throws(
		() => interpolateCertificateText('ข้อความ {ชื่อ', { ชื่อ: 'กมล' }),
		(error) => error instanceof CertificateInterpolationError && error.kind === 'invalid_syntax'
	);
});

test('converts PDF points and millimetres without changing the source geometry', async () => {
	const { millimetresToPoints, pointsToMillimetres } =
		await import('../../src/lib/certificates/layout.ts');

	assert.ok(Math.abs(millimetresToPoints(25.4) - 72) < 1e-9);
	assert.ok(Math.abs(pointsToMillimetres(72) - 25.4) < 1e-9);
});

test('uses the explicit background transform contract for every PDF page rotation', async () => {
	const { backgroundPageTransform, displayedPageSize, normalizePageRotation } =
		await import('../../src/lib/certificates/layout.ts');

	const expected = new Map([
		[0, { x: 0, y: 0, rotation: 0 }],
		[90, { x: 200, y: 0, rotation: 90 }],
		[180, { x: 100, y: 200, rotation: 180 }],
		[270, { x: 0, y: 100, rotation: 270 }]
	]);

	for (const [rotation, transform] of expected) {
		assert.deepEqual(backgroundPageTransform(100, 200, rotation), transform);
		assert.deepEqual(
			displayedPageSize(100, 200, rotation),
			rotation === 90 || rotation === 270
				? { width: 200, height: 100 }
				: { width: 100, height: 200 }
		);
	}
	assert.equal(normalizePageRotation(-90), 270);
	assert.equal(normalizePageRotation(450), 90);
	assert.equal(normalizePageRotation(-720), 0);
	assert.throws(() => normalizePageRotation(45), /rotation/i);
	assert.throws(() => backgroundPageTransform(100, 200, 45), /rotation/i);
});

test('auto-shrink stays bounded and selects the largest fitting size', async () => {
	const { chooseAutoShrinkFontSize } = await import('../../src/lib/certificates/layout.ts');

	const fitted = chooseAutoShrinkFontSize({
		fontSize: 32,
		minFontSize: 12,
		autoShrink: true,
		fits: (size) => size <= 18
	});
	assert.ok(fitted >= 17.99 && fitted <= 18);
	assert.equal(
		chooseAutoShrinkFontSize({
			fontSize: 32,
			minFontSize: 12,
			autoShrink: true,
			fits: () => false
		}),
		12
	);
	assert.equal(
		chooseAutoShrinkFontSize({
			fontSize: 32,
			minFontSize: 12,
			autoShrink: false,
			fits: () => false
		}),
		32
	);
});

test('measures Thai glyph ink and shadow inside the text frame', async () => {
	const { measureCertificateTextLayout } =
		await import('../../src/lib/certificates/text-layout.browser.ts');
	const context = {
		font: '',
		textAlign: 'start',
		textBaseline: 'alphabetic',
		measureText(value) {
			const fontSize = Number.parseFloat(this.font.match(/([\d.]+)px/u)?.[1] ?? '20');
			const width = Array.from(value).length * fontSize * 0.45;
			return {
				width,
				actualBoundingBoxAscent: fontSize * 1.08,
				actualBoundingBoxDescent: fontSize * 0.24,
				actualBoundingBoxLeft: fontSize * 0.04,
				actualBoundingBoxRight: width + fontSize * 0.03
			};
		}
	};

	const measured = measureCertificateTextLayout(context, {
		text: 'ปั้น น้ำ ผู้เข้าร่วม กิจกรรม',
		fontSize: 20,
		minFontSize: 12,
		autoShrink: true,
		lineHeight: 1.1,
		frameWidth: 180,
		frameHeight: 30,
		alignment: 'center',
		shadow: { offsetX: 1, offsetY: -1, blur: 2 },
		fontForSize: (fontSize) => `normal 700 ${fontSize}px "Sarabun"`
	});

	assert.equal(measured.fits, true);
	assert.ok(measured.fontSize < 20);
	assert.ok(measured.bounds.top >= 0);
	assert.ok(measured.bounds.bottom <= 30);
	assert.ok(measured.bounds.left >= 0);
	assert.ok(measured.bounds.right <= 180);
	assert.ok(measured.lines.every((line) => line.baseline > 0));
});

test('sanitizes PDF filenames and enforces the 200-certificate browser batch limit', async () => {
	const { sanitizeCertificateFilename, validateCertificateBatchSize } =
		await import('../../src/lib/certificates/download.ts');

	assert.equal(sanitizeCertificateFilename(' ../../ใบ:ทดสอบ?.PDF '), 'ใบ-ทดสอบ.pdf');
	assert.equal(sanitizeCertificateFilename('...'), 'เกียรติบัตร.pdf');
	assert.equal(sanitizeCertificateFilename('ผลงาน'), 'ผลงาน.pdf');
	assert.doesNotThrow(() => validateCertificateBatchSize(1));
	assert.doesNotThrow(() => validateCertificateBatchSize(200));
	assert.throws(() => validateCertificateBatchSize(0), /อย่างน้อย 1/);
	assert.throws(() => validateCertificateBatchSize(201), /ไม่เกิน 200/);
});
