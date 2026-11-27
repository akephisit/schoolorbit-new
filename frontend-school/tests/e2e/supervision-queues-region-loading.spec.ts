import { revalidatePage } from './fixtures/page-revalidation';
import { expect, test, type Locator, type Page, type Request } from '@playwright/test';
import { existsSync } from 'node:fs';
import { readFile, readdir } from 'node:fs/promises';
import type { MenuGroup } from '../../src/lib/api/menu.js';
import {
	id,
	year,
	nextYear,
	templateId,
	timetableOption,
	savedPeriod,
	path,
	mock,
	navigate
} from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.clock.setFixedTime(new Date('2026-09-30T09:00:00Z'));
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
const assigned = ['supervision.read.assigned', 'supervision.evaluate.assigned'];
const own = ['supervision.read.own', 'supervision.request.own'];
const observationPath = '/api/supervision/observations';
const minePath = '/staff/academic/supervision';
const personalHref = `${minePath}?academicYearId=${year}&academicTermId=${id(8)}`;
const supervisionMenu: MenuGroup[] = [
	{
		code: 'academic_supervision',
		name: 'นิเทศการสอน',
		icon: 'ClipboardCheck',
		workspaceCode: 'academic',
		workspaceName: 'วิชาการ',
		workspaceIcon: 'School',
		workspaceOrder: 1,
		displayOrder: 1,
		items: [
			{
				id: 'supervision',
				code: 'supervision',
				name: 'นิเทศการสอน',
				path: minePath,
				icon: 'ClipboardCheck'
			}
		]
	}
];

function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => (release = resolve));
	return { promise, release };
}

async function holdMineModule(page: Page) {
	const directory = '.svelte-kit/generated/build/client-optimized/nodes';
	const manifestFile = '.svelte-kit/output/client/.vite/manifest.json';
	const files = existsSync(directory) && existsSync(manifestFile) ? await readdir(directory) : [];
	const nodes = await Promise.all(
		files.map(async (file) => ({
			file,
			source: await readFile(`${directory}/${file}`, 'utf8')
		}))
	);
	const node = nodes.find(({ source }) => source.includes(`/routes/(app)${minePath}/+page.svelte`));
	const manifest: Record<string, { file: string; name?: string; isEntry?: boolean }> = node
		? JSON.parse(await readFile(manifestFile, 'utf8'))
		: {};
	const entry = Object.values(manifest).find(
		(item) => item.isEntry && item.name === `nodes/${node?.file.replace(/\.js$/, '')}`
	);
	const modulePath = entry ? `/${entry.file}` : null;
	const gate = deferred();
	let requested = false;
	await page.route(
		(url) => {
			const pathname = decodeURIComponent(url.pathname);
			return pathname === modulePath || pathname.endsWith(`/routes/(app)${minePath}/+page.svelte`);
		},
		async (route) => {
			requested = true;
			await gate.promise;
			await route.continue();
		}
	);
	return { ...gate, requested: () => requested };
}

test('opening Mine from another supervision tab keeps the workspace visible while its module loads', async ({
	page
}) => {
	await mock(page, { term: true, menuGroups: supervisionMenu });
	const module = await holdMineModule(page);
	try {
		await page.goto(path('templates'));
		await expect(
			page.locator('aside').getByRole('link', { name: 'นิเทศการสอน', exact: true })
		).toBeVisible();
		const navigation = page.getByRole('navigation', { name: 'เมนูนิเทศการสอน' });
		await navigation.getByRole('link', { name: 'ของฉัน', exact: true }).click();
		await expect.poll(module.requested).toBe(true);
		await expect(navigation.getByRole('link', { name: 'แบบประเมิน', exact: true })).toHaveAttribute(
			'aria-current',
			'page'
		);
		await expect(page.getByTestId('menu-navigation-pending')).toHaveCount(0);
		await expect(page.getByTestId('supervision-templates-ready')).toBeVisible();
	} finally {
		module.release();
	}
	await expect(page.getByRole('link', { name: 'ของฉัน', exact: true })).toHaveAttribute(
		'aria-current',
		'page'
	);
});

