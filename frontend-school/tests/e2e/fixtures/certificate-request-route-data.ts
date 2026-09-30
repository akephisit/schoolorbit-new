import type { Page, Route } from '@playwright/test';
import type {
	CertificateIssueRequestDetail,
	IssuedCertificateSummary,
	CertificateRenderManifest
} from '../../../src/lib/api/certificates';
import { mockCampaigns, campaignId, secondCampaignId } from './certificate-campaign-route-data';
import { id, actor, year } from './staff-home-route-data';
export const queuePath = '/api/certificates/issue-requests',
	requestId = id(260),
	secondRequestId = id(261),
	issuedId = id(262);
export const historyPath = (selected = campaignId) =>
	`/api/certificates/campaigns/${selected}/issue-requests`;
export const issuedPath = (selected = campaignId) =>
	`/api/certificates/campaigns/${selected}/issued`;
export async function mockRequests(
	page: Page,
	options: {
		hold?: string;
		holdAt?: number;
		fail?: string;
		failAt?: number;
		permissions?: string[];
	} = {}
) {
	const base = await mockCampaigns(page, {
		permissions: options.permissions ?? [
			'certificate.read.school',
			'certificate.submit.school',
			'certificate.issue.school',
			'certificate.download.school',
			'certificate.revoke.school'
		]
	});
	const timestamp = '2026-10-01T00:00:00Z';
	const requests: CertificateIssueRequestDetail[] = [requestId, secondRequestId].map(
		(selectedId, index) => ({
			id: selectedId,
			campaignId: index ? secondCampaignId : campaignId,
			campaignName: index ? 'คำขอที่สอง' : 'คำขอแรก',
			ownerOrganizationUnitId: null,
			ownerOrganizationUnitName: null,
			status: 'pending',
			submittedBy: actor,
			submittedByName: 'ผู้ส่งทดสอบ',
			reviewedBy: null,
			reviewedByName: null,
			submittedAt: timestamp,
			reviewedAt: null,
			returnedAt: null,
			withdrawnAt: null,
			issuedAt: null,
			returnNote: null,
			issueCodes: [],
			itemCount: 1,
			templateCount: 1,
			readyCount: 1,
			reviewCount: 0,
			invalidCount: 0,
			createdAt: timestamp,
			updatedAt: timestamp,
			capabilities: { canWithdraw: true, canStartReview: true, canReturn: false, canIssue: false },
			items: [
				{
					candidateId: id(263),
					templateId: id(264),
					templateName: 'แม่แบบทดสอบ',
					recipientType: 'external',
					title: null,
					firstName: 'ผู้รับ',
					lastName: index ? 'รายการสอง' : 'รายการแรก',
					activityItem: null,
					awardOrRole: null,
					validationStatus: 'ready',
					validationCodes: []
				}
			]
		})
	);
	let issued: IssuedCertificateSummary[] = [
		{
			id: issuedId,
			campaignId,
			campaignName: 'กิจกรรมแรก',
			templateId: id(264),
			templateName: 'แม่แบบทดสอบ',
			ownerOrganizationUnitId: null,
			ownerOrganizationUnitName: null,
			academicYearId: year,
			academicYearValue: 2569,
			activitySequence: 1,
			certificateSequence: 1,
			certificateNumber: '2569-0001-000001-0',
			recipientType: 'external',
			title: null,
			firstName: 'ผู้รับ',
			lastName: 'ใบแรก',
			issueDate: '2026-10-01',
			activityItem: null,
			awardOrRole: null,
			status: 'issued',
			replacementForCertificateId: null,
			replacedByCertificateId: null,
			replacementCandidateId: null,
			createdAt: timestamp,
			capabilities: { canRead: true, canDownload: true, canRevoke: true }
		}
	];
	const reads: URL[] = [],
		writes: string[] = [],
		counts = new Map<string, number>();
	let release = () => {};
	const held = new Promise<void>((r) => {
		release = r;
	});
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	await page.route(
		(url) =>
			url.pathname.startsWith(queuePath) ||
			/\/campaigns\/[^/]+\/(issue-requests|issued|render-manifests)$/.test(url.pathname) ||
			url.pathname.startsWith(`/api/certificates/${issuedId}`),
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method(),
				kind =
					method === 'GET'
						? path
						: path.endsWith('render-manifest') || path.endsWith('render-manifests')
							? 'manifest'
							: 'mutation';
			const ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (method === 'GET') reads.push(url);
			else writes.push(path);
			const selected = requests.find((x) => x.id === path.split('/')[4]);
			const snapshot = selected
				? structuredClone(selected)
				: requests
						.filter(
							(x) =>
								(!path.includes('/campaigns/') || path.includes(x.campaignId)) &&
								(!url.searchParams.get('status') || x.status === url.searchParams.get('status'))
						)
						.map((x) => {
							const { items: _items, ...summary } = x;
							return summary;
						});
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, 'ส่วนนี้ไม่พร้อม', 503);
			if (method === 'GET' && path.endsWith('/issued'))
				return reply(route, structuredClone(issued));
			if (method === 'GET' && selected) return reply(route, snapshot);
			if (method === 'GET') return reply(route, snapshot);
			if (selected && method === 'POST') {
				if (path.endsWith('/withdraw'))
					Object.assign(selected, {
						status: 'withdrawn',
						capabilities: {
							canWithdraw: false,
							canStartReview: false,
							canReturn: false,
							canIssue: false
						}
					});
				if (path.endsWith('/review'))
					Object.assign(selected, {
						status: 'reviewing',
						capabilities: {
							canWithdraw: false,
							canStartReview: false,
							canReturn: true,
							canIssue: true
						}
					});
				if (path.endsWith('/return')) {
					const payload = route.request().postDataJSON();
					Object.assign(selected, {
						status: 'returned',
						returnNote: payload.returnNote,
						issueCodes: payload.issueCodes,
						capabilities: {
							canWithdraw: false,
							canStartReview: false,
							canReturn: false,
							canIssue: false
						}
					});
				}
				return reply(route, selected);
			}
			if (path.endsWith('/revoke')) {
				issued = issued.map((x) => ({
					...x,
					status: 'revoked',
					capabilities: { canRead: true, canDownload: false, canRevoke: false }
				}));
				return reply(route, { certificate: issued[0], replacementCandidate: null });
			}
			if (kind === 'manifest') {
				const certificate = issued[0];
				const manifest: CertificateRenderManifest = {
					templateId: certificate.templateId,
					certificateNumber: certificate.certificateNumber,
					suggestedFilename: 'synthetic.pdf',
					layout: { schemaVersion: 1, elements: [] },
					pageGeometry: {
						paperLabel: 'A4',
						rotation: 0,
						displayedWidthPoints: 842,
						displayedHeightPoints: 595,
						mediaBox: { xPoints: 0, yPoints: 0, widthPoints: 842, heightPoints: 595 },
						cropBox: { xPoints: 0, yPoints: 0, widthPoints: 842, heightPoints: 595 }
					},
					backgroundGrant: {
						fileId: id(270),
						url: '/synthetic-background.pdf',
						expiresAt: '2099-01-01T00:00:00Z'
					},
					fontGrants: [],
					imageGrants: [],
					builtInFonts: [],
					qrPayload: 'synthetic-proof',
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
				return reply(route, path.endsWith('render-manifests') ? [manifest] : manifest);
			}
			return reply(route, 'unmocked request workflow', 404);
		}
	);
	return { ...base, reads, writes, release, count: (path: string) => counts.get(path) ?? 0 };
}
