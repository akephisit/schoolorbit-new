import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-public-verification-test';
const virtualModuleId = 'virtual:certificate-public-verification-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-public-verification-stub:';

const publicCertificateApiStub = `
	export async function verifyCertificateManually(payload, options = {}) {
		return window.__certificatePublicApi.verifyManual(payload, options);
	}
	export async function verifyCertificateByQr(payload, options = {}) {
		return window.__certificatePublicApi.verifyQr(payload, options);
	}
	export async function createPublicCertificateRenderManifest(payload, options = {}) {
		return window.__certificatePublicApi.render(payload, options);
	}
`;

const apiClientStub = `
	export class ApiClientError extends Error {
		constructor(message, status) {
			super(message);
			this.name = 'ApiClientError';
			this.status = status;
		}
	}
`;

const rendererStub = `
	export async function loadCertificateRenderer() {
		window.__certificatePublicRendererLoads += 1;
		return {
			renderPreview: async (manifest, canvas, options = {}) => {
				options.signal?.throwIfAborted();
				const attempt = ++window.__certificatePublicPreviewAttempts;
				await window.__certificatePublicPreviewControl.beforeRender(options.signal);
				if (!window.__certificatePublicPreviewControl.ignoreAbortAfterWait()) {
					options.signal?.throwIfAborted();
				}
				const scale = options.scale ?? 1;
				canvas.width = Math.max(1, Math.round(manifest.pageGeometry.displayedWidthPoints * scale));
				canvas.height = Math.max(1, Math.round(manifest.pageGeometry.displayedHeightPoints * scale));
				const context = canvas.getContext('2d');
				context.fillStyle = attempt === 1 ? '#dc2626' : '#16a34a';
				context.fillRect(0, 0, canvas.width, canvas.height);
				window.__certificatePublicPreviews.push(manifest.certificateNumber);
				return {
					widthPoints: manifest.pageGeometry.displayedWidthPoints,
					heightPoints: manifest.pageGeometry.displayedHeightPoints,
					widthPixels: canvas.width,
					heightPixels: canvas.height
				};
			},
			buildCertificatePdf: async (manifests) => {
				window.__certificatePublicBuilds.push(manifests.map((item) => item.certificateNumber));
				return new Uint8Array([37, 80, 68, 70]);
			}
		};
	}
`;

const downloadStub = `
	export function downloadCertificatePdf(bytes, filename) {
		window.__certificatePublicDownloads.push({ byteLength: bytes.byteLength, filename });
	}
`;

const stubModules = new Map([
	['$lib/api/client', apiClientStub],
	['$lib/api/public-certificates', publicCertificateApiStub],
	['$lib/certificates/renderer', rendererStub],
	['$lib/certificates/download', downloadStub]
]);

function findStubModule(id: string): string | undefined {
	if (stubModules.has(id)) return id;
	for (const stubId of stubModules.keys()) {
		const resolvedPath = path.resolve(frontendRoot, 'src/lib', stubId.slice('$lib/'.length));
		if (id === resolvedPath || id === `${resolvedPath}.ts` || id === `${resolvedPath}.svelte`) {
			return stubId;
		}
	}
}