test('narrow Mine return keeps its list and counts through history and held reads, and revokes a denied refresh', async ({
	page
}) => {
	await page.setViewportSize({ width: 390, height: 900 });
	await mock(page, {
		term: true,
		menuGroups: supervisionMenu,
		additionalObservations: [31, 32, 33].map((value) => ({
			id: id(value),
			status: 'completed' as const
		}))
	});
	const gate = deferred();
	let hold = false;
	let deny = false;
	await page.route(
		(url) =>
			[observationPath, '/api/supervision/cycles', '/api/supervision/templates/summaries'].includes(
				url.pathname
			),
		async (route) => {
			if (hold) await gate.promise;
			if (deny && new URL(route.request().url()).pathname === observationPath)
				return route.fulfill({
					status: 403,
					json: { success: false, error: 'ไม่มีสิทธิ์ดูรายการนิเทศ' }
				});
			await route.fallback();
		}
	);
	const navigation = page.getByRole('navigation', { name: 'เมนูนิเทศการสอน' });
	const mine = navigation.getByRole('link', { name: 'ของฉัน', exact: true });
	const templates = navigation.getByRole('link', { name: 'แบบประเมิน', exact: true });
	const metrics = page.getByTestId('supervision-personal-status');
	const list = page.getByTestId('supervision-observations');
	const heading = page.getByRole('heading', { name: 'รายการของฉัน', exact: true });
	const pending = metrics
		.locator('dt', { hasText: /^รออนุมัติคำขอ$/ })
		.locator('xpath=following-sibling::dd');

	await page.goto(personalHref);
	await expect(pending).toHaveText('1');
	const initialHeadingY = (await heading.boundingBox())?.y;
	await templates.click();
	await expect(page.getByTestId('supervision-templates-ready')).toBeVisible();
	hold = true;
	try {
		await mine.click();
		await expect(mine).toHaveAttribute('aria-current', 'page');
		await expect(list).toHaveAttribute('aria-busy', 'true');
		await expect(list.locator('[data-supervision-own-list]')).toBeVisible();
		await expect(pending).toHaveText('1');
		await expect(list.getByRole('status')).toHaveCount(0);
		await expect(page.getByTestId('menu-navigation-pending')).toHaveCount(0);
		await expect.poll(async () => (await heading.boundingBox())?.y).toBe(initialHeadingY);
		const main = page.locator('main');
		await main.hover();
		await page.mouse.wheel(0, 220);
		await expect.poll(() => main.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
		await page.goBack();
		await expect(templates).toHaveAttribute('aria-current', 'page');
		await page.goForward();
		await expect(mine).toHaveAttribute('aria-current', 'page');
		await expect(list).toHaveAttribute('aria-busy', 'true');
		await expect(pending).toHaveText('1');
	} finally {
		hold = false;
		gate.release();
	}
	await expect(list).toHaveAttribute('aria-busy', 'false');
	await expect(pending).toHaveText('1');
	await templates.click();
	await expect(templates).toHaveAttribute('aria-current', 'page');
	deny = true;
	await mine.click();
	await expect(list.getByText('ยังไม่มีสิทธิ์ดูรายการนิเทศ', { exact: true })).toBeVisible();
	await expect(metrics).toHaveCount(0);
	await expect(list.locator('[data-supervision-own-list]')).toHaveCount(0);
});

for (const first of ['read', 'write'] as const) {
	test(`Mine reconciles a read overlapping a request change when the ${first} finishes first`, async ({
		page
	}) => {
		const api = await mock(page, { term: true, hold: 'patch' });
		const navigation = page.getByRole('navigation', { name: 'เมนูนิเทศการสอน' });
		const metrics = page.getByTestId('supervision-personal-status');
		const list = page.getByTestId('supervision-observations');
		const gate = deferred();
		const returnPath = `${observationPath}/${id(30)}/return-request`;
		await page.goto(personalHref);
		await expect(
			metrics.locator('dt', { hasText: /^รออนุมัติคำขอ$/ }).locator('xpath=following-sibling::dd')
		).toHaveText('1');
		await navigation.getByRole('link', { name: 'คำขอจอง', exact: true }).click();
		const submitted = page.waitForRequest(
			(request) => new URL(request.url()).pathname === returnPath
		);
		await page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true }).click();
		await submitted;
		await page.route(
			(url) => url.pathname === observationPath,
			async (route) => {
				await gate.promise;
				await route.fallback();
			},
			{ times: 1 }
		);
		try {
			await navigation.getByRole('link', { name: 'ของฉัน', exact: true }).click();
			await expect(list.getByRole('status', { name: 'กำลังโหลดรายการนิเทศ' })).toBeVisible();
			await expect(list.locator('[data-supervision-own-list]')).toHaveCount(0);
			const returned = page.waitForResponse(
				(response) => new URL(response.url()).pathname === returnPath
			);
			if (first === 'read') {
				const received = page.waitForResponse(
					(response) => new URL(response.url()).pathname === observationPath
				);
				gate.release();
				await received;
				await expect(list).toHaveAttribute('aria-busy', 'true');
				await expect(list.locator('[data-supervision-own-list]')).toHaveCount(0);
			}
			api.release();
			await returned;
		} finally {
			api.release();
			gate.release();
		}
		await expect(list.getByText('ส่งกลับให้แก้ไข', { exact: true })).toBeVisible();
		await expect(list).toHaveAttribute('aria-busy', 'false');
		expect(api.count(observationPath)).toBe(4);
	});
}

