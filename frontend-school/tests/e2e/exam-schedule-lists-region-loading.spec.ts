import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const yearId = '10000000-0000-4000-8000-000000000011';
const termOne = '20000000-0000-4000-8000-000000000011';
const termTwo = '20000000-0000-4000-8000-000000000012';
const roundId = '30000000-0000-4000-8000-000000000011';

const roundsUrl = (termId = termOne) =>
	`/staff/academic/exam-schedules?academicYearId=${yearId}&academicTermId=${termId}`;
const staffUrl = (termId = termOne) =>
	`/staff/exams?academicYearId=${yearId}&academicTermId=${termId}`;

function round(termId: string, name: string) {
	return {
		id: roundId,
		academicYearId: yearId,
		academicTermId: termId,
		name,
		description: null,
		examKind: 'midterm',
		status: 'draft',
		rowVersion: 1,
		publishedAt: null,
		createdAt: '2026-05-01T00:00:00Z',
		updatedAt: '2026-05-01T00:00:00Z'
	};
}

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function installShell(page: Page) {
	await page.route('**/api/**', async (route) => {
		const url = new URL(route.request().url());
		if (url.pathname === '/api/auth/me') {
			await fulfill(route, {
				id: '90000000-0000-4000-8000-000000000011',
				username: 'exam-reader',
				firstName: 'ทดสอบ',
				lastName: 'ตารางสอบ',
				userType: 'staff',
				status: 'ACTIVE',
				permissions: ['*']
			});
			return;
		}
		if (url.pathname === '/api/academic/context/options') {
			await fulfill(route, {
				activeAcademicYearId: yearId,
				activeAcademicTermId: termOne,
				years: [{ id: yearId, name: 'ปีการศึกษา 2569', year: 2569, status: 'active' }],
				terms: [termOne, termTwo].map((id, index) => ({
					id,
					academicYearId: yearId,
					name: `ภาคเรียนที่ ${index + 1}`,
					code: String(index + 1),
					sequence: index + 1,
					termType: 'regular',
					status: 'active',
					startDate: '2026-05-01',
					endDate: '2027-03-31',
					includedInYearResult: true,
					blocksYearClosure: true
				}))
			});
			return;
		}
		if (url.pathname === '/api/menu/user') {
			await fulfill(route, { groups: [] });
			return;
		}
		await fulfill(route, []);
	});
}

async function navigateInApp(page: Page, href: string) {
	await page.evaluate((destination) => {
		const link = document.createElement('a');
		link.href = destination;
		link.id = 'test-exam-navigation';
		link.textContent = 'เปลี่ยนภาคเรียน';
		link.style.cssText = 'position:fixed;top:160px;left:500px;z-index:9999;background:white';
		document.body.append(link);
	}, href);
	await page.locator('#test-exam-navigation').click();
}

test('round list shows first skeleton, starts one read, and clears the previous term', async ({
	page
}) => {
	await installShell(page);
	const pending: Route[] = [];
	await page.route('**/api/academic/exam-schedules?**', (route) => {
		pending.push(route);
	});
	await page.goto(roundsUrl());
	await expect.poll(() => pending.length).toBe(1);
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(pending[0], [round(termOne, 'รอบภาคต้น')]);
	await expect(page.getByTestId('exam-rounds-ready')).toBeVisible();
	await expect(page.getByText('รอบภาคต้น')).toBeVisible();
	await navigateInApp(page, roundsUrl(termTwo));
	await expect.poll(() => pending.length).toBe(2);
	await expect(page.getByText('รอบภาคต้น')).toHaveCount(0);
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(pending[1], [round(termTwo, 'รอบภาคปลาย')]);
	await expect(page.getByText('รอบภาคปลาย')).toBeVisible();
});

test('round refresh retains rows and retries only its list request after failure', async ({
	page
}) => {
	await installShell(page);
	const pending: Route[] = [];
	await page.route('**/api/academic/exam-schedules?**', (route) => {
		pending.push(route);
	});
	await page.goto(roundsUrl());
	await expect.poll(() => pending.length).toBe(1);
	await fulfill(pending[0], [round(termOne, 'รอบเดิม')]);
	const ready = page.getByTestId('exam-rounds-ready');
	await expect(ready).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช' }).click();
	await expect.poll(() => pending.length).toBe(2);
	await expect(ready).toHaveAttribute('aria-busy', 'true');
	await expect(page.getByText('รอบเดิม')).toBeVisible();
	await pending[1].fulfill({
		status: 503,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'รายการยังไม่พร้อม' })
	});
	await expect(ready.getByRole('alert')).toContainText('รายการยังไม่พร้อม');
	await ready.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect.poll(() => pending.length).toBe(3);
	await fulfill(pending[2], [round(termOne, 'รอบล่าสุด')]);
	await expect(page.getByText('รอบล่าสุด')).toBeVisible();
	await expect(ready.getByRole('alert')).toHaveCount(0);
});

test('deleting a round locally cannot be undone by an older list response', async ({ page }) => {
	await installShell(page);
	const pending: Route[] = [];
	await page.route('**/api/academic/exam-schedules?**', (route) => {
		pending.push(route);
	});
	await page.route(`**/api/academic/exam-schedules/${roundId}`, (route) => fulfill(route, {}));
	await page.goto(roundsUrl());
	await expect.poll(() => pending.length).toBe(1);
	await fulfill(pending[0], [round(termOne, 'รอบที่จะลบ')]);
	await expect(page.getByText('รอบที่จะลบ')).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช' }).click();
	await expect.poll(() => pending.length).toBe(2);
	await page.getByRole('button', { name: 'ลบรอบสอบ รอบที่จะลบ' }).click();
	await page.getByRole('button', { name: 'ลบรอบสอบถาวร' }).click();
	await expect(page.getByText('รอบที่จะลบ')).toHaveCount(0);
	await fulfill(pending[1], [round(termOne, 'รอบที่จะลบ')]).catch(() => undefined);
	await expect(page.getByText('รอบที่จะลบ')).toHaveCount(0);
});

test('staff schedule keeps its previous result during refresh and clears another term', async ({
	page
}) => {
	await installShell(page);
	const pending: Route[] = [];
	await page.route('**/api/staff/exam-schedules?**', (route) => {
		pending.push(route);
	});
	await page.goto(staffUrl());
	await expect.poll(() => pending.length).toBe(1);
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(pending[0], []);
	const ready = page.getByTestId('staff-exams-ready');
	await expect(ready).toBeVisible();
	await page.getByRole('button', { name: 'รีเฟรช' }).click();
	await expect.poll(() => pending.length).toBe(2);
	await expect(ready).toHaveAttribute('aria-busy', 'true');
	await pending[1].fulfill({
		status: 503,
		contentType: 'application/json',
		body: JSON.stringify({ success: false, error: 'ตารางสอบยังไม่พร้อม' })
	});
	await expect(ready.getByRole('alert')).toContainText('ตารางสอบยังไม่พร้อม');
	await navigateInApp(page, staffUrl(termTwo));
	await expect.poll(() => pending.length).toBe(3);
	await expect(ready).toHaveCount(0);
	await expect(page.locator('main [data-slot="skeleton"]').first()).toBeVisible();
	await fulfill(pending[2], []);
	await expect(page.getByTestId('staff-exams-ready')).toBeVisible();
});