function harnessPlugin(): Plugin {
	return {
		name: 'certificate-public-verification-test-harness',
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
				import { ApiClientError } from '$lib/api/client';
				import '/src/routes/layout.css';
				import PublicCertificateVerification from '/src/lib/components/certificates/PublicCertificateVerification.svelte';

				const query = new URL(window.location.href).searchParams;
				const mode = query.get('mode') ?? 'manual';
				const certificateNumber = '2569-0042-000123-4';
				const verificationCalls = [];
				const renderCalls = [];
				window.__certificatePublicRendererLoads = 0;
				window.__certificatePublicBuilds = [];
				window.__certificatePublicDownloads = [];
				window.__certificatePublicPreviews = [];
				window.__certificatePublicPreviewAttempts = 0;
				let failPreviewCount = mode === 'preview-error' ? 1 : 0;
				let holdNextPreview = mode === 'loading' || mode === 'stale';
				let releaseHeldPreview = null;
				let verificationAttempt = 0;
				let manifestAttempt = 0;

				window.__certificatePublicPreviewControl = {
					async beforeRender() {
						if (failPreviewCount > 0) {
							failPreviewCount -= 1;
							throw new Error('controlled preview failure');
						}
						if (!holdNextPreview) return;
						holdNextPreview = false;
						await new Promise((resolve) => { releaseHeldPreview = resolve; });
					},
					release() {
						const release = releaseHeldPreview;
						releaseHeldPreview = null;
						release?.();
					},
					ignoreAbortAfterWait: () => mode === 'stale'
				};

				function result(status) {
					const refreshedReceipt =
						(mode === 'expired' || mode === 'revoked-after-expiry') && verificationAttempt > 1
							? 'refreshed-public-render-receipt'
							: 'receipt-for-public-render';
					return {
						status,
						certificateNumber,
						title: 'เด็กหญิง', firstName: 'กมลชนก', lastName: 'ใจดี',
						campaignName: 'กิจกรรมวันภาษาไทย', academicYear: 2569,
						templateName: 'แบบรางวัลการแข่งขัน', activityItem: 'การแข่งขันคำคม',
						awardOrRole: 'รองชนะเลิศอันดับที่ 1', issueDate: '2026-08-14',
						issuerSchoolName: 'โรงเรียนตัวอย่าง',
						replacementCertificateNumber: status === 'revoked' ? '2569-0042-000124-2' : null,
						receipt: status === 'issued' ? refreshedReceipt : null,
						receiptExpiresAt: status === 'issued' ? '2099-01-01T00:00:00Z' : null
					};
				}

				function manualResult(status, payload) {
					return {
						...result(status),
						firstName: payload.firstName,
						lastName: payload.lastName
					};
				}

				function manifest() {
					return {
						templateId: '10000000-0000-4000-8000-000000000001', certificateNumber,
						suggestedFilename: certificateNumber + '.pdf', layout: { schemaVersion: 1, elements: [] },
						pageGeometry: { paperLabel: 'A4 แนวนอน', rotation: 0,
							displayedWidthPoints: 842, displayedHeightPoints: 595,
							mediaBox: { xPoints: 0, yPoints: 0, widthPoints: 842, heightPoints: 595 },
							cropBox: { xPoints: 0, yPoints: 0, widthPoints: 842, heightPoints: 595 } },
						backgroundGrant: { fileId: '20000000-0000-4000-8000-000000000001', url: '/background.pdf', expiresAt: '2099-01-01T00:00:00Z' },
						fontGrants: [], imageGrants: [], builtInFonts: [], qrPayload: 'public-proof', recipientValues: {},
						campaignValues: { academicYear: '2569', campaignName: 'กิจกรรมวันภาษาไทย', eventDate: '2026-08-01',
							issueDate: '2026-08-14', ownerOrganizationUnitName: 'กลุ่มสาระภาษาไทย', schoolName: 'โรงเรียนตัวอย่าง' }
					};
				}

				window.__certificatePublicApi = {
					async verifyManual(payload, options = {}) {
						options.signal?.throwIfAborted();
						verificationAttempt += 1;
						verificationCalls.push({ kind: 'manual', payload: structuredClone(payload), hashAtCall: window.location.hash });
						return manualResult(
							mode === 'revoked-after-expiry' && verificationAttempt > 1 ? 'revoked' : 'issued',
							payload
						);
					},
					async verifyQr(payload, options = {}) {
						options.signal?.throwIfAborted();
						verificationAttempt += 1;
						verificationCalls.push({ kind: 'qr', payload: structuredClone(payload), hashAtCall: window.location.hash });
						return result(mode === 'revoked' ? 'revoked' : 'issued');
					},
					async render(payload, options = {}) {
						options.signal?.throwIfAborted();
						manifestAttempt += 1;
						renderCalls.push(structuredClone(payload));
						if (
							(mode === 'expired' || mode === 'revoked-after-expiry') &&
							payload.receipt === 'receipt-for-public-render'
						) {
							throw new ApiClientError('ไม่พบข้อมูลที่ตรงกัน', 404);
						}
						return manifest();
					}
				};

				window.certificatePublicHarness = {
					verificationCalls: () => structuredClone(verificationCalls),
					renderCalls: () => structuredClone(renderCalls),
					rendererLoads: () => window.__certificatePublicRendererLoads,
					previews: () => structuredClone(window.__certificatePublicPreviews),
					builds: () => structuredClone(window.__certificatePublicBuilds),
					downloads: () => structuredClone(window.__certificatePublicDownloads),
					releaseHeldPreview: () => window.__certificatePublicPreviewControl.release()
				};

				mount(PublicCertificateVerification, {
					target: document.getElementById('app'),
					props: {
						initialNumber: mode === 'qr' || mode === 'revoked' ? certificateNumber : '',
						autoVerifyQr: mode === 'qr' || mode === 'revoked'
					}
				});
			`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				const pathname = new URL(request.url ?? '/', 'http://test').pathname;
				if (pathname !== harnessPath) return next();
				response.setHeader('Content-Type', 'text/html; charset=utf-8');
				response.setHeader('Referrer-Policy', 'no-referrer');
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
			`node_modules/.vite-certificate-public-verification-${browserName}-${testInfo.workerIndex}`
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

async function completeManualVerification(
	page: Page,
	firstName = 'กมลชนก',
	lastName = 'ใจดี'
): Promise<void> {
	await page
		.getByRole('textbox', { name: 'เลขเกียรติบัตร', exact: true })
		.fill('2569-0042-000123-4');
	await page.getByLabel('ชื่อ', { exact: true }).fill(firstName);
	await page.getByLabel('นามสกุล').fill(lastName);
	await page.getByRole('button', { name: 'ตรวจสอบข้อมูล' }).click();
}

test('manual verification submits three fields and downloads only through the receipt', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=manual`);
	await completeManualVerification(page);

	await expect(page.getByTestId('verification-result')).toContainText('ใช้ได้');
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.verificationCalls()))
		.toEqual([
			{
				kind: 'manual',
				payload: {
					certificateNumber: '2569-0042-000123-4',
					firstName: 'กมลชนก',
					lastName: 'ใจดี'
				},
				hashAtCall: ''
			}
		]);
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.renderCalls()))
		.toEqual([{ receipt: 'receipt-for-public-render' }]);
	await expect(page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว')).toBeVisible();

	await page.getByRole('button', { name: 'ดาวน์โหลดเกียรติบัตร' }).click();
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.renderCalls()))
		.toEqual([{ receipt: 'receipt-for-public-render' }, { receipt: 'receipt-for-public-render' }]);
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.downloads()))
		.toEqual([{ byteLength: 4, filename: '2569-0042-000123-4.pdf' }]);
});

