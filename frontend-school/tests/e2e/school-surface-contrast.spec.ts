import { expect, test, type Locator, type Page, type Route, type TestInfo } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { startSchoolAppearanceServer } from './fixtures/school-appearance-server';
import { mockStaffHome, actor, id, year } from './fixtures/staff-home-route-data';
import { mockStudentSchedules, academicOptions } from './fixtures/student-schedules-route-data';
import {
	DEFAULT_APPEARANCE,
	contrastRatio,
	type SchoolAppearance
} from '../../src/lib/appearance/model.js';
import type {
	AttendanceDetail,
	AttendanceSettings,
	AttendanceWorkspace,
	SaveAttendanceResults
} from '../../src/lib/api/attendance.js';

test.describe.configure({ mode: 'default' });
test.use({ serviceWorkers: 'block' });
let fixture: Awaited<ReturnType<typeof startSchoolAppearanceServer>>;
const artifacts = process.env.E2E_ARTIFACT_DIR;
const pageErrors = new Map<Page, string[]>();
const screenshotPath = (info: TestInfo, name: string) =>
	artifacts ? path.join(artifacts, name) : info.outputPath(name);
test.beforeAll(async () => {
	fixture = await startSchoolAppearanceServer();
	if (artifacts) await mkdir(artifacts, { recursive: true });
});
test.afterAll(async () => fixture?.close());
test.beforeEach(async ({ page }) => {
	fixture.reset();
	const errors: string[] = [];
	pageErrors.set(page, errors);
	page.on('pageerror', (error) => errors.push(error.message));
	await page.addInitScript(() => localStorage.removeItem('ui-preferences'));
});
test.afterEach(({ page }) => {
	expect(pageErrors.get(page)).toEqual([]);
	pageErrors.delete(page);
});
const appearances: { name: string; appearance: SchoolAppearance }[] = [
	...(['teal', 'blue', 'purple'] as const).map((palette) => ({
		name: palette,
		appearance: { ...DEFAULT_APPEARANCE, palette }
	})),
	...[
		['pale', '#dbeafe'],
		['white', '#ffffff'],
		['dark', '#111827']
	].map(([name, primary]) => ({ name, appearance: { ...DEFAULT_APPEARANCE, primary } }))
];
const widths = [390, 614, 1543];
const modes = ['light', 'dark'] as const;

// Resolve opacity over actual ancestor surfaces rather than assuming every control is on white.
async function paint(locator: Locator) {
	return locator.evaluate((element) => {
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 1;
		const context = canvas.getContext('2d')!;
		const hex = () =>
			'#' +
			Array.from(context.getImageData(0, 0, 1, 1).data.slice(0, 3))
				.map((channel) => channel.toString(16).padStart(2, '0'))
				.join('');
		const layer = (color: string) => {
			context.fillStyle = color;
			context.fillRect(0, 0, 1, 1);
		};
		layer('#ffffff');
		const ancestors: Element[] = [];
		for (let parent = element.parentElement; parent; parent = parent.parentElement)
			ancestors.unshift(parent);
		for (const parent of ancestors) layer(getComputedStyle(parent).backgroundColor);
		const surface = hex(),
			css = getComputedStyle(element);
		layer(css.backgroundColor);
		const background = hex();
		layer(css.color);
		const foreground = hex();
		layer(background);
		layer(css.borderTopColor);
		return { background, foreground, border: hex(), surface, shadow: css.boxShadow };
	});
}
async function boundary(locator: Locator) {
	await expect(locator).toBeVisible();
	const colors = await paint(locator);
	expect(
		contrastRatio(colors.border, colors.surface),
		JSON.stringify(colors)
	).toBeGreaterThanOrEqual(3);
	return colors;
}
async function readable(locator: Locator) {
	const colors = await paint(locator);
	expect(
		contrastRatio(colors.foreground, colors.background),
		JSON.stringify(colors)
	).toBeGreaterThanOrEqual(4.5);
	return colors;
}
async function settle(locator: Locator) {
	await locator.evaluate(() => new Promise(requestAnimationFrame));
	await expect
		.poll(() =>
			locator.evaluate(
				(element) =>
					element
						.getAnimations()
						.filter(
							(animation) =>
								animation.playState === 'running' &&
								animation.effect?.getComputedTiming().iterations !== Infinity
						).length
			)
		)
		.toBe(0);
}
async function selected(locator: Locator) {
	await settle(locator);
	const before = await readable(locator);
	expect(before.shadow).toContain('inset');
	await locator.hover();
	await settle(locator);
	await expect.poll(async () => (await paint(locator)).background).toBe(before.background);
	expect((await paint(locator)).shadow).toContain('inset');
}
async function open(page: Page, path: string, mode: 'light' | 'dark', width: number) {
	await page.setViewportSize({ width, height: 1000 });
	await page.emulateMedia({ colorScheme: mode, reducedMotion: 'reduce' });
	const response = await page.goto(`${fixture.baseUrl}${path}`);
	expect(response?.status()).toBe(200);
	if (path === '/__appearance-primitives') {
		await expect(page.getByTestId('controls-card')).toBeVisible();
		await page.evaluate(
			(dark) => document.documentElement.classList.toggle('dark', dark),
			mode === 'dark'
		);
	} else await expect(page.locator('html')).toHaveAttribute('data-schoolorbit-app-mounted', 'true');
	await expect(page.locator('body')).toBeVisible();
	await expect
		.poll(() => page.locator('html').evaluate((element) => element.classList.contains('dark')))
		.toBe(path === '/' || path === '/login' ? false : mode === 'dark');
}

