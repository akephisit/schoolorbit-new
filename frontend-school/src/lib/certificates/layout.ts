export const PDF_POINTS_PER_INCH = 72;
export const MILLIMETRES_PER_INCH = 25.4;
export const CERTIFICATE_RENDER_DPI = 300;

export type SupportedPageRotation = 0 | 90 | 180 | 270;

export type PageSize = {
	width: number;
	height: number;
};

export type BackgroundPageTransform = {
	x: number;
	y: number;
	rotation: SupportedPageRotation;
};

type AutoShrinkOptions = {
	fontSize: number;
	minFontSize: number;
	autoShrink: boolean;
	fits: (fontSize: number) => boolean;
};

function assertPositiveFinite(value: number, name: string): void {
	if (!Number.isFinite(value) || value <= 0) {
		throw new Error(`${name} must be a positive finite number`);
	}
}

export function supportedPageRotation(rotation: number): SupportedPageRotation {
	if (rotation === 0 || rotation === 90 || rotation === 180 || rotation === 270) {
		return rotation;
	}
	throw new Error(`Unsupported certificate page rotation: ${rotation}`);
}

export function normalizePageRotation(rotation: number): SupportedPageRotation {
	if (!Number.isSafeInteger(rotation)) {
		throw new Error(`Unsupported certificate page rotation: ${rotation}`);
	}
	return supportedPageRotation(((rotation % 360) + 360) % 360);
}

export function millimetresToPoints(millimetres: number): number {
	return (millimetres * PDF_POINTS_PER_INCH) / MILLIMETRES_PER_INCH;
}

export function pointsToMillimetres(points: number): number {
	return (points * MILLIMETRES_PER_INCH) / PDF_POINTS_PER_INCH;
}

export function pointsToPixels(points: number, dpi = CERTIFICATE_RENDER_DPI): number {
	assertPositiveFinite(dpi, 'dpi');
	return (points * dpi) / PDF_POINTS_PER_INCH;
}

export function displayedPageSize(
	sourceWidth: number,
	sourceHeight: number,
	rotation: number
): PageSize {
	assertPositiveFinite(sourceWidth, 'sourceWidth');
	assertPositiveFinite(sourceHeight, 'sourceHeight');
	const normalizedRotation = supportedPageRotation(rotation);
	return normalizedRotation === 90 || normalizedRotation === 270
		? { width: sourceHeight, height: sourceWidth }
		: { width: sourceWidth, height: sourceHeight };
}

export function backgroundPageTransform(
	sourceWidth: number,
	sourceHeight: number,
	rotation: number
): BackgroundPageTransform {
	assertPositiveFinite(sourceWidth, 'sourceWidth');
	assertPositiveFinite(sourceHeight, 'sourceHeight');
	switch (supportedPageRotation(rotation)) {
		case 0:
			return { x: 0, y: 0, rotation: 0 };
		case 90:
			return { x: sourceHeight, y: 0, rotation: 90 };
		case 180:
			return { x: sourceWidth, y: sourceHeight, rotation: 180 };
		case 270:
			return { x: 0, y: sourceWidth, rotation: 270 };
	}
}

export function chooseAutoShrinkFontSize({
	fontSize,
	minFontSize,
	autoShrink,
	fits
}: AutoShrinkOptions): number {
	assertPositiveFinite(fontSize, 'fontSize');
	assertPositiveFinite(minFontSize, 'minFontSize');
	if (minFontSize > fontSize) throw new Error('minFontSize must not exceed fontSize');
	if (!autoShrink || fits(fontSize)) return fontSize;
	if (!fits(minFontSize)) return minFontSize;

	let lower = minFontSize;
	let upper = fontSize;
	for (let iteration = 0; iteration < 20; iteration += 1) {
		const candidate = (lower + upper) / 2;
		if (fits(candidate)) lower = candidate;
		else upper = candidate;
	}

	return Math.max(minFontSize, Math.min(fontSize, Math.floor(lower * 100) / 100));
}
