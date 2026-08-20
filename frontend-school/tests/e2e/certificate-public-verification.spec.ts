import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-public-verification-test';
const virtualModuleId = 'virtual:certificate-public-verification-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-public-verification-stub:';

const publicCertificateApiStub = `
	export async function verifyCertificateManually(payload, options) {
		return window.__certificatePublicApi.verifyManual(payload, options);
	}
	export async function verifyCertificateByQr(payload, options) {
		return window.__certificatePublicApi.verifyQr(payload, options);
	}
	export async function createPublicCertificateRenderManifest(payload, options) {
		return window.__certificatePublicApi.render(payload, options);
	}
`;

const rendererStub = `
	export async function loadCertificateRenderer() {
		window.__certificatePublicRendererLoads += 1;
		return {
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

				function result(status) {
					return {
						status,
						certificateNumber,
						title: 'เด็กหญิง', firstName: 'กมลชนก', lastName: 'ใจดี',
						campaignName: 'กิจกรรมวันภาษาไทย', academicYear: 2569,
						templateName: 'แบบรางวัลการแข่งขัน', activityItem: 'การแข่งขันคำคม',
						awardOrRole: 'รองชนะเลิศอันดับที่ 1', issueDate: '2026-08-14',
						issuerSchoolName: 'โรงเรียนตัวอย่าง',
						replacementCertificateNumber: status === 'revoked' ? '2569-0042-000124-2' : null,
						receipt: status === 'issued' ? 'receipt-for-public-render' : null,
						receiptExpiresAt: status === 'issued' ? '2099-01-01T00:00:00Z' : null
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
					async verifyManual(payload) {
						verificationCalls.push({ kind: 'manual', payload: structuredClone(payload), hashAtCall: window.location.hash });
						return result('issued');
					},
					async verifyQr(payload) {
						verificationCalls.push({ kind: 'qr', payload: structuredClone(payload), hashAtCall: window.location.hash });
						return result(mode === 'revoked' ? 'revoked' : 'issued');
					},
					async render(payload) {
						renderCalls.push(structuredClone(payload));
						return manifest();
					}
				};

				window.certificatePublicHarness = {
					verificationCalls: () => structuredClone(verificationCalls),
					renderCalls: () => structuredClone(renderCalls),
					rendererLoads: () => window.__certificatePublicRendererLoads,
					builds: () => structuredClone(window.__certificatePublicBuilds),
					downloads: () => structuredClone(window.__certificatePublicDownloads)
				};

				mount(PublicCertificateVerification, {
					target: document.getElementById('app'),
					props: {
						initialNumber: mode === 'manual' ? '' : certificateNumber,
						autoVerifyQr: mode !== 'manual'
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

test('manual verification submits three fields and downloads only through the receipt', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}?mode=manual`);
	await page
		.getByRole('textbox', { name: 'เลขเกียรติบัตร', exact: true })
		.fill('2569-0042-000123-4');
	await page.getByLabel('ชื่อ', { exact: true }).fill('กมลชนก');
	await page.getByLabel('นามสกุล').fill('ใจดี');
	await page.getByRole('button', { name: 'ตรวจสอบข้อมูล' }).click();

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

	await page.getByRole('button', { name: 'ดาวน์โหลดเกียรติบัตร' }).click();
	await expect
		.poll(() => page.evaluate(() => window.certificatePublicHarness.renderCalls()))
		.toEqual([{ receipt: 'receipt-for-public-render' }]);
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
		certificatePublicHarness: {
			verificationCalls(): Array<{
				kind: string;
				payload: Record<string, string>;
				hashAtCall: string;
			}>;
			renderCalls(): Array<{ receipt: string }>;
			rendererLoads(): number;
			builds(): string[][];
			downloads(): Array<{ byteLength: number; filename: string }>;
		};
	}
}
