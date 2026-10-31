import { sessionState } from './fixtures/session-state';
import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const year = '10000000-0000-4000-8000-000000000001';
const term = '20000000-0000-4000-8000-000000000001';
const group = '60000000-0000-4000-8000-000000000001';
const otherGroup = '60000000-0000-4000-8000-000000000002';

function fulfill(route: Route, data: unknown, status = 200) {
	return route.fulfill({
		status,
		contentType: 'application/json',
		body: JSON.stringify(status < 400 ? { success: true, data } : { success: false, error: data })
	});
}

test('course list and selected workspace render while policy is pending; learner tab stays lazy', async ({
	page
}) => {
	const requests: string[] = [];
	let policyAttempts = 0;
	let releasePolicy!: () => void;
	const policyGate = new Promise<void>((resolve) => (releasePolicy = resolve));
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			requests.push(url.pathname);
			if (url.pathname === '/api/auth/me')
				return fulfill(route, {
					id: '30000000-0000-4000-8000-000000000001',
					username: 'teacher',
					firstName: 'ครู',
					displayName: 'ครู ทดสอบ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					createdAt: '2026-09-01T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: ['academic_result.read.school']
				});
			if (url.pathname === '/api/academic/context/options')
				return fulfill(route, {
					defaultAcademicYearId: year,
					defaultAcademicTermId: term,
					years: [
						{
							isVisible: true,
							id: year,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						}
					],
					terms: [
						{
							isVisible: true,
							id: term,
							academicYearId: year,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
			if (url.pathname === '/api/academic/results/readiness')
				return fulfill(route, {
					courses: [
						{
							subjectId: '40000000-0000-4000-8000-000000000001',
							groups: [
								{
									learningGroupId: group,
									learningOfferingId: '50000000-0000-4000-8000-000000000001',
									subjectId: '40000000-0000-4000-8000-000000000001',
									offeringName: 'คณิตศาสตร์พื้นฐาน',
									groupName: 'ม.1/1',
									assigned: true,
									ready: true,
									locked: false,
									blockers: []
								},
								{
									learningGroupId: otherGroup,
									learningOfferingId: '50000000-0000-4000-8000-000000000001',
									subjectId: '40000000-0000-4000-8000-000000000001',
									offeringName: 'คณิตศาสตร์พื้นฐาน',
									groupName: 'ม.1/2',
									assigned: false,
									ready: true,
									locked: false,
									blockers: []
								}
							]
						}
					],
					activities: []
				});
			if (url.pathname === '/api/academic/results/policies') {
				policyAttempts += 1;
				if (policyAttempts === 1) {
					await policyGate;
					return fulfill(route, 'temporarily unavailable', 503);
				}
				return fulfill(route, []);
			}
			if (url.pathname === `/api/academic/results/groups/${group}/course`)
				return fulfill(route, {
					learningGroupId: group,
					locked: false,
					canManage: false,
					canConfirm: false,
					confirmationIsCurrent: false,
					confirmation: null,
					blockers: [],
					students: [],
					sourceChecksum: 'source',
					rosterChecksum: 'roster'
				});
			if (url.pathname === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (url.pathname === '/api/menu/user') return fulfill(route, { groups: [] });
			if (url.pathname === '/api/me/work-items/counts')
				return fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
			if (url.pathname === '/api/notifications')
				return fulfill(route, { items: [], unread_count: 0 });
			if (url.pathname === '/api/school/settings') return fulfill(route, 'forbidden', 403);
			return fulfill(route, {});
		}
	);
	try {
		await page.goto(`/staff/academic/results?academicYearId=${year}&academicTermId=${term}`);
		await expect(page.getByRole('heading', { name: 'ผลการเรียนที่คำนวณแล้ว' })).toBeVisible();
		await expect(page.getByText('เกณฑ์ที่ใช้อยู่')).toBeVisible();
		expect(
			requests.filter((path) => path === '/api/academic/learner-evaluations/subjects')
		).toEqual([]);
		expect(requests.filter((path) => path === '/api/academic/results/readiness')).toHaveLength(1);
		expect(requests.filter((path) => path.includes('/api/academic/results/groups/'))).toEqual([
			`/api/academic/results/groups/${group}/course`
		]);
	} finally {
		releasePolicy();
	}
	await expect(page.getByText('โหลดเกณฑ์ตัดผลไม่สำเร็จ')).toBeVisible();
	await expect(page.getByRole('heading', { name: 'ผลการเรียนที่คำนวณแล้ว' })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByText('ยังไม่มีเกณฑ์ตัดผลที่เปิดใช้งาน')).toBeVisible();
	expect(policyAttempts).toBe(2);
	let releaseReadiness!: () => void;
	const readinessGate = new Promise<void>((resolve) => (releaseReadiness = resolve));
	await page.route(
		(url) => url.pathname === '/api/academic/results/readiness',
		async (route) => {
			await readinessGate;
			await route.fallback();
		}
	);
	try {
		await page.goto(
			`/staff/academic/results?academicYearId=${year}&academicTermId=${term}&learningGroupId=${group}`
		);
		await expect(page.getByRole('heading', { name: 'ผลการเรียนที่คำนวณแล้ว' })).toBeVisible();
		await expect(page.locator('[data-slot="skeleton"]').first()).toBeVisible();
	} finally {
		releaseReadiness();
	}
});

test('learner-only reader falls back from the default course URL to the permitted section', async ({
	page
}) => {
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me')
				return fulfill(route, {
					id: '30000000-0000-4000-8000-000000000001',
					username: 'teacher',
					firstName: 'ครู',
					displayName: 'ครู ทดสอบ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					createdAt: '2026-09-01T00:00:00Z',
					email: null,
					nationalId: null,
					phone: null,
					profileImageFileId: null,
					permissions: ['academic_learner_evaluation.read.school']
				});
			if (url.pathname === '/api/academic/context/options')
				return fulfill(route, {
					defaultAcademicYearId: year,
					defaultAcademicTermId: term,
					years: [
						{
							isVisible: true,
							id: year,
							name: 'ปีการศึกษา 2569',
							year: 2569,
							status: 'active'
						}
					],
					terms: [
						{
							isVisible: true,
							id: term,
							academicYearId: year,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
			if (
				url.pathname === '/api/academic/results/readiness' ||
				url.pathname === '/api/academic/results/policies'
			)
				return fulfill(route, 'forbidden', 403);
			if (url.pathname === '/api/academic/learner-evaluations/subjects') return fulfill(route, []);
			if (url.pathname === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (url.pathname === '/api/menu/user') return fulfill(route, { groups: [] });
			if (url.pathname === '/api/me/work-items/counts')
				return fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
			if (url.pathname === '/api/notifications')
				return fulfill(route, { items: [], unread_count: 0 });
			if (url.pathname === '/api/school/settings') return fulfill(route, 'forbidden', 403);
			return fulfill(route, {});
		}
	);
	await page.goto(`/staff/academic/results?academicYearId=${year}&academicTermId=${term}`);
	await expect(page.getByText('ยังไม่มีกลุ่มสำหรับเตรียมผล')).toBeVisible();
	await expect.poll(() => new URL(page.url()).searchParams.get('section')).toBe('learner');
});

async function mockResultConfirmation(
	page: Page,
	pending: Promise<void>,
	onConfirm: () => void,
	canConfirm = true
) {
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			if (url.pathname === '/api/auth/me')
				return fulfill(route, {
					id: '30000000-0000-4000-8000-000000000001',
					username: 'teacher',
					firstName: 'ครู',
					displayName: 'ครู ทดสอบ',
					lastName: 'ทดสอบ',
					userType: 'staff',
					status: 'ACTIVE',
					session: sessionState(),
					createdAt: '2026-09-01T00:00:00Z',
					permissions: ['academic_result.read.assigned', 'academic_result.manage.assigned']
				});
			if (url.pathname === '/api/academic/context/options')
				return fulfill(route, {
					defaultAcademicYearId: year,
					defaultAcademicTermId: term,
					years: [{ id: year, year: 2569, name: '2569', status: 'active', isVisible: true }],
					terms: [
						{
							id: term,
							academicYearId: year,
							name: 'ภาคเรียนที่ 1',
							code: '1',
							sequence: 1,
							termType: 'regular',
							status: 'active',
							isVisible: true,
							includedInYearResult: true,
							blocksYearClosure: true
						}
					]
				});
			const groups = [group, otherGroup].map((id, index) => ({
				learningGroupId: id,
				learningOfferingId: '50000000-0000-4000-8000-000000000001',
				offeringName: 'คณิตศาสตร์',
				groupName: `ม.1/${index + 1}`,
				assigned: true,
				ready: true,
				locked: false,
				blockers: []
			}));
			if (url.pathname === '/api/academic/results/readiness')
				return fulfill(route, {
					courses: [{ subjectId: '40000000-0000-4000-8000-000000000001', groups }],
					activities: groups
				});
			if (url.pathname === '/api/academic/results/policies') return fulfill(route, []);
			if (url.pathname.includes('/api/academic/results/groups/')) {
				expect(url.searchParams.get('academicYearId')).toBe(year);
				expect(url.searchParams.get('academicTermId')).toBe(term);
				const selectedGroup = url.pathname.split('/groups/')[1]!.split('/')[0]!;
				const workspace = {
					learningGroupId: selectedGroup,
					locked: false,
					canManage: true,
					canConfirm,
					confirmationIsCurrent: false,
					confirmation: null,
					blockers: [],
					students: [],
					sourceChecksum: 'source',
					rosterChecksum: 'roster'
				};
				if (url.pathname.endsWith('/confirm')) {
					onConfirm();
					await pending;
					return fulfill(route, { ...workspace, confirmationIsCurrent: true });
				}
				return fulfill(route, workspace);
			}
			if (url.pathname === '/api/notifications/stream')
				return route.fulfill({ status: 200, contentType: 'text/event-stream', body: '' });
			if (url.pathname === '/api/menu/user') return fulfill(route, { groups: [] });
			if (url.pathname === '/api/notifications')
				return fulfill(route, { items: [], unread_count: 0 });
			if (url.pathname === '/api/me/work-items/counts')
				return fulfill(route, {
					open: 0,
					dueSoon: 0,
					overdue: 0,
					submitted: 0,
					closed: 0,
					total: 0
				});
			return fulfill(route, {});
		}
	);
}

for (const section of ['course', 'activity'] as const) {
	for (const destination of ['current', 'group', 'roundtrip', 'section'] as const) {
		test(`${destination === 'current' ? 'current' : 'late'} ${section} confirmation respects ${destination} selection`, async ({
			page
		}) => {
			let release!: () => void;
			const pending = new Promise<void>((resolve) => (release = resolve));
			let confirmationRequests = 0;
			await mockResultConfirmation(page, pending, () => {
				confirmationRequests += 1;
			});
			await page.goto(
				`/staff/academic/results?academicYearId=${year}&academicTermId=${term}&section=${section}`
			);
			await page
				.getByRole('button', {
					name: section === 'course' ? 'ยืนยันผลห้องนี้' : 'ยืนยันผลกิจกรรม',
					exact: true
				})
				.click();
			await expect.poll(() => confirmationRequests).toBe(1);
			if (destination === 'section') {
				await page
					.getByRole('tab', { name: section === 'course' ? 'กิจกรรม' : 'รายวิชา', exact: true })
					.click();
			} else if (destination !== 'current') {
				await page.locator('#result-group').click();
				await page.getByRole('option', { name: /ม.1\/2/ }).click();
				await expect(page.locator('#result-group')).toContainText('ม.1/2');
				if (destination === 'roundtrip') {
					await page.locator('#result-group').click();
					await page.getByRole('option', { name: /ม.1\/1/ }).click();
					await expect(page.locator('#result-group')).toContainText('ม.1/1');
				}
			}
			const response = page.waitForResponse((response) =>
				new URL(response.url()).pathname.endsWith('/confirm')
			);
			release();
			await (await response).finished();
			await page.evaluate(
				() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
			);
			const message = section === 'course' ? 'ยืนยันผลห้องนี้แล้ว' : 'ยืนยันผลกิจกรรมแล้ว';
			if (destination === 'current') {
				await expect(
					page.getByRole('button', { name: 'ยืนยันอีกครั้ง', exact: true })
				).toBeEnabled();
				await expect(page.getByText(message, { exact: true })).toBeVisible();
			} else {
				const selectedSection =
					destination === 'section' ? (section === 'course' ? 'activity' : 'course') : section;
				await expect(
					page.getByRole('button', {
						name: selectedSection === 'course' ? 'ยืนยันผลห้องนี้' : 'ยืนยันผลกิจกรรม',
						exact: true
					})
				).toBeEnabled();
				await expect(page.getByText(message, { exact: true })).toBeHidden();
				await expect(
					page.getByText(selectedSection === 'course' ? 'ยืนยันห้องนี้แล้ว' : 'ยืนยันกลุ่มแล้ว', {
						exact: true
					})
				).toBeHidden();
			}
		});
	}
}

for (const section of ['course', 'activity'] as const) {
	test(`helper cannot confirm ${section} results in the selected term`, async ({ page }) => {
		let confirmationRequests = 0;
		await mockResultConfirmation(
			page,
			Promise.resolve(),
			() => {
				confirmationRequests += 1;
			},
			false
		);
		await page.goto(
			`/staff/academic/results?academicYearId=${year}&academicTermId=${term}&section=${section}`
		);
		await expect(
			page.getByRole('button', {
				name: section === 'course' ? 'ยืนยันผลห้องนี้' : 'ยืนยันผลกิจกรรม',
				exact: true
			})
		).toBeDisabled();
		expect(confirmationRequests).toBe(0);
	});
}
