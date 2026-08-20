import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const reviewComponentPath = path.resolve(
	frontendRoot,
	'src/lib/components/certificates/CertificateIssueRequestReview.svelte'
);
const harnessPath = '/__certificate-request-test';
const virtualModuleId = 'virtual:certificate-request-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const navigationStubModuleId = 'virtual:certificate-request-navigation-stub';
const resolvedNavigationStubModuleId = `\0${navigationStubModuleId}`;
const stubPrefix = '\0certificate-request-stub:';

const navigationStub = `
	const callbacks = new Set();
	export const afterNavigate = (callback) => {
		callbacks.add(callback);
		queueMicrotask(callback);
	};
	export const triggerAfterNavigate = () => {
		for (const callback of callbacks) callback();
	};
	window.__triggerCertificateAfterNavigate = triggerAfterNavigate;
`;

const pathsStub = `
	export const resolve = (path) => path;
`;

const certificateApiStub = `
	export async function listCertificateCampaignIssueRequests(campaignId) {
		return window.__certificateRequestApi.listCampaignRequests(campaignId);
	}
	export async function withdrawCertificateIssueRequest(requestId) {
		return window.__certificateRequestApi.withdraw(requestId);
	}
	export async function listCertificateIssueRequests(query) {
		return window.__certificateRequestApi.listQueue(query);
	}
	export async function getCertificateIssueRequest(requestId) {
		return window.__certificateRequestApi.getRequest(requestId);
	}
	export async function startCertificateIssueRequestReview(requestId) {
		return window.__certificateRequestApi.startReview(requestId);
	}
	export async function returnCertificateIssueRequest(requestId, payload) {
		return window.__certificateRequestApi.returnRequest(requestId, payload);
	}
	export async function issueCertificates(requestId, payload) {
		return window.__certificateRequestApi.issue(requestId, payload);
	}
	export async function createCertificateTemplatePreviewManifest(templateId, payload) {
		return window.__certificateRequestApi.preview(templateId, payload);
	}
`;

const rendererStub = `
	export async function loadCertificateRenderer() {
		return { renderPreview: async () => undefined };
	}
`;

const stubModules = new Map([
	['$app/navigation', navigationStub],
	['$app/paths', pathsStub],
	['$lib/api/certificates', certificateApiStub],
	['$lib/certificates/renderer', rendererStub]
]);

function findStubModule(id: string): string | undefined {
	if (stubModules.has(id)) return id;
	if (id.includes('/@sveltejs/kit/src/runtime/app/navigation.js')) return '$app/navigation';
	if (id.includes('/@sveltejs/kit/src/runtime/app/paths.js')) return '$app/paths';
	for (const stubId of stubModules.keys()) {
		if (!stubId.startsWith('$lib/')) continue;
		const resolvedPath = path.resolve(frontendRoot, 'src/lib', stubId.slice('$lib/'.length));
		if (
			id === resolvedPath ||
			id === `${resolvedPath}.ts` ||
			id === `${resolvedPath}.js` ||
			id === `${resolvedPath}.svelte`
		) {
			return stubId;
		}
	}
}

