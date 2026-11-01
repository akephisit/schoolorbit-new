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
	return { id: marker.id, startPosition: { left: 13, top, pageNumber } };
}

test('fits the crest to actual header span boundaries with both axes centered', () => {
	for (const bottom of [90, 150]) {
		const layout = createTimetablePdfLogoLayout('data:image/png;base64,crest');
		const frame = layout.register(55, 2);
		layout.capture(markerPosition(frame.top, 33));
		layout.capture(markerPosition(frame.bottom, bottom));
		const calls = [];
		layout.draw({ switchToPage: (page) => calls.push(page), image: (...args) => calls.push(args) });
		assert.deepEqual(calls, [
			0,
			[
				'data:image/png;base64,crest',
				15,
				33,
				{ fit: [51, bottom - 37], align: 'center', valign: 'center' }
			]
		]);
	}
});

test('logo layout preserves absent-logo behavior and refuses missing or split header geometry', () => {
	createTimetablePdfLogoLayout(null).draw({ image: () => assert.fail('no configured crest') });
	const layout = createTimetablePdfLogoLayout('crest');
	const frame = layout.register(24, 1);
	assert.throws(() => layout.draw({}), /ไม่พบตำแหน่ง/);
	layout.capture(markerPosition(frame.top, 20));
	layout.capture(markerPosition(frame.bottom, 90, 2));
	assert.throws(() => layout.draw({}), /ไม่พบตำแหน่ง/);
});
