import { expect, test, type Locator, type Page } from '@playwright/test';
import type { SupervisionEvaluator } from '../../src/lib/api/supervision';
import { id, mock, navigate, path, timetableOption, year } from './fixtures/supervision-route-data';

const observationPath = `/api/supervision/observations/${id(30)}`;
const availabilityPath = `${observationPath}/evaluator-availability`;
const detailPath = `/staff/academic/supervision/${id(30)}?academicYearId=${year}`;
const requestsPath = path('requests');
const reason = 'ขอจัดช่วงคาบใหม่';
const primary = 'ผู้ประเมินหลักที่บันทึก';
const optional = 'ผู้ช่วยประเมินที่บันทึก';
const detailAddition = 'ผู้ประเมินที่เพิ่มจากรายละเอียด';
const calendarAddition = 'ผู้ประเมินที่เพิ่มจากปฏิทิน';
const names = [primary, optional, detailAddition, calendarAddition];
const evaluatorAvailability = names.map((name, index) => ({
	id: id(32 + index),
	name,
	available: true
}));
const savedEvaluators: SupervisionEvaluator[] = evaluatorAvailability
	.slice(0, 2)
	.map((staff, index) => ({
		id: id(185 + index),
		observationId: id(30),
		evaluatorUserId: staff.id,
		evaluatorDisplayName: staff.name,
		isRequired: index === 0,
		roleLabel: index === 0 ? 'หัวหน้าทีม' : 'ผู้ช่วยสังเกตการสอน',
		status: 'assigned',
		createdAt: '2026-10-01T09:00:00Z',
		updatedAt: '2026-10-01T09:00:00Z'
	}));

const initialAssignments = savedEvaluators.map(({ evaluatorUserId, isRequired, roleLabel }) => ({
	evaluatorUserId,
	isRequired,
	roleLabel
}));
const approval = (scope: Page | Locator) =>
	scope.getByRole('button', { name: 'อนุมัติและมอบหมาย', exact: true });
const returnRequest = (scope: Page | Locator) =>
	scope.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true });
const saveEvaluators = (scope: Page | Locator) =>
	scope.getByRole('button', { name: 'บันทึกผู้ประเมิน', exact: true });
const returnReason = (scope: Page | Locator) =>
	scope.getByRole('textbox', { name: 'ส่งกลับคำขอ', exact: true });
const remove = (scope: Page | Locator, name: string) =>
	scope.getByRole('button', { name: `ลบผู้ประเมิน ${name}`, exact: true });
const dialog = (page: Page) => page.getByRole('dialog', { name: 'ตรวจคำขอจองนิเทศ' });
const agenda = (page: Page) => page.getByRole('region', { name: /1 ต.ค. 2569/ });
const openReview = (scope: Page | Locator) =>
	scope.getByRole('button', { name: 'ตรวจคำขอ', exact: true }).click();

const otherTeacher = 'ครู รายการที่ยังจัดการได้';
const otherRequest = {
	id: id(40),
	observedDisplayName: otherTeacher,
	canManage: true,
	status: 'requested' as const,
	evaluators: savedEvaluators
};

function requestCard(page: Page, teacher: string) {
	return page
		.getByRole('tabpanel', { name: 'คำขอจอง' })
		.locator('[data-slot="card"]')
		.filter({
			has: page.getByRole('heading', { name: teacher, exact: true })
		});
}

async function refreshWithoutDismissingPicker(page: Page) {
	await page.evaluate(() => {
		const link = document.createElement('a');
		link.href = location.href;
		link.dataset.sveltekitPreloadData = 'false';
		document.body.append(link);
		link.click();
		link.remove();
	});
}

async function selectEvaluator(page: Page, scope: Locator, name: string) {
	await scope.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน', exact: true }).click();
	await page.getByRole('option', { name, exact: true }).click();
	await page.keyboard.press('Escape');
	await expect(scope).toBeVisible();
	await expect(remove(scope, name)).toBeVisible();
}

