import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-issued-test';
const virtualModuleId = 'virtual:certificate-issued-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-issued-stub:';

const navigationStub = `
	export const afterNavigate = (callback) => { queueMicrotask(callback); };
`;

const pathsStub = `
	export const resolve = (path) => path;
`;

const certificateApiStub = `
	export async function listIssuedCertificates(campaignId, query) {
		return window.__certificateIssuedApi.list(campaignId, query);
	}
	export async function createIssuedCertificateRenderManifest(certificateId) {
		return window.__certificateIssuedApi.singleManifest(certificateId);
	}
	export async function createIssuedCertificateRenderManifests(campaignId, payload) {
		return window.__certificateIssuedApi.batchManifests(campaignId, payload);
	}
	export async function revokeIssuedCertificate(certificateId, payload) {
		return window.__certificateIssuedApi.revoke(certificateId, payload);
	}
`;

const rendererStub = `
	export async function loadCertificateRenderer() {
		return {
			buildCertificatePdf: async (manifests) => {
				window.__certificateIssuedRenderCalls.push(manifests.map((item) => item.pageGeometry.paperLabel));
				return new Uint8Array([37, 80, 68, 70]);
			}
		};
	}
`;

const downloadStub = `
	export const MAX_CERTIFICATE_BATCH_SIZE = 200;
	export function validateCertificateBatchSize(count) {
		if (!Number.isSafeInteger(count) || count < 1) throw new Error('ต้องเลือกเกียรติบัตรอย่างน้อย 1 ใบ');
		if (count > 200) throw new Error('สร้าง PDF ได้ครั้งละไม่เกิน 200 ใบ');
	}
	export function downloadCertificatePdf(bytes, filename) {
		window.__certificateIssuedDownloads.push({ byteLength: bytes.byteLength, filename });
	}
`;

