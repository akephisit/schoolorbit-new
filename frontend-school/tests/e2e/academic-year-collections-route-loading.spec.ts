import { expect, test, type Page, type Route } from '@playwright/test';

test.use({ serviceWorkers: 'block' });

const yearId = '10000000-0000-4000-8000-000000000001';
const nextYearId = '10000000-0000-4000-8000-000000000002';
const primaryPaths = {
	homerooms: [
		'/api/academic/homerooms',
		'/api/academic/homeroom-advisors',
		'/api/lookup/grade-levels',
		'/api/academic/study-program-options'
	],
	studentYears: [
		'/api/academic/student-years',
		'/api/academic/placements',
		'/api/academic/homerooms'
	]
} as const;

function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { promise, release };
}

async function fulfill(route: Route, data: unknown) {
	await route.fulfill({
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function mockYearCollections(
	page: Page,
	options: {
		gate?: { path: string; yearId: string; promise: Promise<void> };
		failOnce?: string;
		homeroomsByYear?: boolean;
	} = {}
) {
	const counts = new Map<string, number>();
	await page.route(
		(url) => url.pathname.startsWith('/api/'),
		async (route) => {
			const url = new URL(route.request().url());
			const path = url.pathname;
			if (path === '/api/auth/me') {
				await fulfill(route, {
					id: '90000000-0000-4000-8000-000000000001',
					username: 'year-collections-reader',
					firstName: 'ทดสอบ',
					lastName: 'วิชาการ',
					userType: 'staff',
					status: 'ACTIVE',
					permissions: ['*']
				});
				return;
			}
			if (path === '/api/academic/context/options') {
				await fulfill(route, {
					activeAcademicYearId: yearId,
					activeAcademicTermId: null,
					years: [
						{ id: yearId, name: 'ปีการศึกษา 2570', year: 2570, status: 'active' },
						{ id: nextYearId, name: 'ปีการศึกษา 2571', year: 2571, status: 'planning' }
					],
					terms: []
				});
				return;
			}
			if (path === '/api/menu/user') {
				await fulfill(route, { groups: [] });
				return;
			}
			const count = (counts.get(path) ?? 0) + 1;
			counts.set(path, count);
			if (
				options.gate?.path === path &&
				options.gate.yearId === url.searchParams.get('academicYearId')
			)
				await options.gate.promise;
			if (options.failOnce === path && count === 1) {
				await route.fulfill({
					status: 503,
					contentType: 'application/json',
					body: JSON.stringify({ success: false, error: 'ข้อมูลยังไม่พร้อม' })
				});
				return;
			}
			if (options.homeroomsByYear && path === '/api/academic/homerooms') {
				const requestedYearId = url.searchParams.get('academicYearId');
				await fulfill(route, [
					{
						id: requestedYearId,
						name: requestedYearId === yearId ? 'ห้องปีเดิม' : 'ห้องปีใหม่',
						academicYearId: requestedYearId,
						gradeLevelId: '30000000-0000-4000-8000-000000000001',
						studyProgramId: '40000000-0000-4000-8000-000000000001',
						roomNumber: '1',
						capacity: 40,
						rowVersion: 1
					}
				]);
				return;
			}
			await fulfill(route, []);
		}
	);
	return (path: string) => counts.get(path) ?? 0;
}

test('homerooms starts four visible reads, shows a stable skeleton, and keeps staff lazy', async ({
	page
}) => {
	const gate = deferred();
	const count = await mockYearCollections(page, {
		gate: { path: '/api/academic/homerooms', yearId, promise: gate.promise },
		homeroomsByYear: true
	});
	try {
		await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}`);
		for (const path of primaryPaths.homerooms) await expect.poll(() => count(path)).toBe(1);
		await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
		await expect(page.getByTestId('homerooms-ready')).toHaveCount(0);
		expect(count('/api/lookup/staff')).toBe(0);
	} finally {
		gate.release();
	}
	await expect(page.getByTestId('homerooms-ready')).toBeVisible();
	for (const path of primaryPaths.homerooms) expect(count(path)).toBe(1);
	expect(count('/api/lookup/staff')).toBe(0);
});

test('student years starts three visible reads and loads form options only on open', async ({
	page
}) => {
	const gate = deferred();
	const count = await mockYearCollections(page, {
		gate: { path: '/api/academic/student-years', yearId, promise: gate.promise }
	});
	try {
		await page.goto(`/staff/academic/student-years?academicYearId=${yearId}`);
		for (const path of primaryPaths.studentYears) await expect.poll(() => count(path)).toBe(1);
		await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
		await expect(page.getByTestId('student-years-ready')).toHaveCount(0);
		for (const path of [
			'/api/lookup/grade-levels',
			'/api/academic/study-program-options',
			'/api/academic/student-years/candidates'
		])
			expect(count(path)).toBe(0);
	} finally {
		gate.release();
	}
	await expect(page.getByTestId('student-years-ready')).toBeVisible();
	await page.getByRole('button', { name: 'เพิ่มนักเรียนในปีนี้' }).click();
	await expect(page.getByRole('dialog')).toBeVisible();
	for (const path of [
		'/api/lookup/grade-levels',
		'/api/academic/study-program-options',
		'/api/academic/student-years/candidates'
	])
		await expect.poll(() => count(path)).toBe(1);
});

test('student-year region retries its failed collection without eager form reads', async ({
	page
}) => {
	const count = await mockYearCollections(page, { failOnce: '/api/academic/placements' });
	await page.goto(`/staff/academic/student-years?academicYearId=${yearId}`);
	await expect(page.getByRole('button', { name: 'ลองอีกครั้ง' })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.getByTestId('student-years-ready')).toBeVisible();
	for (const path of primaryPaths.studentYears) expect(count(path)).toBe(2);
	expect(count('/api/lookup/grade-levels')).toBe(0);
	expect(count('/api/academic/study-program-options')).toBe(0);
});

test('switching years while an old homeroom read is pending shows only the new year', async ({
	page
}) => {
	const gate = deferred();
	const count = await mockYearCollections(page, {
		gate: { path: '/api/academic/homerooms', yearId, promise: gate.promise },
		homeroomsByYear: true
	});
	try {
		await page.goto(`/staff/academic/homerooms?academicYearId=${yearId}`);
		await expect.poll(() => count('/api/academic/homerooms')).toBe(1);
		await page.getByLabel('เลือกปีการศึกษา', { exact: true }).click();
		await page.getByRole('option', { name: /ปีการศึกษา 2571/ }).click();
		await expect(page).toHaveURL(new RegExp(`academicYearId=${nextYearId}`));
		await expect(page.getByText('ห้องปีใหม่', { exact: true })).toBeVisible();
		expect(count('/api/academic/homerooms')).toBe(2);
	} finally {
		gate.release();
	}
	await expect(page.getByText('ห้องปีเดิม', { exact: true })).toHaveCount(0);
});
