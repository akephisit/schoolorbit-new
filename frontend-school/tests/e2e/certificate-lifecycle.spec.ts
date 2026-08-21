import { expect, test } from '@playwright/test';
import type {
	APIRequestContext,
	APIResponse,
	Browser,
	BrowserContext,
	Download,
	Locator,
	Page,
	Response as BrowserResponse
} from '@playwright/test';
import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PDFDocument } from 'pdf-lib';
import type { components } from '../../src/lib/api/generated/school-api';

type Schemas = components['schemas'];
type AcademicYear = Schemas['AcademicYearLookupItem'];
type CurrentUser = Schemas['CurrentUserResponse'];
type OrganizationUnit = Schemas['OrganizationUnitLookupItem'];
type CertificateCampaign = Schemas['CertificateCampaignDetail'];
type CertificateTemplate = Schemas['CertificateTemplateDetail'];
type CertificateTemplateAsset = Schemas['CertificateTemplateAsset'];
type CertificateFontUploadInspection = Schemas['CertificateFontUploadInspection'];
type CertificateLayout = Schemas['CertificateLayoutV1'];
type FileMetadata = Schemas['FileMetadata'];
type CertificateCandidateAccount = Schemas['CertificateCandidateAccount'];
type CertificateCandidate = Schemas['CertificateCandidateDetail'];
type CertificateCandidateImportResult = Schemas['CertificateCandidateImportResult'];
type CertificateCandidateBulkResult = Schemas['CertificateCandidateBulkResult'];
type CertificateIssueRequest = Schemas['CertificateIssueRequestDetail'];
type IssueCertificateOutcome = Schemas['IssueCertificateOutcome'];
type IssuedCertificate = Schemas['IssuedCertificateSummary'];
type CertificateRenderManifest = Schemas['CertificateRenderManifest'];
type RevokeCertificateResult = Schemas['RevokeCertificateResult'];
type CertificateCampaignPurgeImpact = Schemas['CertificateCampaignPurgeImpact'];
type CertificateCampaignPurgeStatus = Schemas['CertificateCampaignPurgeStatus'];

type ApiFetchOptions = NonNullable<Parameters<APIRequestContext['fetch']>[1]>;
type ApiCallOptions = Pick<ApiFetchOptions, 'data' | 'multipart'>;

interface ApiEnvelope<T> {
	success: boolean;
	data?: T;
}

interface ApiOutcome<T> {
	status: number;
	data: T | null;
}

interface Credentials {
	username: string;
	password: string;
}

interface ActorSession {
	context: BrowserContext;
	page: Page;
	api: SchoolApi;
	user: CurrentUser;
}

interface UploadedFile {
	fileId: string;
	templateId: string;
}

interface LifecycleState {
	campaignId: string | null;
	campaignName: string | null;
	uploadedFiles: UploadedFile[];
}

const preparerUsername = process.env.E2E_CERT_PREPARER_USERNAME;
const preparerPassword = process.env.E2E_CERT_PREPARER_PASSWORD;
const issuerUsername = process.env.E2E_CERT_ISSUER_USERNAME;
const issuerPassword = process.env.E2E_CERT_ISSUER_PASSWORD;
const studentUsername = process.env.E2E_CERT_STUDENT_USERNAME;
const studentPassword = process.env.E2E_CERT_STUDENT_PASSWORD;
const hasLifecycleCredentials = [
	preparerUsername,
	preparerPassword,
	issuerUsername,
	issuerPassword,
	studentUsername,
	studentPassword
].every((value) => typeof value === 'string' && value.length > 0);

const baseURL =
	process.env.E2E_BASE_URL ||
	process.env.SMOKE_TENANT_URL ||
	`https://${process.env.SMOKE_SUBDOMAIN || 'sandbox'}.schoolorbit.app`;
const apiURL = (
	process.env.E2E_API_URL ||
	process.env.SMOKE_API_URL ||
	'https://school-api.schoolorbit.app'
).replace(/\/$/, '');
const primaryOrigin = new URL(baseURL).origin;
const schoolSubdomain =
	process.env.SMOKE_SUBDOMAIN || new URL(primaryOrigin).hostname.split('.')[0];
const publicManualVerificationPath = '/api/public/certificates/verify/manual';
const publicQrVerificationPath = '/api/public/certificates/verify/qr';
const fontPath = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../../static/fonts/Sarabun-Regular.ttf'
);
const onePixelPng = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
	'base64'
);

