import {
	DEFAULT_APPEARANCE as DEFAULT_SCHOOL_APPEARANCE,
	validateAppearance as validateSchoolAppearance,
	isPalette,
	isFont,
	isDensity,
	isHexColor,
	type SchoolAppearance,
	type PreviewMode
} from '../appearance/model.ts';
export {
	PALETTES,
	FONTS,
	isPalette,
	isFont,
	isDensity,
	isHexColor,
	palettePrimary,
	appearanceStyle,
	appearanceTokens,
	contrastRatio
} from '../appearance/model.ts';
export type { Palette, PreviewFont, Density, PreviewMode } from '../appearance/model.ts';
export const APPEARANCE_STORAGE_KEY = 'schoolorbit:design-preview:appearance:v1';
export type Appearance = SchoolAppearance & { version: 1 };
export const DEFAULT_APPEARANCE: Appearance = { ...DEFAULT_SCHOOL_APPEARANCE, version: 1 };
export function validateAppearance(value: unknown): Appearance | null {
	if (typeof value !== 'object' || value === null || !('version' in value) || value.version !== 1)
		return null;
	const appearance = validateSchoolAppearance(value);
	return appearance ? { version: 1, ...appearance } : null;
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
	const palette = params.get('palette'),
		font = params.get('font'),
		density = params.get('density'),
		primary = params.get('primary');
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
