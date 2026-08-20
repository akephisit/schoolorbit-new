import type { CertificateRenderManifest } from '$lib/api/certificates';

export type CertificatePreviewOptions = {
	scale?: number;
	signal?: AbortSignal;
};

export type CertificatePreviewResult = {
	widthPoints: number;
	heightPoints: number;
	widthPixels: number;
	heightPixels: number;
};

export type CertificateBackgroundInspection = CertificateRenderManifest['pageGeometry'];

export interface CertificateRenderer {
	inspectBackgroundPdf(file: Blob, signal?: AbortSignal): Promise<CertificateBackgroundInspection>;
	prepareFontAliases(
		manifest: CertificateRenderManifest,
		layout: CertificateRenderManifest['layout'],
		signal?: AbortSignal
	): Promise<Record<string, string>>;
	renderPreview(
		manifest: CertificateRenderManifest,
		canvas: HTMLCanvasElement,
		options?: CertificatePreviewOptions
	): Promise<CertificatePreviewResult>;
	buildCertificatePdf(manifests: readonly CertificateRenderManifest[]): Promise<Uint8Array>;
}

export async function loadCertificateRenderer(): Promise<CertificateRenderer> {
	const { createCertificateRenderer } = await import('./renderer.browser');
	return createCertificateRenderer();
}
