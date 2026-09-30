import type { Page, Route } from '@playwright/test';
import type {
	CertificateCandidateDetail,
	CertificateTemplateDetail,
	CertificateRenderManifest
} from '../../../src/lib/api/certificates';
import {
	mockCampaigns,
	campaignId,
	secondCampaignId,
	campaignsPath
} from './certificate-campaign-route-data';
import { id } from './staff-home-route-data';
export { campaignId, secondCampaignId };
export const templateId = id(290),
	templatePath = `/api/certificates/templates/${templateId}`;
export const campaignPath = `${campaignsPath}/${campaignId}`,
	templatesPath = `${campaignPath}/templates`,
	candidatesPath = `${campaignPath}/candidates`;
export const recipientRoute = (selected = campaignId) =>
	`/staff/certificates/${selected}/recipients`;
export const templateRoute = `/staff/certificates/${campaignId}/templates`,
	editorRoute = `${templateRoute}/${templateId}/editor`;
export async function mockPreparation(
	page: Page,
	options: { hold?: string; holdAt?: number; fail?: string; failAt?: number; reader?: boolean } = {}
) {
	const base = await mockCampaigns(page, {
		permissions: options.reader
			? ['certificate.read.school']
			: [
					'certificate.read.school',
					'certificate.create.school',
					'certificate.update.school',
					'certificate.delete.school',
					'certificate.submit.school'
				]
	});
	const timestamp = '2026-10-01T00:00:00Z';
	const template: CertificateTemplateDetail = {
		id: templateId,
		campaignId,
		name: 'แบบแรก',
		allowedRecipientTypes: ['external', 'student'],
		assets: [],
		backgroundFileId: id(291),
		capabilities: {
			canRead: true,
			canUpdate: !options.reader,
			canDelete: !options.reader,
			canPreview: true
		},
		createdAt: timestamp,
		updatedAt: timestamp,
		isActive: true,
		isReady: true,
		issuedCertificateCount: 0,
		missingVariableCertificateCount: 0,
		layout: { schemaVersion: 1, elements: [] },
		pageGeometry: null,
		safeMarginPoints: 28,
		showSafeArea: true
	};
	const candidate: CertificateCandidateDetail = {
		id: id(292),
		campaignId,
		batchId: null,
		accountFirstName: null,
		accountLastName: null,
		accountTitle: null,
		importedFirstName: 'ผู้รับ',
		importedLastName: 'คนแรก',
		importedTitle: null,
		activityItem: 'กิจกรรม',
		awardOrRole: 'ผู้ร่วม',
		customValues: {},
		deletedAt: null,
		duplicateConfirmed: false,
		createdAt: timestamp,
		updatedAt: timestamp,
		matchedUserId: null,
		matchStatus: 'not_applicable',
		recipientType: 'external',
		selectedNameSource: 'file',
		staffUsername: null,
		studentId: null,
		templateId,
		templateName: template.name,
		validationCodes: [],
		validationStatus: 'ready',
		capabilities: {
			canChooseName: false,
			canConfirmDuplicate: false,
			canConfirmExternal: false,
			canDelete: !options.reader,
			canUpdate: !options.reader
		}
	};
	let candidates = [candidate];
	const reads: string[] = [],
		writes: string[] = [],
		counts = new Map<string, number>();
	let release = () => {};
	const held = new Promise<void>((r) => (release = r));
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	await page.route(
		(url) => url.pathname.startsWith('/api/certificates/'),
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method(),
				kind = method === 'GET' ? path : 'mutation';
			const ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (method === 'GET') reads.push(path);
			else writes.push(path);
			const snapshot = structuredClone(candidates);
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, 'ส่วนนี้ไม่พร้อม', 503);
			if (path === campaignPath || path === `${campaignsPath}/${secondCampaignId}`)
				return route.fallback();
			if (method === 'GET' && path.endsWith('/candidates'))
				return reply(route, {
					items: snapshot.filter(
						(c) =>
							(!url.searchParams.has('status') ||
								c.validationStatus === url.searchParams.get('status')) &&
							(!url.searchParams.has('search') ||
								`${c.importedFirstName} ${c.importedLastName}`.includes(
									url.searchParams.get('search')!
								))
					),
					summary: {
						totalCount: snapshot.length,
						readyCount: snapshot.length,
						reviewCount: 0,
						invalidCount: 0
					}
				});
			if (method === 'GET' && path.endsWith('/templates') && path.includes('/campaigns/'))
				return reply(route, [template]);
			if (method === 'GET' && path === templatePath) return reply(route, template);
			if (method === 'GET' && path === `${templatePath}/variables`)
				return reply(route, { variables: ['recipientName'] });
			if (method === 'GET' && path === `${templatePath}/fonts`) return reply(route, { items: [] });
			if (method === 'PUT' && path === templatePath) {
				Object.assign(template, route.request().postDataJSON(), {
					updatedAt: '2026-10-02T00:00:00Z'
				});
				return reply(route, template);
			}
			if (method === 'DELETE' && path === `/api/certificates/candidates/${candidate.id}`) {
				candidates = [];
				return reply(route, { ...candidate, deletedAt: timestamp });
			}
			if (method === 'DELETE' && path === templatePath)
				return reply(route, { disposition: 'deleted', detachedFileCount: 0 });
			if (method === 'POST' && path.endsWith('/preview-manifest')) {
				const manifest: CertificateRenderManifest = {
					templateId,
					certificateNumber: 'PREVIEW',
					suggestedFilename: 'synthetic.pdf',
					layout: template.layout,
					pageGeometry: {
						paperLabel: 'A4',
						rotation: 0,
						displayedWidthPoints: 842,
						displayedHeightPoints: 595,
						mediaBox: { xPoints: 0, yPoints: 0, widthPoints: 842, heightPoints: 595 },
						cropBox: { xPoints: 0, yPoints: 0, widthPoints: 842, heightPoints: 595 }
					},
					backgroundGrant: {
						fileId: id(291),
						url: '/synthetic-background.pdf',
						expiresAt: '2099-01-01T00:00:00Z'
					},
					fontGrants: [],
					imageGrants: [],
					builtInFonts: [],
					qrPayload: 'synthetic',
					recipientValues: {},
					campaignValues: {
						academicYear: '2569',
						campaignName: 'กิจกรรมแรก',
						eventDate: '2026-10-01',
						issueDate: '2026-10-01',
						ownerOrganizationUnitName: 'โรงเรียน',
						schoolName: 'โรงเรียนทดสอบ'
					}
				};
				return reply(route, manifest);
			}
			return reply(route, 'unmocked preparation', 404);
		}
	);
	return { ...base, reads, writes, release, count: (p: string) => counts.get(p) ?? 0 };
}
