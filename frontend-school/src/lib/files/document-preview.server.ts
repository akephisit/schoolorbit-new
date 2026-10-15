import type { PdfPreview } from './document-preview.js';
export async function loadPdfPreview(_blob: Blob, _signal: AbortSignal): Promise<PdfPreview> {
	throw new Error('เปิดเอกสารได้ในเบราว์เซอร์');
}