function harnessPlugin(): Plugin {
	return {
		name: 'certificate-request-test-harness',
		enforce: 'pre',
		resolveId(id) {
			if (id === virtualModuleId) return resolvedVirtualModuleId;
			if (id === navigationStubModuleId) return resolvedNavigationStubModuleId;
			const stubId = findStubModule(id);
			if (stubId) return `${stubPrefix}${stubId}`;
		},
		load(id) {
			if (id === resolvedNavigationStubModuleId) return navigationStub;
			if (id.startsWith(stubPrefix)) return stubModules.get(id.slice(stubPrefix.length));
			if (id !== resolvedVirtualModuleId) return;
			return `
				import { flushSync, mount } from 'svelte';
				import { createClassComponent } from 'svelte/legacy';
				import '/src/routes/layout.css';
				import CertificateSubmitRequestDialog from '/src/lib/components/certificates/CertificateSubmitRequestDialog.svelte';
				import CertificateCampaignRequests from '/src/lib/components/certificates/CertificateCampaignRequests.svelte';
				import CertificateIssueRequestReview from '/src/lib/components/certificates/CertificateIssueRequestReview.svelte';

				const campaignId = '10000000-0000-4000-8000-000000000001';
				const requestId = '20000000-0000-4000-8000-000000000001';
				const secondRequestId = '20000000-0000-4000-8000-000000000002';
				const timestamp = '2026-08-14T02:00:00Z';
				const templates = {
					reward: { id: '30000000-0000-4000-8000-000000000001', name: 'แบบรางวัลการแข่งขัน' },
					speaker: { id: '30000000-0000-4000-8000-000000000002', name: 'แบบวิทยากร' }
				};
				const candidates = [
					{
						id: '40000000-0000-4000-8000-000000000001', recipientType: 'student',
						importedTitle: 'เด็กหญิง', importedFirstName: 'กมลชนก', importedLastName: 'ใจดี',
						accountTitle: 'เด็กหญิง', accountFirstName: 'กมลชนก', accountLastName: 'ใจดี',
						selectedNameSource: 'account', templateId: templates.reward.id,
						templateName: templates.reward.name, validationStatus: 'ready', validationCodes: []
					},
					{
						id: '40000000-0000-4000-8000-000000000002', recipientType: 'external',
						importedTitle: 'คุณ', importedFirstName: 'สายชล', importedLastName: 'คงดี',
						accountTitle: null, accountFirstName: null, accountLastName: null,
						selectedNameSource: 'file', templateId: templates.speaker.id,
						templateName: templates.speaker.name, validationStatus: 'ready', validationCodes: []
					}
				];
				let request = {
					id: requestId, campaignId, campaignName: 'กิจกรรมวันภาษาไทย',
					ownerOrganizationUnitId: '50000000-0000-4000-8000-000000000001',
					ownerOrganizationUnitName: 'กลุ่มสาระภาษาไทย', status: 'pending',
					submittedBy: '60000000-0000-4000-8000-000000000001', submittedByName: 'นายสมชาย ใจดี',
					reviewedBy: null, reviewedByName: null, submittedAt: timestamp, reviewedAt: null,
					returnedAt: null, withdrawnAt: null, issuedAt: null, returnNote: null, issueCodes: [],
					itemCount: 2, templateCount: 2, readyCount: 2, reviewCount: 0, invalidCount: 0,
					createdAt: timestamp, updatedAt: timestamp,
					capabilities: { canWithdraw: true, canStartReview: true, canReturn: false, canIssue: false },
					items: candidates.map((candidate) => ({
						candidateId: candidate.id, templateId: candidate.templateId,
						templateName: candidate.templateName, recipientType: candidate.recipientType,
						title: candidate.importedTitle, firstName: candidate.importedFirstName,
						lastName: candidate.importedLastName, activityItem: 'การแข่งขันคำคม',
						awardOrRole: 'ผู้เข้าร่วม', validationStatus: 'ready', validationCodes: []
					}))
				};
				let secondRequest = structuredClone(request);
				secondRequest.id = secondRequestId;
				secondRequest.campaignName = 'กิจกรรมสัปดาห์วิทยาศาสตร์';
				const submittedIds = [];
				const returnPayloads = [];
				const issuePayloads = [];
				let issueAttempt = 0;
				const pendingReviewResolvers = new Map();
				const reviewCalls = [];
				const view = new URLSearchParams(window.location.search).get('view');
				const reviewingState = (current) => ({
					...current,
					status: 'reviewing',
					reviewedAt: timestamp,
					capabilities: { canWithdraw: false, canStartReview: false, canReturn: true, canIssue: true }
				});
				window.__certificateRequestApi = {
					async listCampaignRequests() { return [structuredClone(request)]; },
					async withdraw() {
						request = { ...request, status: 'withdrawn', withdrawnAt: timestamp,
							capabilities: { ...request.capabilities, canWithdraw: false } };
						return structuredClone(request);
					},
					async listQueue() { return [structuredClone(request)]; },
					async getRequest(id) {
						return structuredClone(id === secondRequestId ? secondRequest : request);
					},
					async startReview(id) {
						if (view === 'race') {
							reviewCalls.push(id);
							return new Promise((resolve) => {
								pendingReviewResolvers.set(id, () => {
									if (id === secondRequestId) {
										secondRequest = reviewingState(secondRequest);
										resolve(structuredClone(secondRequest));
									} else {
										request = reviewingState(request);
										resolve(structuredClone(request));
									}
								});
							});
						}
						request = reviewingState(request);
						return structuredClone(request);
					},
					async returnRequest(id, payload) {
						returnPayloads.push(structuredClone(payload));
						request = { ...request, status: 'returned', returnedAt: timestamp,
							returnNote: payload.returnNote, issueCodes: payload.issueCodes,
							capabilities: { canWithdraw: false, canStartReview: false, canReturn: false, canIssue: false } };
						return structuredClone(request);
					},
					async issue(id, payload) {
						issuePayloads.push(structuredClone(payload));
						issueAttempt += 1;
						if (issueAttempt === 1) throw new Error('เครือข่ายขัดข้อง กรุณาลองอีกครั้ง');
						request = { ...request, status: 'issued', issuedAt: timestamp,
							capabilities: { canWithdraw: false, canStartReview: false, canReturn: false, canIssue: false } };
						return {
							outcome: 'issued', issueRunId: '70000000-0000-4000-8000-000000000001',
							requestId: id, campaignId, activitySequence: 42,
							firstCertificateSequence: 101, lastCertificateSequence: 102,
							certificates: request.items.map((item, index) => ({
								id: '80000000-0000-4000-8000-00000000000' + (index + 1), campaignId,
								campaignName: request.campaignName, ownerOrganizationUnitId: request.ownerOrganizationUnitId,
								ownerOrganizationUnitName: request.ownerOrganizationUnitName, templateId: item.templateId,
								templateName: item.templateName, academicYearId: '90000000-0000-4000-8000-000000000001',
								academicYearValue: 2569, activitySequence: 42, certificateSequence: 101 + index,
								certificateNumber: '2569-0042-00010' + (index + 1) + '-0', recipientType: item.recipientType,
								title: item.title, firstName: item.firstName, lastName: item.lastName,
								activityItem: item.activityItem, awardOrRole: item.awardOrRole, issueDate: '2026-08-14',
								status: 'issued', replacementForCertificateId: null, replacedByCertificateId: null,
								replacementCandidateId: null, createdAt: timestamp,
								capabilities: { canRead: true, canDownload: true, canRevoke: true }
							}))
						};
					},
					async preview() { throw new Error('preview is not used in this workflow test'); }
				};
				window.certificateRequestHarness = {
					submittedIds: () => [...submittedIds],
					returnPayloads: () => structuredClone(returnPayloads),
					issuePayloads: () => structuredClone(issuePayloads),
					requestIds: () => [requestId, secondRequestId],
					reviewCalls: () => [...reviewCalls],
					resolveReview: (id) => {
						const resolve = pendingReviewResolvers.get(id);
						if (!resolve) throw new Error('review request is not pending');
						pendingReviewResolvers.delete(id);
						resolve();
					}
				};

				const target = document.getElementById('app');
				if (view === 'submit') {
					mount(CertificateSubmitRequestDialog, { target, props: {
						open: true, campaignId, campaignName: 'กิจกรรมวันภาษาไทย', candidates,
						onopenchange: () => undefined,
						onsubmit: async (candidateIds) => { submittedIds.push(...candidateIds); }
					} });
				} else if (view === 'history') {
					mount(CertificateCampaignRequests, { target, props: { campaignId, canSubmit: true } });
				} else if (view === 'race') {
					const reviewComponent = createClassComponent({
						component: CertificateIssueRequestReview,
						target,
						props: { requestId, canIssue: true }
					});
					window.certificateRequestHarness.setReviewRequestId = (nextRequestId) => {
						reviewComponent.$set({ requestId: nextRequestId });
						flushSync();
						window.__triggerCertificateAfterNavigate();
					};
				} else {
					mount(CertificateIssueRequestReview, { target, props: { requestId, canIssue: true } });
				}
			`;
		},
		transform(code, id) {
			if (id.split('?')[0] !== reviewComponentPath) return;
			return code.replace("from '$app/navigation'", `from '${navigationStubModuleId}'`);
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				const pathname = new URL(request.url ?? '/', 'http://test').pathname;
				if (pathname !== harnessPath) return next();
				response.setHeader('Content-Type', 'text/html; charset=utf-8');
				response.end(
					`<main id="app"></main><script type="module" src="/@id/${virtualModuleId}"></script>`
				);
			});
		}
	};
}

