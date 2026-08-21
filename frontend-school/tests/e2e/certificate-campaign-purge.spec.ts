import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

declare global {
	interface Window {
		certificatePurgeHarness: {
			calls: () => {
				impact: number;
				start: unknown[];
				status: number;
				retry: number;
				completed: number;
			};
		};
	}
}

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-campaign-purge-test';
const virtualModuleId = 'virtual:certificate-campaign-purge-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-campaign-purge-stub:';

const certificateApiStub = `
	export async function getCertificateCampaignPurgeImpact(campaignId, options) {
		return window.__certificatePurgeApi.impact(campaignId, options);
	}
	export async function startCertificateCampaignPurge(campaignId, payload, options) {
		return window.__certificatePurgeApi.start(campaignId, payload, options);
	}
	export async function getCertificateCampaignPurgeStatus(campaignId, options) {
		return window.__certificatePurgeApi.status(campaignId, options);
	}
	export async function retryCertificateCampaignPurge(campaignId, options) {
		return window.__certificatePurgeApi.retry(campaignId, options);
	}
`;

const apiClientStub = `
	export class ApiClientError extends Error {
		constructor(message, status) {
			super(message);
			this.status = status;
		}
	}
`;

const stubModules = new Map([
	['$lib/api/certificates', certificateApiStub],
	['$lib/api/client', apiClientStub]
]);

function findStubModule(id: string): string | undefined {
	if (stubModules.has(id)) return id;
	for (const stubId of stubModules.keys()) {
		const resolvedPath = path.resolve(frontendRoot, 'src/lib', stubId.slice('$lib/'.length));
		if (id === resolvedPath || id === `${resolvedPath}.ts` || id === `${resolvedPath}.js`) {
			return stubId;
		}
	}
}

function harnessPlugin(): Plugin {
	return {
		name: 'certificate-campaign-purge-test-harness',
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
				import CertificateCampaignPurgeDialog from '/src/lib/components/certificates/CertificateCampaignPurgeDialog.svelte';
				import { ApiClientError } from '$lib/api/client';

				const mode = new URL(window.location.href).searchParams.get('mode') ?? 'start';
				const campaignId = '10000000-0000-4000-8000-000000000001';
				const campaignName = 'กิจกรรมวันภาษาไทย ๒๕๖๙';
				const counts = {
					templateCount: 3, candidateCount: 128, requestCount: 7, openRequestCount: 2,
					issuedCertificateCount: 99, revokedCertificateCount: 4,
					fileCount: 12, totalFileBytes: 5242880
				};
				const calls = { impact: 0, start: [], status: 0, retry: 0, completed: 0 };
				window.__certificatePurgeApi = {
					async impact() {
						calls.impact += 1;
						return { campaignId, campaignName, updatedAt: '2026-08-22T12:00:00Z', counts };
					},
					async start(_campaignId, payload) {
						calls.start.push(structuredClone(payload));
						return { campaignId, phase: 'deleting_files', fileCount: 12, deletedFileCount: 3, lastErrorCode: null };
					},
					async status() {
						calls.status += 1;
						if (mode === 'retry' && calls.retry === 0) {
							return { campaignId, phase: 'failed', fileCount: 12, deletedFileCount: 5, lastErrorCode: 'storage_operation_failed' };
						}
						if (mode === 'start' && calls.status === 1) {
							return { campaignId, phase: 'finalizing', fileCount: 12, deletedFileCount: 12, lastErrorCode: null };
						}
						if (mode === 'start') throw new ApiClientError('not found', 404);
						return { campaignId, phase: 'completed', fileCount: 12, deletedFileCount: 12, lastErrorCode: null };
					},
					async retry() {
						calls.retry += 1;
						return { campaignId, phase: 'deleting_files', fileCount: 12, deletedFileCount: 5, lastErrorCode: null };
					}
				};
				window.certificatePurgeHarness = { calls: () => structuredClone(calls) };

				mount(CertificateCampaignPurgeDialog, {
					target: document.getElementById('app'),
					props: {
						open: true,
						campaignId,
						campaignName,
						initiallyPurging: mode === 'retry',
						onopenchange: () => {},
						oncompleted: () => { calls.completed += 1; }
					}
				});
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
			`node_modules/.vite-certificate-campaign-purge-${browserName}-${testInfo.workerIndex}`
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

test('requires the exact campaign name, shows impact, and polls through completion', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=start`);
	await expect(page.getByRole('heading', { name: 'ลบกิจกรรมถาวร' })).toBeVisible();
	for (const text of ['128', '99', '12', '5 MB', 'คำขอที่ยังไม่จบ']) {
		await expect(page.getByText(text, { exact: true }).first()).toBeVisible();
	}

	const confirmation = page.getByLabel('พิมพ์ชื่อกิจกรรมเพื่อยืนยัน');
	const submit = page.getByRole('button', { name: 'ลบกิจกรรมถาวร' });
	await confirmation.fill('กิจกรรมวันภาษาไทย 2569');
	await expect(submit).toBeDisabled();
	await confirmation.fill('กิจกรรมวันภาษาไทย ๒๕๖๙');
	await expect(submit).toBeEnabled();
	await submit.click();
	await expect(page.getByText('กำลังลบไฟล์และปิดการเข้าถึง')).toBeVisible();
	await expect(page.getByText('กำลังลบข้อมูลในระบบ')).toBeVisible({ timeout: 5_000 });
	await expect
		.poll(() => page.evaluate(() => window.certificatePurgeHarness.calls().completed), {
			timeout: 5_000
		})
		.toBe(1);
	const calls = await page.evaluate(() => window.certificatePurgeHarness.calls());
	expect(calls.start).toEqual([
		{
			confirmationName: 'กิจกรรมวันภาษาไทย ๒๕๖๙',
			expectedUpdatedAt: '2026-08-22T12:00:00Z',
			expectedImpact: {
				templateCount: 3,
				candidateCount: 128,
				requestCount: 7,
				openRequestCount: 2,
				issuedCertificateCount: 99,
				revokedCertificateCount: 4,
				fileCount: 12,
				totalFileBytes: 5242880
			}
		}
	]);
});

test('shows durable failure progress and retries the same purge job', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=retry`);
	await expect(page.getByText('การลบหยุดชั่วคราว')).toBeVisible();
	await expect(page.getByText('ลบไฟล์แล้ว 5 จาก 12 ไฟล์')).toBeVisible();
	await page.getByRole('button', { name: 'ลองลบต่อ' }).click();
	await expect(page.getByText('กำลังลบไฟล์และปิดการเข้าถึง')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificatePurgeHarness.calls().completed), {
			timeout: 5_000
		})
		.toBe(1);
	await expect
		.poll(() => page.evaluate(() => window.certificatePurgeHarness.calls().retry))
		.toBe(1);
});