for (const warm of [false, true]) {
	test(`booking patches Mine immediately with a ${warm ? 'warm' : 'fresh'} list`, async ({
		page
	}) => {
		const api = await mock(page, {
			term: true,
			permissions: [...own, 'supervision.manage.school'],
			bookingItems: [timetableOption(0)]
		});
		const initial = api.observation();
		const booked = {
			...initial,
			id: id(81),
			periods: [
				savedPeriod({ observationId: id(81), lessonSnapshot: { subjectName: 'คาบที่เพิ่งจอง' } })
			]
		};
		const other = {
			...initial,
			id: id(82),
			periods: [
				savedPeriod({
					observationId: id(82),
					lessonSnapshot: { subjectName: 'รายการอื่นจากการอ่านล่าสุด' }
				})
			]
		};
		const oldRead = deferred();
		const newRead = deferred();
		let bookedOnServer = false;
		let holdNext = false;
		let afterBookingReads = 0;
		await page.route(`**${observationPath}/requests`, (route) => {
			bookedOnServer = true;
			return route.fulfill({ json: { success: true, data: booked } });
		});
		await page.route(
			(url) => url.pathname === observationPath,
			async (route) => {
				if (bookedOnServer) {
					afterBookingReads += 1;
					await newRead.promise;
					return route.fulfill({
						json: { success: true, data: { items: [initial, booked, other] } }
					});
				}
				if (holdNext) {
					holdNext = false;
					await oldRead.promise;
					return route.fulfill({ json: { success: true, data: { items: [initial] } } });
				}
				await route.fallback();
			}
		);
		const navigation = page.getByRole('navigation', { name: 'เมนูนิเทศการสอน' });
		const list = page.getByTestId('supervision-observations');
		const requested = page
			.getByTestId('supervision-personal-status')
			.locator('dt', { hasText: /^รออนุมัติคำขอ$/ })
			.locator('xpath=following-sibling::dd');
		await page.goto(personalHref);
		await expect(requested).toHaveText('1');
		try {
			if (warm) {
				await navigation.getByRole('link', { name: 'แบบประเมิน', exact: true }).click();
				holdNext = true;
				await navigation.getByRole('link', { name: 'ของฉัน', exact: true }).click();
				await expect(list).toHaveAttribute('aria-busy', 'true');
				await expect(requested).toHaveText('1');
			}
			await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).click();
			await page.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ }).click();
			await page.getByRole('button', { name: 'ส่งคำขอจอง', exact: true }).click();
			await expect(
				list.getByRole('heading', { name: 'คาบที่เพิ่งจอง', exact: true })
			).toBeVisible();
			await expect(requested).toHaveText('2');
			if (warm) {
				await expect(list).toHaveAttribute('aria-busy', 'true');
				await expect.poll(() => afterBookingReads).toBe(1);
			}
		} finally {
			oldRead.release();
			newRead.release();
		}
		if (warm) {
			await expect(
				list.getByRole('heading', { name: 'รายการอื่นจากการอ่านล่าสุด', exact: true })
			).toBeVisible();
			await expect(requested).toHaveText('3');
		}
		await expect(list).toHaveAttribute('aria-busy', 'false');
		expect(afterBookingReads).toBe(warm ? 1 : 0);
	});
}

for (const hold of ['cycles', 'summaries'] as const) {
	test(`requests list renders independently of delayed ${hold}`, async ({ page }) => {
		const api = await mock(page, { hold });
		await page.goto(path('requests'));
		await expect(
			page.getByTestId('supervision-observations').getByText('ครู ในคิว', { exact: true }).first()
		).toBeVisible();
		const details = page.getByRole('button', { name: /ดูรายละเอียด .*คาบ/ }).first();
		await details.click();
		const loadingReference = page.getByRole('status', {
			name: hold === 'cycles' ? 'กำลังโหลดรอบนิเทศ' : 'กำลังโหลดแบบประเมิน',
			exact: true
		});
		await expect(loadingReference).toBeVisible();
		expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(0);
		expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
		api.release();
		await expect(loadingReference).toHaveCount(0);
		await expect(
			page.getByText(hold === 'cycles' ? /^รอบแรก/ : 'แบบประเมินทดสอบ').first()
		).toBeVisible();
	});
}

