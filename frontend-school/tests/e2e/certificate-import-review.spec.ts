import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__certificate-recipient-test';
const virtualModuleId = 'virtual:certificate-recipient-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubPrefix = '\0certificate-recipient-stub:';

const navigationStub = `
	export const afterNavigate = (callback) => { queueMicrotask(callback); };
`;

const apiClientStub = `
	export class ApiClientError extends Error {
		constructor(message, status, retryAfterSeconds) {
			super(message);
			this.name = 'ApiClientError';
			this.status = status;
			this.retryAfterSeconds = retryAfterSeconds;
		}
	}
`;

const certificateApiStub = `
	import { ApiClientError } from '$lib/api/client';

	export async function getCertificateCampaign(id) {
		return window.__certificateRecipientApi.getCampaign(id);
	}
	export async function listCertificateTemplates(id) {
		return window.__certificateRecipientApi.listTemplates(id);
	}
	export async function listCertificateCandidates(id, query) {
		return window.__certificateRecipientApi.listCandidates(id, query);
	}
	export async function importCertificateCandidates(id, payload) {
		return window.__certificateRecipientApi.importCandidates(id, payload);
	}
	export async function bulkUpdateCertificateCandidates(id, payload) {
		return window.__certificateRecipientApi.bulkUpdate(id, payload, ApiClientError);
	}
	export async function updateCertificateCandidate(id, payload) {
		return window.__certificateRecipientApi.updateCandidate(id, payload);
	}
	export async function deleteCertificateCandidate(id) {
		return window.__certificateRecipientApi.deleteCandidate(id);
	}
	export async function searchCertificateCandidateAccounts(id, query) {
		return window.__certificateRecipientApi.searchAccounts(id, query);
	}
	export async function createAccountCertificateCandidate(id, payload) {
		return window.__certificateRecipientApi.createFromAccount(id, payload);
	}
	export async function createManualCertificateCandidate(id, payload) {
		return window.__certificateRecipientApi.createManual(id, payload);
	}
`;

const stubModules = new Map([
	['$app/navigation', navigationStub],
	['$lib/api/client', apiClientStub],
	['$lib/api/certificates', certificateApiStub]
]);

