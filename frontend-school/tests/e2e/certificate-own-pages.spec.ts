import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-own-pages-test';
const virtualModuleId = 'virtual:certificate-own-pages-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-own-pages-stub:';

const certificateApiStub = `
	export async function listOwnCertificates() {
		window.__certificateOwnListCalls += 1;
		return structuredClone(window.__certificateOwnItems);
	}
	export async function createOwnCertificateRenderManifest(certificateId) {
		window.__certificateOwnManifestCalls.push(certificateId);
		return structuredClone(window.__certificateOwnManifest);
	}
`;

const rendererStub = `
	export async function loadCertificateRenderer() {
		return {
			buildCertificatePdf: async (manifests) => {
				window.__certificateOwnRenderCalls.push(manifests.map((item) => item.certificateNumber));
				return new Uint8Array([37, 80, 68, 70]);
			}
		};
	}
`;

const downloadStub = `
	export function downloadCertificatePdf(bytes, filename) {
		window.__certificateOwnDownloads.push({ byteLength: bytes.byteLength, filename });
	}
`;

const stubModules = new Map([
	['$lib/api/certificates', certificateApiStub],
	['$lib/certificates/renderer', rendererStub],
	['$lib/certificates/download', downloadStub]
]);

function findStubModule(id: string): string | undefined {
	if (stubModules.has(id)) return id;
	for (const stubId of stubModules.keys()) {
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
		name: 'certificate-own-pages-test-harness',
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
				import MyCertificateList from '/src/lib/components/certificates/MyCertificateList.svelte';

				const timestamp = '2026-08-15T02:00:00Z';
				const base = {
					campaignId: '10000000-0000-4000-8000-000000000001',
					campaignName: 'กิจกรรมวันภาษาไทย',
					ownerOrganizationUnitId: '20000000-0000-4000-8000-000000000001',
					ownerOrganizationUnitName: 'กลุ่มสาระภาษาไทย',
					academicYearId: '30000000-0000-4000-8000-000000000001',
					academicYearValue: 2569,
					activitySequence: 42,
					recipientType: 'student',
					title: 'เด็กหญิง', firstName: 'กมลชนก', lastName: 'ใจดี',
					issueDate: '2026-08-15', replacementForCertificateId: null,
					replacedByCertificateId: null, replacementCandidateId: null, createdAt: timestamp
				};
				window.__certificateOwnItems = [
					{ ...base, id: '40000000-0000-4000-8000-000000000001',
						templateId: '50000000-0000-4000-8000-000000000001', templateName: 'แบบรางวัลการแข่งขัน',
						certificateSequence: 101, certificateNumber: '2569-0042-000101-0',
						activityItem: 'การแข่งขันคำคม', awardOrRole: 'รองชนะเลิศอันดับที่ 1', status: 'issued',
						capabilities: { canRead: true, canDownload: true, canRevoke: false } },
					{ ...base, id: '40000000-0000-4000-8000-000000000002',
						templateId: '50000000-0000-4000-8000-000000000002', templateName: 'แบบฉบับเดิม',
						certificateSequence: 99, certificateNumber: '2569-0042-000099-7',
						activityItem: null, awardOrRole: 'ผู้เข้าร่วม', status: 'revoked',
						capabilities: { canRead: true, canDownload: false, canRevoke: false } }
				];
				window.__certificateOwnManifest = {
					templateId: '50000000-0000-4000-8000-000000000001',
					certificateNumber: '2569-0042-000101-0', suggestedFilename: '2569-0042-000101-0.pdf',
					layout: { schemaVersion: 1, elements: [] },
					pageGeometry: { paperLabel: 'A4 แนวนอน', rotation: 0,
						displayedWidthPoints: 842, displayedHeightPoints: 595,
						mediaBox: { xPoints: 0, yPoints: 0, widthPoints: 842, heightPoints: 595 },
						cropBox: { xPoints: 0, yPoints: 0, widthPoints: 842, heightPoints: 595 } },
					backgroundGrant: { fileId: '60000000-0000-4000-8000-000000000001',
						url: '/background.pdf', expiresAt: '2099-01-01T00:00:00Z' },
					fontGrants: [], imageGrants: [], builtInFonts: [], qrPayload: 'proof', recipientValues: {},
					campaignValues: { academicYear: '2569', campaignName: base.campaignName,
						eventDate: '2026-08-01', issueDate: base.issueDate,
						ownerOrganizationUnitName: base.ownerOrganizationUnitName, schoolName: 'โรงเรียนตัวอย่าง' }
				};
				window.__certificateOwnListCalls = 0;
				window.__certificateOwnManifestCalls = [];
				window.__certificateOwnRenderCalls = [];
				window.__certificateOwnDownloads = [];
				window.certificateOwnHarness = {
					calls: () => ({
						list: window.__certificateOwnListCalls,
						manifests: structuredClone(window.__certificateOwnManifestCalls),
						renders: structuredClone(window.__certificateOwnRenderCalls),
						downloads: structuredClone(window.__certificateOwnDownloads)
					})
				};

				const portal = new URL(window.location.href).searchParams.get('portal');
				mount(MyCertificateList, {
					target: document.getElementById('app'),
					props: portal === 'student'
						? { title: 'เกียรติบัตรของฉัน', description: 'คลังนักเรียน' }
						: { title: 'เกียรติบัตรที่โรงเรียนออก', description: 'คลังบุคลากร' }
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
			`node_modules/.vite-certificate-own-pages-test-${browserName}-${testInfo.workerIndex}`
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

test('staff own-certificate page shows issued and revoked cards without revoked download', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}?portal=staff`);
	await expect(page.getByRole('heading', { name: 'เกียรติบัตรที่โรงเรียนออก' })).toBeVisible();
	await expect(page.getByTestId('my-certificate-card')).toHaveCount(2);
	const issued = page.getByTestId('my-certificate-card').filter({ hasText: '2569-0042-000101-0' });
	const revoked = page.getByTestId('my-certificate-card').filter({ hasText: '2569-0042-000099-7' });
	await expect(issued.getByRole('button', { name: 'ดาวน์โหลด' })).toBeVisible();
	await expect(revoked.getByText('เพิกถอนแล้ว')).toBeVisible();
	await expect(revoked.getByRole('button', { name: 'ดาวน์โหลด' })).toHaveCount(0);
	await expect(issued.getByRole('link', { name: 'ตรวจสอบสาธารณะ' })).toHaveAttribute(
		'href',
		'/verify/certificate/2569-0042-000101-0'
	);
	await expect.poll(() => page.evaluate(() => window.certificateOwnHarness.calls().list)).toBe(1);
});

test('student own-certificate page downloads an issued certificate through the own manifest', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}?portal=student`);
	await expect(page.getByRole('heading', { name: 'เกียรติบัตรของฉัน' })).toBeVisible();
	const issued = page.getByTestId('my-certificate-card').filter({ hasText: '2569-0042-000101-0' });
	await issued.getByRole('button', { name: 'ดาวน์โหลด' }).click();

	await expect
		.poll(() => page.evaluate(() => window.certificateOwnHarness.calls()))
		.toEqual({
			list: 1,
			manifests: ['40000000-0000-4000-8000-000000000001'],
			renders: [['2569-0042-000101-0']],
			downloads: [{ byteLength: 4, filename: '2569-0042-000101-0.pdf' }]
		});
});

declare global {
	interface Window {
		certificateOwnHarness: {
			calls(): {
				list: number;
				manifests: string[];
				renders: string[][];
				downloads: Array<{ byteLength: number; filename: string }>;
			};
		};
	}
}
