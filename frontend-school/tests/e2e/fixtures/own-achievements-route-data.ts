import type { Page, Route } from '@playwright/test';
import { mockStaffHome, id, actor, year } from './staff-home-route-data';
import type { Achievement } from '../../../src/lib/types/achievement';
import type { IssuedCertificateSummary } from '../../../src/lib/api/certificates';
export const achievementPath = '/api/achievements',
	ownCertificatePath = '/api/me/certificates',
	lookupPath = '/api/lookup/staff';
export async function mockOwnAchievements(
	page: Page,
	options: {
		hold?: string;
		holdAt?: number;
		fail?: string;
		failAt?: number;
		permissions?: string[];
		userType?: string;
	} = {}
) {
	const base = await mockStaffHome(page, {
		permissions: options.permissions ?? [
			'achievement.read.own',
			'achievement.read.all',
			'achievement.create.own',
			'achievement.create.all',
			'achievement.update.own',
			'achievement.delete.own',
			'certificate.read.own'
		],
		userType: options.userType
	});
	const timestamp = '2026-10-01T00:00:00Z';
	let achievements: Achievement[] = [actor, id(231)].map((user_id, index) => ({
		id: id(232 + index),
		user_id,
		title: index ? 'ผลงานบุคลากรอื่น' : 'ผลงานของฉัน',
		description: 'ข้อมูลสังเคราะห์',
		achievement_date: '2026-10-01',
		image_file_id: null,
		created_by: actor,
		created_at: timestamp,
		updated_at: timestamp,
		user_first_name: 'ทดสอบ',
		user_last_name: 'รายการ',
		user_profile_image_file_id: null
	}));
	const certificate: IssuedCertificateSummary = {
		id: id(234),
		campaignId: id(235),
		campaignName: 'เกียรติบัตรของบัญชี',
		templateId: id(236),
		templateName: 'แบบทดสอบ',
		ownerOrganizationUnitId: null,
		ownerOrganizationUnitName: null,
		academicYearId: year,
		academicYearValue: 2569,
		activitySequence: 1,
		certificateSequence: 1,
		certificateNumber: '2569-0001-000001-0',
		recipientType: options.userType === 'student' ? 'student' : 'staff',
		title: null,
		firstName: 'ทดสอบ',
		lastName: 'บัญชี',
		issueDate: '2026-10-01',
		activityItem: null,
		awardOrRole: null,
		status: 'issued',
		replacementForCertificateId: null,
		replacedByCertificateId: null,
		replacementCandidateId: null,
		createdAt: timestamp,
		capabilities: { canRead: true, canDownload: true, canRevoke: false }
	};
	const counts = new Map<string, number>(),
		reads: URL[] = [],
		writes: string[] = [];
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const reply = (route: Route, data: unknown, status = 200) =>
		route.fulfill({
			status,
			json: status < 400 ? { success: true, data } : { success: false, error: data }
		});
	await page.route(
		(url) =>
			url.pathname.startsWith(achievementPath) ||
			url.pathname.startsWith(ownCertificatePath) ||
			url.pathname === lookupPath,
		async (route) => {
			const url = new URL(route.request().url()),
				path = url.pathname,
				method = route.request().method(),
				kind = method === 'GET' ? path : 'mutation',
				ordinal = (counts.get(kind) ?? 0) + 1;
			counts.set(kind, ordinal);
			if (method === 'GET') reads.push(url);
			else writes.push(path);
			const snapshot = achievements
				.filter(
					(record) =>
						!url.searchParams.get('user_id') || record.user_id === url.searchParams.get('user_id')
				)
				.map((x) => ({ ...x }));
			if (options.hold === kind && ordinal === (options.holdAt ?? 1)) await held;
			if (options.fail === kind && ordinal === (options.failAt ?? 1))
				return reply(route, 'รายการนี้ไม่พร้อม', 503);
			if (path === lookupPath)
				return reply(route, [
					{ id: actor, name: 'บัญชีของฉัน' },
					{ id: id(231), name: 'บุคลากรอื่น' }
				]);
			if (path === ownCertificatePath) return reply(route, [certificate]);
			if (method === 'GET') return reply(route, snapshot);
			const resourceId = path.split('/').at(-1);
			if (method === 'DELETE') {
				achievements = achievements.filter((x) => x.id !== resourceId);
				return reply(route, {});
			}
			const payload = route.request().postDataJSON();
			const saved: Achievement = {
				...achievements.find((x) => x.id === resourceId),
				...payload,
				id: method === 'POST' ? id(237) : resourceId!,
				user_id: payload.user_id ?? actor,
				image_file_id: payload.image_file_id ?? null,
				description: payload.description ?? null,
				created_by: actor,
				created_at: timestamp,
				updated_at: timestamp,
				user_first_name: 'ทดสอบ',
				user_last_name: 'รายการ',
				user_profile_image_file_id: null
			};
			achievements = [...achievements.filter((x) => x.id !== saved.id), saved];
			return reply(route, saved);
		}
	);
	return { ...base, reads, writes, release, count: (path: string) => counts.get(path) ?? 0 };
}
