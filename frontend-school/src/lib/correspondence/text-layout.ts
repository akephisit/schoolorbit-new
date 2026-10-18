export type TextMeasure = (text: string, size: number) => number;

/** Canonical PDF points; the compositor uses the same four-point border inset. */
export function fitAnnotationBox(
	text: string,
	width: number,
	height: number,
	measure: TextMeasure,
	border = false
) {
	const padding = border ? 4 : 0;
	return {
		...fitAnnotationText(text, width - 2 * padding, height - 2 * padding, measure),
		padding
	};
}

/** Matches the authoritative compositor's character wrapping and Thai mark attachment. */
export function fitAnnotationText(
	text: string,
	width: number,
	height: number,
	measure: TextMeasure
): { lines: string[]; size: number; overflow: boolean } {
	if (![width, height].every(Number.isFinite) || width <= 0 || height <= 0)
		return { lines: [], size: 5, overflow: true };
	const normalized = text.replace(/\r\n/gu, '\n').replace(/\r/gu, '\n').replace(/\t/gu, '    ');
	for (let size = 14; size >= 5; size -= 0.5) {
		const lines: string[] = [];
		for (const paragraph of normalized.split('\n')) {
			let line = '';
			for (const character of paragraph) {
				const combining = /[\u0e31\u0e34-\u0e3a\u0e47-\u0e4e]/u.test(character);
				if (!combining && line && measure(line + character, size) > width) {
					lines.push(line);
					line = character;
				} else line += character;
			}
			lines.push(line);
		}
		if (lines.length * size * 1.3 <= height && lines.every((line) => measure(line, size) <= width))
			return { lines, size, overflow: false };
		if (size === 5) return { lines, size, overflow: true };
	}
	return { lines: [], size: 5, overflow: true };
}

// One immutable bundled font/measurement context; no document text or user drafts are retained.
let fontMeasurePromise: Promise<TextMeasure> | undefined;
export function loadAnnotationTextMeasure(): Promise<TextMeasure> {
	fontMeasurePromise ??= createAnnotationTextMeasure().catch((cause) => {
		fontMeasurePromise = undefined;
		throw cause;
	});
	return fontMeasurePromise;
}
async function createAnnotationTextMeasure(): Promise<TextMeasure> {
	const face = new FontFace('CorrespondenceSarabun', 'url(/fonts/Sarabun-Regular.ttf)');
	const loaded = await face.load();
	document.fonts.add(loaded);
	const context = document.createElement('canvas').getContext('2d');
	if (!context) throw new Error('เตรียมแบบอักษรสำหรับหมายเหตุไม่สำเร็จ');
	return (text, size) => {
		context.font = `${size}px CorrespondenceSarabun`;
		return context.measureText(text).width;
	};
}
