export const MAX_CERTIFICATE_PREVIEW_DPR = 2;
export const MAX_CERTIFICATE_PREVIEW_RENDER_SCALE = 2;

export type CertificatePreviewState = 'idle' | 'loading' | 'ready' | 'error';

export type CertificatePreviewFitInput = {
	availableWidth: number;
	availableHeight: number;
	pageWidthPoints: number;
	pageHeightPoints: number;
	devicePixelRatio: number;
};

export type CertificatePreviewFit = {
	logicalScale: number;
	cssWidth: number;
	cssHeight: number;
	renderScale: number;
};

function finitePositive(value: number): boolean {
	return Number.isFinite(value) && value > 0;
}

export function calculateCertificatePreviewFit(
	input: CertificatePreviewFitInput
): CertificatePreviewFit | null {
	const values = [
		input.availableWidth,
		input.availableHeight,
		input.pageWidthPoints,
		input.pageHeightPoints
	];
	if (!values.every(finitePositive)) return null;

	const logicalScale = Math.min(
		input.availableWidth / input.pageWidthPoints,
		input.availableHeight / input.pageHeightPoints
	);
	const deviceScale = Math.min(
		MAX_CERTIFICATE_PREVIEW_DPR,
		Math.max(1, finitePositive(input.devicePixelRatio) ? input.devicePixelRatio : 1)
	);

	return {
		logicalScale,
		cssWidth: input.pageWidthPoints * logicalScale,
		cssHeight: input.pageHeightPoints * logicalScale,
		renderScale: Math.min(MAX_CERTIFICATE_PREVIEW_RENDER_SCALE, logicalScale * deviceScale)
	};
}