const stubModules = new Map([
	['$app/navigation', navigationStub],
	['$app/paths', pathsStub],
	['$lib/api/certificates', certificateApiStub],
	['$lib/certificates/renderer', rendererStub],
	['$lib/certificates/download', downloadStub]
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
		name: 'certificate-issued-test-harness',
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
				import CertificateIssuedTable from '/src/lib/components/certificates/CertificateIssuedTable.svelte';

				const campaignId = '10000000-0000-4000-8000-000000000001';
				const timestamp = '2026-08-14T02:00:00Z';
				const base = {
					campaignId, campaignName: 'กิจกรรมวันภาษาไทย',
					ownerOrganizationUnitId: '20000000-0000-4000-8000-000000000001',
					ownerOrganizationUnitName: 'กลุ่มสาระภาษาไทย',
					academicYearId: '30000000-0000-4000-8000-000000000001', academicYearValue: 2569,
					activitySequence: 42, issueDate: '2026-08-14', replacementForCertificateId: null,
					replacedByCertificateId: null, replacementCandidateId: null, createdAt: timestamp
				};
				let certificates = [
					{ ...base, id: '40000000-0000-4000-8000-000000000001', templateId: '50000000-0000-4000-8000-000000000001',
						templateName: 'แบบรางวัลการแข่งขัน', certificateSequence: 101,
						certificateNumber: '2569-0042-000101-0', recipientType: 'student', title: 'เด็กหญิง',
						firstName: 'กมลชนก', lastName: 'ใจดี', activityItem: 'การแข่งขันคำคม',
						awardOrRole: 'รองชนะเลิศอันดับที่ 1', status: 'issued',
						capabilities: { canRead: true, canDownload: true, canRevoke: true } },
					{ ...base, id: '40000000-0000-4000-8000-000000000002', templateId: '50000000-0000-4000-8000-000000000002',
						templateName: 'แบบวิทยากร', certificateSequence: 102,
						certificateNumber: '2569-0042-000102-8', recipientType: 'external', title: 'คุณ',
						firstName: 'สายชล', lastName: 'คงดี', activityItem: null, awardOrRole: 'วิทยากร', status: 'issued',
						capabilities: { canRead: true, canDownload: true, canRevoke: true } },
					{ ...base, id: '40000000-0000-4000-8000-000000000003', templateId: '50000000-0000-4000-8000-000000000001',
						templateName: 'แบบรางวัลการแข่งขัน', certificateSequence: 99,
						certificateNumber: '2569-0042-000099-7', recipientType: 'staff', title: 'นาย',
						firstName: 'สมชาย', lastName: 'ใจดี', activityItem: null, awardOrRole: 'กรรมการ', status: 'revoked',
						capabilities: { canRead: true, canDownload: false, canRevoke: false } }
				];
				const manifestRequests = [];
				const revokeRequests = [];
				window.__certificateIssuedDownloads = [];
				window.__certificateIssuedRenderCalls = [];
				function manifest(certificateId) {
					const certificate = certificates.find((item) => item.id === certificateId);
					const portrait = certificateId.endsWith('2');
					return {
						templateId: certificate.templateId, certificateNumber: certificate.certificateNumber,
						suggestedFilename: certificate.certificateNumber + '.pdf', layout: { schemaVersion: 1, elements: [] },
						pageGeometry: { paperLabel: portrait ? 'A4 แนวตั้ง' : 'A4 แนวนอน', rotation: 0,
							displayedWidthPoints: portrait ? 595 : 842, displayedHeightPoints: portrait ? 842 : 595,
							mediaBox: { xPoints: 0, yPoints: 0, widthPoints: portrait ? 595 : 842, heightPoints: portrait ? 842 : 595 },
							cropBox: { xPoints: 0, yPoints: 0, widthPoints: portrait ? 595 : 842, heightPoints: portrait ? 842 : 595 } },
						backgroundGrant: { fileId: '60000000-0000-4000-8000-000000000001', url: '/background.pdf', expiresAt: '2099-01-01T00:00:00Z' },
						fontGrants: [], imageGrants: [], builtInFonts: [], qrPayload: 'proof', recipientValues: {},
						campaignValues: { academicYear: '2569', campaignName: base.campaignName, eventDate: '2026-08-01',
							issueDate: base.issueDate, ownerOrganizationUnitName: base.ownerOrganizationUnitName, schoolName: 'โรงเรียนตัวอย่าง' }
					};
				}
				window.__certificateIssuedApi = {
					async list() { return structuredClone(certificates); },
					async singleManifest(certificateId) {
						manifestRequests.push({ kind: 'single', certificateIds: [certificateId] });
						return structuredClone(manifest(certificateId));
					},
					async batchManifests(id, payload) {
						manifestRequests.push({ kind: 'batch', certificateIds: [...payload.certificateIds] });
						return payload.certificateIds.map((certificateId) => structuredClone(manifest(certificateId)));
					},
					async revoke(certificateId, payload) {
						revokeRequests.push({ certificateId, payload: structuredClone(payload) });
						const current = certificates.find((item) => item.id === certificateId);
						const replacementCandidate = payload.createReplacementCandidate
							? { id: '70000000-0000-4000-8000-000000000001', campaignId,
								templateId: current.templateId, validationStatus: 'ready' }
							: null;
						const updated = { ...current, status: 'revoked', replacementCandidateId: replacementCandidate?.id ?? null,
							capabilities: { canRead: true, canDownload: false, canRevoke: false },
							customValues: {}, issueRunId: '80000000-0000-4000-8000-000000000001', schoolName: 'โรงเรียนตัวอย่าง',
							ownerOrganizationUnitNameSnapshot: base.ownerOrganizationUnitName, revokedBy: '90000000-0000-4000-8000-000000000001',
							revokedAt: timestamp, revocationReason: payload.reason, updatedAt: timestamp };
						certificates = certificates.map((item) => item.id === certificateId ? updated : item);
						return { certificate: structuredClone(updated), replacementCandidate };
					}
				};
				window.certificateIssuedHarness = {
					manifestRequests: () => structuredClone(manifestRequests),
					renderCalls: () => structuredClone(window.__certificateIssuedRenderCalls),
					downloads: () => structuredClone(window.__certificateIssuedDownloads),
					revokeRequests: () => structuredClone(revokeRequests)
				};

				mount(CertificateIssuedTable, { target: document.getElementById('app'), props: {
					campaignId, canRead: true, canDownload: true, canRevoke: true
				} });
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
			`node_modules/.vite-certificate-issued-test-${browserName}-${testInfo.workerIndex}`
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

test('single download uses the lazy renderer only for an issued certificate', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	const issuedRow = page.locator('tr').filter({ hasText: '2569-0042-000101-0' });
	const revokedRow = page.locator('tr').filter({ hasText: '2569-0042-000099-7' });
	await issuedRow.getByRole('button', { name: 'ดาวน์โหลด' }).click();
	await expect
		.poll(() => page.evaluate(() => window.certificateIssuedHarness.downloads()))
		.toEqual([{ byteLength: 4, filename: '2569-0042-000101-0.pdf' }]);
	await expect(revokedRow.getByRole('button', { name: 'ดาวน์โหลด' })).toHaveCount(0);
});

test('batch download preserves selection order across mixed page sizes', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	await page.getByLabel('เลือก 2569-0042-000102-8').check();
	await page.getByLabel('เลือก 2569-0042-000101-0').check();
	await page.getByRole('button', { name: 'ดาวน์โหลดที่เลือก 2 ใบ' }).click();
	await expect(page.getByRole('heading', { name: 'ดาวน์โหลดรวม 2 ใบ' })).toBeVisible();
	await page.getByRole('button', { name: 'สร้าง PDF รวม' }).click();

	await expect
		.poll(() => page.evaluate(() => window.certificateIssuedHarness.manifestRequests()))
		.toEqual([
			{
				kind: 'batch',
				certificateIds: [
					'40000000-0000-4000-8000-000000000002',
					'40000000-0000-4000-8000-000000000001'
				]
			}
		]);
	await expect
		.poll(() => page.evaluate(() => window.certificateIssuedHarness.renderCalls()))
		.toEqual([['A4 แนวตั้ง', 'A4 แนวนอน']]);
});

