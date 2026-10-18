import assert from 'node:assert/strict';
import test from 'node:test';
import { hexToRgb, rgbToHex } from '../../src/lib/components/ui/color-picker/color.ts';

test('RGB editing preserves opaque six-digit colors including pale presets and channel endpoints', () => {
	for (const hex of ['#000000', '#ffffff', '#dbeafe', '#fce7f3', '#167166', '#808080', '#0102ff']) {
		assert.equal(rgbToHex(hexToRgb(hex)), hex);
	}
	assert.equal(rgbToHex([0, 127, 255]), '#007fff');
	assert.equal(rgbToHex([-1, 127.6, 300]), '#0080ff');
	const next = hexToRgb('#dbeafe');
	next[0] = 220;
	assert.equal(rgbToHex(next), '#dceafe');
});
