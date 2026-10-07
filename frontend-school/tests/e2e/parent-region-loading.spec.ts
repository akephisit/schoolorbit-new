import { test, expect } from '@playwright/test';
import {
	mockParent,
	childRoute,
	child,
	otherChild,
	deniedChild,
	year,
	nextYear
} from './fixtures/parent-route-data';
test.use({ serviceWorkers: 'block' });
test('timetable starts and renders while independent child profile is held', async ({ page }) => {
	const api = await mockParent(page, { hold: 'profile' });
	await page.goto(childRoute('timetable'));
	await expect(page.getByText('วิชาคนแรกภาคหนึ่ง', { exact: true })).toBeVisible();
	expect(api.count('context')).toBe(1);
	expect(api.count('profile')).toBe(1);
	expect(api.count('timetable')).toBe(1);
	await expect(
		page.getByRole('status', { name: 'กำลังโหลดข้อมูลนักเรียน', exact: true })
	).toBeVisible();
	api.release();
	await expect(page.getByTestId('parent-child-profile-region')).toContainText('นักเรียนคนแรก');
});
test('held timetable leaves child profile usable', async ({ page }) => {
	const api = await mockParent(page, { hold: 'timetable' });
	await page.goto(childRoute('timetable'));
	await expect(page.getByTestId('parent-child-profile-region')).toContainText('นักเรียนคนแรก');
	await expect(
		page.getByRole('status', { name: 'กำลังโหลดตารางเรียน', exact: true })
	).toBeVisible();
	api.release();
	await expect(page.getByText('วิชาคนแรกภาคหนึ่ง', { exact: true })).toBeVisible();
	expect(api.count('profile')).toBe(1);
});
test('child profile retries independently of ready timetable', async ({ page }) => {
	const api = await mockParent(page, { fail: 'profile' });
	await page.goto(childRoute('timetable'));
	await expect(page.getByText('วิชาคนแรกภาคหนึ่ง', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองข้อมูลนักเรียนอีกครั้ง', exact: true }).click();
	await expect(page.getByTestId('parent-child-profile-region')).toContainText('นักเรียนคนแรก');
	expect(api.count('context')).toBe(1);
	expect(api.count('timetable')).toBe(1);
	expect(api.count('profile')).toBe(2);
});
for (const route of ['/parent', childRoute()]) {
	test(`${route}: first profile read has its own skeleton`, async ({ page }) => {
		const api = await mockParent(page, { hold: 'profile' });
		await page.goto(route);
		await expect(
			page.getByRole('status', {
				name: route === '/parent' ? 'กำลังโหลดข้อมูลผู้ปกครอง' : 'กำลังโหลดข้อมูลนักเรียน',
				exact: true
			})
		).toBeVisible();
		api.release();
		await expect(page.getByTestId('parent-profile-region')).toContainText('ห้องปีเดิม');
	});
}
for (const resource of ['timetable', 'exams', 'calendar'] as const) {
	const text =
		resource === 'timetable'
			? 'วิชาคนแรกภาคหนึ่ง'
			: resource === 'exams'
				? 'รอบคนแรกภาคหนึ่ง'
				: 'กิจกรรมคนแรก 2026-10';
	test(`${resource}: local error retry does not refetch context or sibling`, async ({ page }) => {
		const api = await mockParent(page, { fail: resource });
		await page.goto(childRoute(resource));
		await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(
			resource === 'calendar'
				? page.getByRole('heading', { name: text, exact: true })
				: page.getByText(text, { exact: true })
		).toBeVisible();
		expect(api.count(resource)).toBe(2);
		expect(api.count('context')).toBe(resource === 'calendar' ? 0 : 1);
		expect(api.count('profile')).toBe(resource === 'timetable' ? 1 : 0);
	});
	test(`${resource}: changed child ignores a late prior response and Back restores child`, async ({
		page
	}) => {
		const api = await mockParent(page, { hold: resource });
		const pending = page.waitForResponse(
			(r) =>
				r.url().includes(`/students/${child}/`) &&
				!r.url().includes('options') &&
				(resource !== 'exams' || r.url().includes('exam-schedules'))
		);
		await page.goto(childRoute(resource));
		await expect.poll(() => api.count(resource)).toBe(1);
		await page.evaluate(
			(url) => {
				const a = document.createElement('a');
				a.href = url;
				a.textContent = 'เปลี่ยนลูก';
				document.querySelector('main')?.append(a);
			},
			childRoute(resource, otherChild)
		);
		await page.getByRole('link', { name: 'เปลี่ยนลูก', exact: true }).click();
		const next = text.replace('คนแรก', 'คนสอง');
		await expect(
			resource === 'calendar'
				? page.getByRole('heading', { name: next, exact: true })
				: page.getByText(next, { exact: true })
		).toBeVisible();
		api.release();
		await pending;
		await page.evaluate(
			() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())))
		);
		await expect(page.getByText(text, { exact: true })).toHaveCount(0);
		await page.goBack();
		await expect(
			resource === 'calendar'
				? page.getByRole('heading', { name: text, exact: true })
				: page.getByText(text, { exact: true })
		).toBeVisible();
	});
	test(`${resource}: denied child context never starts dependent reads`, async ({ page }) => {
		const api = await mockParent(page);
		await page.goto(childRoute(resource, deniedChild));
		await expect(
			page.getByRole('button', {
				name: resource === 'calendar' ? 'ลองอีกครั้ง' : 'ลองบริบทอีกครั้ง',
				exact: true
			})
		).toBeVisible();
		expect(api.count(resource)).toBe(resource === 'calendar' ? 1 : 0);
		if (resource === 'calendar')
			await expect(page.getByTestId('parent-calendar-region')).toContainText(
				'ไม่มีสิทธิ์เข้าถึงลูกคนนี้'
			);
		expect(api.count('profile')).toBe(0);
		expect(api.writes).toEqual([]);
	});
}
for (const [route, label] of [
	['/parent', 'นักเรียนคนแรก'],
	[childRoute(), 'ห้องปีเดิม']
] as const) {
	test(`${route}: profile retained failure and focused retry`, async ({ page }) => {
		const api = await mockParent(page, { fail: 'profile', failAt: 2 });
		await page.goto(route);
		await expect(page.getByTestId('parent-profile-region')).toContainText(label);
		await page.getByRole('button', { name: 'โหลดข้อมูลใหม่', exact: true }).click();
		await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
		await expect(page.getByTestId('parent-profile-region')).toContainText(label);
		expect(api.count('profile')).toBe(3);
		expect(api.count('context')).toBe(1);
	});
	test(`${route}: year change and Back repair no duplicate primary`, async ({ page }) => {
		const api = await mockParent(page);
		await page.goto(route);
		await expect(page).toHaveURL(new RegExp(`academicYearId=${year}`));
		await page.getByRole('button', { name: 'ปีการศึกษา', exact: true }).click();
		await page.getByRole('option', { name: '2570', exact: true }).click();
		await expect(page).toHaveURL(new RegExp(`academicYearId=${nextYear}`));
		await expect(page.getByText(/ห้องปีถัดไป/).first()).toBeVisible();
		await page.goBack();
		await expect(page.getByText(/ห้องปีเดิม/).first()).toBeVisible();
		expect(api.count('profile')).toBe(3);
	});
}
test('calendar month history only refreshes events', async ({ page }) => {
	const api = await mockParent(page);
	await page.goto(childRoute('calendar'));
	await expect(
		page.getByRole('heading', { name: 'กิจกรรมคนแรก 2026-10', exact: true })
	).toBeVisible();
	await page.getByRole('button', { name: 'เดือนถัดไป', exact: true }).click();
	await expect(page).toHaveURL(/month=2026-11/);
	await expect(
		page.getByRole('heading', { name: 'กิจกรรมคนแรก 2026-11', exact: true })
	).toBeVisible();
	await page.goBack();
	await expect(
		page.getByRole('heading', { name: 'กิจกรรมคนแรก 2026-10', exact: true })
	).toBeVisible();
	expect(api.count('calendar')).toBe(3);
	expect(api.count('context')).toBe(0);
});
test('logout during child prerequisite never launches child primary', async ({ page }) => {
	const api = await mockParent(page, { hold: 'context' });
	const pending = page.waitForResponse((r) => r.url().includes('/academic-context/options'));
	await page.goto(childRoute('exams'));
	await expect.poll(() => api.count('context')).toBe(1);
	await page.getByRole('banner').getByRole('button').last().click();
	await page.getByRole('menuitem', { name: 'ออกจากระบบ' }).click();
	await expect(page).toHaveURL(/\/login/);
	api.release();
	await pending;
	await page.evaluate(
		() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())))
	);
	expect(api.count('exams')).toBe(0);
});
