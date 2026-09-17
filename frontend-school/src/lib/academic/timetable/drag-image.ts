interface DragImageRect {
	left: number;
	top: number;
	width: number;
	height: number;
}

export function dragImageOffset(
	clientX: number,
	clientY: number,
	rect: DragImageRect
): { x: number; y: number } {
	return {
		x: Math.round(Math.min(Math.max(clientX - rect.left, 0), rect.width)),
		y: Math.round(Math.min(Math.max(clientY - rect.top, 0), rect.height))
	};
}

export function alignDragImageToPointer(event: DragEvent, element: HTMLElement): void {
	if (!event.dataTransfer) return;
	const { x, y } = dragImageOffset(event.clientX, event.clientY, element.getBoundingClientRect());
	event.dataTransfer.setDragImage(element, x, y);
}