test('requests first list is a skeleton, with no false empty state', async ({ page }) => {
	const api = await mock(page, { hold: 'observations' });
	await page.goto(path('requests'));
	await expect(page.getByTestId('supervision-observations').getByRole('status')).toBeVisible();
	await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toHaveCount(0);
	api.release();
	await expect(page.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
});

test('queue retry rereads only its failed observation list', async ({ page }) => {
	const api = await mock(page, { fail: 'observations' });
	await page.goto(path('requests'));
	const region = page.getByTestId('supervision-observations');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	expect(api.count(observationPath)).toBe(2);
	expect(api.count('/api/supervision/cycles')).toBe(1);
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
});

test('queue refresh retains usable rows through an error', async ({ page }) => {
	const api = await mock(page, { fail: 'observations', failAt: 2 });
	await page.goto(path('requests'));
	const region = page.getByTestId('supervision-observations');
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	await revalidatePage(page);
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await expect(region.getByText('ครู ในคิว', { exact: true }).first()).toBeVisible();
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region.getByText('region ไม่พร้อม')).toHaveCount(0);
	expect(api.count(observationPath)).toBe(3);
	expect(api.count('/api/supervision/cycles')).toBe(2);
});

test('a late old-year list cannot overwrite the next year', async ({ page }) => {
	const api = await mock(page, { hold: 'observations' });
	await page.goto(path('requests'));
	await expect(page.getByTestId('supervision-observations').getByRole('status')).toBeVisible();
	await navigate(page, path('requests', nextYear));
	await expect(page.getByText('ครู ปีใหม่', { exact: true }).first()).toBeVisible();
	api.release();
	await expect(page.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);
});

test('returning a request supersedes a pending stale refresh', async ({ page }) => {
	const api = await mock(page, { hold: 'observations', holdAt: 2 });
	await page.goto(path('requests'));
	await expect(page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true })).toBeVisible();
	await revalidatePage(page);
	await expect(page.getByTestId('supervision-observations')).toHaveAttribute('aria-busy', 'true');
	await page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true }).click();
	await expect(page.getByText('ไม่มีคำขอจองที่รออนุมัติ')).toBeVisible();
	api.release();
	await expect(page.getByRole('button', { name: 'ส่งกลับคำขอ', exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(2);
});

test('evaluator picker loads on open and retries only its options', async ({ page }) => {
	const api = await mock(page, { fail: 'evaluator' });
	await page.goto(path('requests'));
	await page.getByRole('combobox', { name: /เลือกผู้ประเมิน/ }).click();
	await expect(page.getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ผู้ประเมินที่ว่าง', { exact: true })).toBeVisible();
	expect(api.count(observationPath)).toBe(1);
	expect(api.count(`/api/supervision/observations/${id(30)}/evaluator-availability`)).toBe(2);
});

test('evaluation rubric is lazy and a typed submit patches only the observation', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: assigned,
		observationStatus: 'planned',
		fail: 'detail'
	});
	await page.goto(path('evaluate'));
	await expect(page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true })).toBeVisible();
	expect(api.count(`/api/supervision/templates/${templateId}`)).toBe(0);
	await page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('region ไม่พร้อม')).toBeVisible();
	await dialog.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(dialog.getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	await dialog.getByRole('button', { name: '3', exact: true }).click();
	await dialog.getByRole('button', { name: 'ส่งผลประเมิน', exact: true }).click();
	await expect(page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(1);
	expect(api.count('/api/supervision/templates/summaries')).toBe(1);
});