for (const { name, appearance } of appearances) {
	test(`${name} shared controls keep visible boundaries, focus and selected markers`, async ({
		page
	}) => {
		test.setTimeout(120_000);
		fixture.set({ appearance, revision: 9 });
		for (const mode of modes)
			for (const width of widths) {
				await open(page, '/__appearance-primitives', mode, width);
				const card = await paint(page.getByTestId('controls-card'));
				expect(contrastRatio(card.background, card.surface)).toBeGreaterThanOrEqual(1.1);
				expect((await paint(page.getByTestId('controls-tray'))).background).not.toBe(
					card.background
				);
				for (const name of [
					'surface-input',
					'surface-textarea',
					'surface-select',
					'surface-outline',
					'surface-secondary',
					'surface-checkbox',
					'surface-radio',
					'surface-switch',
					'tray-input'
				])
					await boundary(page.getByTestId(name));
				for (const name of [
					'surface-outline',
					'surface-secondary',
					'surface-select',
					'surface-muted-text'
				])
					await readable(page.getByTestId(name));
				await expect(page.getByTestId('surface-disabled')).toBeDisabled();
				const input = page.getByTestId('surface-input');
				await input.focus();
				expect((await boundary(input)).shadow).not.toBe('none');
				await page.keyboard.type('รายการทดสอบ');
				await expect(input).toHaveValue('รายการทดสอบ');
				for (const name of ['surface-secondary', 'surface-pressed']) {
					const button = page.getByTestId(name);
					if ((await button.getAttribute('aria-pressed')) !== 'true') await button.click();
					await selected(button);
				}
				for (const name of ['surface-current', 'surface-tab', 'surface-toggle'])
					await selected(page.getByTestId(name));
				for (const name of ['surface-checkbox', 'surface-switch']) {
					const control = page.getByTestId(name);
					await control.click();
					await expect(control).toBeChecked();
					await boundary(control);
				}
				const primary = await readable(page.getByTestId('primary-button'));
				if (appearance.primary) expect(primary.background).toBe(appearance.primary);
				await boundary(page.getByTestId('primary-button'));
			}
	});
}

const term = academicOptions.activeAcademicTermId!,
	room = id(901),
	pupil = id(902),
	round = id(903),
	date = '2026-10-09';
const attendancePath = `/staff/attendance?academicYearId=${year}&academicTermId=${term}&date=${date}`;
const reply = (route: Route, data: unknown) =>
	route.fulfill({ json: { success: true, data }, headers: { 'X-CSRF-Token': 'synthetic-csrf' } });
