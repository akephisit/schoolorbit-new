import { apiClient, requireApiData, type ApiRequestOptions } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type ManualCertificateVerificationRequest = Schemas['ManualCertificateVerificationRequest'];
export type QrCertificateVerificationRequest = Schemas['QrCertificateVerificationRequest'];
export type PublicCertificateVerificationData = Schemas['PublicCertificateVerificationData'];
export type PublicCertificateRenderRequest = Schemas['PublicCertificateRenderRequest'];
export type CertificateRenderManifest = Schemas['CertificateRenderManifest'];

const GENERIC_VERIFICATION_ERROR = 'ไม่พบข้อมูลที่ตรงกัน';

export async function verifyCertificateManually(
	payload: ManualCertificateVerificationRequest,
	options: ApiRequestOptions = {}
): Promise<PublicCertificateVerificationData> {
	const response = await apiClient.postPublic<PublicCertificateVerificationData>(
		'/api/public/certificates/verify/manual',
		payload,
		options
	);
	return requireApiData(response, GENERIC_VERIFICATION_ERROR);
}

export async function verifyCertificateByQr(
	payload: QrCertificateVerificationRequest,
	options: ApiRequestOptions = {}
): Promise<PublicCertificateVerificationData> {
	const response = await apiClient.postPublic<PublicCertificateVerificationData>(
		'/api/public/certificates/verify/qr',
		payload,
		options
	);
	return requireApiData(response, GENERIC_VERIFICATION_ERROR);
}

export async function createPublicCertificateRenderManifest(
	payload: PublicCertificateRenderRequest,
	options: ApiRequestOptions = {}
): Promise<CertificateRenderManifest> {
	const response = await apiClient.postPublic<CertificateRenderManifest>(
		'/api/public/certificates/render-manifest',
		payload,
		options
	);
	return requireApiData(response, 'ไม่สามารถสร้างไฟล์เกียรติบัตรได้');
}
