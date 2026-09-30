import type { Page, Route } from '@playwright/test';
import type { CertificateCampaignDetail } from '../../../src/lib/api/certificates';
import { mockStaffHome, id, actor, year } from './staff-home-route-data';
export const campaignId = id(250),
	secondCampaignId = id(251);
export const campaignsPath = '/api/certificates/campaigns',
	ownersPath = '/api/certificates/owner-options',
	yearsPath = '/api/lookup/academic-years';
export const overviewPath = (selected = campaignId) => `/staff/certificates/${selected}/overview`;
export async function mockCampaigns(
	page: Page,
	options: {
		hold?: string;
		holdAt?: number;
		fail?: string;
		failAt?: number;
		permissions?: string[];
		issued?: boolean;
	} = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? [
			'certificate.read.school',
			'certificate.create.school',
			'certificate.update.school',
			'certificate.delete.school'
		]
	});
	const timestamp = '2026-10-01T00:00:00Z';
	const records: CertificateCampaignDetail[] = [campaignId, secondCampaignId].map((id, index) => ({
		id,
		academicYearId: year,
		academicYearValue: 2569,
		academicYearName: 'ปีการศึกษา 2569',
		ownerOrganizationUnitId: null,
		ownerOrganizationUnitCode: null,
		ownerOrganizationUnitName: null,
		name: index ? 'กิจกรรมที่สอง' : 'กิจกรรมแรก',
		eventDate: '2026-10-01',
		status: 'active',
		activitySequence: options.issued ? 1 : null,
		nextCertificateSequence: 1,
		templateCount: 0,
		candidateCount: 0,
		issuedCertificateCount: 0,
		hasOpenIssueRequest: false,
		createdBy: actor,
		updatedBy: actor,
		createdAt: timestamp,
		updatedAt: timestamp,
		capabilities: {
			canRead: true,
			canUpdate: true,
			canPrepareCandidates: true,
			canDelete: true,
			canSubmit: true,
			canDownload: true,
			canChangeStatus: true,
			canManageTemplates: true
		}
	}));
	const reads: string[] = [],
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
		(url) => url.pathname.startsWith('/api/certificates/') || url.pathname === yearsPath,
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method(),
				kind = method === 'GET' ? path : 'mutation';
			const ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (method === 'GET') reads.push(path);
			else writes.push(path);
			const selected = records.find((x) => x.id === path.split('/')[4]),
				snapshot = selected ? { ...selected } : records.map((x) => ({ ...x }));
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, 'ส่วนนี้ไม่พร้อม', 503);
			if (path === yearsPath)
				return reply(route, [{ id: year, year: 2569, name: 'ปีการศึกษา 2569', status: 'active' }]);
			if (path === ownersPath)
				return reply(route, [
					{ id: id(252), code: 'ACADEMIC', name: 'ฝ่ายวิชาการ', display_order: 0, is_active: true }
				]);
			if (method === 'GET' && path === campaignsPath) return reply(route, snapshot);
			if (method === 'GET' && selected && path === `${campaignsPath}/${selected.id}`)
				return reply(route, snapshot);
			if (method === 'POST' && path === campaignsPath) {
				const payload = route.request().postDataJSON();
				const saved = { ...records[0], ...payload, id: id(253) };
				records.push(saved);
				return reply(route, saved);
			}
			if (method === 'PUT' && selected) {
				const payload = route.request().postDataJSON();
				Object.assign(selected, {
					name: payload.name ?? selected.name,
					status: payload.status ?? selected.status,
					eventDate: payload.eventDate ?? selected.eventDate,
					updatedAt: '2026-10-02T00:00:00Z'
				});
				return reply(route, selected);
			}
			return reply(route, 'unmocked certificate workflow', 404);
		}
	);
	return { ...base, reads, writes, release, count: (path: string) => counts.get(path) ?? 0 };
}