test('closing evaluation supersedes its pending rubric', async ({ page }) => {
	const api = await mock(page, {
		permissions: assigned,
		hold: 'detail',
		observationStatus: 'planned'
	});
	await page.goto(path('evaluate'));
	await page.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	await expect(page.getByRole('dialog').getByRole('status')).toBeVisible();
	await page.keyboard.press('Escape');
	api.release();
	await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('write-only evaluation permission does not grant observation reads', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.evaluate.assigned'] });
	await page.goto(path('evaluate'));
	await expect(page.getByText('ยังไม่มีสิทธิ์ดูรายการนิเทศ')).toBeVisible();
	await expect(page.getByRole('tablist', { name: 'มุมมองรายการประเมิน' })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(0);
	expect(api.count('/api/supervision/cycles')).toBe(0);
	expect(api.count('/api/supervision/templates/summaries')).toBe(0);
});

test('an empty successful evaluation read keeps both views available', async ({ page }) => {
	const api = await mock(page, { permissions: assigned });
	api.setObservationScope('hidden');
	await page.goto(path('evaluate'));
	await expect(page.getByText('ยังไม่มีรายการที่ต้องประเมิน')).toBeVisible();
	await page.getByRole('tab', { name: 'ปฏิทิน', exact: true }).click();
	await expect(page.getByText('ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้')).toBeVisible();
	expect(api.count(observationPath)).toBe(1);
});

async function submitFromCalendar(page: Page) {
	await page.goto(path('evaluate'));
	await page.getByRole('tab', { name: 'ปฏิทิน', exact: true }).click();
	const calendar = page.getByRole('tabpanel', { name: 'ปฏิทิน', exact: true });
	const day = calendar.locator('[data-calendar-date="2026-10-01"]');
	await day.click();
	await expect(day).toHaveAttribute('aria-pressed', 'true');
	await calendar.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog.getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')).toBeVisible();
	await dialog.getByRole('button', { name: '3', exact: true }).click();
	const submit = dialog.getByRole('button', { name: 'ส่งผลประเมิน', exact: true });
	await expect(submit).toBeInViewport();
	const submitPath = `${observationPath}/${id(30)}/evaluations/me/submit`;
	const pending = page.waitForRequest((request) => new URL(request.url()).pathname === submitPath);
	await submit.click();
	return { calendar, day, dialog, request: await pending };
}

async function revalidateWhileEvaluating(dialog: Locator) {
	// Dispatch same-URL navigation from inside the modal while its submit is pending.
	await dialog.evaluate((content) => {
		const link = document.createElement('a');
		link.href = window.location.href;
		link.dataset.sveltekitPreloadData = 'off';
		content.append(link);
		link.click();
		link.remove();
	});
}

async function finishSuccessfulRequest(page: Page, request: Request) {
	const response = await request.response();
	expect(response?.ok()).toBe(true);
	await response?.finished();
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
}

test('a denied evaluation refresh closes the form and discards an in-flight submit result', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: assigned,
		observationStatus: 'planned',
		hold: 'patch',
		fail: 'observations',
		failAt: 2,
		failStatus: 403
	});
	const { calendar, day, dialog, request } = await submitFromCalendar(page);
	await revalidateWhileEvaluating(dialog);
	await expect(page.getByText('ยังไม่มีสิทธิ์ดูรายการนิเทศ')).toBeVisible();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByRole('tablist', { name: 'มุมมองรายการประเมิน' })).toHaveCount(0);
	await expect(page.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);

	api.setObservationScope('hidden');
	await revalidatePage(page);
	await expect(calendar.getByText('ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้')).toBeVisible();
	api.release();
	await finishSuccessfulRequest(page, request);
	await day.click();
	await expect(page.getByText('ส่งผลประเมินแล้ว', { exact: true })).toHaveCount(0);
	await expect(calendar.getByText('ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้')).toBeVisible();
	expect(api.count(observationPath)).toBe(3);
});

test('an evaluation submit overlapping a refresh still applies a fresh authorization decision', async ({
	page
}) => {
	const api = await mock(page, {
		permissions: assigned,
		observationStatus: 'planned',
		hold: 'patch'
	});
	const { calendar, dialog, request } = await submitFromCalendar(page);
	const releaseOlderRead = Promise.withResolvers<void>();
	const releaseFreshRead = Promise.withResolvers<void>();
	const staleObservation = api.observation();
	let refreshes = 0;
	await page.route(
		(url) => url.pathname === observationPath,
		async (route) => {
			if (route.request().method() !== 'GET') return route.fallback();
			if (++refreshes === 1) {
				await releaseOlderRead.promise;
				return route.fulfill({ json: { success: true, data: { items: [staleObservation] } } });
			}
			await releaseFreshRead.promise;
			return route.fulfill({ status: 403, json: { success: false, error: 'denied' } });
		}
	);
	const pendingRead = page.waitForRequest(
		(read) => read.method() === 'GET' && new URL(read.url()).pathname === observationPath
	);
	await revalidateWhileEvaluating(dialog);
	const olderRead = await pendingRead;
	await expect(page.getByTestId('supervision-observations')).toHaveAttribute('aria-busy', 'true');
	api.release();
	await finishSuccessfulRequest(page, request);
	await expect.poll(() => refreshes).toBe(2);
	await expect(dialog).toHaveCount(0);
	await expect(calendar.getByText('ส่งผลแล้ว', { exact: true })).toBeVisible();

	releaseFreshRead.resolve();
	const denied = page.getByText('ยังไม่มีสิทธิ์ดูรายการนิเทศ');
	await expect(denied).toBeVisible();
	releaseOlderRead.resolve();
	await finishSuccessfulRequest(page, olderRead);
	await expect(denied).toBeVisible();
	await expect(page.getByRole('tablist', { name: 'มุมมองรายการประเมิน' })).toHaveCount(0);
	await expect(page.getByText('ครู ในคิว', { exact: true })).toHaveCount(0);
	expect(api.count(observationPath)).toBe(1);
	expect(refreshes).toBe(2);
});

