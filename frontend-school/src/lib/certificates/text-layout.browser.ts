import { chooseAutoShrinkFontSize } from './layout.ts';

export type CertificateTextAlignment = 'left' | 'center' | 'right';

export type CertificateTextShadowMetrics = {
	offsetX: number;
	offsetY: number;
	blur: number;
};

export type CertificateTextBounds = {
	left: number;
	top: number;
	right: number;
	bottom: number;
};

export type MeasuredCertificateTextLine = {
	text: string;
	x: number;
	baseline: number;
	inkBounds: CertificateTextBounds;
	bounds: CertificateTextBounds;
};

export type MeasuredCertificateTextLayout = {
	fontSize: number;
	lineHeight: number;
	lines: MeasuredCertificateTextLine[];
	bounds: CertificateTextBounds;
	fits: boolean;
};

export type MeasureCertificateTextLayoutInput = {
	text: string;
	fontSize: number;
	minFontSize: number;
	autoShrink: boolean;
	lineHeight: number;
	frameWidth: number;
	frameHeight: number;
	alignment: CertificateTextAlignment;
	shadow?: CertificateTextShadowMetrics | null;
	fontForSize: (fontSize: number) => string;
};

type TextMeasureContext = Pick<
	CanvasRenderingContext2D,
	'font' | 'measureText' | 'textAlign' | 'textBaseline'
>;

type LineMetrics = {
	text: string;
	width: number;
	ascent: number;
	descent: number;
	left: number;
	right: number;
};

const fitTolerance = 0.01;

function wordSegments(text: string): string[] {
	if (typeof Intl.Segmenter !== 'undefined') {
		return Array.from(
			new Intl.Segmenter('th', { granularity: 'word' }).segment(text),
			(segment) => segment.segment
		);
	}
	return Array.from(text);
}

function graphemeSegments(text: string): string[] {
	if (typeof Intl.Segmenter !== 'undefined') {
		return Array.from(
			new Intl.Segmenter('th', { granularity: 'grapheme' }).segment(text),
			(segment) => segment.segment
		);
	}
	return Array.from(text);
}

function trimLineEnd(value: string): string {
	return value.replace(/\s+$/u, '');
}

function wrapSingleParagraph(
	context: TextMeasureContext,
	paragraph: string,
	maxWidth: number
): string[] {
	if (!paragraph) return [''];
	const lines: string[] = [];
	let current = '';

	const pushCurrent = () => {
		lines.push(trimLineEnd(current));
		current = '';
	};

	const appendPiece = (piece: string) => {
		if (!current && /^\s+$/u.test(piece)) return;
		const candidate = current + piece;
		if (context.measureText(candidate).width <= maxWidth) {
			current = candidate;
			return;
		}
		if (current) pushCurrent();
		if (context.measureText(piece).width <= maxWidth) {
			current = piece.replace(/^\s+/u, '');
			return;
		}

		for (const grapheme of graphemeSegments(piece)) {
			const graphemeCandidate = current + grapheme;
			if (current && context.measureText(graphemeCandidate).width > maxWidth) pushCurrent();
			current += grapheme;
		}
	};

	for (const segment of wordSegments(paragraph)) appendPiece(segment);
	if (current || lines.length === 0) pushCurrent();
	return lines;
}

function wrappedLines(context: TextMeasureContext, text: string, maxWidth: number): string[] {
	return text
		.replace(/\r\n?/gu, '\n')
		.split('\n')
		.flatMap((paragraph) => wrapSingleParagraph(context, paragraph, maxWidth));
}

