/** Coordinates are stored against the unrotated page, independently of display size. */
export interface PagePoint {
	x: number;
	y: number;
}
export interface InkPath {
	points: PagePoint[];
}
export type PageRotation = 0 | 90 | 180 | 270;

export function unit(value: number): number {
	return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0;
}

export function fromDisplay(point: PagePoint, rotation: PageRotation): PagePoint {
	const x = unit(point.x),
		y = unit(point.y);
	switch (rotation) {
		case 90:
			return { x: y, y: 1 - x };
		case 180:
			return { x: 1 - x, y: 1 - y };
		case 270:
			return { x: 1 - y, y: x };
		default:
			return { x, y };
	}
}

export function toDisplay(point: PagePoint, rotation: PageRotation): PagePoint {
	return fromDisplay(point, ((360 - rotation) % 360) as PageRotation);
}

export function pointerPoint(
	clientX: number,
	clientY: number,
	bounds: Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>,
	rotation: PageRotation = 0
): PagePoint {
	return fromDisplay(
		{
			x: unit((clientX - bounds.left) / bounds.width),
			y: unit((clientY - bounds.top) / bounds.height)
		},
		rotation
	);
}

export function pathData(points: PagePoint[], rotation: PageRotation = 0): string {
	return points
		.map((point, index) => {
			const displayed = toDisplay(point, rotation);
			return `${index ? 'L' : 'M'} ${displayed.x * 1000} ${displayed.y * 1000}`;
		})
		.join(' ');
}

export function hasInk(paths: InkPath[]): boolean {
	return paths.some(
		(path) =>
			path.points.length > 1 &&
			path.points.some(
				(point) =>
					Math.abs(point.x - path.points[0].x) + Math.abs(point.y - path.points[0].y) > 0.003
			)
	);
}
