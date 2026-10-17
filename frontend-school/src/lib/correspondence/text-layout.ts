export type TextMeasure = (text: string, size: number) => number;

/** Matches the authoritative compositor's character wrapping and Thai mark attachment. */
export function fitAnnotationText(
	text: string,
	width: number,
	height: number,
	measure: TextMeasure
): { lines: string[]; size: number; overflow: boolean } {
	for (let size = 14; size >= 5; size -= 0.5) {
		const lines: string[] = [];
		for (const paragraph of text.split('\n')) {
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
		if (lines.length * size * 1.3 <= height) return { lines, size, overflow: false };
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
