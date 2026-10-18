export const APPEARANCE_STORAGE_KEY = 'schoolorbit:design-preview:appearance:v1';
export const PALETTES = [
	{
		value: 'teal',
		label: 'เขียวอมฟ้า',
		description: 'อบอุ่น สงบ เป็นมิตร',
		primary: '#167166',
		background: '#f6f7f3'
	},
	{
		value: 'blue',
		label: 'น้ำเงิน',
		description: 'ชัดเจน น่าเชื่อถือ',
		primary: '#245fc2',
		background: '#f3f6fb'
	},
	{
		value: 'purple',
		label: 'ม่วง',
		description: 'ร่วมสมัย มีเอกลักษณ์',
		primary: '#7850a5',
		background: '#f8f5fa'
	}
] as const;
export const FONTS = [
	{ value: 'kanit', label: 'Kanit', family: 'Kanit Preview' },
	{ value: 'noto', label: 'Noto Sans Thai', family: 'Noto Sans Thai Preview' },
	{ value: 'sarabun', label: 'Sarabun', family: 'Sarabun Preview' }
] as const;
export type Palette = (typeof PALETTES)[number]['value'];
export type PreviewFont = (typeof FONTS)[number]['value'];
export type Density = 'comfortable' | 'compact';
export type PreviewMode = 'light' | 'dark';
export interface Appearance {
	version: 1;
	palette: Palette;
	font: PreviewFont;
	density: Density;
	primary: string | null;
}
export const DEFAULT_APPEARANCE: Appearance = {
	version: 1,
	palette: 'teal',
	font: 'kanit',
	density: 'comfortable',
	primary: null
};
export function isPalette(value: unknown): value is Palette {
	return PALETTES.some((palette) => palette.value === value);
}
export function isFont(value: unknown): value is PreviewFont {
	return FONTS.some((font) => font.value === value);
}
export function isDensity(value: unknown): value is Density {
	return value === 'comfortable' || value === 'compact';
}
export function isHexColor(value: unknown): value is string {
	return typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value);
}
export function validateAppearance(value: unknown): Appearance | null {
	if (
		typeof value !== 'object' ||
		value === null ||
		!('version' in value) ||
		value.version !== 1 ||
		!('palette' in value) ||
		!isPalette(value.palette) ||
		!('font' in value) ||
		!isFont(value.font) ||
		!('density' in value) ||
		!isDensity(value.density) ||
		!('primary' in value) ||
		(value.primary !== null && !isHexColor(value.primary))
	)
		return null;
	return {
		version: 1,
		palette: value.palette,
		font: value.font,
		density: value.density,
		primary: typeof value.primary === 'string' ? value.primary.toLowerCase() : null
	};
}
export function readStoredAppearance(raw: string | null): Appearance | null {
	if (raw === null) return null;
	try {
		return validateAppearance(JSON.parse(raw));
	} catch {
		return null;
	}
}
export function appearanceFromQuery(params: URLSearchParams): Appearance {
	const palette = params.get('palette');
	const font = params.get('font');
	const density = params.get('density');
	const primary = params.get('primary');
	return {
		version: 1,
		palette: isPalette(palette) ? palette : 'teal',
		font: isFont(font) ? font : 'kanit',
		density: isDensity(density) ? density : 'comfortable',
		primary: isHexColor(primary) ? primary.toLowerCase() : null
	};
}
export function staffPreviewUrl(appearance: Appearance, mode: PreviewMode): string {
	const params = new URLSearchParams({
		palette: appearance.palette,
		font: appearance.font,
		density: appearance.density,
		mode
	});
	if (appearance.primary) params.set('primary', appearance.primary);
	return `/dev/staff-design-preview?${params.toString()}`;
}
export function palettePrimary(appearance: Appearance): string {
	return (
		appearance.primary ??
		(PALETTES.find((palette) => palette.value === appearance.palette) ?? PALETTES[0]).primary
	);
}
function rgb(hex: string): number[] {
	return [1, 3, 5].map((start) => Number.parseInt(hex.slice(start, start + 2), 16));
}
function mix(first: string, second: string, amount: number): string {
	const end = rgb(second);
	return (
		'#' +
		rgb(first)
			.map((channel, index) =>
				Math.round(channel * (1 - amount) + end[index] * amount)
					.toString(16)
					.padStart(2, '0')
			)
			.join('')
	);
}
function luminance(hex: string): number {
	const channels = rgb(hex).map((channel) => {
		const value = channel / 255;
		return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
	});
	return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
}
export function contrastRatio(first: string, second: string): number {
	const a = luminance(first),
		b = luminance(second);
	return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}