function safeApiRequestPath(requestPath: string): string {
	const privateSuffix = requestPath.search(/[?#]/u);
	return privateSuffix === -1 ? requestPath : requestPath.slice(0, privateSuffix);
}

class SchoolApi {
	private csrfToken = '';

	constructor(private readonly requestContext: APIRequestContext) {}

	async initialize(): Promise<CurrentUser> {
		const user = await this.request<CurrentUser>('GET', '/api/auth/me');
		if (!this.csrfToken) throw new Error('Authenticated session did not provide a CSRF token.');
		return user;
	}

	async request<T>(method: string, requestPath: string, options: ApiCallOptions = {}): Promise<T> {
		const response = await this.send(method, requestPath, options);
		const safePath = safeApiRequestPath(requestPath);
		if (!response.ok()) {
			const status = response.status();
			await response.dispose();
			throw new Error(`School API ${method} ${safePath} returned HTTP ${status}.`);
		}
		let envelope: ApiEnvelope<T>;
		try {
			envelope = (await response.json()) as ApiEnvelope<T>;
		} finally {
			await response.dispose();
		}
		if (envelope.success !== true || envelope.data === undefined) {
			throw new Error(`School API ${method} ${safePath} returned an invalid envelope.`);
		}
		return envelope.data;
	}

	async requestOutcome<T>(
		method: string,
		requestPath: string,
		emptyStatuses: readonly number[],
		options: ApiCallOptions = {}
	): Promise<ApiOutcome<T>> {
		const response = await this.send(method, requestPath, options);
		const status = response.status();
		const safePath = safeApiRequestPath(requestPath);
		if (emptyStatuses.includes(status)) {
			await response.dispose();
			return { status, data: null };
		}
		if (!response.ok()) {
			await response.dispose();
			throw new Error(`School API ${method} ${safePath} returned HTTP ${status}.`);
		}
		let envelope: ApiEnvelope<T>;
		try {
			envelope = (await response.json()) as ApiEnvelope<T>;
		} finally {
			await response.dispose();
		}
		if (envelope.success !== true || envelope.data === undefined) {
			throw new Error(`School API ${method} ${safePath} returned an invalid envelope.`);
		}
		return { status, data: envelope.data };
	}

	async expectFailure(
		method: string,
		requestPath: string,
		statuses: readonly number[],
		options: ApiCallOptions = {}
	): Promise<void> {
		const response = await this.send(method, requestPath, options);
		const status = response.status();
		await response.dispose();
		expect(statuses).toContain(status);
	}

	async bestEffort(
		method: string,
		requestPath: string,
		options: ApiCallOptions = {}
	): Promise<number | null> {
		try {
			const response = await this.send(method, requestPath, options);
			const status = response.status();
			await response.dispose();
			return status;
		} catch {
			return null;
		}
	}

	private async send(
		method: string,
		requestPath: string,
		options: ApiCallOptions
	): Promise<APIResponse> {
		const headers: Record<string, string> = {
			Accept: 'application/json',
			Origin: primaryOrigin,
			'X-School-Subdomain': schoolSubdomain
		};
		if (!['GET', 'HEAD', 'OPTIONS'].includes(method.toUpperCase()) && this.csrfToken) {
			headers['X-CSRF-Token'] = this.csrfToken;
		}
		const fetchOptions: ApiFetchOptions = {
			method,
			headers,
			failOnStatusCode: false
		};
		if (options.data !== undefined) fetchOptions.data = options.data;
		if (options.multipart !== undefined) fetchOptions.multipart = options.multipart;
		let response: APIResponse;
		try {
			response = await this.requestContext.fetch(`${apiURL}${requestPath}`, fetchOptions);
		} catch {
			throw new Error(
				`School API ${method} ${safeApiRequestPath(requestPath)} request failed without a response.`
			);
		}
		const nextCsrf = response.headers()['x-csrf-token'];
		if (nextCsrf) this.csrfToken = nextCsrf;
		return response;
	}
}

async function login(
	browser: Browser,
	credentials: Credentials,
	expectedLanding: RegExp
): Promise<ActorSession> {
	const context = await browser.newContext({ baseURL: primaryOrigin, acceptDownloads: true });
	const page = await context.newPage();
	try {
		await page.goto(`${primaryOrigin}/login`);
		await expect(page.getByRole('heading', { name: 'เข้าสู่ระบบ' })).toBeVisible();
		await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill(credentials.username);
		await page.getByLabel('รหัสผ่าน').fill(credentials.password);
		const landing = page.waitForURL(expectedLanding, { timeout: 20_000 });
		await page.getByRole('button', { name: 'เข้าสู่ระบบ' }).click();
		await landing;
		const api = new SchoolApi(context.request);
		const user = await api.initialize();
		return { context, page, api, user };
	} catch {
		await context.close();
		throw new Error('Dedicated certificate lifecycle login failed.');
	}
}

async function createBackgroundPdf(width: number, height: number): Promise<Buffer> {
	const document = await PDFDocument.create();
	document.addPage([width, height]);
	document.setTitle('SchoolOrbit certificate lifecycle background');
	return Buffer.from(await document.save());
}

async function waitForFileReady(
	api: SchoolApi,
	fileId: string,
	templateId: string
): Promise<FileMetadata> {
	const deadline = Date.now() + 120_000;
	while (Date.now() < deadline) {
		const metadata = await api.request<FileMetadata>(
			'GET',
			`/api/files/${encodeURIComponent(fileId)}?resource_id=${encodeURIComponent(templateId)}`
		);
		if (metadata.lifecycleStatus === 'ready') return metadata;
		if (metadata.lifecycleStatus === 'failed' || metadata.lifecycleStatus === 'quarantined') {
			throw new Error(`Template upload entered ${metadata.lifecycleStatus} state.`);
		}
		await new Promise((resolve) => setTimeout(resolve, 1_000));
	}
	throw new Error('Template upload did not become ready before the lifecycle timeout.');
}

async function uploadTemplateFile(
	api: SchoolApi,
	state: LifecycleState,
	templateId: string,
	purpose:
		| 'certificate_template_background'
		| 'certificate_template_image'
		| 'certificate_template_font',
	file: { name: string; mimeType: string; buffer: Buffer }
): Promise<FileMetadata> {
	const uploaded = await api.request<FileMetadata>('POST', '/api/files', {
		multipart: {
			purpose,
			resource_id: templateId,
			file
		}
	});
	state.uploadedFiles.push({ fileId: uploaded.id, templateId });
	return waitForFileReady(api, uploaded.id, templateId);
}

function buildLayout(
	template: CertificateTemplate,
	assets?: { image: CertificateTemplateAsset; font: CertificateTemplateAsset }
): CertificateLayout {
	if (!template.pageGeometry) throw new Error('Ready certificate template has no page geometry.');
	const width = template.pageGeometry.displayedWidthPoints;
	const height = template.pageGeometry.displayedHeightPoints;
	const textElement: CertificateLayout['elements'][number] = {
		type: 'text',
		id: randomUUID(),
		content: 'มอบให้ {ชื่อ} {นามสกุล}',
		frame: { x: width * 0.2, y: height * 0.42, width: width * 0.6, height: 64 },
		rotation: 0,
		fontSource: assets ? { type: 'asset', asset_id: assets.font.id } : { type: 'built_in' },
		fontFamily: assets?.font.fontFamily ?? 'Sarabun',
		fontWeight: assets?.font.fontWeight ?? 400,
		fontStyle: assets?.font.fontStyle ?? 'normal',
		fontSize: 30,
		minFontSize: 14,
		color: '#17324d',
		alignment: 'center',
		lineHeight: 1.2,
		autoShrink: true,
		shadow: null
	};
	const elements: CertificateLayout['elements'] = [
		textElement,
		{
			type: 'qr',
			id: randomUUID(),
			frame: { x: width - 88, y: height - 88, width: 68, height: 68 },
			rotation: 0
		}
	];
	if (assets) {
		elements.splice(1, 0, {
			type: 'image',
			id: randomUUID(),
			frame: { x: 24, y: 24, width: 52, height: 52 },
			rotation: 0,
			assetId: assets.image.id,
			lockAspectRatio: true,
			aspectRatio: 1
		});
	}
	return { schemaVersion: 1, elements };
}

async function attachInitialBackground(
	api: SchoolApi,
	template: CertificateTemplate,
	fileId: string
): Promise<CertificateTemplate> {
	return api.request<CertificateTemplate>(
		'PUT',
		`/api/certificates/templates/${encodeURIComponent(template.id)}/background`,
		{
			data: { fileId, geometryAction: 'preserve', previewConfirmed: false }
		}
	);
}

async function saveTemplateLayout(
	api: SchoolApi,
	template: CertificateTemplate,
	layout: CertificateLayout
): Promise<CertificateTemplate> {
	return api.request<CertificateTemplate>(
		'PUT',
		`/api/certificates/templates/${encodeURIComponent(template.id)}`,
		{
			data: {
				expectedUpdatedAt: template.updatedAt,
				layout,
				safeMarginPoints: 28.3464567,
				showSafeArea: true
			}
		}
	);
}

async function findAccount(
	api: SchoolApi,
	campaignId: string,
	recipientType: 'student' | 'staff',
	search: string,
	userId: string
): Promise<CertificateCandidateAccount> {
	const query = new URLSearchParams({ recipientType, search });
	const accounts = await api.request<CertificateCandidateAccount[]>(
		'GET',
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/candidates/account-search?${query}`
	);
	const account = accounts.find((candidate) => candidate.userId === userId);
	if (!account) throw new Error(`Dedicated ${recipientType} account is not searchable.`);
	return account;
}

async function withdrawCertificateIssueRequest(
	api: SchoolApi,
	requestId: string
): Promise<CertificateIssueRequest> {
	return api.request<CertificateIssueRequest>(
		'POST',
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}/withdraw`
	);
}

async function startCertificateIssueRequestReview(
	api: SchoolApi,
	requestId: string
): Promise<CertificateIssueRequest> {
	return api.request<CertificateIssueRequest>(
		'POST',
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}/review`
	);
}

async function returnCertificateIssueRequest(
	api: SchoolApi,
	requestId: string
): Promise<CertificateIssueRequest> {
	return api.request<CertificateIssueRequest>(
		'POST',
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}/return`,
		{
			data: {
				issueCodes: ['reviewer_requested_changes'],
				returnNote: 'ทดสอบส่งกลับก่อนออกเลขจริง'
			}
		}
	);
}

async function issueCertificates(
	api: SchoolApi,
	requestId: string,
	idempotencyKey: string
): Promise<IssueCertificateOutcome> {
	return api.request<IssueCertificateOutcome>(
		'POST',
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}/issue`,
		{ data: { idempotencyKey } }
	);
}

async function revokeIssuedCertificate(
	api: SchoolApi,
	certificateId: string
): Promise<RevokeCertificateResult> {
	return api.request<RevokeCertificateResult>(
		'POST',
		`/api/certificates/${encodeURIComponent(certificateId)}/revoke`,
		{
			data: {
				reason: 'ข้อมูลบนฉบับเดิมต้องออกใหม่ในการทดสอบ lifecycle',
				createReplacementCandidate: true
			}
		}
	);
}

async function assertPdfDownload(download: Download): Promise<void> {
	expect(download.suggestedFilename()).toMatch(/\.pdf$/iu);
	const downloadPath = await download.path();
	if (!downloadPath) throw new Error('Browser download did not expose a local file.');
	const bytes = await readFile(downloadPath);
	expect(bytes.subarray(0, 4).toString('ascii')).toBe('%PDF');
}

async function clickAndReadPdf(page: Page, button: Locator): Promise<void> {
	const downloadPromise = page.waitForEvent('download', { timeout: 120_000 });
	await button.click();
	await assertPdfDownload(await downloadPromise);
}

async function submitManualVerification(
	page: Page,
	certificate: IssuedCertificate,
	firstName = certificate.firstName,
	lastName = certificate.lastName
): Promise<void> {
	await page.getByLabel('เลขเกียรติบัตร', { exact: true }).fill(certificate.certificateNumber);
	await page.getByLabel('ชื่อ', { exact: true }).fill(firstName);
	await page.getByLabel('นามสกุล', { exact: true }).fill(lastName);
	await page.getByRole('button', { name: 'ตรวจสอบข้อมูล' }).click();
}

async function scrubQrFragment(page: Page): Promise<void> {
	try {
		await page.evaluate(() => {
			window.history.replaceState(
				window.history.state,
				'',
				`${window.location.pathname}${window.location.search}`
			);
		});
	} catch {
		// The page may already be closed; never surface the proof-bearing navigation error.
	}
}

async function openQrVerification(page: Page, targetUrl: string): Promise<BrowserResponse> {
	try {
		const [response] = await Promise.all([
			page.waitForResponse((candidate) => candidate.url().includes(publicQrVerificationPath), {
				timeout: 120_000
			}),
			page.goto(targetUrl)
		]);
		return response;
	} catch {
		await scrubQrFragment(page);
		throw new Error('Public QR verification navigation failed.');
	}
}

async function expectQrFragmentCleared(page: Page): Promise<void> {
	try {
		await expect.poll(() => page.evaluate(() => window.location.hash.length === 0)).toBe(true);
	} catch {
		await scrubQrFragment(page);
		throw new Error('Public QR verification did not clear its URL fragment.');
	}
}

async function purgeLifecycleCampaign(api: SchoolApi, state: LifecycleState): Promise<void> {
	const campaignId = state.campaignId;
	const confirmationName = state.campaignName;
	if (!campaignId) return;
	if (!confirmationName) throw new Error('Lifecycle campaign purge is missing safe fixture state.');

	const impactPath = `/api/certificates/campaigns/${encodeURIComponent(campaignId)}/purge-impact`;
	const statusPath = `/api/certificates/campaigns/${encodeURIComponent(campaignId)}/purge-status`;
	const impactOutcome = await api.requestOutcome<CertificateCampaignPurgeImpact>(
		'GET',
		impactPath,
		[404, 409]
	);
	if (impactOutcome.status === 404) return;

	let purgeStatus: CertificateCampaignPurgeStatus;
	if (impactOutcome.status === 409) {
		const existing = await api.requestOutcome<CertificateCampaignPurgeStatus>(
			'GET',
			statusPath,
			[404]
		);
		if (!existing.data) return;
		purgeStatus = existing.data;
	} else {
		const impact = impactOutcome.data;
		if (!impact || impact.campaignId !== campaignId || impact.campaignName !== confirmationName) {
			throw new Error('Lifecycle campaign purge impact did not match the isolated fixture.');
		}
		purgeStatus = await api.request<CertificateCampaignPurgeStatus>(
			'POST',
			`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/purge`,
			{
				data: {
					confirmationName,
					expectedUpdatedAt: impact.updatedAt,
					expectedImpact: impact.counts
				}
			}
		);
	}

	const deadline = Date.now() + 5 * 60_000;
	let retryCount = 0;
	while (Date.now() < deadline) {
		if (purgeStatus.phase === 'completed') return;
		if (purgeStatus.phase === 'failed') {
			if (retryCount >= 3) {
				throw new Error('Lifecycle campaign purge exhausted its guarded retries.');
			}
			retryCount += 1;
			const retried = await api.requestOutcome<CertificateCampaignPurgeStatus>(
				'POST',
				`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/purge/retry`,
				[404]
			);
			if (!retried.data) return;
			purgeStatus = retried.data;
			continue;
		}

		await new Promise((resolve) => setTimeout(resolve, 1_000));
		const nextStatus = await api.requestOutcome<CertificateCampaignPurgeStatus>(
			'GET',
			statusPath,
			[404]
		);
		if (!nextStatus.data) return;
		purgeStatus = nextStatus.data;
	}
	throw new Error('Lifecycle campaign purge did not complete before its safe timeout.');
}

async function cleanupLifecycleResources(
	api: SchoolApi | null,
	state: LifecycleState
): Promise<void> {
	if (!api) return;
	try {
		await purgeLifecycleCampaign(api, state);
		return;
	} catch {
		// Fall back only for unattached leftovers from a partially-created isolated fixture.
	}
	for (const upload of state.uploadedFiles.toReversed()) {
		await api.bestEffort(
			'DELETE',
			`/api/files/${encodeURIComponent(upload.fileId)}?resource_id=${encodeURIComponent(upload.templateId)}`
		);
	}
}

async function closeSession(session: ActorSession | null): Promise<void> {
	if (!session) return;
	await session.api.bestEffort('POST', '/api/auth/logout');
	try {
		await session.context.close();
	} catch {
		// Continue closing the remaining isolated lifecycle contexts.
	}
}

async function closePublicContext(context: BrowserContext | null): Promise<void> {
	if (!context) return;
	try {
		await context.close();
	} catch {
		// The browser may already have closed the anonymous context.
	}
}

test.use({ screenshot: 'off', trace: 'off', video: 'off' });

test.describe.serial('complete certificate issuance lifecycle', () => {
	test('prepares, issues, downloads, verifies, revokes, and replaces certificates', async ({
		browser
	}) => {
		test.skip(!hasLifecycleCredentials, 'Set all E2E_CERT_* credentials for the live lifecycle.');
		test.setTimeout(15 * 60_000);
		if (!hasLifecycleCredentials) return;

		const preparerCredentials: Credentials = {
			username: preparerUsername!,
			password: preparerPassword!
		};
		const issuerCredentials: Credentials = {
			username: issuerUsername!,
			password: issuerPassword!
		};
		const studentCredentials: Credentials = {
			username: studentUsername!,
			password: studentPassword!
		};
		const state: LifecycleState = {
			campaignId: null,
			campaignName: null,
			uploadedFiles: []
		};
		let preparer: ActorSession | null = null;
		let issuer: ActorSession | null = null;
		let student: ActorSession | null = null;
		let publicContext: BrowserContext | null = null;
		let lifecyclePhase = 'actor login';

		try {
			preparer = await login(browser, preparerCredentials, /\/staff\/?(?:[?#].*)?$/);
			issuer = await login(browser, issuerCredentials, /\/staff\/?(?:[?#].*)?$/);
			student = await login(browser, studentCredentials, /\/student\/?(?:[?#].*)?$/);

			expect(preparer.user.userType).toBe('staff');
			expect(issuer.user.userType).toBe('staff');
			expect(student.user.userType).toBe('student');
			expect(new Set([preparer.user.id, issuer.user.id, student.user.id]).size).toBe(3);

			lifecyclePhase = 'campaign authorization';
			const academicYears = await preparer.api.request<AcademicYear[]>(
				'GET',
				'/api/lookup/academic-years?active_only=false'
			);
			const academicYear = academicYears.find((year) => year.is_current) ?? academicYears[0];
			if (!academicYear) throw new Error('Lifecycle tenant has no academic year.');
			const ownerOptions = await preparer.api.request<OrganizationUnit[]>(
				'GET',
				'/api/certificates/owner-options'
			);
			const owner = ownerOptions[0];
			if (!owner) throw new Error('Lifecycle preparer has no exact owner organization unit.');
			const organizationUnits = await preparer.api.request<OrganizationUnit[]>(
				'GET',
				'/api/lookup/organization-units?active_only=false'
			);
			const allowedOwnerIds = new Set(ownerOptions.map((option) => option.id));
			const unauthorizedOwner = organizationUnits.find(
				(unit) => unit.is_active && !allowedOwnerIds.has(unit.id)
			);
			if (!unauthorizedOwner) {
				throw new Error(
					'Lifecycle preparer fixture requires a second active organization unit outside its exact scope.'
				);
			}

			const suffix = randomUUID().slice(0, 8);
			const eventDate = new Date().toISOString().slice(0, 10);
			await preparer.api.expectFailure('POST', '/api/certificates/campaigns', [403], {
				data: {
					academicYearId: academicYear.id,
					ownerOrganizationUnitId: unauthorizedOwner.id,
					name: `ข้ามขอบเขต ${suffix}`,
					eventDate
				}
			});

			const campaign = await preparer.api.request<CertificateCampaign>(
				'POST',
				'/api/certificates/campaigns',
				{
					data: {
						academicYearId: academicYear.id,
						ownerOrganizationUnitId: owner.id,
						name: `วงจรเกียรติบัตร E2E ${suffix}`,
						eventDate
					}
				}
			);
			state.campaignId = campaign.id;
			state.campaignName = campaign.name;
			expect(campaign.ownerOrganizationUnitId).toBe(owner.id);
			expect(campaign.capabilities.canPrepareCandidates).toBe(true);

			lifecyclePhase = 'template asset and layout preparation';
			let studentTemplate = await preparer.api.request<CertificateTemplate>(
				'POST',
				`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/templates`,
				{
					data: {
						name: `แบบนักเรียนและภายนอก ${suffix}`,
						allowedRecipientTypes: ['student', 'external']
					}
				}
			);
			let staffTemplate = await preparer.api.request<CertificateTemplate>(
				'POST',
				`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/templates`,
				{
					data: {
						name: `แบบบุคลากร ${suffix}`,
						allowedRecipientTypes: ['staff']
					}
				}
			);

			const studentBackground = await uploadTemplateFile(
				preparer.api,
				state,
				studentTemplate.id,
				'certificate_template_background',
				{
					name: `student-${suffix}.pdf`,
					mimeType: 'application/pdf',
					buffer: await createBackgroundPdf(841.89, 595.28)
				}
			);
			studentTemplate = await attachInitialBackground(
				preparer.api,
				studentTemplate,
				studentBackground.id
			);
			const staffBackground = await uploadTemplateFile(
				preparer.api,
				state,
				staffTemplate.id,
				'certificate_template_background',
				{
					name: `staff-${suffix}.pdf`,
					mimeType: 'application/pdf',
					buffer: await createBackgroundPdf(595.28, 419.53)
				}
			);
			staffTemplate = await attachInitialBackground(
				preparer.api,
				staffTemplate,
				staffBackground.id
			);

			const imageFile = await uploadTemplateFile(
				preparer.api,
				state,
				studentTemplate.id,
				'certificate_template_image',
				{ name: `seal-${suffix}.png`, mimeType: 'image/png', buffer: onePixelPng }
			);
			studentTemplate = await preparer.api.request<CertificateTemplate>(
				'POST',
				`/api/certificates/templates/${encodeURIComponent(studentTemplate.id)}/assets`,
				{
					data: {
						fileId: imageFile.id,
						kind: 'image',
						displayName: 'ตราสัญลักษณ์ทดสอบ',
						rightsConfirmed: false
					}
				}
			);
			const fontFile = await uploadTemplateFile(
				preparer.api,
				state,
				studentTemplate.id,
				'certificate_template_font',
				{
					name: `sarabun-${suffix}.ttf`,
					mimeType: 'font/ttf',
					buffer: await readFile(fontPath)
				}
			);
			const fontInspection = await preparer.api.request<CertificateFontUploadInspection>(
				'POST',
				`/api/certificates/templates/${encodeURIComponent(studentTemplate.id)}/assets/fonts/inspect`,
				{
					data: {
						fileIds: [fontFile.id]
					}
				}
			);
			expect(fontInspection.files).toHaveLength(1);
			expect(fontInspection.files[0].status).toBe('ready');
			studentTemplate = await preparer.api.request<CertificateTemplate>(
				'POST',
				`/api/certificates/templates/${encodeURIComponent(studentTemplate.id)}/assets/fonts/batch`,
				{
					data: {
						fileIds: [fontFile.id],
						rightsConfirmed: true
					}
				}
			);
			const imageAsset = studentTemplate.assets.find((asset) => asset.fileId === imageFile.id);
			const fontAsset = studentTemplate.assets.find((asset) => asset.fileId === fontFile.id);
			if (!imageAsset || !fontAsset || !fontAsset.fontFamily || !fontAsset.fontStyle) {
				throw new Error('Template image/font assets were not inspected and attached.');
			}
			studentTemplate = await saveTemplateLayout(
				preparer.api,
				studentTemplate,
				buildLayout(studentTemplate, { image: imageAsset, font: fontAsset })
			);
			staffTemplate = await saveTemplateLayout(
				preparer.api,
				staffTemplate,
				buildLayout(staffTemplate)
			);
			expect(studentTemplate.isReady).toBe(true);
			expect(staffTemplate.isReady).toBe(true);
			expect(studentTemplate.pageGeometry?.displayedWidthPoints).not.toBe(
				staffTemplate.pageGeometry?.displayedWidthPoints
			);

			lifecyclePhase = 'recipient preparation';
			const studentAccount = await findAccount(
				preparer.api,
				campaign.id,
				'student',
				student.user.username,
				student.user.id
			);
			if (!studentAccount.studentId) {
				throw new Error('Dedicated lifecycle student has no student ID.');
			}
			const staffAccount = await findAccount(
				preparer.api,
				campaign.id,
				'staff',
				preparer.user.username,
				preparer.user.id
			);

			const staffCandidateResult = await preparer.api.request<CertificateCandidateImportResult>(
				'POST',
				`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/candidates/account-search`,
				{
					data: {
						userId: staffAccount.userId,
						templateId: staffTemplate.id,
						activityItem: 'วิทยากรอบรม',
						awardOrRole: 'วิทยากร'
					}
				}
			);
			const staffCandidate = staffCandidateResult.candidates[0];
			if (!staffCandidate) throw new Error('Staff candidate was not created.');

			const manualFirstName = `บุคคลภายนอก${suffix.slice(0, 4)}`;
			const manualLastName = 'ทดสอบวงจร';
			const manualCandidateResult = await preparer.api.request<CertificateCandidateImportResult>(
				'POST',
				`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/candidates/manual`,
				{
					data: {
						title: 'นาย',
						firstName: manualFirstName,
						lastName: manualLastName,
						templateId: studentTemplate.id,
						activityItem: 'การแข่งขันคำคม',
						awardOrRole: 'ผู้เข้าร่วม'
					}
				}
			);
			const manualCandidate = manualCandidateResult.candidates[0];
			if (!manualCandidate) throw new Error('Manual external candidate was not created.');

			const importedExternalStudentId = `E2E-${suffix}`;
			const importResult = await preparer.api.request<CertificateCandidateImportResult>(
				'POST',
				`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/candidates/import`,
				{
					data: {
						source: 'csv',
						headers: [
							'ประเภทผู้รับ',
							'รหัสนักเรียน',
							'ชื่อผู้ใช้บุคลากร',
							'คำนำหน้า',
							'ชื่อ',
							'นามสกุล',
							'รายการกิจกรรม',
							'รางวัลหรือบทบาท',
							'แบบเกียรติบัตร'
						],
						rows: [
							{
								recipientType: 'student',
								studentId: studentAccount.studentId,
								staffUsername: null,
								title: studentAccount.title,
								firstName: `${studentAccount.firstName}ทดสอบ`,
								lastName: studentAccount.lastName,
								activityItem: 'การแข่งขันคำคม',
								awardOrRole: 'รองชนะเลิศอันดับที่ 1',
								templateName: studentTemplate.name,
								customValues: {}
							},
							{
								recipientType: 'student',
								studentId: importedExternalStudentId,
								staffUsername: null,
								title: 'เด็กหญิง',
								firstName: 'กมลชนก',
								lastName: `ภายนอก${suffix.slice(0, 4)}`,
								activityItem: 'การแข่งขันคำคม',
								awardOrRole: 'ชนะเลิศ',
								templateName: studentTemplate.name,
								customValues: {}
							}
						]
					}
				}
			);
			const studentCandidate = importResult.candidates.find(
				(candidate) => candidate.studentId === studentAccount.studentId
			);
			const importedExternalCandidate = importResult.candidates.find(
				(candidate) => candidate.studentId === importedExternalStudentId
			);
			if (!studentCandidate || !importedExternalCandidate) {
				throw new Error('Imported lifecycle candidates were not returned.');
			}
			expect(studentCandidate.matchStatus).toBe('name_mismatch');
			expect(studentCandidate.validationCodes).toContain('name_source_required');
			expect(importedExternalCandidate.matchStatus).toBe('not_found');
			expect(importedExternalCandidate.validationCodes).toContain('account_not_found');

			const chosenName = await preparer.api.request<CertificateCandidateBulkResult>(
				'POST',
				`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/candidates/bulk`,
				{
					data: {
						operation: 'choose_name',
						candidateIds: [studentCandidate.id],
						nameSource: 'account'
					}
				}
			);
			const confirmedExternal = await preparer.api.request<CertificateCandidateBulkResult>(
				'POST',
				`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/candidates/bulk`,
				{
					data: {
						operation: 'confirm_external',
						candidateIds: [importedExternalCandidate.id]
					}
				}
			);
			const readyStudent = chosenName.candidates[0];
			const readyImportedExternal = confirmedExternal.candidates[0];
			if (!readyStudent || !readyImportedExternal) {
				throw new Error('Candidate review actions returned no candidate.');
			}
			for (const candidate of [
				readyStudent,
				staffCandidate,
				manualCandidate,
				readyImportedExternal
			]) {
				expect(candidate.validationStatus).toBe('ready');
			}
			expect(readyImportedExternal.recipientType).toBe('external');

			const allCandidateIds = [
				readyStudent.id,
				staffCandidate.id,
				manualCandidate.id,
				readyImportedExternal.id
			];
			const submitRequest = async (candidateIds: string[]): Promise<CertificateIssueRequest> => {
				const request = await preparer!.api.request<CertificateIssueRequest>(
					'POST',
					`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/issue-requests`,
					{ data: { candidateIds } }
				);
				return request;
			};

			lifecyclePhase = 'request transitions and issuance';
			const withdrawnRequest = await submitRequest(allCandidateIds);
			expect(
				(await withdrawCertificateIssueRequest(preparer.api, withdrawnRequest.id)).status
			).toBe('withdrawn');

			const returnedRequest = await submitRequest(allCandidateIds);
			expect(
				(await startCertificateIssueRequestReview(issuer.api, returnedRequest.id)).status
			).toBe('reviewing');
			expect((await returnCertificateIssueRequest(issuer.api, returnedRequest.id)).status).toBe(
				'returned'
			);

			const firstBatchCandidateIds = [readyStudent.id, staffCandidate.id, manualCandidate.id];
			const firstRequest = await submitRequest(firstBatchCandidateIds);
			await startCertificateIssueRequestReview(issuer.api, firstRequest.id);
			await preparer.api.expectFailure(
				'POST',
				`/api/certificates/issue-requests/${encodeURIComponent(firstRequest.id)}/issue`,
				[403],
				{ data: { idempotencyKey: randomUUID() } }
			);
			const firstIdempotencyKey = randomUUID();
			const firstIssue = await issueCertificates(issuer.api, firstRequest.id, firstIdempotencyKey);
			if (firstIssue.outcome !== 'issued') throw new Error('First lifecycle request was returned.');
			expect(firstIssue.certificates).toHaveLength(3);
			const retriedFirstIssue = await issueCertificates(
				issuer.api,
				firstRequest.id,
				firstIdempotencyKey
			);
			expect(retriedFirstIssue).toEqual(firstIssue);

			const secondRequest = await submitRequest([readyImportedExternal.id]);
			await startCertificateIssueRequestReview(issuer.api, secondRequest.id);
			const secondIssue = await issueCertificates(issuer.api, secondRequest.id, randomUUID());
			if (secondIssue.outcome !== 'issued')
				throw new Error('Second lifecycle request was returned.');
			expect(secondIssue.firstCertificateSequence).toBe(firstIssue.lastCertificateSequence + 1);

			const issuedCertificates = [...firstIssue.certificates, ...secondIssue.certificates];
			const studentCertificate = issuedCertificates.find(
				(certificate) => certificate.recipientType === 'student'
			);
			const staffCertificate = issuedCertificates.find(
				(certificate) => certificate.recipientType === 'staff'
			);
			const externalCertificates = issuedCertificates.filter(
				(certificate) => certificate.recipientType === 'external'
			);
			if (!studentCertificate || !staffCertificate || externalCertificates.length !== 2) {
				throw new Error('Issued lifecycle recipient mix is incomplete.');
			}

			lifecyclePhase = 'administrative and own certificate downloads';
			await issuer.page.goto(
				`${primaryOrigin}/staff/certificates/${encodeURIComponent(campaign.id)}/issued`
			);
			await expect(issuer.page.getByRole('heading', { name: 'ใบที่ออกแล้ว' })).toBeVisible({
				timeout: 30_000
			});
			await expect(
				issuer.page.getByText(studentCertificate.certificateNumber, { exact: true })
			).toBeVisible();
			await clickAndReadPdf(
				issuer.page,
				issuer.page.getByRole('button', {
					name: `ดาวน์โหลด ${studentCertificate.certificateNumber}`
				})
			);
			for (const certificate of firstIssue.certificates) {
				await issuer.page
					.getByRole('checkbox', { name: `เลือก ${certificate.certificateNumber}` })
					.check();
			}
			await issuer.page.getByRole('button', { name: /ดาวน์โหลดที่เลือก/ }).click();
			await expect(issuer.page.getByRole('dialog')).toContainText('หนึ่งไฟล์ หลายขนาดกระดาษ');
			await clickAndReadPdf(
				issuer.page,
				issuer.page.getByRole('dialog').getByRole('button', { name: 'สร้าง PDF รวม' })
			);

			const preparerOwn = await preparer.api.request<IssuedCertificate[]>(
				'GET',
				'/api/me/certificates'
			);
			const studentOwn = await student.api.request<IssuedCertificate[]>(
				'GET',
				'/api/me/certificates'
			);
			expect(preparerOwn.some((certificate) => certificate.id === staffCertificate.id)).toBe(true);
			expect(studentOwn.some((certificate) => certificate.id === studentCertificate.id)).toBe(true);
			for (const external of externalCertificates) {
				expect(preparerOwn.some((certificate) => certificate.id === external.id)).toBe(false);
				expect(studentOwn.some((certificate) => certificate.id === external.id)).toBe(false);
			}

			await preparer.page.goto(`${primaryOrigin}/staff/achievements`);
			await expect(preparer.page).toHaveURL(/\/staff\/achievements\/issued\/?$/);
			await expect(
				preparer.page.getByText(staffCertificate.certificateNumber, { exact: true })
			).toBeVisible();
			await student.page.goto(`${primaryOrigin}/student/certificates`);
			await expect(
				student.page.getByText(studentCertificate.certificateNumber, { exact: true })
			).toBeVisible();
			await clickAndReadPdf(
				student.page,
				student.page
					.getByTestId('my-certificate-card')
					.filter({ hasText: studentCertificate.certificateNumber })
					.getByRole('button', { name: 'ดาวน์โหลด' })
			);

			lifecyclePhase = 'public verification';
			const issuedManifest = await issuer.api.request<CertificateRenderManifest>(
				'POST',
				`/api/certificates/${encodeURIComponent(studentCertificate.id)}/render-manifest`
			);
			let canonicalQrUrl: URL;
			try {
				canonicalQrUrl = new URL(issuedManifest.qrPayload);
			} catch {
				throw new Error('Issued manifest returned an invalid QR verification URL.');
			}
			expect(canonicalQrUrl.searchParams.has('proof')).toBe(false);
			const proofValues = new URLSearchParams(canonicalQrUrl.hash.slice(1)).getAll('proof');
			expect(proofValues.length).toBe(1);
			const proof = proofValues[0];
			if (!proof) throw new Error('Issued manifest has no QR proof fragment.');
			const localQrUrl = `${primaryOrigin}${canonicalQrUrl.pathname}${canonicalQrUrl.search}#proof=${encodeURIComponent(proof)}`;

			publicContext = await browser.newContext({ baseURL: primaryOrigin, acceptDownloads: true });
			const publicPage = await publicContext.newPage();
			await publicPage.goto(`${primaryOrigin}/verify/certificate`);
			await submitManualVerification(publicPage, studentCertificate);
			await expect(publicPage.getByTestId('verification-result')).toContainText(
				studentCertificate.certificateNumber
			);
			await clickAndReadPdf(publicPage, publicPage.getByTestId('public-certificate-download'));

			const manualFailureResponse = publicPage.waitForResponse(
				(response) =>
					response.url().includes(publicManualVerificationPath) && response.status() === 404
			);
			await submitManualVerification(publicPage, studentCertificate, 'ชื่อไม่ตรง', 'นามสกุลไม่ตรง');
			await manualFailureResponse;
			await expect(publicPage.getByTestId('verification-error')).toContainText(
				'ไม่พบข้อมูลที่ตรงกัน'
			);

			const qrPage = await publicContext.newPage();
			const qrResponse = await openQrVerification(qrPage, localQrUrl);
			expect(qrResponse.status()).toBe(200);
			await expectQrFragmentCleared(qrPage);
			await expect(qrPage.getByTestId('verification-result')).toContainText(
				studentCertificate.certificateNumber
			);
			await clickAndReadPdf(qrPage, qrPage.getByTestId('public-certificate-download'));

			lifecyclePhase = 'revocation and replacement issuance';
			const revoked = await revokeIssuedCertificate(issuer.api, studentCertificate.id);
			expect(revoked.certificate.status).toBe('revoked');
			if (!revoked.replacementCandidate) {
				throw new Error('Revocation did not create a replacement candidate.');
			}
			expect(revoked.replacementCandidate.validationStatus).toBe('needs_review');
			await issuer.api.expectFailure(
				'POST',
				`/api/certificates/${encodeURIComponent(studentCertificate.id)}/render-manifest`,
				[409]
			);

			const replacementDraft = await preparer.api.request<CertificateCandidate>(
				'GET',
				`/api/certificates/candidates/${encodeURIComponent(revoked.replacementCandidate.id)}`
			);
			let readyReplacement = await preparer.api.request<CertificateCandidate>(
				'PUT',
				`/api/certificates/candidates/${encodeURIComponent(replacementDraft.id)}`,
				{
					data: {
						expectedUpdatedAt: replacementDraft.updatedAt,
						templateId: replacementDraft.templateId,
						recipientType: replacementDraft.recipientType,
						studentId: replacementDraft.studentId,
						staffUsername: replacementDraft.staffUsername,
						importedTitle: replacementDraft.importedTitle,
						importedFirstName: replacementDraft.importedFirstName,
						importedLastName: replacementDraft.importedLastName,
						selectedNameSource: 'account',
						activityItem: replacementDraft.activityItem,
						awardOrRole: replacementDraft.awardOrRole,
						customValues: replacementDraft.customValues
					}
				}
			);
			if (readyReplacement.validationCodes.includes('duplicate_candidate')) {
				const confirmedReplacement = await preparer.api.request<CertificateCandidateBulkResult>(
					'POST',
					`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}/candidates/bulk`,
					{
						data: {
							operation: 'confirm_duplicate',
							candidateIds: [readyReplacement.id]
						}
					}
				);
				const confirmedCandidate = confirmedReplacement.candidates[0];
				if (!confirmedCandidate) {
					throw new Error('Replacement duplicate review returned no candidate.');
				}
				readyReplacement = confirmedCandidate;
			}
			expect(readyReplacement.validationStatus).toBe('ready');
			const replacementRequest = await submitRequest([readyReplacement.id]);
			await startCertificateIssueRequestReview(issuer.api, replacementRequest.id);
			const replacementIssue = await issueCertificates(
				issuer.api,
				replacementRequest.id,
				randomUUID()
			);
			if (replacementIssue.outcome !== 'issued') {
				throw new Error('Replacement lifecycle request was returned.');
			}
			const replacementCertificate = replacementIssue.certificates[0];
			if (!replacementCertificate) throw new Error('Replacement certificate was not issued.');
			expect(replacementCertificate.replacementForCertificateId).toBe(studentCertificate.id);
			expect(replacementCertificate.certificateSequence).toBe(
				secondIssue.lastCertificateSequence + 1
			);

			const studentOwnAfterReplacement = await student.api.request<IssuedCertificate[]>(
				'GET',
				'/api/me/certificates'
			);
			const oldOwn = studentOwnAfterReplacement.find(
				(certificate) => certificate.id === studentCertificate.id
			);
			const newOwn = studentOwnAfterReplacement.find(
				(certificate) => certificate.id === replacementCertificate.id
			);
			expect(oldOwn?.status).toBe('revoked');
			expect(oldOwn?.capabilities.canDownload).toBe(false);
			expect(newOwn?.status).toBe('issued');

			lifecyclePhase = 'revoked and replacement visibility';
			await student.page.goto(`${primaryOrigin}/student/certificates`);
			const oldCard = student.page
				.getByTestId('my-certificate-card')
				.filter({ hasText: studentCertificate.certificateNumber });
			await expect(oldCard.getByText('เพิกถอนแล้ว')).toBeVisible();
			await expect(oldCard.getByRole('button', { name: 'ดาวน์โหลด' })).toHaveCount(0);
			const replacementCard = student.page
				.getByTestId('my-certificate-card')
				.filter({ hasText: replacementCertificate.certificateNumber });
			await expect(replacementCard.getByRole('button', { name: 'ดาวน์โหลด' })).toBeVisible();

			await publicPage.goto(`${primaryOrigin}/verify/certificate`);
			await submitManualVerification(publicPage, studentCertificate);
			await expect(publicPage.getByTestId('verification-result')).toContainText('เพิกถอนแล้ว');
			await expect(publicPage.getByTestId('public-certificate-download')).toHaveCount(0);
			await expect(publicPage.getByTestId('verification-result')).toContainText(
				replacementCertificate.certificateNumber
			);

			const revokedQrPage = await publicContext.newPage();
			expect((await openQrVerification(revokedQrPage, localQrUrl)).status()).toBe(200);
			await expectQrFragmentCleared(revokedQrPage);
			await expect(revokedQrPage.getByTestId('verification-result')).toContainText('เพิกถอนแล้ว');
			await expect(revokedQrPage.getByTestId('public-certificate-download')).toHaveCount(0);

			lifecyclePhase = 'permanent campaign purge';
			await purgeLifecycleCampaign(preparer.api, state);
			await preparer.api.expectFailure(
				'GET',
				`/api/certificates/campaigns/${encodeURIComponent(campaign.id)}`,
				[404]
			);
			for (const certificate of [studentCertificate, replacementCertificate]) {
				await publicPage.goto(`${primaryOrigin}/verify/certificate`);
				const missingVerification = publicPage.waitForResponse(
					(response) =>
						response.url().includes(publicManualVerificationPath) && response.status() === 404
				);
				await submitManualVerification(publicPage, certificate);
				await missingVerification;
				await expect(publicPage.getByTestId('verification-error')).toContainText(
					'ไม่พบข้อมูลที่ตรงกัน'
				);
			}

			const studentOwnAfterPurge = await student.api.request<IssuedCertificate[]>(
				'GET',
				'/api/me/certificates'
			);
			expect(
				studentOwnAfterPurge.some(
					(certificate) =>
						certificate.id === studentCertificate.id || certificate.id === replacementCertificate.id
				)
			).toBe(false);
			for (const upload of state.uploadedFiles) {
				await preparer.api.expectFailure(
					'GET',
					`/api/files/${encodeURIComponent(upload.fileId)}?resource_id=${encodeURIComponent(upload.templateId)}`,
					[404]
				);
			}
			state.campaignId = null;
			state.campaignName = null;
		} catch {
			throw new Error(
				`Certificate lifecycle failed during ${lifecyclePhase}; sensitive details were suppressed.`
			);
		} finally {
			await cleanupLifecycleResources(preparer?.api ?? null, state);
			await closePublicContext(publicContext);
			await closeSession(student);
			await closeSession(issuer);
			await closeSession(preparer);
		}
	});
});