async function showCalendar(page: Page) {
	await page.getByRole('tab', { name: 'ปฏิทิน', exact: true }).click();
	await expect(agenda(page)).toBeVisible();
}

test.use({ serviceWorkers: 'block', timezoneId: 'UTC' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-10-01T03:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});

test('detail and calendar save durable evaluators and approval preserves roles', async ({
	page
}, testInfo) => {
	await page.setViewportSize({ width: 375, height: 820 });
	const api = await mock(page, {
		observation: { evaluators: savedEvaluators },
		evaluatorAvailability,
		hold: 'patch'
	});
	await page.goto(detailPath);
	await expect(page.getByRole('button', { name: 'ตรวจคำขอ', exact: true })).toBeVisible();
	expect(api.count(availabilityPath)).toBe(0);
	await openReview(page);
	const review = dialog(page);
	await expect(review.getByText('คาบ 1', { exact: true })).toBeVisible();
	await expect(remove(review, primary)).toBeVisible();
	await expect(remove(review, optional)).toBeVisible();
	await selectEvaluator(page, review, detailAddition);
	await saveEvaluators(review).click();
	await expect(review.getByRole('button', { name: 'กำลังบันทึก...' })).toBeDisabled();
	await expect(approval(review)).toBeDisabled();
	await expect(returnRequest(review)).toBeDisabled();
	await expect.poll(() => api.writes).toEqual([`PUT ${observationPath}/evaluators`]);
	api.release();
	await expect(review).toHaveCount(0);
	await expect(page.getByText(detailAddition, { exact: true })).toBeVisible();
	const detailReads = api.count(availabilityPath);

	await navigate(page, requestsPath);
	await expect(approval(page)).toBeEnabled();
	for (const name of names.slice(0, 3)) await expect(remove(page, name)).toBeVisible();
	expect(api.count(availabilityPath)).toBe(detailReads);
	await returnReason(page).fill(reason);
	await showCalendar(page);
	await expect(agenda(page).getByText(detailAddition, { exact: true })).toBeVisible();
	await openReview(agenda(page));
	await expect(approval(review)).toBeEnabled();
	await expect(returnReason(review)).toHaveValue(reason);
	expect(api.count(availabilityPath)).toBe(detailReads);
	for (const theme of ['light', 'dark']) {
		await page.evaluate(
			(value) => document.documentElement.classList.toggle('dark', value === 'dark'),
			theme
		);
		await expect(review).toBeInViewport();
		expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
			true
		);
		await page.screenshot({ path: testInfo.outputPath(`calendar-review-mobile-${theme}.png`) });
	}
	await selectEvaluator(page, review, calendarAddition);
	await saveEvaluators(review).click();
	await expect
		.poll(() => api.writes)
		.toEqual([`PUT ${observationPath}/evaluators`, `PUT ${observationPath}/evaluators`]);
	await expect(approval(review)).toBeEnabled();
	await expect(returnReason(review)).toHaveValue(reason);
	await review.getByRole('button', { name: 'ปิด', exact: true }).click();
	await expect(agenda(page).getByText(calendarAddition, { exact: true })).toBeVisible();
	await page.getByRole('tab', { name: 'คำขอจอง', exact: true }).click();
	await expect(remove(page, calendarAddition)).toBeVisible();
	await expect(returnReason(page)).toHaveValue(reason);
	await showCalendar(page);
	await openReview(agenda(page));
	await approval(review).click();
	await expect(review).toHaveCount(0);
	await expect(agenda(page).getByText('นัดหมายแล้ว', { exact: true })).toBeVisible();
	await expect(agenda(page).getByText(calendarAddition, { exact: true })).toBeVisible();
	await expect(agenda(page).getByRole('button', { name: 'ตรวจคำขอ' })).toHaveCount(0);
	expect(api.count('/api/supervision/observations')).toBe(1);
	expect(api.writes).toEqual([
		`PUT ${observationPath}/evaluators`,
		`PUT ${observationPath}/evaluators`,
		`POST ${observationPath}/approve-request`
	]);
	const afterDetailSave = [...initialAssignments, { evaluatorUserId: id(34), isRequired: true }];
	const afterCalendarSave = [...afterDetailSave, { evaluatorUserId: id(35), isRequired: true }];
	expect(api.payloads).toEqual([
		{ evaluators: afterDetailSave },
		{ evaluators: afterCalendarSave },
		{ evaluators: afterCalendarSave }
	]);
});

