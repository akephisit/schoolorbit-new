const POINTS_PER_MILLIMETRE = 72 / 25.4;
const PAPER_TOLERANCE_MM = 1;

type PaperGeometry = {
	widthPoints: number;
	heightPoints: number;
	rotation: number;
};

const standardPapers = [
	{ name: 'A4', widthMm: 210, heightMm: 297 },
	{ name: 'A5', widthMm: 148, heightMm: 210 },
	{ name: 'Letter', widthMm: 215.9, heightMm: 279.4 }
] as const;

function normalizedRotation(rotation: number): number {
	return ((Math.round(rotation) % 360) + 360) % 360;
}

function displayedSize(geometry: PaperGeometry): { widthMm: number; heightMm: number } {
	const widthMm = geometry.widthPoints / POINTS_PER_MILLIMETRE;
	const heightMm = geometry.heightPoints / POINTS_PER_MILLIMETRE;
	const rotation = normalizedRotation(geometry.rotation);

	return rotation === 90 || rotation === 270
		? { widthMm: heightMm, heightMm: widthMm }
		: { widthMm, heightMm };
}

function withinTolerance(actual: number, expected: number): boolean {
	return Math.abs(actual - expected) <= PAPER_TOLERANCE_MM;
}

function formatMillimetres(value: number): string {
	const rounded = Math.round(value * 10) / 10;
	return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
}

export function describePaper(geometry: PaperGeometry): string {
	const { widthMm, heightMm } = displayedSize(geometry);
	const orientation = widthMm > heightMm ? 'แนวนอน' : 'แนวตั้ง';

	for (const paper of standardPapers) {
		const portrait =
			withinTolerance(widthMm, paper.widthMm) && withinTolerance(heightMm, paper.heightMm);
		const landscape =
			withinTolerance(widthMm, paper.heightMm) && withinTolerance(heightMm, paper.widthMm);
		if (portrait || landscape) return `${paper.name} ${orientation}`;
	}

	return `ขนาดกำหนดเอง ${formatMillimetres(widthMm)} × ${formatMillimetres(heightMm)} มม.`;
}
