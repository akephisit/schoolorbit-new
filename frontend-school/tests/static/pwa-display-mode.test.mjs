import assert from 'node:assert/strict';
import test from 'node:test';
import { isStandalonePWA } from '../../src/lib/pwa/display-mode.ts';

test('installed mode recognizes desktop media query and iOS while ordinary browser and SSR stay public', () => {
	const savedWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
	const savedNavigator = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
	try {
		Reflect.deleteProperty(globalThis, 'window');
		assert.equal(isStandalonePWA(), false);
		for (const [media, ios, expected] of [
			[false, false, false],
			[true, false, true],
			[false, true, true]
		]) {
			Object.defineProperty(globalThis, 'window', {
				configurable: true,
				value: { matchMedia: () => ({ matches: media }) }
			});
			Object.defineProperty(globalThis, 'navigator', {
				configurable: true,
				value: { standalone: ios }
			});
			assert.equal(isStandalonePWA(), expected);
		}
	} finally {
		for (const [name, descriptor] of [
			['window', savedWindow],
			['navigator', savedNavigator]
		]) {
			if (descriptor) Object.defineProperty(globalThis, name, descriptor);
			else Reflect.deleteProperty(globalThis, name);
		}
	}
});
