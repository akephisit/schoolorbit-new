export interface ChartTransform {
	x: number;
	y: number;
	scale: number;
}
export const clampScale = (scale: number) => Math.min(2.5, Math.max(0.08, scale));
export function zoomChart(
	view: ChartTransform,
	scale: number,
	anchor: { x: number; y: number }
): ChartTransform {
	const next = clampScale(scale),
		ratio = next / view.scale;
	return {
		x: anchor.x - (anchor.x - view.x) * ratio,
		y: anchor.y - (anchor.y - view.y) * ratio,
		scale: next
	};
}
export function fitChart(
	width: number,
	height: number,
	contentWidth: number,
	contentHeight: number
): ChartTransform {
	const scale = clampScale(
		Math.min(
			1,
			(width - 32) / Math.max(1, contentWidth),
			(height - 32) / Math.max(1, contentHeight)
		)
	);
	return { scale, x: (width - contentWidth * scale) / 2, y: (height - contentHeight * scale) / 2 };
}
