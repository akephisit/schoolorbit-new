import { expect, test, type Page, type Route } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';

test.use({ serviceWorkers: 'block' });
const url = '/staff/academic/promotion/policies';
const actor = '10000000-0000-4000-8000-000000000001';
const source = '20000000-0000-4000-8000-000000000001';
const target = '20000000-0000-4000-8000-000000000002';
const program = '30000000-0000-4000-8000-000000000001';
const curriculum = '40000000-0000-4000-8000-000000000001';
const timestamp = '2026-09-11T12:00:00Z';
const rule = {
	fromGradeLevelId: source,
	fromStudyProgramId: program,
	targetGradeLevelId: target,
	targetStudyProgramId: program,
	successOutcome: 'promote',
	minimumEarnedCredits: '10.50',
	requireNoExceptionalOutcomes: true,
	requireActivitiesPassed: true,
	minimumLearnerLevel: 1
};

function reply(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		headers: { 'X-CSRF-Token': 'synthetic-csrf' },
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

async function mock(
	page: Page,
	level: 'reader' | 'manager' | 'approver' = 'approver',
	progressionAdmin = false
) {
	const writes: Array<{ name: string; rules: Array<typeof rule> }> = [];
	const progressionWrites: components['schemas']['ReplaceGradeProgressionsRequest'][] = [];
	const referenceReads: string[] = [];
	await page.route(
		(request) => request.pathname.startsWith('/api/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname;
			if (path === '/api/auth/me')
				return reply(route, {
					id: actor,
					username: 'E2E-LIFECYCLE-policy',
					firstName: 'วิชาการ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'active',
					profileImageFileId: null,
					primaryRoleName: null,
					permissions: [
						'academic_promotion.read.school',
						...(level === 'reader' ? [] : ['academic_promotion.manage.school']),
						...(level === 'approver' ? ['academic_promotion.approve.school'] : []),
						...(progressionAdmin ? ['academic_year.manage.school'] : [])
					]
				} satisfies components['schemas']['CurrentUserResponse']);
			if (path === '/api/academic/lifecycle/promotion-policies/options') {
				referenceReads.push(path);
				return reply(route, {
					grades: [
						{ id: source, levelType: 'secondary', year: 1, isActive: true },
						{ id: target, levelType: 'secondary', year: 2, isActive: true }
					],
					programs: [
						{
							id: program,
							code: 'SCI',
							name: 'วิทย์–คณิต',
							curriculumId: curriculum,
							curriculumName: 'หลักสูตรสถานศึกษา',
							versionName: '2569',
							status: 'published'
						}
					],
					progressionSet: {
						rowVersion: 1,
						progressions: [
							{
								id: '50000000-0000-4000-8000-000000000001',
								fromGradeLevelId: source,
								toGradeLevelId: target,
								transitionKind: 'promote',
								curriculumId: null,
								isActive: true,
								createdAt: timestamp,
								updatedAt: timestamp
							},
							{
								id: '50000000-0000-4000-8000-000000000002',
								fromGradeLevelId: source,
								toGradeLevelId: null,
								transitionKind: 'graduate',
								curriculumId: null,
								isActive: true,
								createdAt: timestamp,
								updatedAt: timestamp
							}
						]
					}
				});
			}
			if (path === '/api/academic/lifecycle/promotion-policies') {
				if (route.request().method() === 'POST') {
					const input = route.request().postDataJSON();
					writes.push(input);
					return reply(route, {
						id: '60000000-0000-4000-8000-000000000002',
						...input,
						progressionRowVersion: 1,
						reviewedBy: actor,
						reviewedAt: timestamp
					});
				}
				return reply(route, [
					{
						id: '60000000-0000-4000-8000-000000000001',
						name: 'เกณฑ์ที่ยืนยันแล้ว',
						rules: [rule],
						progressionRowVersion: 1,
						reviewedBy: actor,
						reviewedAt: timestamp
					}
				]);
			}
			if (path === '/api/academic/grade-progressions' && route.request().method() === 'PUT') {
				const input: components['schemas']['ReplaceGradeProgressionsRequest'] = route
					.request()
					.postDataJSON();
				progressionWrites.push(input);
				return reply(route, {
					rowVersion: input.rowVersion + 1,
					progressions: input.progressions.map((mapping, index) => ({
						...mapping,
						id: `50000000-0000-4000-8000-${String(index + 10).padStart(12, '0')}`,
						createdAt: timestamp,
						updatedAt: timestamp
					}))
				});
			}
			if (path === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (path === '/api/school/public')
				return reply(route, { schoolName: 'โรงเรียนทดสอบ', logoFileId: null });
			if (path === '/api/menu/user') return reply(route, { groups: [] });
			if (path === '/api/notifications') return reply(route, { items: [], unread_count: 0 });
			if (path === '/api/me/work-items/counts')
				return reply(route, { open: 0, dueSoon: 0, overdue: 0, submitted: 0, closed: 0, total: 0 });
			return reply(route, 'ไม่เปิดใช้ในชุดทดสอบนี้', 403);
		}
	);
	return { writes, referenceReads, progressionWrites };
}

test('policy readers inspect named rules without management-only requests', async ({ page }) => {
	const observed = await mock(page, 'reader');
	await page.goto(url);
	await expect(
		page.getByRole('heading', { name: 'เกณฑ์การเลื่อนชั้น', exact: true })
	).toBeVisible();
	await expect(page.getByText('เกณฑ์ที่ยืนยันแล้ว', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'สร้างเกณฑ์รุ่นใหม่' })).toHaveCount(0);
	expect(observed.referenceReads).toHaveLength(0);
	await page.getByRole('button', { name: 'ดูเกณฑ์' }).click();
	await expect(page.getByText('วิทย์–คณิต', { exact: false }).first()).toBeVisible();
	expect(observed.referenceReads).toHaveLength(1);
	expect(observed.writes).toHaveLength(0);
});

test('policy management alone cannot approve a version', async ({ page }) => {
	await mock(page, 'manager');
	await page.goto(url);
	await expect(page.getByText('เกณฑ์ที่ยืนยันแล้ว', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'สร้างเกณฑ์รุ่นใหม่' })).toHaveCount(0);
});