// Reuse the staff shell and typed attendance responses. Domain concurrency/camera tests stay in attendance.spec.ts.
async function mockAttendanceSurface(page: Page) {
	await mockStaffHome(page, {
		permissions: [
			'attendance.read.assigned',
			'attendance.update.assigned',
			'attendance.manage.school',
			'academic_context.read.school'
		]
	});
	await page.route(
		(url) => url.pathname === '/api/school/public/appearance',
		(route) => route.continue()
	);
	await page.route(
		(url) =>
			['/api/academic/context/options', '/api/me/academic-context/options'].includes(url.pathname),
		(route) => reply(route, academicOptions)
	);
	const settings: AttendanceSettings = {
		academicTermId: term,
		rowVersion: 1,
		archived: false,
		autoFinalizeFrom: date,
		configuration: {
			enabled: true,
			weekdays: [1, 2, 3, 4, 5],
			lateAfter: '08:00:00',
			digestTimes: ['16:00:00'],
			evidenceDays: 30,
			faceDistance: 0.45,
			faceMargin: 0.1,
			activityCountsAsPresent: true
		}
	};
	let detail: AttendanceDetail = {
		session: {
			id: round,
			academicTermId: term,
			date,
			kind: 'arrival',
			sourceKey: room,
			title: 'เข้าโรงเรียน ม.1/1',
			teacherIds: [actor],
			homeroomId: room,
			learningGroupId: null,
			offeringId: null,
			specialRoundId: null,
			startTime: '08:00:00',
			endTime: '08:15:00',
			countOverride: null,
			cancelled: false,
			cancellationReason: null,
			savedAt: null,
			savedBy: null,
			rowVersion: 1,
			rosterToken: 'synthetic-surface-roster'
		},
		counted: true,
		writable: true,
		students: [
			{
				studentId: pupil,
				studentAcademicYearId: id(904),
				displayName: 'นักเรียนทดสอบ',
				classNumber: 1,
				result: 'unchecked',
				origin: 'none',
				observedAt: null,
				processedAt: null,
				evidenceFileId: null,
				arrivalAt: null,
				arrivalResult: null,
				rowVersion: 1
			}
		]
	};
	const writes: SaveAttendanceResults[] = [];
	await page.route(
		(url) => url.pathname.startsWith('/api/attendance/'),
		async (route) => {
			const path = new URL(route.request().url()).pathname;
			if (path.endsWith('/workspace'))
				return reply(route, {
					date,
					counted: true,
					lessonChoices: [],
					sessions: [detail.session],
					settings
				} satisfies AttendanceWorkspace);
			if (path === `/api/attendance/sessions/${round}`) {
				if (route.request().method() === 'PUT') {
					const saved = route.request().postDataJSON() as SaveAttendanceResults;
					writes.push(saved);
					detail = {
						...detail,
						session: {
							...detail.session,
							rowVersion: detail.session.rowVersion + 1,
							savedAt: `${date}T01:00:00Z`,
							savedBy: actor
						},
						students: detail.students.map((row) => ({
							...row,
							result:
								saved.students.find((entry) => entry.studentId === row.studentId)?.result ??
								row.result
						}))
					};
				}
				return reply(route, detail);
			}
			if (path.endsWith('/options'))
				return reply(route, {
					students: [],
					teachers: [],
					audiences: [],
					specials: [],
					devices: []
				});
			throw new Error(`Unexpected attendance appearance request: ${path}`);
		}
	);
	return { writes };
}
for (const { name, appearance } of appearances) {
	test(`${name} real attendance keeps active navigation and rounds distinct while editing and saving`, async ({
		page
	}, testInfo) => {
		test.setTimeout(120_000);
		fixture.set({ appearance, revision: 9 });
		const { writes } = await mockAttendanceSurface(page);
		for (const mode of modes)
			for (const width of widths) {
				await open(page, attendancePath, mode, width);
				await selected(
					page
						.getByRole('navigation', { name: 'เมนูเช็คชื่อ' })
						.getByRole('link', { name: 'เช็คชื่อ', exact: true })
				);
				const kind = page.getByRole('group', { name: 'ประเภทรอบเช็คชื่อ' });
				await selected(kind.getByRole('button', { name: 'เข้าโรงเรียน', exact: true }));
				const unselected = kind.getByRole('button', { name: 'รายคาบ', exact: true }),
					ordinary = await boundary(unselected);
				await unselected.hover();
				await expect
					.poll(async () => (await paint(unselected)).background)
					.not.toBe(ordinary.background);
				expect((await paint(unselected)).shadow).not.toContain('inset');
				const roundButton = page.getByRole('button', { name: /08:00.*เข้าโรงเรียน ม.1\/1/ });
				if (width < 1280) {
					await page.getByRole('button', { name: 'รอบเช็คชื่อ', exact: true }).click();
					await page.getByRole('option', { name: /08:00.*เข้าโรงเรียน ม.1\/1/ }).click();
				} else {
					await roundButton.click();
					await expect(roundButton).toHaveAttribute('aria-pressed', 'true');
					await selected(roundButton);
				}
				const results = page.getByRole('group', { name: 'ผลของ นักเรียนทดสอบ', exact: true });
				await expect(results).toBeVisible();
				const startedPresent = await results
					.getByRole('radio', { name: 'มา', exact: true })
					.isChecked();
				if (width === 1543) {
					for (const [label, tone] of [
						['มา', 'success'],
						['สาย', 'warning'],
						['ขาด', 'destructive'],
						['หนีเรียน', 'destructive'],
						['ลาป่วย', 'primary'],
						['ลากิจ', 'primary'],
						['กิจกรรม', 'chart-4']
					]) {
						const status = results.getByRole('radio', { name: label, exact: true });
						await status.click();
						await expect(status).toBeChecked();
						await settle(status);
						const before = await paint(status);
						// Compare against the existing domain tone on its actual parent, independent of school brand.
						const expected = await status.evaluate(
							(element, { tone, mode }) => {
								const probe = document.createElement('span');
								probe.style.backgroundColor = `color-mix(in oklab, var(--${tone}) ${mode === 'dark' ? 20 : 15}%, transparent)`;
								element.parentElement!.append(probe);
								const value = getComputedStyle(probe).backgroundColor;
								probe.remove();
								return value;
							},
							{ tone, mode }
						);
						await page.mouse.move(0, 0);
						await status.hover();
						await settle(status);
						const hover = await readable(status);
						expect(hover.foreground).toBe(before.foreground);
						expect(hover.shadow).toContain('inset');
						const ink = await status.evaluate((element) => getComputedStyle(element).color);
						expect(hover.shadow).toContain(ink);
						expect(
							await status.evaluate((element) => getComputedStyle(element).backgroundColor)
						).toBe(expected);
					}
				}
				const present = results.getByRole('radio', { name: 'มา', exact: true });
				const changedToLate = startedPresent;
				const result = changedToLate
					? results.getByRole('radio', { name: 'สาย', exact: true })
					: present;
				await result.click();
				await expect(result).toBeChecked();
				await settle(result);
				const chosen = await readable(result),
					neutral = await readable(results.getByRole('radio', { name: 'ขาด', exact: true }));
				expect(chosen.background).not.toBe(neutral.background);
				const reason = page.getByLabel('เหตุผลแก้ไข / งดคาบ', { exact: true });
				await reason.fill('ปรับผลจากข้อมูลทดสอบ');
				await boundary(reason);
				const save = page.getByRole('button', { name: 'บันทึก', exact: true });
				await expect(save).toBeEnabled();
				await readable(save);
				await boundary(save);
				if (width === widths[0]) {
					const count = writes.length;
					await save.click();
					await expect.poll(() => writes.length).toBe(count + 1);
					expect(writes.at(-1)?.students).toContainEqual(
						expect.objectContaining({
							studentId: pupil,
							result: changedToLate ? 'late' : 'present'
						})
					);
					await expect(result).toBeChecked();
				}
				expect(
					await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 2)
				).toBe(true);
				await page.getByRole('main').evaluate((element) => {
					element.scrollTop = 0;
				});
				await page.screenshot({
					path: screenshotPath(testInfo, `attendance-${name}-${mode}-${width}.png`),
					fullPage: true
				});
			}
	});
}
for (const role of ['public', 'staff', 'student'] as const) {
	test(`${role} consumers retain readable shared actions and school typography`, async ({
		page
	}, testInfo) => {
		test.setTimeout(90_000);
		fixture.set({
			appearance: { ...DEFAULT_APPEARANCE, primary: '#dbeafe', font: 'sarabun' },
			revision: 9
		});
		if (role === 'student') await mockStudentSchedules(page);
		else if (role === 'staff') await mockStaffHome(page);
		if (role !== 'public')
			await page.route(
				(url) => url.pathname === '/api/school/public/appearance',
				(route) => route.continue()
			);
		for (const mode of modes)
			for (const width of widths) {
				await open(page, role === 'public' ? '/' : `/${role}`, mode, width);
				if (role === 'student')
					await expect(page.getByTestId('student-profile-region')).toBeVisible();
				if (role === 'staff') await expect(page.getByTestId('staff-work-counts')).toBeVisible();
				if (role === 'public') await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
				expect(
					await page.locator('body').evaluate((element) => getComputedStyle(element).fontFamily)
				).toContain('School Sarabun');
				const actions = page.locator('main [data-slot="button"]').filter({ visible: true });
				expect(await actions.count()).toBeGreaterThan(0);
				await readable(actions.first());
				expect(
					await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 2)
				).toBe(true);
				await page.getByRole('main').evaluate((element) => {
					element.scrollTop = 0;
				});
				await page.screenshot({
					path: screenshotPath(testInfo, `${role}-system-${mode}-${width}.png`),
					fullPage: true
				});
			}
	});
}