export function appearanceTokens(appearance: Appearance, mode: PreviewMode) {
	const dark = mode === 'dark';
	const palette = PALETTES.find((item) => item.value === appearance.palette) ?? PALETTES[0];
	const requested = palettePrimary(appearance);
	const card = dark ? '#1d302a' : '#ffffff';
	const background = dark ? '#152521' : palette.background;
	const foreground = dark ? '#e8efe8' : '#203a37';
	const primaryForeground = dark ? '#142b25' : '#ffffff';
	const surfaces = (primary: string) => ({
		secondary: mix(card, primary, dark ? 0.12 : 0.08),
		accent: mix(card, primary, dark ? 0.15 : 0.08),
		welcome: mix(card, primary, dark ? 0.12 : 0.09)
	});
	let primary = requested;
	for (let step = 0; step <= 100; step++) {
		primary = mix(requested, dark ? '#ffffff' : '#000000', step / 100);
		const candidateSurfaces = [card, background, ...Object.values(surfaces(primary))];
		if (
			contrastRatio(primary, primaryForeground) >= 4.5 &&
			contrastRatio(mix(primary, card, 0.1), primaryForeground) >= 4.5 &&
			contrastRatio(mix(primary, background, 0.1), primaryForeground) >= 4.5 &&
			candidateSurfaces.every((surface) => contrastRatio(primary, surface) >= 4.5)
		)
			break;
	}
	const tinted = surfaces(primary);
	return {
		requested,
		hoverOnCard: mix(primary, card, 0.1),
		hoverOnBackground: mix(primary, background, 0.1),
		adjusted: primary !== requested,
		primary,
		primaryForeground,
		background,
		foreground,
		card,
		...tinted,
		border: dark ? '#354b3e' : '#e0e7df',
		mutedForeground: dark ? '#afc1b4' : '#61746d',
		warning: dark ? '#efc37c' : '#98651b',
		warningSurface: dark ? '#453d2c' : '#fcf3df',
		destructive: dark ? '#f4a49a' : '#b54441',
		destructiveSurface: dark ? '#4a3531' : '#fbecea',
		success: dark ? '#91d4ac' : '#276442',
		successSurface: dark ? '#243c2d' : '#e9f3eb'
	};
}
export function appearanceStyle(appearance: Appearance, mode: PreviewMode): string {
	const tokens = appearanceTokens(appearance, mode);
	const font = FONTS.find((item) => item.value === appearance.font) ?? FONTS[0];
	const css = {
		background: tokens.background,
		foreground: tokens.foreground,
		card: tokens.card,
		'card-foreground': tokens.foreground,
		popover: tokens.card,
		'popover-foreground': tokens.foreground,
		primary: tokens.primary,
		'primary-foreground': tokens.primaryForeground,
		secondary: tokens.secondary,
		'secondary-foreground': tokens.foreground,
		muted: tokens.secondary,
		'muted-foreground': tokens.mutedForeground,
		accent: tokens.accent,
		'accent-foreground': tokens.foreground,
		destructive: tokens.destructive,
		border: tokens.border,
		input: tokens.border,
		ring: tokens.primary,
		warning: tokens.warning,
		'warning-surface': tokens.warningSurface,
		'destructive-surface': tokens.destructiveSurface,
		success: tokens.success,
		'success-surface': tokens.successSurface,
		welcome: tokens.welcome,
		sidebar: tokens.card,
		'preview-font': `'${font.family}', sans-serif`
	};
	return Object.entries(css)
		.map(([name, value]) => `--${name}:${value}`)
		.join(';');
}