async function select(page: Page, label: string, option: string) {
	await page.getByRole('button', { name: label, exact: true }).click();
	await page.getByRole('option', { name: option, exact: true }).click();
}

test('policy approval records exact criteria only after explicit confirmation', async ({
	page
}) => {
	const observed = await mock(page);
	await page.goto(url);
	await page.getByRole('button', { name: 'สร้างเกณฑ์รุ่นใหม่' }).click();
	await page.getByLabel('ชื่อเกณฑ์').fill('เกณฑ์ทดสอบรุ่นใหม่');
	await select(page, 'ชั้นต้นทาง', 'ม.1');
	await select(page, 'แผนต้นทาง', 'SCI · วิทย์–คณิต · รุ่น 2569');
	await select(page, 'ชั้นปลายทาง', 'ม.2');
	await select(page, 'แผนปลายทาง', 'SCI · วิทย์–คณิต · รุ่น 2569');
	await page.getByLabel('หน่วยกิตที่ได้ขั้นต่ำของปีนี้').fill('10.50');
	await page.getByRole('button', { name: 'ตรวจทานเกณฑ์' }).click();
	expect(observed.writes).toHaveLength(0);
	await expect(
		page.getByText('การยืนยันนี้ยังไม่ย้ายห้องและไม่สร้างข้อมูลนักเรียนปีใหม่')
	).toBeVisible();
	await page.getByRole('button', { name: 'ยืนยันเกณฑ์รุ่นใหม่', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(observed.writes).toEqual([{ name: 'เกณฑ์ทดสอบรุ่นใหม่', rules: [rule] }]);
	await expect(page.getByText('เกณฑ์ทดสอบรุ่นใหม่', { exact: true })).toBeVisible();
});

test('graduation criteria clear destination fields and mobile dialogs have a close control', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 844 });
	const observed = await mock(page);
	await page.goto(url);
	await page.getByRole('button', { name: 'สร้างเกณฑ์รุ่นใหม่' }).click();
	await select(page, 'ผลที่เสนอเมื่อผ่านเกณฑ์', 'จบการศึกษา');
	await expect(page.getByRole('button', { name: 'ชั้นปลายทาง', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'ตรวจทานเกณฑ์' })).toBeDisabled();
	await page.getByRole('button', { name: 'Close', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveCount(0);
	expect(observed.writes).toHaveLength(0);
});

test('school year managers explicitly update progression configuration without approving a policy', async ({
	page
}) => {
	const observed = await mock(page, 'approver', true);
	await page.goto(url);
	await page.getByRole('button', { name: 'สร้างเกณฑ์รุ่นใหม่' }).click();
	await page.getByRole('button', { name: 'จัดการลำดับชั้น' }).click();
	const dialog = page.getByRole('dialog', { name: 'จัดการลำดับชั้น', exact: true });
	await dialog.getByRole('button', { name: 'ลบกฎ 2', exact: true }).click();
	await dialog.getByRole('button', { name: 'บันทึกลำดับชั้น', exact: true }).click();
	await expect(dialog).toHaveCount(0);
	expect(observed.progressionWrites).toEqual([
		{
			rowVersion: 1,
			progressions: [
				{
					fromGradeLevelId: source,
					toGradeLevelId: target,
					transitionKind: 'promote',
					curriculumId: null,
					isActive: true
				}
			]
		}
	]);
	expect(observed.writes).toHaveLength(0);
});