for (const surface of ['calendar', 'detail'] as const) {
	test(`returning from ${surface} applies the reason and removes the pending request`, async ({
		page
	}) => {
		const api = await mock(page, {
			observation: { evaluators: savedEvaluators },
			evaluatorAvailability
		});
		await page.goto(surface === 'calendar' ? requestsPath : detailPath);
		if (surface === 'calendar') {
			await showCalendar(page);
			await openReview(agenda(page));
		} else await openReview(page);
		const review = dialog(page);
		await returnReason(review).fill(reason);
		await returnRequest(review).click();
		await expect(review).toHaveCount(0);
		if (surface === 'calendar') {
			await expect(agenda(page)).toContainText('ไม่มีรายการจองนิเทศในวันนี้');
			await page.getByRole('tab', { name: 'คำขอจอง', exact: true }).click();
			await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toBeVisible();
			await navigate(page, detailPath);
		}
		await expect(page.getByTestId('supervision-current-return-reason')).toContainText(reason);
		await expect(page.getByRole('button', { name: 'ตรวจคำขอ', exact: true })).toHaveCount(0);
		expect(api.writes).toEqual([`POST ${observationPath}/return-request`]);
		expect(api.payloads).toEqual([{ comment: reason }]);
	});
}

test('detail approval uses saved names without selecting them again', async ({ page }) => {
	const api = await mock(page, {
		observation: { evaluators: savedEvaluators },
		evaluatorAvailability
	});
	await page.goto(detailPath);
	await openReview(page);
	const review = dialog(page);
	await expect(remove(review, primary)).toBeVisible();
	await approval(review).click();
	await expect(review).toHaveCount(0);
	await expect(page.getByTestId('supervision-detail')).toContainText('นัดหมายแล้ว');
	await expect(page.getByRole('button', { name: 'แก้ผู้ประเมิน', exact: true })).toBeVisible();
	expect(api.writes).toEqual([`POST ${observationPath}/approve-request`]);
	expect(api.payloads).toEqual([{ evaluators: initialAssignments }]);
});

test('editing a saved period rechecks the new window before approving in detail', async ({
	page
}) => {
	const api = await mock(page, {
		observation: { evaluators: savedEvaluators },
		bookingItems: [timetableOption(1)]
	});
	const checkedPeriods: string[] = [];
	await page.route(`**${availabilityPath}`, (route) => {
		const period = api.observation().periods[0];
		const periodLabel = period.lessonSnapshot.periodLabel ?? '';
		checkedPeriods.push(periodLabel);
		return route.fulfill({
			json: {
				success: true,
				data: {
					items: evaluatorAvailability,
					overlap: { pendingCount: 0, confirmedCount: 0, blocked: periodLabel === 'คาบ 1' },
					withinCycleDates: true,
					overlappingBookings: []
				}
			}
		});
	});
	await page.goto(detailPath);
	await openReview(page);
	const review = dialog(page);
	await expect(review.getByText('มีนัดทับกัน อนุมัติไม่ได้', { exact: true })).toBeVisible();
	await expect(approval(review)).toBeDisabled();
	await review.getByRole('button', { name: 'ปิด', exact: true }).click();
	await page.getByRole('button', { name: 'แก้คาบ/วันเวลา', exact: true }).click();
	const editor = page.getByRole('dialog', { name: 'แก้คาบ/วันเวลา' });
	await editor.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ }).click();
	await editor.getByRole('button', { name: 'บันทึกคาบนิเทศ', exact: true }).click();
	await expect(editor).toHaveCount(0);
	await openReview(page);
	await expect(review.getByText('คาบ 2', { exact: true })).toBeVisible();
	await expect(approval(review)).toBeEnabled();
	expect(checkedPeriods).toEqual(['คาบ 1', 'คาบ 2']);
	expect(api.writes).toEqual([`PATCH ${observationPath}`]);
});

