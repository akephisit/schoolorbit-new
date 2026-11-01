import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import * as fontkit from 'fontkit';
import { createTimetablePdfLogoLayout } from '../../src/lib/utils/timetable-pdf-logo-layout.ts';

test('ships genuine unmodified TH Sarabun New styles with the original embedding exception', () => {
	const notice = readFileSync(
		new URL('../../static/fonts/THSarabunNew-LICENSE.txt', import.meta.url),
		'utf8'
	);
	for (const style of ['', '-Bold', '-Italic', '-BoldItalic']) {
		const bytes = readFileSync(
			new URL(`../../static/fonts/THSarabunNew${style}.ttf`, import.meta.url)
		);
		const font = fontkit.create(bytes);
		assert.equal(font.familyName, 'TH Sarabun New');
		assert.equal(font.postscriptName, `THSarabunNew${style}`);
		assert.ok(font.hasGlyphForCodePoint('ก'.codePointAt(0)));
		assert.ok(notice.includes(createHash('sha256').update(bytes).digest('hex')));
	}
	assert.match(notice, /either version 2 of the License, or \(at your option\) any later version/);
	assert.match(notice, /embed this font or unaltered portions/);
	assert.match(notice, /GNU GENERAL PUBLIC LICENSE/);
});

function markerPosition(marker, top, pageNumber = 1) {
	return {
		id: marker.id,
		startPosition: { left: 10, top: 30, pageNumber },
		canvas: [{ type: 'line', x1: 13, y1: top, x2: 13, y2: top }]
	};
}

test('fits the crest to actual header span boundaries with both axes centered', () => {
	for (const bottom of [90, 150]) {
		const layout = createTimetablePdfLogoLayout('data:image/png;base64,crest');
		const frame = layout.register(55, 2, 1);
		layout.capture(markerPosition(frame.top, 33));
		layout.capture(markerPosition(frame.bottom, bottom));
		const calls = [];
		layout.draw({ switchToPage: (page) => calls.push(page), image: (...args) => calls.push(args) });
		assert.deepEqual(calls, [
			0,
			[
				'data:image/png;base64,crest',
				15,
				32.5,
				{ fit: [51, bottom - 37], align: 'center', valign: 'center' }
			]
		]);
	}
});

test('logo layout preserves absent-logo behavior and refuses missing or split header geometry', () => {
	createTimetablePdfLogoLayout(null).draw({ image: () => assert.fail('no configured crest') });
	const layout = createTimetablePdfLogoLayout('crest');
	const frame = layout.register(24, 1, 0.5);
	assert.throws(() => layout.draw({}), /ไม่พบตำแหน่ง/);
	layout.capture(markerPosition(frame.top, 20));
	layout.capture(markerPosition(frame.bottom, 90, 2));
	assert.throws(() => layout.draw({}), /ไม่พบตำแหน่ง/);
});

test('measured full and mini header frames agree with enclosing border centers', () => {
	for (const fixture of [
		{ width: 55, padding: 2, line: 1, left: 13, top: 33, bottom: 157.167, center: [40.5, 92.5835] },
		{ width: 24, padding: 1, line: 0.5, left: 11.5, top: 11.5, bottom: 71.5, center: [23.5, 40.25] }
	]) {
		const layout = createTimetablePdfLogoLayout('crest');
		const frame = layout.register(fixture.width, fixture.padding, fixture.line);
		for (const [marker, y] of [
			[frame.top, fixture.top],
			[frame.bottom, fixture.bottom]
		]) {
			layout.capture({
				id: marker.id,
				startPosition: { left: 10, top: 10, pageNumber: 1 },
				canvas: [{ type: 'line', x1: fixture.left, y1: y, x2: fixture.left, y2: y }]
			});
		}
		layout.draw({
			switchToPage: () => {},
			image: (_logo, x, y, options) => {
				assert.ok(Math.abs(x + options.fit[0] / 2 - fixture.center[0]) < 0.000001);
				assert.ok(Math.abs(y + options.fit[1] / 2 - fixture.center[1]) < 0.000001);
			}
		});
	}
});
