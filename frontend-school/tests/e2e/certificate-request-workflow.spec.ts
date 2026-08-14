import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-request-test';
const virtualModuleId = 'virtual:certificate-request-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-request-stub:';

const navigationStub = `
	export const afterNavigate = (callback) => { queueMicrotask(callback); };
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
	if (id.endsWith('/@sveltejs/kit/src/runtime/app/navigation.js')) return '$app/navigation';
	if (id.endsWith('/@sveltejs/kit/src/runtime/app/paths.js')) return '$app/paths';
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
			const stubId = findStubModule(id);
			if (stubId) return `${stubPrefix}${stubId}`;
		},
		load(id) {
			if (id.startsWith(stubPrefix)) return stubModules.get(id.slice(stubPrefix.length));
			if (id !== resolvedVirtualModuleId) return;
			return `
				import { mount } from 'svelte';
				import '/src/routes/layout.css';
				import CertificateSubmitRequestDialog from '/src/lib/components/certificates/CertificateSubmitRequestDialog.svelte';
				import CertificateCampaignRequests from '/src/lib/components/certificates/CertificateCampaignRequests.svelte';
				import CertificateIssueRequestReview from '/src/lib/components/certificates/CertificateIssueRequestReview.svelte';

				const campaignId = '10000000-0000-4000-8000-000000000001';
				const requestId = '20000000-0000-4000-8000-000000000001';
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
				const submittedIds = [];
				const returnPayloads = [];
				window.__certificateRequestApi = {
					async listCampaignRequests() { return [structuredClone(request)]; },
					async withdraw() {
						request = { ...request, status: 'withdrawn', withdrawnAt: timestamp,
							capabilities: { ...request.capabilities, canWithdraw: false } };
						return structuredClone(request);
					},
					async listQueue() { return [structuredClone(request)]; },
					async getRequest() { return structuredClone(request); },
					async startReview() {
						request = { ...request, status: 'reviewing', reviewedAt: timestamp,
							capabilities: { canWithdraw: false, canStartReview: false, canReturn: true, canIssue: true } };
						return structuredClone(request);
					},
					async returnRequest(id, payload) {
						returnPayloads.push(structuredClone(payload));
						request = { ...request, status: 'returned', returnedAt: timestamp,
							returnNote: payload.returnNote, issueCodes: payload.issueCodes,
							capabilities: { canWithdraw: false, canStartReview: false, canReturn: false, canIssue: false } };
						return structuredClone(request);
					},
					async preview() { throw new Error('preview is not used in this workflow test'); }
				};
				window.certificateRequestHarness = {
					submittedIds: () => [...submittedIds],
					returnPayloads: () => structuredClone(returnPayloads)
				};

				const view = new URLSearchParams(window.location.search).get('view');
				const target = document.getElementById('app');
				if (view === 'submit') {
					mount(CertificateSubmitRequestDialog, { target, props: {
						open: true, campaignId, campaignName: 'กิจกรรมวันภาษาไทย', candidates,
						onopenchange: () => undefined,
						onsubmit: async (candidateIds) => { submittedIds.push(...candidateIds); }
					} });
				} else if (view === 'history') {
					mount(CertificateCampaignRequests, { target, props: { campaignId, canSubmit: true } });
				} else {
					mount(CertificateIssueRequestReview, { target, props: { requestId, canIssue: true } });
				}
			`;
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

declare global {
	interface Window {
		certificateRequestHarness: {
			submittedIds(): string[];
			returnPayloads(): Array<{ issueCodes: string[]; returnNote: string }>;
		};
	}
}
