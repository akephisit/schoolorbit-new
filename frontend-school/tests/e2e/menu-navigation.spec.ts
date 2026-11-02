import { readFile, readdir } from 'node:fs/promises';
import { expect, test, type Locator, type Page, type Route } from '@playwright/test';
import type { MenuGroup } from '../../src/lib/api/menu.js';
import { mockStaffHome, nextYear, year } from './fixtures/staff-home-route-data';

test.use({ serviceWorkers: 'block' });

const homeroomPath = '/staff/academic/homerooms';
const studentYearPath = '/staff/academic/student-years';
const homeroomLabel = 'เปิดห้องประจำชั้น';
const studentYearLabel = 'เปิดนักเรียนประจำปี';
const menuGroups: MenuGroup[] = [
	{
		code: 'navigation-fixture',
		name: 'เมนูทดสอบ',
		icon: 'Circle',
		workspaceCode: 'home',
		workspaceName: 'งานของฉัน',
		workspaceIcon: 'Home',
		workspaceOrder: 1,
		displayOrder: 1,
		items: [
			{ id: 'rooms', name: homeroomLabel, path: homeroomPath, displayOrder: 1 },
			{ id: 'students', name: studentYearLabel, path: studentYearPath, displayOrder: 2 }
		].map((item) => ({ ...item, code: item.id, icon: 'School' }))
	}
];

function deferred() {
	let release = () => {};
	const promise = new Promise<void>((resolve) => (release = resolve));
	return { promise, release };
}

function fulfill(route: Route, data: unknown) {
	return route.fulfill({
		contentType: 'application/json',
		body: JSON.stringify({ success: true, data })
	});
}

async function mockNavigation(page: Page, permissions = ['*']) {
	await mockStaffHome(page, { permissions, menuGroups });
	const state = {
		homeroomGate: null as Promise<void> | null,
		homeroomReads: 0,
		workGate: null as Promise<void> | null,
		workReads: 0
	};
	await page.route(
		(url) =>
			url.pathname.startsWith('/api/academic/') &&
			!url.pathname.startsWith('/api/academic/context/'),
		async (route) => {
			if (new URL(route.request().url()).pathname === '/api/academic/homerooms') {
				state.homeroomReads++;
				await state.homeroomGate;
				await fulfill(route, [
					{
						id: '30000000-0000-4000-8000-000000000001',
						name: 'ห้องที่โหลดเสร็จ',
						academicYearId: year,
						gradeLevelId: '40000000-0000-4000-8000-000000000001',
						studyProgramId: '50000000-0000-4000-8000-000000000001',
						roomNumber: '1',
						capacity: 40,
						rowVersion: 1
					}
				]);
			} else await fulfill(route, []);
		}
	);
	await page.route('**/api/lookup/**', (route) => fulfill(route, []));
	await page.route('**/api/me/work-items', async (route) => {
		state.workReads++;
		await state.workGate;
		await fulfill(route, []);
	});
	await page.route('**/api/correspondence/options**', (route) =>
		fulfill(route, { templates: [], organizations: [], users: [], roles: [] })
	);
	return state;
}

// Resolve the actual production route module from Kit's generated node and Vite manifest.
// Holding this module reproduces the interval before the destination component can render.
async function holdPageModule(page: Page, pathname: string) {
	const directory = '.svelte-kit/generated/client-optimized/nodes';
	const nodes = await Promise.all(
		(await readdir(directory)).map(async (file) => ({
			file,
			source: await readFile(`${directory}/${file}`, 'utf8')
		}))
	);
	const node = nodes.find((entry) =>
		entry.source.includes(`/routes/(app)${pathname}/+page.svelte`)
	);
	expect(node, `production node for ${pathname}`).toBeDefined();
	const manifest: Record<string, { file: string }> = JSON.parse(
		await readFile('.svelte-kit/output/client/.vite/manifest.json', 'utf8')
	);
	const modulePath = `/${manifest[`${directory}/${node!.file}`].file}`;
	const gate = deferred();
	let requested = false;
	await page.route(
		(url) => url.pathname === modulePath,
		async (route) => {
			requested = true;
			await gate.promise;
			await route.continue();
		}
	);
	return { ...gate, requested: () => requested };
}

async function openSidebar(page: Page) {
	const button = page.getByRole('button', { name: 'Open Menu', exact: true });
	if (await button.isVisible()) await button.click();
}