test('recent conflicts keep saved names visible and an intentionally empty draft stays empty', async ({
	page
}) => {
	const api = await mock(page, {
		observation: { evaluators: savedEvaluators },
		evaluatorAvailability: evaluatorAvailability.map((staff) => ({
			...staff,
			available: staff.name !== optional
		}))
	});
	await page.goto(requestsPath);
	await expect(approval(page)).toBeEnabled();
	await page.getByRole('button', { name: 'ตรวจช่วงคาบและผู้ประเมิน' }).click();
	await expect(page.getByText(`${optional} · ไม่ว่าง`, { exact: true })).toBeVisible();
	await expect(approval(page)).toBeDisabled();
	await remove(page, optional).click();
	await expect(approval(page)).toBeEnabled();
	await remove(page, primary).click();
	await showCalendar(page);
	await expect(agenda(page).getByText(primary, { exact: true })).toBeVisible();
	await openReview(agenda(page));
	const review = dialog(page);
	await expect(review.getByText('ยังไม่ได้เลือกผู้ประเมิน', { exact: true })).toBeVisible();
	await expect(approval(review)).toBeDisabled();
	await expect(saveEvaluators(review)).toBeDisabled();
	expect(api.count(availabilityPath)).toBe(1);
	expect(api.writes).toEqual([]);
});

test('a readable out-of-scope request shows saved names without review actions or reads', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: ['supervision.manage.organization_unit', 'supervision.read.school'],
		canManage: false,
		observation: { evaluators: savedEvaluators }
	});
	await page.goto(requestsPath);
	await expect(
		page.getByTestId('supervision-request-review').getByText(primary, { exact: true })
	).toBeVisible();
	await expect(page.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน' })).toBeDisabled();
	await expect(approval(page)).toBeDisabled();
	await expect(returnRequest(page)).toBeDisabled();
	await showCalendar(page);
	await expect(agenda(page).getByText(primary, { exact: true })).toBeVisible();
	await expect(agenda(page).getByRole('button', { name: 'ตรวจคำขอ' })).toHaveCount(0);
	await agenda(page).getByRole('link', { name: 'รายละเอียด ครู ในคิว' }).click();
	await expect(
		page.getByTestId('supervision-detail').getByText(primary, { exact: true })
	).toBeVisible();
	await expect(page.getByRole('button', { name: 'ตรวจคำขอ', exact: true })).toHaveCount(0);
	expect(api.count(availabilityPath)).toBe(0);
	expect(api.writes).toEqual([]);
});