test('revoke removes download and links the optional replacement candidate', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	let row = page.locator('tr').filter({ hasText: '2569-0042-000101-0' });
	await row.getByRole('button', { name: 'เพิกถอน' }).click();
	await page.getByLabel('เหตุผลการเพิกถอน').fill('ชื่อรางวัลในใบเดิมไม่ถูกต้อง');
	await page.getByLabel('สร้างรายการทดแทนให้แก้ไขและส่งออกใหม่').check();
	await page.getByRole('button', { name: 'ยืนยันเพิกถอน' }).click();

	row = page.locator('tr').filter({ hasText: '2569-0042-000101-0' });
	await expect(row.getByText('เพิกถอนแล้ว')).toBeVisible();
	await expect(row.getByRole('button', { name: 'ดาวน์โหลด' })).toHaveCount(0);
	await expect(row.getByRole('link', { name: 'ไปแก้รายการทดแทน' })).toHaveAttribute(
		'href',
		/recipients#candidate-70000000-0000-4000-8000-000000000001$/
	);
	await expect
		.poll(() => page.evaluate(() => window.certificateIssuedHarness.revokeRequests()))
		.toEqual([
			{
				certificateId: '40000000-0000-4000-8000-000000000001',
				payload: {
					reason: 'ชื่อรางวัลในใบเดิมไม่ถูกต้อง',
					createReplacementCandidate: true
				}
			}
		]);
});

declare global {
	interface Window {
		certificateIssuedHarness: {
			manifestRequests(): Array<{ kind: string; certificateIds: string[] }>;
			renderCalls(): string[][];
			downloads(): Array<{ byteLength: number; filename: string }>;
			revokeRequests(): Array<{
				certificateId: string;
				payload: { reason: string; createReplacementCandidate: boolean };
			}>;
		};
	}
}