for (const closeBeforeRefresh of [false, true]) {
	test(`a successful evaluation refresh discards a withdrawn assignment with the form ${closeBeforeRefresh ? 'closed' : 'open'}`, async ({
		page
	}) => {
		const api = await mock(page, {
			permissions: assigned,
			observationStatus: 'planned',
			hold: 'patch'
		});
		const { calendar, day, dialog, request } = await submitFromCalendar(page);
		api.setObservationScope('hidden');
		if (closeBeforeRefresh) {
			await page.keyboard.press('Escape');
			await expect(dialog).toHaveCount(0);
			await revalidatePage(page);
		} else {
			await revalidateWhileEvaluating(dialog);
		}
		await expect(dialog).toHaveCount(0);
		await expect(day).toHaveAttribute('aria-pressed', 'true');
		const emptyAgenda = calendar.getByText('ไม่มีรายการนิเทศที่ได้รับมอบหมายในวันนี้');
		await expect(emptyAgenda).toBeVisible();
		api.release();
		await finishSuccessfulRequest(page, request);
		await expect(page.getByText('ส่งผลประเมินแล้ว', { exact: true })).toHaveCount(0);
		await expect(emptyAgenda).toBeVisible();
		expect(api.count(observationPath)).toBe(2);
	});
}

test('mine loads its list first and keeps booking timetable behind opening the workflow', async ({
	page
}) => {
	const api = await mock(page, { permissions: own, term: true, fail: 'timetable' });
	await page.goto(path('').replace('/supervision/?', '/supervision?'));
	await expect(
		page
			.getByTestId('supervision-observations')
			.getByRole('heading', { name: 'วิชาทดสอบ', exact: true })
	).toBeVisible();
	expect(api.count('/api/supervision/booking-options')).toBe(0);
	await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).click();
	await expect(page.getByText('region ไม่พร้อม')).toBeVisible();
	await page.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(page.getByText('ไม่พบคาบสอนในสัปดาห์นี้')).toBeVisible();
	expect(api.count('/api/supervision/booking-options')).toBe(2);
	expect(api.count(observationPath)).toBe(1);
});

