export interface PdfPreview {
	pageCount: number;
	pageSize(page: number): Promise<{ width: number; height: number }>;
	renderPage(page: number, canvas: HTMLCanvasElement, signal: AbortSignal): Promise<void>;
	dispose(): Promise<void>;
}
export async function loadPdfPreview(blob: Blob, signal: AbortSignal): Promise<PdfPreview> {
	const { openPdfPreview } = await import('./document-preview.browser.js');
	return openPdfPreview(blob, signal);
}