let devServer: ViteDevServer;
let baseUrl: string;

test.beforeAll(async ({ browserName }, testInfo) => {
	devServer = await createServer({
		root: frontendRoot,
		cacheDir: path.resolve(
			frontendRoot,
			`node_modules/.vite-certificate-request-test-${browserName}-${testInfo.workerIndex}`
		),
		logLevel: 'error',
		plugins: [harnessPlugin()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await devServer.listen();
	const address = devServer.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Vite test server did not start');
	baseUrl = `http://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
	await devServer.close();
});

test('submit groups ready recipients by template before confirmation', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?view=submit`);
	await expect(page.getByRole('heading', { name: 'ส่งคำขอออกเกียรติบัตร' })).toBeVisible();
	await expect(page.getByText('แบบรางวัลการแข่งขัน')).toBeVisible();
	await expect(page.getByText('แบบวิทยากร')).toBeVisible();
	await page.getByRole('button', { name: 'ยืนยันส่ง 2 รายการ' }).click();
	await expect
		.poll(() => page.evaluate(() => window.certificateRequestHarness.submittedIds()))
		.toEqual(['40000000-0000-4000-8000-000000000001', '40000000-0000-4000-8000-000000000002']);
});

test('withdraw changes a pending campaign request to withdrawn history', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?view=history`);
	await expect(page.getByText('รอตรวจ')).toBeVisible();
	await page.getByRole('button', { name: 'ถอนคำขอ' }).click();
	await expect(page.getByText('ถอนแล้ว')).toBeVisible();
});

test('return moves a reviewing request to returned with typed reasons', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?view=review`);
	await page.getByRole('button', { name: 'เริ่มตรวจคำขอ' }).click();
	await page.getByLabel('ผู้ตรวจขอให้แก้ไข').check();
	await page.getByLabel('หมายเหตุส่งกลับ').fill('แก้ชื่อกิจกรรมให้ตรงกับเอกสารต้นทาง');
	await page.getByRole('button', { name: 'ส่งกลับให้แก้ไข' }).click();
	await expect(page.getByRole('heading', { name: 'ส่งกลับแล้ว' })).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificateRequestHarness.returnPayloads().at(-1)))
		.toEqual({
			issueCodes: ['reviewer_requested_changes'],
			returnNote: 'แก้ชื่อกิจกรรมให้ตรงกับเอกสารต้นทาง'
		});
});

