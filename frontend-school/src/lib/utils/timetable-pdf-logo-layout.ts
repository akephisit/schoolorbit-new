import type { ContentCanvas, Node, TCreatedPdf } from 'pdfmake/interfaces';

type PdfDocument = Awaited<ReturnType<TCreatedPdf['getStream']>>;
interface LogoFrame {
	width: number;
	padding: number;
	top?: Node['startPosition'];
	bottom?: Node['startPosition'];
}

/** Draw the crest after layout, centered in its measured three-row header span. */
export function createTimetablePdfLogoLayout(logo: string | null) {
	const frames: LogoFrame[] = [];
	const markers = new Map<string, { frame: LogoFrame; edge: 'top' | 'bottom' }>();
	return {
		register(width: number, padding: number) {
			const frame: LogoFrame = { width, padding };
			const index = frames.push(frame) - 1;
			const marker = (edge: 'top' | 'bottom'): ContentCanvas & { id: string } => {
				const id = `timetable-logo-${index}-${edge}`;
				markers.set(id, { frame, edge });
				return {
					id,
					canvas: [{ type: 'line', x1: 0, y1: 0, x2: 0, y2: 0, lineColor: '#ffffff' }]
				};
			};
			return { top: marker('top'), bottom: marker('bottom') };
		},
		capture(node: Node): void {
			const marker = node.id ? markers.get(node.id) : undefined;
			if (marker) marker.frame[marker.edge] = node.startPosition;
		},
		draw(document: PdfDocument): void {
			if (!logo) return;
			for (const frame of frames) {
				if (!frame.top || !frame.bottom || frame.top.pageNumber !== frame.bottom.pageNumber) {
					throw new Error('ไม่พบตำแหน่งช่องตราโรงเรียนใน PDF');
				}
				const height = frame.bottom.top - frame.top.top;
				if (height <= 4) throw new Error('ขนาดช่องตราโรงเรียนใน PDF ไม่ถูกต้อง');
				document.switchToPage(frame.top.pageNumber - 1);
				document.image(logo, frame.top.left + 2, frame.top.top - frame.padding + 2, {
					fit: [frame.width - 4, height - 4],
					align: 'center',
					valign: 'center'
				});
			}
		}
	};
}