test('QR verification removes the proof fragment before the POST begins', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=qr#proof=opaque-qr-proof`);

	await expect(page).toHaveURL(`${baseUrl}${harnessPath}?mode=qr`);
	await expect(page.getByTestId('verification-result')).toContainText('ใช้ได้');
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.verificationCalls()))
		.toEqual([
			{
				kind: 'qr',
				payload: {
					certificateNumber: '2569-0042-000123-4',
					proof: 'opaque-qr-proof'
				},
				hashAtCall: ''
			}
		]);
	await expect(page.locator('body')).not.toContainText('opaque-qr-proof');
	await expect(page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.renderCalls()))
		.toEqual([{ receipt: 'receipt-for-public-render' }]);
});

test('revoked QR result shows replacement but never requests a render manifest', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=revoked#proof=revoked-proof`);

	const result = page.getByTestId('verification-result');
	await expect(result).toContainText('เพิกถอนแล้ว');
	await expect(result).toContainText('2569-0042-000124-2');
	await expect(page.getByRole('button', { name: 'ดาวน์โหลดเกียรติบัตร' })).toHaveCount(0);
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.renderCalls()))
		.toEqual([]);
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.rendererLoads()))
		.toBe(0);
});

test('issued public preview reports font and render progress', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=loading`);
	await completeManualVerification(page);
	await expect(page.getByText('กำลังสร้างภาพเกียรติบัตร…')).toBeVisible();
	await page.evaluate(() => window.certificatePublicHarness.releaseHeldPreview());
	await expect(page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว')).toBeVisible();
});

test('issued preview retry re-verifies one expired receipt', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=expired`);
	await completeManualVerification(page);
	await expect(page.getByText('สร้างภาพเกียรติบัตรไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองโหลดภาพอีกครั้ง' }).click();
	await expect(page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.verificationCalls().length))
		.toBe(2);
});

test('preview failure keeps verified details and PDF download usable', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=preview-error`);
	await completeManualVerification(page);
	const verifiedResult = page.getByTestId('verification-result');
	await expect(verifiedResult).toContainText('ใช้ได้');
	await expect(verifiedResult).toContainText('กมลชนก ใจดี');
	await expect(page.getByText('สร้างภาพเกียรติบัตรไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ดาวน์โหลดเกียรติบัตร' }).click();
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.downloads()))
		.toEqual([{ byteLength: 4, filename: '2569-0042-000123-4.pdf' }]);
});

test('receipt retry that discovers revocation removes preview and download', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=revoked-after-expiry`);
	await completeManualVerification(page);
	await expect(page.getByText('สร้างภาพเกียรติบัตรไม่สำเร็จ')).toBeVisible();
	await page.getByRole('button', { name: 'ลองโหลดภาพอีกครั้ง' }).click();
	await expect(page.getByTestId('verification-result')).toContainText('เพิกถอนแล้ว');
	await expect(page.getByRole('button', { name: 'ดาวน์โหลดเกียรติบัตร' })).toHaveCount(0);
	await expect(page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว')).toHaveCount(0);
});