test('a refreshed row that loses management closes its picker and retires only its review draft', async ({
	page
}) => {
	const api = await mock(page, {
		observation: { evaluators: savedEvaluators },
		additionalObservations: [otherRequest],
		evaluatorAvailability,
		hold: 'observations',
		holdAt: 2
	});
	const secondRead = Promise.withResolvers<void>();
	const releaseRead = Promise.withResolvers<void>();
	const privateOverlapName = 'ครู จากผลตรวจช่วงคาบเดิม';
	let availabilityReads = 0;
	await page.route(`**${availabilityPath}`, async (route) => {
		const read = ++availabilityReads;
		if (read === 2) {
			secondRead.resolve();
			await releaseRead.promise;
		}
		await route.fulfill({
			json: {
				success: true,
				data: {
					items: evaluatorAvailability,
					overlap: { pendingCount: 1, confirmedCount: 0, blocked: false },
					withinCycleDates: true,
					overlappingBookings: [
						{
							observedDisplayName: privateOverlapName,
							observedAt: '2026-10-01T09:00:00Z',
							status: 'requested'
						}
					]
				}
			}
		});
	});
	await page.goto(requestsPath);
	const first = requestCard(page, 'ครู ในคิว');
	const second = requestCard(page, otherTeacher);
	const picker = first.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน' });
	await returnReason(second).fill('ร่างของอีกรายการ');
	await remove(second, optional).click();
	await returnReason(first).fill(reason);
	await selectEvaluator(page, first, detailAddition);
	await picker.click();
	await secondRead.promise;
	await expect(page.getByPlaceholder('ค้นหาครูผู้ประเมิน...')).toBeVisible();
	await expect(first.getByText(privateOverlapName, { exact: true })).toBeVisible();
	api.setObservationScope('read');
	const abortedRead = page.waitForEvent('requestfailed', {
		predicate: (request) => new URL(request.url()).pathname === availabilityPath
	});
	await refreshWithoutDismissingPicker(page);
	const region = page.getByTestId('supervision-observations');
	await expect(region).toHaveAttribute('aria-busy', 'true');
	await expect(picker).toHaveAttribute('aria-expanded', 'true');
	api.release();
	await expect(picker).toBeDisabled();
	await abortedRead;
	releaseRead.resolve();
	await expect(page.getByPlaceholder('ค้นหาครูผู้ประเมิน...')).toHaveCount(0);
	await expect(first.getByTestId('supervision-request-availability')).toHaveCount(0);
	await expect(page.getByText(privateOverlapName, { exact: true })).toHaveCount(0);
	await expect(first.getByText(detailAddition, { exact: true })).toHaveCount(0);
	await expect(returnReason(first)).toHaveValue('');
	await expect(first.getByText(primary, { exact: true })).toBeVisible();
	await expect(approval(first)).toBeDisabled();
	await expect(returnReason(second)).toHaveValue('ร่างของอีกรายการ');
	await expect(remove(second, optional)).toHaveCount(0);
	await expect(approval(second)).toBeEnabled();

	api.setObservationScope('manage');
	await refreshWithoutDismissingPicker(page);
	await expect(picker).toBeEnabled();
	await expect(picker).toHaveAttribute('aria-expanded', 'false');
	await expect(returnReason(first)).toHaveValue('');
	await expect(first.getByText(detailAddition, { exact: true })).toHaveCount(0);
	await expect(returnReason(second)).toHaveValue('ร่างของอีกรายการ');
	expect(availabilityReads).toBe(2);
	expect(api.writes).toEqual([]);
});

test('a held approval cannot reinsert a request removed by a fresh list', async ({ page }) => {
	const api = await mock(page, {
		observation: { evaluators: savedEvaluators },
		additionalObservations: [otherRequest],
		hold: 'patch'
	});
	await page.goto(requestsPath);
	const first = requestCard(page, 'ครู ในคิว');
	const second = requestCard(page, otherTeacher);
	await returnReason(second).fill('เก็บร่างนี้ไว้');
	await approval(first).click();
	const endpoint = `${observationPath}/approve-request`;
	await expect.poll(() => api.writes).toEqual([`POST ${endpoint}`]);
	api.setObservationScope('hidden');
	await refreshWithoutDismissingPicker(page);
	await expect(first).toHaveCount(0);
	await expect(approval(second)).toBeDisabled();
	const settled = page.waitForResponse((response) => new URL(response.url()).pathname === endpoint);
	api.release();
	const response = await settled;
	expect(response.ok()).toBe(true);
	await response.finished();
	await expect(approval(second)).toBeEnabled();
	await expect(returnReason(second)).toHaveValue('เก็บร่างนี้ไว้');
	await expect(page.getByText('อนุมัติคำขอและมอบหมายผู้ประเมินแล้ว', { exact: true })).toHaveCount(
		0
	);
	await showCalendar(page);
	await expect(agenda(page).getByText('ครู ในคิว', { exact: true })).toHaveCount(0);
	await expect(agenda(page).getByText(otherTeacher, { exact: true })).toBeVisible();
	expect(api.count(availabilityPath)).toBe(0);
});