test.describe('supervision at 360 pixels', () => {
	test.use({ viewport: { width: 360, height: 800 }, hasTouch: true });

	test('booking shows two complete lesson columns with the date pinned while scrolling', async ({
		page
	}) => {
		const longSubject = 'คณิตศาสตร์พื้นฐานและการแก้ปัญหาในชีวิตประจำวันสำหรับนักเรียนมัธยมศึกษา';
		const adjacentSubject = 'ภาษาไทยสำหรับนักเรียนมัธยมต้น';
		const adjacentRoom = 'ห้องปฏิบัติการวิทยาศาสตร์';
		const stackedSlot = {
			observedDate: '2026-09-30',
			startTime: '08:00:00',
			endTime: '09:00:00',
			periodName: 'คาบ 1'
		};
		await mock(page, {
			permissions: own,
			term: true,
			bookingItems: [
				timetableOption(0),
				timetableOption(1, {
					subjectName: adjacentSubject,
					roomName: adjacentRoom,
					bookingAvailability: { pendingCount: 2, confirmedCount: 1, blocked: false }
				}),
				timetableOption(2),
				timetableOption(3),
				timetableOption(4, { ...stackedSlot, subjectName: longSubject, homeroomName: 'ม.2/1' }),
				timetableOption(5, { ...stackedSlot, homeroomName: 'ม.2/2' })
			]
		});
		await page.goto(path('').replace('/supervision/?', '/supervision?'));
		await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).tap();
		const grid = page.getByTestId('supervision-booking-grid');
		const rowDates = grid.getByRole('rowheader').locator('time');
		const expectUniformDayRows = async () => {
			const heights = await grid
				.getByRole('rowheader')
				.evaluateAll((headers) =>
					headers.map((header) => header.closest('tr')!.getBoundingClientRect().height)
				);
			expect(heights).toHaveLength(5);
			expect(Math.max(...heights) - Math.min(...heights)).toBeLessThan(1);
		};
		await expect(rowDates).toHaveCount(5);
		expect(
			await rowDates.evaluateAll((dates) => dates.map((date) => date.getAttribute('datetime')))
		).toEqual(['2026-09-28', '2026-09-29', '2026-09-30', '2026-10-01', '2026-10-02']);
		await expectUniformDayRows();
		const stackedCell = grid.getByRole('cell').filter({ hasText: longSubject });
		await expect(stackedCell.getByRole('button')).toHaveCount(2);
		await expect(stackedCell.getByText(longSubject, { exact: true })).toBeVisible();
		expect(await stackedCell.evaluate((cell) => cell.scrollHeight <= cell.clientHeight)).toBe(true);
		const thursday = grid.locator('time[datetime="2026-10-01"]');
		await expect(thursday.locator('span')).toHaveText(['พฤหัส', '1 ต.ค.']);
		const weekday = await thursday.locator('span').first().boundingBox();
		const date = await thursday.locator('span').last().boundingBox();
		expect(date!.y).toBeGreaterThanOrEqual(weekday!.y + weekday!.height);

		const scrollport = grid.locator('[data-slot="table-container"]');
		const initial = await scrollport.boundingBox();
		const secondPeriod = await grid.getByRole('columnheader', { name: /^คาบ 2 / }).boundingBox();
		expect(secondPeriod!.x + secondPeriod!.width).toBeLessThanOrEqual(initial!.x + initial!.width);
		expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(360);
		const first = grid.getByRole('button', { name: /^เลือก คาบ 1 ม.1\/1/ });
		const second = grid.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ });
		const third = grid.getByRole('button', { name: /^เลือก คาบ 3 ม.1\/1/ });
		const fourth = grid.getByRole('button', { name: /^เลือก คาบ 4 ม.1\/1/ });
		const otherDay = grid.getByRole('button', { name: /^เลือก คาบ 1 ม.2\/1/ });
		const subjectLabel = second.getByText(adjacentSubject, { exact: true });
		const statusLabel = second.getByText('รออนุมัติ 2 · นัดหมาย 1 · จองขนานได้', { exact: true });
		const roomLabel = second.getByText(adjacentRoom, { exact: true });
		const cardSizes = () =>
			Promise.all(
				[second, subjectLabel, roomLabel, statusLabel].map(async (part) => {
					const { width, height } = (await part.boundingBox())!;
					return { width, height };
				})
			);
		await expect(grid.getByRole('button')).toHaveCount(6);
		await expect(third).toBeEnabled();
		await expect(otherDay).toBeEnabled();
		const sizesBefore = await cardSizes();
		await expect(first).toHaveAccessibleDescription(/วิชาปัจจุบัน 1.*ว่าง/);
		const unselectedFill = await first.evaluate(
			(button) => getComputedStyle(button).backgroundColor
		);
		await first.focus();
		await first.press('Enter');
		await expect(first).toHaveAttribute('aria-pressed', 'true');
		await expect(first).toBeEnabled();
		await expect
			.poll(() => first.evaluate((button) => getComputedStyle(button).backgroundColor))
			.not.toBe(unselectedFill);
		await expect(grid.getByText('เลือกแล้ว', { exact: true })).toHaveCount(0);
		await expect(grid.getByRole('button')).toHaveCount(6);
		await expect(third).toHaveJSProperty('disabled', true);
		await expect(otherDay).toHaveJSProperty('disabled', true);
		expect(await cardSizes()).toEqual(sizesBefore);
		await expectUniformDayRows();
		await page.keyboard.press('Tab');
		await expect(second).toBeFocused();
		await second.tap();
		const selection = page.getByTestId('supervision-booking-selection');
		await expect(selection).toContainText('2 คาบต่อเนื่อง');
		await expect(first).toHaveAttribute('aria-pressed', 'true');
		await expect(second).toHaveAttribute('aria-pressed', 'true');
		await expect(third).toBeEnabled();
		await expect(fourth).toHaveJSProperty('disabled', true);
		expect(await cardSizes()).toEqual(sizesBefore);
		await expectUniformDayRows();
		await second.tap();
		await expect(first).toHaveAttribute('aria-pressed', 'true');
		await expect(second).toHaveAttribute('aria-pressed', 'false');
		await expect(third).toBeDisabled();
		await expect(selection).toContainText('1 คาบต่อเนื่อง · 08:00–09:00');
		await second.tap();
		await first.press('Space');
		await expect(first).toHaveAttribute('aria-pressed', 'false');
		await expect(second).toHaveAttribute('aria-pressed', 'true');
		await expect(third).toBeEnabled();
		await expect(selection).toContainText('1 คาบต่อเนื่อง · 09:00–10:00');
		await second.tap();
		await expect(second).toHaveAttribute('aria-pressed', 'false');
		await expect(fourth).toBeEnabled();
		await expect(otherDay).toBeEnabled();
		await expect(selection).toContainText('ยังไม่ได้เลือกคาบ');
		await expect(selection.getByRole('button', { name: 'ส่งคำขอจอง', exact: true })).toBeDisabled();
		expect(await cardSizes()).toEqual(sizesBefore);
		await expectUniformDayRows();

		const pinned = grid.getByRole('columnheader', { name: 'วันที่' });
		const pinnedBefore = await pinned.boundingBox();
		const scrolled = await scrollport.evaluate((element) => {
			element.scrollLeft = element.scrollWidth;
			return element.scrollLeft;
		});
		expect(scrolled).toBeGreaterThan(0);
		const pinnedAfter = await pinned.boundingBox();
		expect(pinnedAfter?.x).toBe(pinnedBefore?.x);
	});

	test('booking uses the custom term timetable and keeps open days without lessons', async ({
		page
	}) => {
		await mock(page, {
			permissions: own,
			term: true,
			schoolDays: ['SUN', 'SAT', 'THU', 'TUE'],
			bookingItems: [
				timetableOption(0),
				timetableOption(1, { observedDate: '2026-10-03' }),
				timetableOption(2, { observedDate: '2026-10-02' })
			]
		});
		await page.goto(path('').replace('/supervision/?', '/supervision?'));
		await page.getByRole('button', { name: 'จองคาบนิเทศ', exact: true }).tap();
		const grid = page.getByTestId('supervision-booking-grid');
		const rowDates = grid.getByRole('rowheader').locator('time');
		await expect(rowDates).toHaveCount(4);
		expect(
			await rowDates.evaluateAll((dates) => dates.map((date) => date.getAttribute('datetime')))
		).toEqual(['2026-09-29', '2026-10-01', '2026-10-03', '2026-10-04']);
		const emptyTeachingDay = grid
			.getByRole('row')
			.filter({ has: page.locator('time[datetime="2026-09-29"]') });
		await expect(emptyTeachingDay).toBeVisible();
		await expect(emptyTeachingDay.getByRole('button')).toHaveCount(0);
		await expect(grid.getByRole('columnheader', { name: /^คาบ 3 / })).toHaveCount(0);
		const saturday = grid.getByRole('button', { name: /^เลือก คาบ 2 ม.1\/1/ });
		await saturday.tap();
		await expect(saturday).toHaveAttribute('aria-pressed', 'true');
	});

	test('evaluation calendar is as wide as the requests calendar and still opens the rubric', async ({
		page
	}) => {
		await mock(page, {
			permissions: ['supervision.manage.school', 'supervision.read.school', ...assigned],
			observationStatus: 'planned'
		});
		const tab = page.getByRole('tab', { name: 'ปฏิทิน', exact: true });
		const panel = page.getByRole('tabpanel', { name: 'ปฏิทิน', exact: true });
		const firstDate = panel.locator('[data-calendar-date="2026-09-27"]');
		const lastDate = panel.locator('[data-calendar-date="2026-10-03"]');
		await page.goto(path('requests'));
		await tab.tap();
		const requestsFirst = await firstDate.boundingBox();
		const requestsLast = await lastDate.boundingBox();
		await page.goto(path('evaluate'));
		await expect(
			page.getByRole('heading', { name: 'รายการที่ได้รับมอบหมายให้ประเมิน' })
		).toBeVisible();
		await tab.tap();
		const evaluationFirst = await firstDate.boundingBox();
		const evaluationLast = await lastDate.boundingBox();
		expect(evaluationFirst?.x).toBe(requestsFirst?.x);
		expect(evaluationLast!.x + evaluationLast!.width).toBe(requestsLast!.x + requestsLast!.width);
		const day = panel.locator('[data-calendar-date="2026-10-01"]');
		await day.tap();
		await expect(day).toHaveAttribute('aria-pressed', 'true');
		await panel.getByRole('button', { name: 'เปิดแบบประเมิน', exact: true }).tap();
		await expect(
			page.getByRole('dialog').getByText('หัวข้อ rubric ที่โหลดเมื่อเปิด')
		).toBeVisible();
	});
});

test('the retired approvals URL has no approval view or observation reads', async ({ page }) => {
	const api = await mock(page);
	await page.goto(path('approvals'));
	await expect(page.getByRole('heading', { name: 'ไม่พบรายการนิเทศ', exact: true })).toBeVisible();
	await expect(page.getByRole('link', { name: /รับรอง\/อนุมัติ/ })).toHaveCount(0);
	expect(api.reads.filter((value) => value.startsWith('/api/supervision/'))).toEqual([]);
});

test('denied queue route cannot preload an otherwise readable list', async ({ page }) => {
	const api = await mock(page, { permissions: ['supervision.read.own'] });
	await page.goto(path('requests', year));
	await expect(page).toHaveURL(/\/403/);
	expect(api.count(observationPath)).toBe(0);
});
