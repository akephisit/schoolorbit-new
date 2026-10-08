import type { Content, ContentCanvas, ContentStack, Node, TCreatedPdf } from 'pdfmake/interfaces';

type Position = Node['startPosition'];
type PdfDocument = Awaited<ReturnType<TCreatedPdf['getStream']>>;
interface Frame {
	width: number;
	radius: number;
	lineWidth: number;
	start?: Position;
	end?: Position;
}

/** Use layout markers so corners follow actual table heights, including wrapped text. */
export function createTimetablePdfCorners() {
	const frames: Frame[] = [];
	const markers = new Map<string, { frame: Frame; edge: 'start' | 'end' }>();
	return {
		wrap(table: Content, width: number, radius: number, lineWidth: number): ContentStack {
			const frame: Frame = { width, radius, lineWidth };
			const index = frames.push(frame) - 1;
			const marker = (edge: 'start' | 'end'): Content => {
				const id = `timetable-corner-${index}-${edge}`;
				markers.set(id, { frame, edge });
				const content: ContentCanvas & { id: string } = {
					id,
					canvas: [{ type: 'line', x1: 0, y1: 0, x2: 0, y2: 0, lineColor: '#ffffff' }]
				};
				return content;
			};
			return { stack: [marker('start'), table, marker('end')] };
		},
		capture(node: Node): false {
			const marker = node.id ? markers.get(node.id) : undefined;
			if (marker) marker.frame[marker.edge] = node.startPosition;
			return false;
		},
		draw(document: PdfDocument): void {
			for (const frame of frames) {
				if (!frame.start || !frame.end) throw new Error('ไม่พบตำแหน่งกรอบตาราง PDF');
				const half = frame.lineWidth / 2;
				for (const edge of ['start', 'end'] as const) {
					const position = edge === 'start' ? frame.start : frame.end;
					document.switchToPage(position.pageNumber - 1);
					const y = position.top + (edge === 'start' ? half : -half);
					const sy = edge === 'start' ? 1 : -1;
					for (const side of ['left', 'right'] as const) {
						const x = position.left + (side === 'left' ? half : frame.width - half);
						const sx = side === 'left' ? 1 : -1;
						const point = (u: number, v: number) => `${x + sx * u} ${y + sy * v}`;
						const r = frame.radius;
						const control = r * (1 - 0.5522847498);
						const arc = `M ${point(r, 0)} C ${point(control, 0)} ${point(0, control)} ${point(0, r)}`;
						// Remove the square border and corner fill, then restore the curved border.
						const mask = `M ${point(-2, -2)} L ${point(r, -2)} L ${point(r, 0)} C ${point(control, 0)} ${point(0, control)} ${point(0, r)} L ${point(-2, r)} Z`;
						document.save();
						document.fillColor('#ffffff').path(mask).fill();
						document.lineWidth(frame.lineWidth).strokeColor('#9ca3af').path(arc).stroke();
						document.restore();
					}
				}
			}
		}
	};
}
