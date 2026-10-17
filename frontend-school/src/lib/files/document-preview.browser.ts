import { getDocument, GlobalWorkerOptions } from 'pdfjs-dist';
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url';
import type { PdfPreview } from './document-preview.js';
GlobalWorkerOptions.workerSrc = workerUrl;

export async function openPdfPreview(blob: Blob, signal: AbortSignal): Promise<PdfPreview> {
	signal.throwIfAborted();
	const bytes = new Uint8Array(await blob.arrayBuffer());
	signal.throwIfAborted();
	const task = getDocument({ data: bytes, stopAtErrors: true });
	const abort = () => {
		void task.destroy();
	};
	signal.addEventListener('abort', abort, { once: true });
	try {
		const document = await task.promise;
		signal.throwIfAborted();
		return {
			pageCount: document.numPages,
			async pageSize(pageNumber) {
				const page = await document.getPage(pageNumber);
				const viewport = page.getViewport({ scale: 1 });
				return { width: viewport.width, height: viewport.height };
			},
			async renderPage(pageNumber, canvas, renderSignal) {
				renderSignal.throwIfAborted();
				const page = await document.getPage(pageNumber);
				renderSignal.throwIfAborted();
				const base = page.getViewport({ scale: 1 });
				// Bound raster memory while retaining readable zoom through the original download.
				const viewport = page.getViewport({
					scale: Math.min(2, 1600 / Math.max(base.width, base.height))
				});
				canvas.width = Math.ceil(viewport.width);
				canvas.height = Math.ceil(viewport.height);
				const context = canvas.getContext('2d');
				if (!context) throw new Error('ไม่สามารถแสดงเอกสารได้');
				const render = page.render({ canvas, canvasContext: context, viewport });
				const cancel = () => render.cancel();
				renderSignal.addEventListener('abort', cancel, { once: true });
				try {
					await render.promise;
				} finally {
					renderSignal.removeEventListener('abort', cancel);
				}
			},
			async dispose() {
				signal.removeEventListener('abort', abort);
				await task.destroy();
			}
		};
	} catch (error) {
		signal.removeEventListener('abort', abort);
		await task.destroy();
		throw error;
	}
}