test('issued registry layout keeps preview primary on desktop and status first on mobile', async ({
	page
}) => {
	await page.setViewportSize({ width: 1440, height: 900 });
	await page.goto(`${baseUrl}${harnessPath}?mode=manual`);
	await completeManualVerification(page, 'กมลชนก', 'ใจดี');
	const preview = page.getByTestId('public-certificate-preview-region');
	const details = page.getByTestId('public-certificate-details');
	await expect(preview).toBeVisible();
	expect((await preview.boundingBox())?.x).toBeLessThan((await details.boundingBox())?.x ?? 0);

	await page.setViewportSize({ width: 390, height: 844 });
	const status = page.getByTestId('public-certificate-status');
	expect((await status.boundingBox())?.y).toBeLessThan((await preview.boundingBox())?.y ?? 0);
	expect((await preview.boundingBox())?.y).toBeLessThan((await details.boundingBox())?.y ?? 0);
});

test('public fullscreen exits with Escape and returns to the verified result', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=manual`);
	await completeManualVerification(page, 'กมลชนก', 'ใจดี');
	await page.getByRole('button', { name: 'ขยายเต็มจอ' }).click();
	const fullscreen = page.getByRole('dialog', {
		name: 'เกียรติบัตรที่ตรวจสอบแล้วแบบเต็มจอ'
	});
	await expect(fullscreen).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(fullscreen).toBeHidden();
	await expect(page.getByTestId('verification-result')).toBeVisible();
});

test('a new verification clears and aborts the previous certificate preview', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=stale`);
	await completeManualVerification(page, 'คนแรก', 'ทดสอบ');
	await expect(page.getByText('กำลังสร้างภาพเกียรติบัตร…')).toBeVisible();
	await page.getByRole('button', { name: 'ตรวจสอบหมายเลขอื่น' }).click();
	await completeManualVerification(page, 'คนที่สอง', 'ทดสอบ');
	await page.evaluate(() => window.certificatePublicHarness.releaseHeldPreview());
	await expect(page.getByTestId('verification-result')).toContainText('คนที่สอง');
	await expect(page.getByTestId('verification-result')).not.toContainText('คนแรก');
});

test('a cancelled resize render cannot overwrite the latest preview canvas', async ({ page }) => {
	await page.setViewportSize({ width: 1280, height: 900 });
	await page.goto(`${baseUrl}${harnessPath}?mode=stale`);
	await completeManualVerification(page);
	await expect(page.getByText('กำลังสร้างภาพเกียรติบัตร…')).toBeVisible();
	await page.setViewportSize({ width: 1040, height: 900 });
	const canvas = page.getByLabel('ภาพเกียรติบัตรที่ตรวจสอบแล้ว');
	await expect(canvas).toBeVisible();
	await page.evaluate(() => window.certificatePublicHarness.releaseHeldPreview());
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.previews().length))
		.toBe(2);
	await expect
		.poll(() =>
			canvas.evaluate((node) => {
				const context = (node as HTMLCanvasElement).getContext('2d');
				if (!context) throw new Error('preview canvas context missing');
				return Array.from(context.getImageData(0, 0, 1, 1).data);
			})
		)
		.toEqual([22, 163, 74, 255]);
});

declare global {
	interface Window {
		__certificatePublicApi: {
			verifyManual(payload: Record<string, string>, options?: unknown): Promise<unknown>;
			verifyQr(payload: Record<string, string>, options?: unknown): Promise<unknown>;
			render(payload: { receipt: string }, options?: unknown): Promise<unknown>;
		};
		__certificatePublicRendererLoads: number;
		__certificatePublicBuilds: string[][];
		__certificatePublicDownloads: Array<{ byteLength: number; filename: string }>;
		__certificatePublicPreviews: string[];
		__certificatePublicPreviewAttempts: number;
		__certificatePublicPreviewControl: {
			beforeRender(signal?: AbortSignal): Promise<void>;
			release(): void;
			ignoreAbortAfterWait(): boolean;
		};
		certificatePublicHarness: {
			verificationCalls(): Array<{
				kind: string;
				payload: Record<string, string>;
				hashAtCall: string;
			}>;
			renderCalls(): Array<{ receipt: string }>;
			rendererLoads(): number;
			previews(): string[];
			builds(): string[][];
			downloads(): Array<{ byteLength: number; filename: string }>;
			releaseHeldPreview(): void;
		};
	}
}
