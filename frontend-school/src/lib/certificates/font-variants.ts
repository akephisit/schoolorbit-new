import type { SchoolFontSummary } from '../api/school-fonts';
import type { TextCertificateElement } from './editor-state';

export type CertificateFontVariant = {
	source: TextCertificateElement['fontSource'];
	family: string;
	familyKey: string;
	familyLabel: string;
	weight: number;
	style: TextCertificateElement['fontStyle'];
	label: string;
};

export type CertificateFontVariantPatch = Pick<
	TextCertificateElement,
	'fontSource' | 'fontFamily' | 'fontWeight' | 'fontStyle'
>;

const styleOrder: Record<TextCertificateElement['fontStyle'], number> = {
	normal: 0,
	italic: 1
};

function normalizedFamily(value: string): string {
	return value.normalize('NFKC').trim().toLocaleLowerCase('en-US');
}

function compareText(left: string, right: string): number {
	return left < right ? -1 : left > right ? 1 : 0;
}

function variantLabel(weight: number, style: TextCertificateElement['fontStyle']): string {
	return `${weight} · ${style === 'italic' ? 'ตัวเอียง' : 'ตัวปกติ'}`;
}

function compareVariants(left: CertificateFontVariant, right: CertificateFontVariant): number {
	return (
		compareText(left.familyKey, right.familyKey) ||
		styleOrder[left.style] - styleOrder[right.style] ||
		left.weight - right.weight ||
		compareText(sourceKey(left.source), sourceKey(right.source))
	);
}

function sourceKey(source: TextCertificateElement['fontSource']): string {
	return source.type === 'school_font' ? `school_font:${source.font_id}` : 'built_in';
}

function cloneSource(
	source: TextCertificateElement['fontSource']
): TextCertificateElement['fontSource'] {
	return source.type === 'school_font'
		? { type: 'school_font', font_id: source.font_id }
		: { type: 'built_in' };
}

function closestRegularVariant(
	variants: readonly CertificateFontVariant[]
): CertificateFontVariant | null {
	return (
		[...variants].sort(
			(left, right) =>
				Math.abs(left.weight - 400) - Math.abs(right.weight - 400) ||
				left.weight - right.weight ||
				compareVariants(left, right)
		)[0] ?? null
	);
}

export function certificateFontVariants(
	fonts: readonly SchoolFontSummary[]
): CertificateFontVariant[] {
	const builtIn: CertificateFontVariant[] = [400, 700].map((weight) => ({
		source: { type: 'built_in' },
		family: 'Sarabun',
		familyKey: 'built_in:sarabun',
		familyLabel: 'Sarabun (มาตรฐาน)',
		weight,
		style: 'normal',
		label: variantLabel(weight, 'normal')
	}));
	const uploaded = fonts.map(
		(font): CertificateFontVariant => ({
			source: { type: 'school_font', font_id: font.id },
			family: font.fontFamily,
			familyKey: `school_font:${normalizedFamily(font.fontFamily)}`,
			familyLabel: `${font.fontFamily} (คลังโรงเรียน)`,
			weight: font.fontWeight,
			style: font.fontStyle,
			label: variantLabel(font.fontWeight, font.fontStyle)
		})
	);
	return [...builtIn, ...uploaded].sort(compareVariants);
}

export function findCertificateFontVariant(
	variants: readonly CertificateFontVariant[],
	element: TextCertificateElement
): CertificateFontVariant | null {
	return (
		variants.find(
			(variant) =>
				variant.family === element.fontFamily &&
				variant.weight === element.fontWeight &&
				variant.style === element.fontStyle &&
				sourceKey(variant.source) === sourceKey(element.fontSource)
		) ?? null
	);
}

export function selectFontFamily(
	variants: readonly CertificateFontVariant[],
	familyKey: string
): CertificateFontVariant | null {
	const family = variants.filter((variant) => variant.familyKey === familyKey);
	return (
		family.find((variant) => variant.style === 'normal' && variant.weight === 400) ??
		closestRegularVariant(family.filter((variant) => variant.style === 'normal')) ??
		[...family].sort(compareVariants)[0] ??
		null
	);
}

export function selectFontWeight(
	variants: readonly CertificateFontVariant[],
	current: CertificateFontVariant | null,
	weight: number
): CertificateFontVariant | null {
	if (!current || !Number.isInteger(weight)) return null;
	return (
		variants.find(
			(variant) =>
				variant.familyKey === current.familyKey &&
				variant.weight === weight &&
				variant.style === current.style
		) ?? null
	);
}

export function toggleBoldVariant(
	variants: readonly CertificateFontVariant[],
	current: CertificateFontVariant | null
): CertificateFontVariant | null {
	if (!current) return null;
	const sameStyle = variants.filter(
		(variant) => variant.familyKey === current.familyKey && variant.style === current.style
	);
	if (current.weight !== 700) {
		return sameStyle.find((variant) => variant.weight === 700) ?? null;
	}
	return closestRegularVariant(sameStyle.filter((variant) => variant.weight !== 700));
}

export function toggleItalicVariant(
	variants: readonly CertificateFontVariant[],
	current: CertificateFontVariant | null
): CertificateFontVariant | null {
	if (!current) return null;
	const targetStyle = current.style === 'italic' ? 'normal' : 'italic';
	return (
		variants.find(
			(variant) =>
				variant.familyKey === current.familyKey &&
				variant.weight === current.weight &&
				variant.style === targetStyle
		) ?? null
	);
}

export function fontVariantPatch(variant: CertificateFontVariant): CertificateFontVariantPatch {
	return {
		fontSource: cloneSource(variant.source),
		fontFamily: variant.family,
		fontWeight: variant.weight,
		fontStyle: variant.style
	};
}