function finiteMetric(value: number | undefined, fallback: number): number {
	return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function measureLine(context: TextMeasureContext, text: string, fontSize: number): LineMetrics {
	const metrics = context.measureText(text);
	const width = Math.max(0, finiteMetric(metrics.width, 0));
	const hasInk = text.length > 0;
	const measuredAscent = finiteMetric(metrics.actualBoundingBoxAscent, 0);
	const measuredDescent = finiteMetric(metrics.actualBoundingBoxDescent, 0);
	return {
		text,
		width,
		ascent:
			hasInk && measuredAscent > 0 ? measuredAscent : Math.max(fontSize * 1.15, measuredAscent),
		descent:
			hasInk && measuredDescent > 0 ? measuredDescent : Math.max(fontSize * 0.35, measuredDescent),
		left: hasInk ? finiteMetric(metrics.actualBoundingBoxLeft, 0) : 0,
		right: hasInk ? finiteMetric(metrics.actualBoundingBoxRight, width) : 0
	};
}

function shadowExtent(shadow: CertificateTextShadowMetrics | null | undefined): number {
	return shadow ? Math.max(0, shadow.blur) * 2 : 0;
}

function outerBounds(
	inkBounds: CertificateTextBounds,
	shadow: CertificateTextShadowMetrics | null | undefined,
	safety: number
): CertificateTextBounds {
	const blur = shadowExtent(shadow);
	const offsetX = shadow?.offsetX ?? 0;
	const offsetY = shadow?.offsetY ?? 0;
	return {
		left: inkBounds.left + Math.min(0, offsetX - blur) - safety,
		top: inkBounds.top + Math.min(0, offsetY - blur) - safety,
		right: inkBounds.right + Math.max(0, offsetX + blur) + safety,
		bottom: inkBounds.bottom + Math.max(0, offsetY + blur) + safety
	};
}

function horizontalOrigin(
	alignment: CertificateTextAlignment,
	frameWidth: number,
	lineWidth: number
): number {
	switch (alignment) {
		case 'left':
			return 0;
		case 'center':
			return (frameWidth - lineWidth) / 2;
		case 'right':
			return frameWidth - lineWidth;
	}
}

function shiftedBounds(bounds: CertificateTextBounds, x: number, y: number): CertificateTextBounds {
	return {
		left: bounds.left + x,
		top: bounds.top + y,
		right: bounds.right + x,
		bottom: bounds.bottom + y
	};
}

function layoutAt(
	context: TextMeasureContext,
	input: MeasureCertificateTextLayoutInput,
	fontSize: number
): MeasuredCertificateTextLayout {
	const previousFont = context.font;
	const previousAlignment = context.textAlign;
	const previousBaseline = context.textBaseline;
	try {
		context.font = input.fontForSize(fontSize);
		context.textAlign = 'left';
		context.textBaseline = 'alphabetic';
		const lineHeight = fontSize * input.lineHeight;
		const safety = Math.max(1, Math.ceil(fontSize * 0.03));
		const metrics = wrappedLines(context, input.text, input.frameWidth).map((line) =>
			measureLine(context, line, fontSize)
		);
		const provisional = metrics.map((line, index) => {
			const x = horizontalOrigin(input.alignment, input.frameWidth, line.width);
			const baseline = index * lineHeight;
			const inkBounds = {
				left: x - line.left,
				top: baseline - line.ascent,
				right: x + line.right,
				bottom: baseline + line.descent
			};
			return { line, x, baseline, inkBounds, bounds: outerBounds(inkBounds, input.shadow, safety) };
		});
		const rawTop = Math.min(...provisional.map((line) => line.bounds.top));
		const rawBottom = Math.max(...provisional.map((line) => line.bounds.bottom));
		const verticalOffset = -rawTop;
		let fits = rawBottom - rawTop <= input.frameHeight + fitTolerance;
		const lines = provisional.map(({ line, x, baseline, inkBounds, bounds }) => {
			const outerWidth = bounds.right - bounds.left;
			fits &&= outerWidth <= input.frameWidth + fitTolerance;
			let horizontalOffset = 0;
			if (bounds.left < 0) horizontalOffset = -bounds.left;
			if (bounds.right + horizontalOffset > input.frameWidth) {
				horizontalOffset += input.frameWidth - (bounds.right + horizontalOffset);
			}
			return {
				text: line.text,
				x: x + horizontalOffset,
				baseline: baseline + verticalOffset,
				inkBounds: shiftedBounds(inkBounds, horizontalOffset, verticalOffset),
				bounds: shiftedBounds(bounds, horizontalOffset, verticalOffset)
			};
		});
		const bounds = {
			left: Math.min(...lines.map((line) => line.bounds.left)),
			top: Math.min(...lines.map((line) => line.bounds.top)),
			right: Math.max(...lines.map((line) => line.bounds.right)),
			bottom: Math.max(...lines.map((line) => line.bounds.bottom))
		};
		return { fontSize, lineHeight, lines, bounds, fits };
	} finally {
		context.font = previousFont;
		context.textAlign = previousAlignment;
		context.textBaseline = previousBaseline;
	}
}

export function measureCertificateTextLayout(
	context: TextMeasureContext,
	input: MeasureCertificateTextLayoutInput
): MeasuredCertificateTextLayout {
	const fontSize = chooseAutoShrinkFontSize({
		fontSize: input.fontSize,
		minFontSize: input.minFontSize,
		autoShrink: input.autoShrink,
		fits: (candidate) => layoutAt(context, input, candidate).fits
	});
	return layoutAt(context, input, fontSize);
}
