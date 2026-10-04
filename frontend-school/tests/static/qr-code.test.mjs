import assert from 'node:assert/strict';
import test from 'node:test';
import { MAX_LOGO_BYTES, validateLogo, validateQrText } from '../../src/lib/tools/qr-code.ts';
import { scanRoutes } from '../../scripts/menu-helpers.ts';

test('QR text rejects whitespace and supports Thai text', () => {
	for (const text of ['', ' \n\t ']) assert.ok(validateQrText(text));
	for (const text of ['สวัสดีครูทุกคน', 'https://example.invalid/กิจกรรม', '  ข้อความ  '])
		assert.equal(validateQrText(text), null);
});

test('logo validation accepts supported images up to 5 MiB', () => {
	for (const type of ['image/png', 'image/jpeg', 'image/webp'])
		assert.equal(validateLogo(new Blob(['image'], { type })), null);
	assert.ok(validateLogo(new Blob(['<svg/>'], { type: 'image/svg+xml' })));
	assert.equal(
		validateLogo(new Blob([new Uint8Array(MAX_LOGO_BYTES)], { type: 'image/png' })),
		null
	);
	assert.ok(validateLogo(new Blob([new Uint8Array(MAX_LOGO_BYTES + 1)], { type: 'image/png' })));
});

test('tools register only the staff catalog with no action permission', async () => {
	const routes = (await scanRoutes(new URL('../..', import.meta.url).pathname)).filter((route) =>
		route.path.startsWith('/staff/tools')
	);
	assert.equal(routes.length, 1);
	assert.equal(routes[0].path, '/staff/tools');
	assert.equal(routes[0].user_type, 'staff');
	assert.equal(routes[0].workspace, 'home');
	assert.equal(routes[0].group, 'main');
	assert.equal(routes[0].permission, undefined);
});