async function expectReadableSelection(link: Locator) {
	const result = await link.evaluate((element) => {
		const style = getComputedStyle(element);
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 1;
		const context = canvas.getContext('2d')!;
		function rgb(color: string) {
			context.clearRect(0, 0, 1, 1);
			context.fillStyle = color;
			context.fillRect(0, 0, 1, 1);
			return [...context.getImageData(0, 0, 1, 1).data];
		}
		function luminance(channels: number[]) {
			const linear = channels.slice(0, 3).map((channel) => {
				const value = channel / 255;
				return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
			});
			return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
		}
		let background = [255, 255, 255];
		const layers: number[][] = [];
		for (let current: Element | null = element; current; current = current.parentElement)
			layers.unshift(rgb(getComputedStyle(current).backgroundColor));
		for (const layer of layers) {
			const alpha = layer[3] / 255;
			background = background.map((channel, index) => layer[index] * alpha + channel * (1 - alpha));
		}
		const foregroundLuminance = luminance(rgb(style.color));
		const backgroundLuminance = luminance(background);
		return {
			contrast:
				(Math.max(foregroundLuminance, backgroundLuminance) + 0.05) /
				(Math.min(foregroundLuminance, backgroundLuminance) + 0.05),
			transition: style.transitionProperty,
			labelTransition: getComputedStyle(element.querySelector('span')!).transitionProperty
		};
	});
	expect(result.contrast).toBeGreaterThanOrEqual(4.5);
	expect(result.transition).toBe('none');
	expect(result.labelTransition).not.toMatch(/all|color/);
}

for (const width of [390, 1440])
	for (const theme of ['light', 'dark'] as const)
		test(`${width}px ${theme}: destination renders before its module and streamed data`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: 900 });
			await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
			const api = await mockNavigation(page);
			const module = await holdPageModule(page, homeroomPath);
			const data = deferred();
			api.homeroomGate = data.promise;
			try {
				await page.goto('/staff/work');
				const source = page
					.locator('main')
					.getByRole('heading', { name: 'งานของฉัน', exact: true });
				await expect(source).toHaveText('งานของฉัน');
				const sourceElement = await source.elementHandle();
				await openSidebar(page);
				const link = page.locator('aside').getByRole('link', { name: homeroomLabel, exact: true });
				await expect(link).toHaveAttribute('href', `${homeroomPath}?academicYearId=${year}`);
				await link.click();
				await expect.poll(module.requested).toBe(true);
				const pending = page.getByTestId('menu-navigation-pending');
				await expect(
					pending.getByRole('heading', { name: homeroomLabel, exact: true })
				).toBeVisible();
				await expect(pending.locator('[data-slot="skeleton"]')).not.toHaveCount(0);
				await expect(source).not.toBeVisible();
				expect(
					await sourceElement!.evaluate(
						(element) => element.isConnected && !!element.closest('[hidden][inert]')
					)
				).toBe(true);
				await expect(link).toHaveAttribute('aria-busy', 'true');
				await expect(link).not.toHaveAttribute('aria-current');
				await expectReadableSelection(link);
				await page.screenshot({ path: testInfo.outputPath('pending-navigation.png') });
				module.release();
				await expect(page).toHaveURL(new RegExp(`${homeroomPath}\\?academicYearId=${year}`));
				await expect(pending).toHaveCount(0);
				await expect(
					page.locator('main').getByRole('heading', { name: 'ห้องประจำชั้น', exact: true })
				).toBeVisible();
				await expect(page.getByTestId('homerooms-ready')).toHaveCount(0);
				await expect(page.locator('main [data-slot="skeleton"]')).not.toHaveCount(0);
				await expect(link).toHaveAttribute('aria-current', 'page');
				data.release();
				await expect(page.getByText('ห้องที่โหลดเสร็จ', { exact: true })).toBeVisible();
				await expect(page.locator('main h1')).toHaveCount(1);
				await page.screenshot({ path: testInfo.outputPath('loaded-navigation.png') });
				if (width > 1024) {
					await link.hover();
					await expectReadableSelection(link);
				}
			} finally {
				module.release();
				data.release();
			}
		});

test('a superseded delayed module cannot replace the latest menu destination', async ({ page }) => {
	await mockNavigation(page);
	const module = await holdPageModule(page, homeroomPath);
	try {
		await page.goto('/staff/work');
		await page.locator('aside').getByRole('link', { name: homeroomLabel, exact: true }).click();
		await expect(page.getByTestId('menu-navigation-pending')).toContainText(homeroomLabel);
		await page.locator('aside').getByRole('link', { name: studentYearLabel, exact: true }).click();
		await expect(page.getByTestId('student-years-ready')).toBeVisible();
		await expect(page).toHaveURL(new RegExp(studentYearPath));
		module.release();
		await expect(page.locator('main h1')).toHaveText('นักเรียนประจำปี');
		await expect(page.getByTestId('menu-navigation-pending')).toHaveCount(0);
	} finally {
		module.release();
	}
});

