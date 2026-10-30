/** Coordinates relative to a semantic subject, independent of the sender's layout. */
export interface RelativePoint {
	u: number;
	v: number;
}

export interface ClientPoint {
	x: number;
	y: number;
}

export interface ClientRect {
	left: number;
	top: number;
	width: number;
	height: number;
}

function unit(value: number): number {
	return Math.min(1, Math.max(0, value));
}

export function relativePointer(point: ClientPoint, rect: ClientRect): RelativePoint | null {
	if (
		![point.x, point.y, rect.left, rect.top, rect.width, rect.height].every(Number.isFinite) ||
		rect.width <= 0 ||
		rect.height <= 0
	)
		return null;
	return {
		u: unit((point.x - rect.left) / rect.width),
		v: unit((point.y - rect.top) / rect.height)
	};
}

export function projectPointer(point: RelativePoint, rect: ClientRect): ClientPoint | null {
	if (
		![point.u, point.v, rect.left, rect.top, rect.width, rect.height].every(Number.isFinite) ||
		rect.width <= 0 ||
		rect.height <= 0
	)
		return null;
	return { x: rect.left + unit(point.u) * rect.width, y: rect.top + unit(point.v) * rect.height };
}

/** Interpolate within the subject before projecting into the recipient's current DOM. */
export function interpolatePointer(
	from: RelativePoint,
	to: RelativePoint,
	elapsed: number,
	duration = 66
): RelativePoint {
	const progress = duration > 0 ? unit(elapsed / duration) : 1;
	return { u: from.u + (to.u - from.u) * progress, v: from.v + (to.v - from.v) * progress };
}

export function intersectRects(left: ClientRect, right: ClientRect): ClientRect | null {
	const x = Math.max(left.left, right.left);
	const y = Math.max(left.top, right.top);
	const width = Math.min(left.left + left.width, right.left + right.width) - x;
	const height = Math.min(left.top + left.height, right.top + right.height) - y;
	return width > 0 && height > 0 ? { left: x, top: y, width, height } : null;
}

export function containsPointer(rect: ClientRect, point: ClientPoint): boolean {
	return (
		point.x >= rect.left &&
		point.y >= rect.top &&
		point.x <= rect.left + rect.width &&
		point.y <= rect.top + rect.height
	);
}

/** A latest-value scheduler: cursor sampling cannot accumulate an outbound queue. */
export function createPointerCoalescer<T, Timer>(dependencies: {
	send(value: T): void;
	setTimer(callback: () => void, delay: number): Timer;
	clearTimer(timer: Timer): void;
	delay?: number;
}) {
	let pending: { value: T } | null = null;
	let timer: Timer | null = null;
	let disposed = false;
	function flush() {
		if (timer !== null) dependencies.clearTimer(timer);
		timer = null;
		const next = pending;
		pending = null;
		if (next && !disposed) dependencies.send(next.value);
	}
	return {
		push(value: T) {
			if (disposed) return;
			pending = { value };
			if (timer === null) timer = dependencies.setTimer(flush, dependencies.delay ?? 40);
		},
		flush,
		cancel() {
			if (timer !== null) dependencies.clearTimer(timer);
			timer = null;
			pending = null;
		},
		dispose() {
			disposed = true;
			if (timer !== null) dependencies.clearTimer(timer);
			timer = null;
			pending = null;
		}
	};
}