function findStubModule(id: string): string | undefined {
	if (stubModules.has(id)) return id;
	if (id.endsWith('/@sveltejs/kit/src/runtime/app/navigation.js')) return '$app/navigation';
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
		name: 'certificate-recipient-test-harness',
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
				import CertificateRecipientWorkspace from '/src/lib/components/certificates/CertificateRecipientWorkspace.svelte';

				const campaignId = '10000000-0000-4000-8000-000000000001';
				const timestamp = '2026-08-14T00:00:00Z';
				const campaign = {
					id: campaignId,
					name: 'กิจกรรมวันภาษาไทย',
					academicYearId: '20000000-0000-4000-8000-000000000001',
					academicYearName: '2569',
					academicYearValue: 2569,
					activitySequence: null,
					candidateCount: 3,
					createdAt: timestamp,
					createdBy: null,
					eventDate: '2026-07-29',
					hasOpenIssueRequest: true,
					issuedCertificateCount: 0,
					nextCertificateSequence: 1,
					ownerOrganizationUnitCode: 'THAI',
					ownerOrganizationUnitId: '30000000-0000-4000-8000-000000000001',
					ownerOrganizationUnitName: 'กลุ่มสาระภาษาไทย',
					status: 'draft',
					templateCount: 2,
					updatedAt: timestamp,
					capabilities: {
						canRead: true,
						canUpdate: false,
						canPrepareCandidates: true,
						canDelete: true,
						canSubmit: true,
						canChangeStatus: true,
						canDownload: false,
						canManageTemplates: true
					}
				};
				const templates = [
					{
						id: '40000000-0000-4000-8000-000000000001',
						campaignId,
						name: 'แบบการแข่งขัน',
						allowedRecipientTypes: ['student', 'external'],
						isActive: true,
						isReady: true,
						issuedCertificateCount: 0,
						missingVariableCertificateCount: 0,
						backgroundFileId: null,
						assets: [],
						layout: null,
						pageGeometry: null,
						safeMarginPoints: 28,
						showSafeArea: true,
						createdAt: timestamp,
						updatedAt: timestamp,
						capabilities: { canRead: true, canUpdate: true, canDelete: true, canPreview: true }
					},
					{
						id: '40000000-0000-4000-8000-000000000002',
						campaignId,
						name: 'แบบบุคลากร',
						allowedRecipientTypes: ['staff'],
						isActive: true,
						isReady: true,
						issuedCertificateCount: 0,
						missingVariableCertificateCount: 0,
						backgroundFileId: null,
						assets: [],
						layout: null,
						pageGeometry: null,
						safeMarginPoints: 28,
						showSafeArea: true,
						createdAt: timestamp,
						updatedAt: timestamp,
						capabilities: { canRead: true, canUpdate: true, canDelete: true, canPreview: true }
					}
				];
				function candidate(overrides) {
					return {
						id: crypto.randomUUID(),
						campaignId,
						batchId: null,
						recipientType: 'external',
						studentId: null,
						staffUsername: null,
						importedTitle: 'คุณ',
						importedFirstName: 'ตัวอย่าง',
						importedLastName: 'ผู้รับ',
						accountTitle: null,
						accountFirstName: null,
						accountLastName: null,
						matchedUserId: null,
						matchStatus: 'not_applicable',
						selectedNameSource: 'file',
						activityItem: 'การแข่งขันคำคม',
						awardOrRole: 'ผู้เข้าร่วม',
						templateId: templates[0].id,
						templateName: templates[0].name,
						customValues: {},
						validationStatus: 'ready',
						validationCodes: [],
						duplicateConfirmed: false,
						deletedAt: null,
						createdAt: timestamp,
						updatedAt: timestamp,
						capabilities: {
							canChooseName: false,
							canConfirmDuplicate: false,
							canConfirmExternal: false,
							canDelete: true,
							canUpdate: true
						},
						...overrides
					};
				}
				let candidates = [
					candidate({
						id: '50000000-0000-4000-8000-000000000001',
						recipientType: 'student',
						studentId: '0069',
						importedTitle: 'เด็กหญิง',
						importedFirstName: 'กมลชนก',
						importedLastName: 'ใจดี',
						accountTitle: 'เด็กหญิง',
						accountFirstName: 'กมลชนก',
						accountLastName: 'ใจดี',
						matchedUserId: '60000000-0000-4000-8000-000000000001',
						matchStatus: 'matched'
					}),
					candidate({
						id: '50000000-0000-4000-8000-000000000002',
						recipientType: 'student',
						studentId: '9999',
						importedFirstName: 'นอกโรงเรียน',
						importedLastName: 'ทดสอบ',
						matchStatus: 'not_found',
						selectedNameSource: null,
						validationStatus: 'needs_review',
						validationCodes: ['account_not_found'],
						capabilities: {
							canChooseName: false,
							canConfirmDuplicate: false,
							canConfirmExternal: true,
							canDelete: true,
							canUpdate: true
						}
					}),
					candidate({
						id: '50000000-0000-4000-8000-000000000003',
						recipientType: 'staff',
						staffUsername: 'inactive.staff',
						importedFirstName: 'บัญชี',
						importedLastName: 'ปิดใช้',
						matchedUserId: '60000000-0000-4000-8000-000000000003',
						matchStatus: 'inactive',
						selectedNameSource: null,
						validationStatus: 'invalid',
						validationCodes: ['account_inactive'],
						capabilities: {
							canChooseName: false,
							canConfirmDuplicate: false,
							canConfirmExternal: false,
							canDelete: true,
							canUpdate: true
						}
					})
				];
				const bulkPayloads = [];
				const importPayloads = [];
				const apiCalls = [];
				const accountSearches = [];
				let rejectNextExternalConfirmation = false;
				function summary(items) {
					return {
						totalCount: items.length,
						readyCount: items.filter((item) => item.validationStatus === 'ready').length,
						reviewCount: items.filter((item) => item.validationStatus === 'needs_review').length,
						invalidCount: items.filter((item) => item.validationStatus === 'invalid').length
					};
				}
				window.__certificateRecipientApi = {
					async getCampaign() { apiCalls.push('campaign'); return structuredClone(campaign); },
					async listTemplates() { apiCalls.push('templates'); return structuredClone(templates); },
					async listCandidates(id, query) {
						apiCalls.push('candidates');
						const search = (query?.search ?? '').toLocaleLowerCase('th');
						const items = candidates.filter((item) =>
							(!query?.status || item.validationStatus === query.status) &&
							(!search || (item.importedFirstName + ' ' + item.importedLastName).toLocaleLowerCase('th').includes(search))
						);
						return { items: structuredClone(items), summary: summary(candidates) };
					},
					async importCandidates(id, payload) {
						importPayloads.push(structuredClone(payload));
						const added = payload.rows.map((row) => candidate({
							id: crypto.randomUUID(),
							recipientType: row.recipientType,
							importedTitle: row.title,
							importedFirstName: row.firstName,
							importedLastName: row.lastName,
							activityItem: row.activityItem,
							awardOrRole: row.awardOrRole
						}));
						candidates = [...added, ...candidates];
						return {
							batch: {
								id: crypto.randomUUID(), campaignId, source: payload.source,
								rowCount: added.length, readyCount: added.length, reviewCount: 0,
								invalidCount: 0, customHeaders: [], createdAt: timestamp
							},
							candidates: structuredClone(added)
						};
					},
					async bulkUpdate(id, payload, ApiClientError) {
						bulkPayloads.push(structuredClone(payload));
						if (payload.operation === 'confirm_external' && rejectNextExternalConfirmation) {
							rejectNextExternalConfirmation = false;
							throw new ApiClientError('พบบัญชีที่ตรงกับรหัสแล้ว กรุณาตรวจสอบรายการอีกครั้ง', 409);
						}
						if (payload.operation === 'confirm_external') {
							candidates = candidates.map((item) => payload.candidateIds.includes(item.id)
								? candidate({ ...item, recipientType: 'external', studentId: null,
									matchStatus: 'external_confirmed', selectedNameSource: 'file',
									validationStatus: 'ready', validationCodes: [],
									capabilities: { ...item.capabilities, canConfirmExternal: false } })
								: item);
						}
						return { updatedCount: payload.candidateIds.length,
							candidates: structuredClone(candidates.filter((item) => payload.candidateIds.includes(item.id))) };
					},
					async updateCandidate(id, payload) {
						const current = candidates.find((item) => item.id === id);
						const updated = candidate({ ...current, ...payload, updatedAt: '2026-08-14T00:00:01Z' });
						candidates = candidates.map((item) => item.id === id ? updated : item);
						return structuredClone(updated);
					},
					async deleteCandidate(id) {
						const current = candidates.find((item) => item.id === id);
						candidates = candidates.filter((item) => item.id !== id);
						return structuredClone({ ...current, deletedAt: timestamp });
					},
					async searchAccounts(id, query) {
						return new Promise((resolve) => accountSearches.push({ query: structuredClone(query), resolve }));
					},
					async createFromAccount() { throw new Error('not used'); },
					async createManual() { throw new Error('not used'); }
				};
				window.certificateRecipientHarness = {
					bulkPayloads: () => structuredClone(bulkPayloads),
					importPayloads: () => structuredClone(importPayloads),
					apiCalls: () => [...apiCalls],
					accountSearchQueries: () => accountSearches.map((search) => structuredClone(search.query)),
					resolveAccountSearch(index, accounts) { accountSearches[index]?.resolve(structuredClone(accounts)); },
					rejectNextExternalConfirmation() { rejectNextExternalConfirmation = true; }
				};

				mount(CertificateRecipientWorkspace, {
					target: document.getElementById('app'),
					props: {
						campaignId,
						canReadCandidates: true
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
			`node_modules/.vite-certificate-recipient-test-${browserName}-${testInfo.workerIndex}`
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

test('reviews imported recipients without sending source files to the backend', async ({
	page
}) => {
	const pageErrors: string[] = [];
	page.on('pageerror', (error) => pageErrors.push(error.message));
	await page.goto(`${baseUrl}${harnessPath}`);
	await expect
		.poll(() => page.evaluate(() => window.certificateRecipientHarness.apiCalls()))
		.toEqual(['campaign', 'templates', 'candidates']);
	expect(pageErrors).toEqual([]);

	await expect(page.getByRole('heading', { name: 'ตรวจรายชื่อผู้รับ' })).toBeVisible();
	await expect(page.getByText('พร้อมออก').first()).toBeVisible();
	await expect(page.getByText('ต้องตรวจสอบ').first()).toBeVisible();
	await expect(page.getByText('ข้อมูลไม่ถูกต้อง').first()).toBeVisible();
	await expect(page.getByText('บัญชีที่พบแล้วไม่สามารถเปลี่ยนเป็นบุคคลภายนอกได้')).toBeVisible();

	const matchedRow = page.getByRole('checkbox', { name: 'เลือกรายการ กมลชนก ใจดี' });
	const unmatchedRow = page.getByRole('checkbox', { name: 'เลือกรายการ นอกโรงเรียน ทดสอบ' });
	const confirmExternal = page.getByRole('button', { name: 'ยืนยันเป็นบุคคลภายนอก' });
	await matchedRow.click();
	await unmatchedRow.click();
	await expect(confirmExternal).toBeDisabled();
	await matchedRow.click();
	await expect(confirmExternal).toBeEnabled();
	await confirmExternal.click();
	await expect(page.getByText('นอกโรงเรียน ทดสอบ')).toBeVisible();
	await expect
		.poll(() => page.evaluate(() => window.certificateRecipientHarness.bulkPayloads().at(-1)))
		.toEqual({
			candidateIds: ['50000000-0000-4000-8000-000000000002'],
			operation: 'confirm_external'
		});

	await page.getByRole('button', { name: 'นำเข้า Excel/CSV' }).click();
	await page.getByLabel('เลือกไฟล์รายชื่อ').setInputFiles({
		name: 'recipients.csv',
		mimeType: 'text/csv',
		buffer: Buffer.from(
			'ประเภทผู้รับ,รหัสนักเรียน,ชื่อผู้ใช้บุคลากร,คำนำหน้า,ชื่อ,นามสกุล,รายการกิจกรรม,รางวัลหรือบทบาท,แบบเกียรติบัตร\nบุคคลภายนอก,,,คุณ,ผู้รับใหม่,ทดสอบ,การแข่งขันคำคม,รองชนะเลิศอันดับที่ 1,แบบการแข่งขัน',
			'utf8'
		)
	});
	await expect(page.getByText('พร้อมนำเข้า 1 รายการ')).toBeVisible();
	await page.getByRole('button', { name: 'นำเข้า 1 รายการ' }).click();
	await expect(page.getByText('ผู้รับใหม่ ทดสอบ')).toBeVisible();

	const importPayload = await page.evaluate(() =>
		window.certificateRecipientHarness.importPayloads().at(-1)
	);
	expect(importPayload).toMatchObject({
		source: 'csv',
		rows: [{ firstName: 'ผู้รับใหม่', lastName: 'ทดสอบ', recipientType: 'external' }]
	});
	expect(importPayload).not.toHaveProperty('file');
	expect(importPayload).not.toHaveProperty('fileName');
});

test('account search ignores a stale recipient-type response', async ({ page }) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	await expect(page.getByText('พร้อมออก').first()).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่มจากบัญชี' }).click();
	await page.getByLabel('ชื่อ รหัสนักเรียน หรือชื่อผู้ใช้').fill('กมล');
	await page.getByRole('button', { name: 'ค้นหา', exact: true }).click();
	await expect
		.poll(() => page.evaluate(() => window.certificateRecipientHarness.accountSearchQueries()))
		.toEqual([{ recipientType: 'student', search: 'กมล' }]);
	await page.getByLabel('ประเภทบัญชี').selectOption('staff');
	await page.evaluate(() =>
		window.certificateRecipientHarness.resolveAccountSearch(0, [
			{
				userId: '70000000-0000-4000-8000-000000000001',
				recipientType: 'student',
				studentId: '0069',
				staffUsername: null,
				title: 'เด็กหญิง',
				firstName: 'ผลลัพธ์เก่า',
				lastName: 'นักเรียน'
			}
		])
	);
	await expect(page.getByText('ผลลัพธ์เก่า นักเรียน')).not.toBeVisible();
});

test('active request still allows an unlocked candidate edit without changing recipient type', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	await expect(page.getByText('พร้อมออก').first()).toBeVisible();
	await page.getByRole('button', { name: 'แก้ไข เด็กหญิงกมลชนก ใจดี' }).click();
	await expect(page.getByRole('heading', { name: 'แก้ไขรายชื่อ' })).toBeVisible();
	await expect(page.getByLabel('ประเภทผู้รับ')).toBeDisabled();
});

test('external confirmation conflict stays visible and disables repeated confirmation', async ({
	page
}) => {
	await page.goto(`${baseUrl}${harnessPath}`);
	await expect(page.getByText('พร้อมออก').first()).toBeVisible();
	await page.evaluate(() => window.certificateRecipientHarness.rejectNextExternalConfirmation());
	await page.getByRole('checkbox', { name: 'เลือกรายการ นอกโรงเรียน ทดสอบ' }).click();
	await page.getByRole('button', { name: 'ยืนยันเป็นบุคคลภายนอก' }).click();

	await expect(page.getByText('พบบัญชีที่ตรงกับรหัสแล้ว กรุณาตรวจสอบรายการอีกครั้ง')).toBeVisible();
	await expect(
		page.getByRole('button', { name: 'ยืนยัน นอกโรงเรียน ทดสอบ เป็นบุคคลภายนอก' })
	).toHaveCount(0);
});

declare global {
	interface Window {
		certificateRecipientHarness: {
			bulkPayloads(): Array<{ candidateIds: string[]; operation: string }>;
			apiCalls(): string[];
			accountSearchQueries(): Array<{ recipientType: string; search: string }>;
			resolveAccountSearch(
				index: number,
				accounts: Array<{
					userId: string;
					recipientType: string;
					studentId: string | null;
					staffUsername: string | null;
					title: string | null;
					firstName: string;
					lastName: string;
				}>
			): void;
			rejectNextExternalConfirmation(): void;
			importPayloads(): Array<{
				source: string;
				rows: Array<{ firstName: string; lastName: string; recipientType: string }>;
			}>;
		};
	}
}
