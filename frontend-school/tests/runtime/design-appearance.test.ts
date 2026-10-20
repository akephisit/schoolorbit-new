import assert from 'node:assert/strict';
import test from 'node:test';
import {
	DEFAULT_APPEARANCE,
	appearanceFromQuery,
	appearanceTokens,
	contrastRatio,
	readStoredAppearance,
	staffPreviewUrl,
	validateAppearance
} from '../../src/lib/design-preview/appearance.ts';

test('browser saves accept only versioned appearance values and strip unrelated fields', () => {
	const appearance = {
		...DEFAULT_APPEARANCE,
		primary: '#AABBCC',
		theme: 'dark',
		tenantId: 'not-persisted'
	};
	assert.deepEqual(readStoredAppearance(JSON.stringify(appearance)), {
		...DEFAULT_APPEARANCE,
		primary: '#aabbcc'
	});
	for (const invalid of [
		null,
		'{',
		'[]',
		JSON.stringify({ ...appearance, version: 2 }),
		JSON.stringify({ ...appearance, font: 'remote-font' }),
		JSON.stringify({ ...appearance, density: 'huge' }),
		JSON.stringify({ ...appearance, palette: 'url(x)' }),
		JSON.stringify({ ...appearance, primary: 'red;position:fixed' })
	]) {
		assert.equal(readStoredAppearance(invalid), null);
	}
});

test('query colors and identifiers are validated; frame URLs stay on the single preview route', () => {
	const settings = appearanceFromQuery(
		new URLSearchParams(
			'palette=blue&font=noto&density=compact&primary=%23AbC123&src=https://example.com'
		)
	);
	assert.deepEqual(settings, {
		version: 1,
		palette: 'blue',
		font: 'noto',
		density: 'compact',
		primary: '#abc123'
	});
	assert.deepEqual(
		appearanceFromQuery(new URLSearchParams('palette=bad&font=evil&density=huge&primary=url(x)')),
		DEFAULT_APPEARANCE
	);
	const url = new URL(staffPreviewUrl(settings, 'dark'), 'https://preview.local');
	assert.equal(url.origin, 'https://preview.local');
	assert.equal(url.pathname, '/dev/staff-design-preview');
	assert.equal(url.searchParams.get('mode'), 'dark');
	assert.deepEqual(appearanceFromQuery(url.searchParams), settings);
	assert.equal(validateAppearance({ ...DEFAULT_APPEARANCE, primary: '#fff' }), null);
});

test('contrast uses WCAG luminance and readable primary colors for both modes and all branded surfaces', () => {
	assert.equal(contrastRatio('#000000', '#ffffff'), 21);
	assert.equal(contrastRatio('#ffffff', '#ffffff'), 1);
	for (const mode of ['light', 'dark'] as const) {
		for (const palette of ['teal', 'blue', 'purple'] as const) {
			for (const primary of [
				null,
				'#ffffff',
				'#000000',
				'#ffff00',
				'#ff0000',
				'#00ff00',
				'#0000ff',
				'#808080',
				'#167166'
			]) {
				const tokens = appearanceTokens({ ...DEFAULT_APPEARANCE, palette, primary }, mode);
				for (const surface of [
					tokens.primaryForeground,
					tokens.background,
					tokens.card,
					tokens.secondary,
					tokens.accent,
					tokens.welcome
				]) {
					assert.ok(
						contrastRatio(tokens.primary, surface) >= 4.5,
						`${mode} ${palette} ${primary}: ${tokens.primary} on ${surface}`
					);
				}
			}
		}
	}
});

test('status meanings are independent of school palette, custom brand and font', () => {
	for (const mode of ['light', 'dark'] as const) {
		const baseline = appearanceTokens(DEFAULT_APPEARANCE, mode);
		const custom = appearanceTokens(
			{ ...DEFAULT_APPEARANCE, palette: 'purple', font: 'sarabun', primary: '#ff00ff' },
			mode
		);
		for (const key of [
			'success',
			'successSurface',
			'warning',
			'warningSurface',
			'destructive',
			'destructiveSurface'
		] as const)
			assert.equal(custom[key], baseline[key]);
	}
});