test('issue confirmation retries with the same idempotency key until issued', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?view=review`);
	await page.getByRole('button', { name: 'เริ่มตรวจคำขอ' }).click();
	await page.getByRole('button', { name: 'ออกเกียรติบัตร 2 ใบ' }).click();
	await expect(page.getByRole('heading', { name: 'ยืนยันออกเลขเกียรติบัตร' })).toBeVisible();
	await expect(page.getByText('ยังไม่มีเลขเกียรติบัตรถูกจอง')).toBeVisible();

	await page.getByRole('button', { name: 'ยืนยันออกเลข 2 ใบ' }).click();
	await expect(page.getByText('เครือข่ายขัดข้อง กรุณาลองอีกครั้ง')).toBeVisible();
	await page.getByRole('button', { name: 'ลองออกอีกครั้ง' }).click();
	await expect(page.getByRole('heading', { name: 'ออกเลขแล้ว' })).toBeVisible();
	await expect(page.getByText('2569-0042-000101-')).toBeVisible();

	const payloads = await page.evaluate(() => window.certificateRequestHarness.issuePayloads());
	expect(payloads).toHaveLength(2);
	expect(payloads[0].idempotencyKey).toMatch(/^[0-9a-f-]{36}$/i);
	expect(payloads[1].idempotencyKey).toBe(payloads[0].idempotencyKey);
});

test('review action loading stays scoped to the request after route changes', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?view=race`);
	const [firstRequestId, secondRequestId] = await page.evaluate(() =>
		window.certificateRequestHarness.requestIds()
	);
	const reviewButton = page.getByRole('button', { name: 'เริ่มตรวจคำขอ' });
	const busyButton = page.getByRole('button', { name: 'กำลังดำเนินการ...' });

	await reviewButton.click();
	await expect(busyButton).toBeDisabled();
	await page.evaluate(
		(id) => window.certificateRequestHarness.setReviewRequestId(id),
		secondRequestId
	);
	await expect(page.getByText('กิจกรรมสัปดาห์วิทยาศาสตร์')).toBeVisible();
	await expect(reviewButton).toBeEnabled();

	await reviewButton.click();
	await expect(busyButton).toBeDisabled();
	await page.evaluate((id) => window.certificateRequestHarness.resolveReview(id), firstRequestId);
	await expect(busyButton).toBeDisabled();
	await page.evaluate((id) => window.certificateRequestHarness.resolveReview(id), secondRequestId);
	await expect(page.getByRole('button', { name: 'ออกเกียรติบัตร 2 ใบ' })).toBeEnabled();
});

declare global {
	interface Window {
		certificateRequestHarness: {
			submittedIds(): string[];
			returnPayloads(): Array<{ issueCodes: string[]; returnNote: string }>;
			issuePayloads(): Array<{ idempotencyKey: string }>;
			requestIds(): [string, string];
			reviewCalls(): string[];
			resolveReview(requestId: string): void;
			setReviewRequestId(requestId: string): void;
		};
	}
}
