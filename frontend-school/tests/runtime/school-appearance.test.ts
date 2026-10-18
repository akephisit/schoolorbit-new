import assert from 'node:assert/strict';
import test from 'node:test';
import {
	DEFAULT_APPEARANCE,
	validateAppearance,
	validateAppearanceResponse,
	schoolAppearanceCss
} from '../../src/lib/appearance/model.ts';
import { AppearanceController } from '../../src/lib/appearance/controller.ts';
import { appearanceStylesheet } from '../../src/lib/appearance/stylesheet.ts';
const initial = { appearance: { ...DEFAULT_APPEARANCE }, revision: 1 };
const next = {
	appearance: { ...DEFAULT_APPEARANCE, font: 'noto' as const, palette: 'purple' as const },
	revision: 2
};

test('production validation strips personal/prototype fields and CSS serializes safe metadata and both modes', () => {
	assert.deepEqual(
		validateAppearance({ ...DEFAULT_APPEARANCE, version: 1, mode: 'dark', primary: '#AABBCC' }),
		{ ...DEFAULT_APPEARANCE, primary: '#aabbcc' }
	);
	for (const value of [
		null,
		[],
		{ ...DEFAULT_APPEARANCE, primary: '#fff' },
		{ ...DEFAULT_APPEARANCE, font: "x';}body{display:none}" }
	])
		assert.equal(validateAppearance(value), null);
	for (const revision of [-1, Infinity, 1.5, '2'])
		assert.equal(validateAppearanceResponse({ ...initial, revision }), null);
	const css = schoolAppearanceCss(next);
	assert.match(css, /html:root\{/);
	assert.match(css, /html:root.dark\{/);
	assert.match(css, /--school-appearance-revision:2;/);
	assert.match(css, /--school-appearance-font:noto;/);
	assert.match(css, /--school-font-family:'School Noto Sans Thai', sans-serif/);
	assert.doesNotMatch(css, /version|localStorage|https?:/);
});

test('CSS revalidates the actual tenant each time, supports 304 and never caches failures', async () => {
	let reads = 0;
	const read = async () => {
		reads++;
		return next;
	};
	const first = await appearanceStylesheet(read, null);
	const etag = first.headers.get('etag');
	assert.equal(first.status, 200);
	assert.equal(first.headers.get('cache-control'), 'private, no-cache, must-revalidate');
	assert.ok(etag);
	const repeated = await appearanceStylesheet(read, etag);
	assert.equal(repeated.status, 304);
	assert.equal(await repeated.text(), '');
	assert.equal(reads, 2);
	const otherTenant = await appearanceStylesheet(async () => initial, etag);
	assert.equal(otherTenant.status, 200);
	assert.notEqual(otherTenant.headers.get('etag'), etag);
	const failure = await appearanceStylesheet(async () => {
		throw new Error('offline');
	}, etag);
	assert.equal(failure.headers.get('cache-control'), 'no-store');
	assert.equal(failure.headers.get('etag'), null);
	assert.match(await failure.text(), /--school-appearance-palette:teal/);
});

test('CSS timeout aborts the read and a late result cannot change the returned fallback', async () => {
	let signal: AbortSignal | undefined;
	let release: ((value: typeof next) => void) | undefined;
	const response = await appearanceStylesheet(
		(input) => {
			signal = input;
			return new Promise((resolve) => {
				release = resolve;
			});
		},
		null,
		5
	);
	assert.equal(signal?.aborted, true);
	const before = await response.text();
	release?.(next);
	assert.match(before, /--school-appearance-revision:0/);
	assert.equal(response.headers.get('cache-control'), 'no-store');
});

test('font preparation gates atomic installation; newer versions fence late work and disposal fences everything', async () => {
	const installed: number[] = [];
	let release: (() => void) | undefined;
	const controller = new AppearanceController(
		initial,
		(font) =>
			font === 'noto'
				? new Promise((resolve) => {
						release = resolve;
					})
				: Promise.resolve(),
		(value) => installed.push(value.revision)
	);
	const pending = controller.apply(next);
	assert.deepEqual(installed, []);
	assert.equal(await controller.apply({ ...initial, revision: 3 }), 'applied');
	release?.();
	assert.equal(await pending, 'stale');
	assert.deepEqual(installed, [3]);
	assert.equal(await controller.apply(next), 'stale');
	controller.dispose();
	assert.equal(await controller.apply({ ...initial, revision: 4 }), 'stale');
	assert.deepEqual(installed, [3]);
});

test('a committed value with failed fonts remains retryable at the same revision without reopening older work', async () => {
	let failed = true;
	const installed: number[] = [];
	const controller = new AppearanceController(
		initial,
		async () => {
			if (failed) throw new Error('font offline');
		},
		(value) => installed.push(value.revision)
	);
	assert.equal(await controller.apply(next), 'font-failed');
	assert.deepEqual(installed, []);
	assert.equal(controller.current.revision, 2);
	assert.equal(await controller.apply(initial), 'stale');
	failed = false;
	assert.equal(await controller.apply(next), 'applied');
	assert.equal(await controller.apply(next), 'stale');
	assert.deepEqual(installed, [2]);
});

test('semantic destructive foreground preserves readable fills while status red remains stable', async () => {
	const { appearanceTokens, contrastRatio } = await import('../../src/lib/appearance/model.ts');
	for (const mode of ['light', 'dark'] as const) {
		const tokens = appearanceTokens(DEFAULT_APPEARANCE, mode);
		assert.ok(contrastRatio(tokens.destructive, tokens.destructiveForeground) >= 4.5);
		const mixed =
			'#' +
			[1, 3, 5]
				.map((index) =>
					Math.round(
						Number.parseInt(tokens.destructive.slice(index, index + 2), 16) * 0.9 +
							Number.parseInt(tokens.background.slice(index, index + 2), 16) * 0.1
					)
						.toString(16)
						.padStart(2, '0')
				)
				.join('');
		assert.ok(contrastRatio(mixed, tokens.destructiveForeground) >= 4.5);
		assert.equal(
			appearanceTokens({ ...DEFAULT_APPEARANCE, primary: '#ffffff' }, mode).destructive,
			tokens.destructive
		);
	}
});