test('primary button opacity hover keeps white and bright custom colors readable', () => {
	for (const mode of ['light', 'dark'] as const) {
		for (const primary of ['#ffffff', '#ffff00', '#000000', '#00ff00']) {
			const tokens = appearanceTokens({ ...DEFAULT_APPEARANCE, primary }, mode);
			assert.ok(contrastRatio(tokens.hoverOnCard, tokens.primaryForeground) >= 4.5);
			assert.ok(contrastRatio(tokens.hoverOnBackground, tokens.primaryForeground) >= 4.5);
		}
	}
});

test('chosen fills remain exact with readable text, hover and boundaries across a dense RGB sample', () => {
	for (const mode of ['light', 'dark'] as const) {
		for (let red = 0; red <= 255; red += 17) {
			for (let green = 0; green <= 255; green += 17) {
				for (let blue = 0; blue <= 255; blue += 17) {
					const primary =
						'#' +
						[red, green, blue].map((channel) => channel.toString(16).padStart(2, '0')).join('');
					const tokens = appearanceTokens({ ...DEFAULT_APPEARANCE, primary }, mode);
					assert.equal(tokens.primaryFill, primary);
					assert.ok(
						contrastRatio(tokens.primaryFill, tokens.primaryFillForeground) >= 4.5,
						`${mode} ${primary} fill`
					);
					assert.ok(
						contrastRatio(tokens.primaryFillHover, tokens.primaryFillForeground) >= 4.5,
						`${mode} ${primary} hover`
					);
					for (const surface of [
						tokens.card,
						tokens.background,
						tokens.muted,
						tokens.secondary,
						tokens.accent
					]) {
						assert.ok(
							contrastRatio(tokens.input, surface) >= 3,
							`${mode} ${primary} input boundary on ${surface}`
						);
						for (const text of [tokens.foreground, tokens.mutedForeground])
							assert.ok(
								contrastRatio(text, surface) >= 4.5,
								`${mode} ${primary} text ${text} on ${surface}`
							);
						assert.ok(
							contrastRatio(tokens.primaryFillBorder, surface) >= 3,
							`${mode} ${primary} boundary on ${surface}`
						);
					}
					for (const surface of [
						tokens.card,
						tokens.background,
						tokens.secondary,
						tokens.accent,
						tokens.welcome
					]) {
						assert.ok(
							contrastRatio(tokens.primary, surface) >= 4.5,
							`${mode} ${primary} readable accent on ${surface}`
						);
					}
				}
			}
		}
	}
});

test('canvas, cards, trays, selection and hover remain separate with extreme custom brands', () => {
	for (const mode of ['light', 'dark'] as const) {
		for (const palette of ['teal', 'blue', 'purple'] as const) {
			for (const primary of [null, '#ffffff', '#000000', '#dbeafe', '#fce7f3']) {
				const tokens = appearanceTokens({ ...DEFAULT_APPEARANCE, palette, primary }, mode);
				assert.ok(
					contrastRatio(tokens.background, tokens.card) >= 1.1,
					`${mode} ${palette} canvas/card`
				);
				assert.notEqual(tokens.muted, tokens.card);
				assert.notEqual(tokens.muted, tokens.secondary);
				assert.notEqual(tokens.secondary, tokens.accent);
				assert.notEqual(tokens.accent, tokens.card);
				assert.notEqual(tokens.input, tokens.border);
			}
		}
	}
});

test('semantic inks stay readable on translucent treatments over neutral surfaces', () => {
	const blend = (surface: string, ink: string, opacity: number) =>
		'#' +
		[1, 3, 5]
			.map((start) =>
				Math.round(
					Number.parseInt(surface.slice(start, start + 2), 16) * (1 - opacity) +
						Number.parseInt(ink.slice(start, start + 2), 16) * opacity
				)
					.toString(16)
					.padStart(2, '0')
			)
			.join('');
	for (const mode of ['light', 'dark'] as const)
		for (const palette of ['teal', 'blue', 'purple'] as const) {
			for (const primary of [null, '#ffffff', '#000000', '#dbeafe', '#fce7f3']) {
				const tokens = appearanceTokens({ ...DEFAULT_APPEARANCE, palette, primary }, mode);
				for (const ink of [tokens.primary, tokens.success, tokens.warning, tokens.destructive]) {
					for (const surface of [tokens.card, tokens.background, tokens.muted])
						for (const opacity of [0.15, 0.2])
							assert.ok(
								contrastRatio(ink, blend(surface, ink, opacity)) >= 4.5,
								`${mode} ${palette} ${primary} ink ${ink} at ${opacity} on ${surface}`
							);
				}
			}
		}
});