test('canceling a real dirty form navigation retains the draft and committed selection', async ({
	page
}) => {
	await mockNavigation(page);
	await page.goto('/staff/correspondence/new');
	const title = page.getByLabel('เรื่อง *', { exact: true });
	await title.fill('ฉบับร่างที่ต้องเก็บไว้');
	await page.locator('aside').getByRole('link', { name: homeroomLabel, exact: true }).click();
	await expect(page.getByRole('alertdialog')).toContainText('มีข้อมูลที่ยังไม่ได้บันทึก');
	await expect(page.getByTestId('menu-navigation-pending')).toHaveCount(0);
	await page.getByRole('button', { name: 'อยู่หน้านี้ต่อ', exact: true }).click();
	await expect(title).toHaveValue('ฉบับร่างที่ต้องเก็บไว้');
	await expect(page).toHaveURL(/\/staff\/correspondence\/new$/);
	await expect(
		page.locator('aside').getByRole('link', { name: homeroomLabel, exact: true })
	).not.toHaveAttribute('aria-busy');
});

test('same-path academic context and hash navigation keep the page-owned region', async ({
	page
}) => {
	const api = await mockNavigation(page);
	await page.goto(`${homeroomPath}?academicYearId=${year}`);
	await expect(page.getByTestId('homerooms-ready')).toBeVisible();
	const gate = deferred();
	api.homeroomGate = gate.promise;
	try {
		await page.getByLabel('เลือกปีการศึกษา', { exact: true }).click();
		await page.getByRole('option', { name: /2570/ }).click();
		await expect(page).toHaveURL(new RegExp(`academicYearId=${nextYear}`));
		await expect(page.getByTestId('menu-navigation-pending')).toHaveCount(0);
		await expect(page.locator('main h1')).toHaveText('ห้องประจำชั้น');
		await page.evaluate(() => {
			const link = document.createElement('a');
			link.href = '#menu-navigation-anchor';
			link.id = 'menu-navigation-hash';
			link.textContent = 'ไปจุดเดิม';
			document.body.append(link);
		});
		await page.locator('#menu-navigation-hash').click();
		await expect(page).toHaveURL(/#menu-navigation-anchor$/);
		await expect(page.getByTestId('menu-navigation-pending')).toHaveCount(0);
	} finally {
		gate.release();
	}
});

test('an unauthorized menu destination never exposes its temporary shell', async ({ page }) => {
	await mockNavigation(page, []);
	const module = await holdPageModule(page, homeroomPath);
	try {
		await page.goto('/staff/work');
		await expect(page.locator('main h1')).toHaveText('งานของฉัน');
		await page.locator('aside').getByRole('link', { name: homeroomLabel, exact: true }).click();
		await expect.poll(module.requested).toBe(true);
		await expect(page.getByTestId('menu-navigation-pending')).toHaveCount(0);
		await expect(page.locator('main h1')).toHaveText('งานของฉัน');
	} finally {
		module.release();
	}
	await expect(page).toHaveURL(/\/403\?/);
});

test('collapsed flyout supports keyboard selection and immediate destination feedback', async ({
	page
}, testInfo) => {
	await mockNavigation(page);
	const module = await holdPageModule(page, homeroomPath);
	try {
		await page.goto('/staff/work');
		await expect(page.locator('main h1')).toHaveText('งานของฉัน');
		await page.getByRole('button', { name: 'Collapse sidebar', exact: true }).click();
		const workspace = page.locator('aside').getByRole('button', { name: 'งานของฉัน', exact: true });
		await workspace.focus();
		await page.keyboard.press('Enter');
		const item = page.getByRole('menuitem', { name: homeroomLabel, exact: true });
		await item.focus();
		await page.keyboard.press('Enter');
		await expect(page.getByTestId('menu-navigation-pending')).toContainText(homeroomLabel);
		await expect(workspace).toHaveClass(/bg-primary/);
		await expect(page.getByRole('menu')).toHaveCount(0);
		await page.screenshot({ path: testInfo.outputPath('collapsed-pending-navigation.png') });
	} finally {
		module.release();
	}
	await expect(page.getByTestId('homerooms-ready')).toBeVisible();
});

test('resolved hover preload is reused and shows the ready destination without a forced delay', async ({
	page
}) => {
	const api = await mockNavigation(page);
	await page.goto('/staff/work');
	await expect(page.locator('main h1')).toHaveText('งานของฉัน');
	const link = page.locator('aside').getByRole('link', { name: homeroomLabel, exact: true });
	const response = page.waitForResponse(
		(result) => new URL(result.url()).pathname === '/api/academic/homerooms'
	);
	await link.hover();
	await response;
	await expect.poll(() => api.homeroomReads).toBe(1);
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
	await link.click();
	await expect(page.getByTestId('homerooms-ready')).toBeVisible();
	await expect(page.getByText('ห้องที่โหลดเสร็จ', { exact: true })).toBeVisible();
	await expect(page.getByTestId('menu-navigation-pending')).toHaveCount(0);
	expect(api.homeroomReads).toBe(1);
});
