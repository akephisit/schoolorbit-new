import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-campaign-display-test';
const virtualModuleId = 'virtual:certificate-campaign-display-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-campaign-display-stub:';

const certificateApiStub = `
	export const campaignFixture = {
			id: '10000000-0000-4000-8000-000000000001',
			academicYearId: '20000000-0000-4000-8000-000000000001',
			academicYearValue: 2569,
			academicYearName: 'ปีการศึกษา 2569',
			ownerOrganizationUnitId: null,
			ownerOrganizationUnitCode: null,
			ownerOrganizationUnitName: null,
			name: 'วันภาษาไทย',
			eventDate: '2026-08-07',
			status: 'draft',
			activitySequence: null,
			nextCertificateSequence: 1,
			templateCount: 0,
			candidateCount: 0,
			issuedCertificateCount: 0,
			hasOpenIssueRequest: false,
			createdBy: null,
			updatedBy: null,
			createdAt: '2026-08-07T00:00:00Z',
			updatedAt: '2026-08-07T00:00:00Z',
			capabilities: {
				canRead: true,
				canUpdate: false,
				canPrepareCandidates: false,
				canDelete: false,
				canSubmit: false,
				canDownload: false,
				canChangeStatus: false,
				canManageTemplates: false
			}
		};
	export async function getCertificateCampaign() {
		return campaignFixture;
	}
	export async function changeCertificateCampaignStatus() { throw new Error('not used'); }
	export async function listCertificateOwnerOptions() { throw new Error('not used'); }
	export async function updateCertificateCampaign() { throw new Error('not used'); }
	export async function getCertificateCampaignPurgeImpact() { throw new Error('not used'); }
	export async function startCertificateCampaignPurge() { throw new Error('not used'); }
	export async function getCertificateCampaignPurgeStatus() { throw new Error('not used'); }
	export async function retryCertificateCampaignPurge() { throw new Error('not used'); }
`;

const stubModules = new Map([
	['$app/navigation', 'export async function goto() {}'],
	[
		'$app/paths',
		"export const resolve = (value) => value; export const base = ''; export const assets = '';"
	],
	[
		'$app/state',
		"export const page = { params: { campaignId: '10000000-0000-4000-8000-000000000001' } };"
	],
	['$lib/api/certificates', certificateApiStub],
	['$lib/api/client', 'export class ApiClientError extends Error {}'],
	['$lib/api/lookup', 'export async function lookupAcademicYears() { return []; }']
]);

function findStubModule(id: string): string | undefined {
	if (stubModules.has(id)) return id;
	for (const stubId of stubModules.keys()) {
		if (!stubId.startsWith('$lib/')) continue;
		const resolvedPath = path.resolve(frontendRoot, 'src/lib', stubId.slice('$lib/'.length));
		if (id === resolvedPath || id === `${resolvedPath}.ts` || id === `${resolvedPath}.js`) {
			return stubId;
		}
	}
}

function harnessPlugin(): Plugin {
	return {
		name: 'certificate-campaign-display-test-harness',
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
				import { setPermissions } from '/src/lib/stores/permissions.ts';
				import { campaignFixture } from '$lib/api/certificates';
				import CertificateCampaignList from '/src/lib/components/certificates/CertificateCampaignList.svelte';
				import CampaignOverview from '/src/routes/(app)/staff/certificates/[campaignId]/overview/+page.svelte';

				setPermissions(['*']);
				const component = new URL(window.location.href).searchParams.get('view') === 'list'
					? CertificateCampaignList
					: CampaignOverview;
				const props = component === CertificateCampaignList
					? { campaigns: [campaignFixture] }
					: {};
				mount(component, { target: document.getElementById('app'), props });
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
			`node_modules/.vite-certificate-campaign-display-${browserName}-${testInfo.workerIndex}`
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

test('campaign overview renders the academic year display name exactly once', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}`);

	await expect(page.getByText('ปีการศึกษา 2569 · 7 สิงหาคม 2569', { exact: true })).toBeVisible();
	await expect(page.getByText(/ปีการศึกษา ปีการศึกษา 2569/)).toHaveCount(0);
});

test('campaign list and year filter render the academic year display name exactly once', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}?view=list`);

	const schedule = page.locator('article span').filter({ hasText: '7 ส.ค. 2569' });
	await expect(schedule).toHaveText('ปีการศึกษา 2569 · 7 ส.ค. 2569');
	await expect(page.getByText(/ปีการศึกษา ปีการศึกษา 2569/)).toHaveCount(0);

	await page.getByLabel('กรองตามปีการศึกษา').click();
	await expect(page.getByRole('option', { name: 'ปีการศึกษา 2569', exact: true })).toBeVisible();
	await expect(page.getByText(/^ปีการศึกษา ปีการศึกษา 2569$/)).toHaveCount(0);
});
