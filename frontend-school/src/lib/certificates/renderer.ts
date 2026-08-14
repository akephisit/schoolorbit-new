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

export interface CertificateRenderer {
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
