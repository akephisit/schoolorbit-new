import { expect, test, type Locator, type Page } from '@playwright/test';
import type { TimetableBlockWorkspace } from '#lib/api/timetable.js';
import {
	installTimetableMock,
	makeSynchronizedTimetableBlock,
	makeTimetableBlock,
	timetableIds
} from './timetable-test-harness.js';

test.use({ serviceWorkers: 'block', viewport: { width: 1920, height: 1080 } });

function boardUrl(view = 'homeroom', ownerId: string = timetableIds.homeroom): string {
	return `/staff/academic/timetable?academicYearId=${timetableIds.year}&academicTermId=${timetableIds.term}&timetableVersionId=${timetableIds.draftVersion}&view=${view}&ownerId=${ownerId}`;
}

function group(
	id: string,
	offeringKind: 'course' | 'activity' = 'course'
): TimetableBlockWorkspace['learningGroups'][number] {
	return {
		id,
		learningOfferingId: timetableIds.offeringA,
		code: `GROUP-${id}`,
		name: 'ม.1/1 คณิตศาสตร์',
		status: 'published',
		rosterStatus: 'published',
		offeringKind,
		offeringCode: offeringKind === 'course' ? 'ค21101' : 'GUIDANCE',
		offeringName: offeringKind === 'course' ? 'คณิตศาสตร์พื้นฐาน' : 'แนะแนว',
		homeroomIds: [timetableIds.homeroom],
		preferredRoomIds: [],
		eligibleInstructors: [
			{
				teacherId: timetableIds.teacherA,
				displayName: 'ครูคณิตศาสตร์ A',
				role: 'primary',
				orderIndex: 0
			}
		]
	};
}

function demand(
	learningGroup: ReturnType<typeof group>
): TimetableBlockWorkspace['ordinaryDemands'][number] {
	return {
		learningGroupId: learningGroup.id,
		learningOfferingId: learningGroup.learningOfferingId,
		offeringCode: learningGroup.offeringCode,
		offeringName: learningGroup.offeringName,
		requiredPeriods: 3,
		scheduledPeriods: 0,
		remainingPeriods: 3,
		homeroomIds: learningGroup.homeroomIds,
		eligibleInstructors: learningGroup.eligibleInstructors
	};
}

async function setTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
	await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
	const toggle = page.getByRole('button', { name: 'Toggle Dark Mode', exact: true });
	await expect(toggle).toBeVisible();
	const storedTheme = await page.evaluate(
		() => JSON.parse(localStorage.getItem('ui-preferences') ?? '{}').theme
	);
	if (storedTheme !== theme) {
		const dark = await page
			.locator('html')
			.evaluate((element) => element.classList.contains('dark'));
		if (dark === (theme === 'dark')) {
			await toggle.click();
			if (theme === 'dark') await expect(page.locator('html')).not.toHaveClass(/\bdark\b/);
			else await expect(page.locator('html')).toHaveClass(/\bdark\b/);
		}
		await toggle.click();
	}
	await assertTheme(page, theme);
}

async function assertTheme(page: Page, theme: 'light' | 'dark'): Promise<void> {
	if (theme === 'dark') await expect(page.locator('html')).toHaveClass(/\bdark\b/);
	else await expect(page.locator('html')).not.toHaveClass(/\bdark\b/);
	await expect
		.poll(() =>
			page.evaluate(() => JSON.parse(localStorage.getItem('ui-preferences') ?? '{}').theme)
		)
		.toBe(theme);
	const tokenBrightness = await page.evaluate(() => {
		const probe = document.createElement('span');
		probe.style.color = 'var(--background)';
		document.body.append(probe);
		const context = document.createElement('canvas').getContext('2d')!;
		context.fillStyle = getComputedStyle(probe).color;
		context.fillRect(0, 0, 1, 1);
		probe.remove();
		return (
			Array.from(context.getImageData(0, 0, 1, 1).data)
				.slice(0, 3)
				.reduce((sum, channel) => sum + channel, 0) /
			(3 * 255)
		);
	});
	if (theme === 'dark') expect(tokenBrightness).toBeLessThan(0.3);
	else expect(tokenBrightness).toBeGreaterThan(0.8);
}

async function colorMetrics(card: Locator, theme: 'light' | 'dark') {
	await assertTheme(card.page(), theme);
	const metrics = await card.evaluate((element) => {
		const style = getComputedStyle(element);
		const context = document.createElement('canvas').getContext('2d')!;
		const rgb = (color: string) => {
			context.clearRect(0, 0, 1, 1);
			context.fillStyle = color;
			context.fillRect(0, 0, 1, 1);
			return Array.from(context.getImageData(0, 0, 1, 1).data).slice(0, 3);
		};
		const luminance = (color: number[]) =>
			color
				.map((value) => {
					const channel = value / 255;
					return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
				})
				.reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);
		const background = rgb(style.backgroundColor);
		const contrast = (color: string) => {
			const values = [luminance(background), luminance(rgb(color))].sort((a, b) => b - a);
			return (values[0] + 0.05) / (values[1] + 0.05);
		};
		const muted = element.querySelector('.text-muted-foreground');
		const referenceColor = (token: string) => {
			const probe = document.createElement('span');
			probe.style.color = `var(${token})`;
			element.append(probe);
			const value = rgb(getComputedStyle(probe).color);
			probe.remove();
			return value;
		};
		return {
			fill: style.backgroundColor,
			background,
			backgroundLuminance: luminance(background),
			backgroundImage: style.backgroundImage,
			textContrast: contrast(style.color),
			mutedContrast: muted ? contrast(getComputedStyle(muted).color) : null,
			leftWidth: style.borderLeftWidth,
			topWidth: style.borderTopWidth,
			border: rgb(style.borderTopColor),
			destructive: referenceColor('--destructive'),
			primary: referenceColor('--primary')
		};
	});
	if (theme === 'dark') expect(metrics.backgroundLuminance).toBeLessThan(0.2);
	else expect(metrics.backgroundLuminance).toBeGreaterThan(0.65);
	return metrics;
}

for (const theme of ['light', 'dark'] as const) {
	for (const kind of ['course', 'activity', 'synchronized'] as const) {
		test(`${kind} keeps its whole-card ${theme} fill through preview, pending save, move and reload`, async ({
			page
		}) => {
			let releaseCreate!: () => void;
			const createGate = new Promise<void>((resolve) => {
				releaseCreate = resolve;
			});
			const learningGroup = group(timetableIds.groupA, kind === 'activity' ? 'activity' : 'course');
			const mock = await installTimetableMock(page, {
				learningGroups: [learningGroup],
				ordinaryDemands: [demand(learningGroup)],
				includeSynchronizedDemand: kind === 'synchronized',
				createGate
			});
			await page.goto(boardUrl());
			await setTheme(page, theme);
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
			const tray = page.getByRole('complementary', { name: 'คาบที่ยังไม่ได้จัด' });
			const trayCard = tray
				.locator('article')
				.filter({ hasText: kind === 'synchronized' ? 'ชุมนุม' : learningGroup.offeringName });
			if (kind === 'synchronized') {
				await trayCard.getByRole('button', { name: 'ยังไม่กำหนดครู' }).click();
				await page.getByRole('button', { name: 'ครูทุกคน', exact: true }).click();
				await page.keyboard.press('Escape');
			}
			const initial = await colorMetrics(trayCard, theme);
			expect(initial.backgroundImage).toBe('none');
			expect(initial.leftWidth).toBe(initial.topWidth);
			expect(initial.textContrast).toBeGreaterThanOrEqual(4.5);
			expect(initial.mutedContrast).toBeGreaterThanOrEqual(4.5);
			const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
			await trayCard.dispatchEvent('dragstart', { dataTransfer });
			const destination = page.locator(
				`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period1}"]`
			);
			await destination.dispatchEvent('dragover', { dataTransfer });
			await expect
				.poll(
					async () =>
						(await colorMetrics(page.locator('[data-timetable-placement-preview]'), theme)).fill
				)
				.toBe(initial.fill);
			expect(mock.createRequestCount() + mock.synchronizedCreateRequestCount()).toBe(0);
			await destination.dispatchEvent('drop', { dataTransfer });
			const pending = page.locator('[data-timetable-lesson-card][data-pending="true"]');
			await expect(pending).toHaveAttribute('aria-busy', 'true');
			await expect.poll(async () => (await colorMetrics(pending, theme)).fill).toBe(initial.fill);
			await expect
				.poll(() => mock.createRequestCount() + mock.synchronizedCreateRequestCount())
				.toBe(1);
			releaseCreate();
			const saved = page.locator(
				`[data-timetable-lesson-card][data-block-id="${timetableIds.createdBlock}"]`
			);
			await expect(saved).toHaveAttribute('aria-busy', 'false');
			await expect.poll(async () => (await colorMetrics(saved, theme)).fill).toBe(initial.fill);
			expect(mock.blocks()[0].blockKind).toBe(kind === 'course' ? 'course' : 'activity');
			const movedDestination = page.locator(
				`td[data-timetable-day="TUE"][data-timetable-period-id="${timetableIds.period2}"]`
			);
			await saved.dispatchEvent('dragstart', { dataTransfer });
			await movedDestination.dispatchEvent('dragover', { dataTransfer });
			await expect
				.poll(
					async () =>
						(await colorMetrics(page.locator('[data-timetable-placement-preview]'), theme)).fill
				)
				.toBe(initial.fill);
			await movedDestination.dispatchEvent('drop', { dataTransfer });
			await expect(saved).toHaveAttribute('aria-busy', 'false');
			await expect.poll(mock.updateRequestCount).toBe(1);
			for (const view of kind === 'synchronized'
				? ['ครู', 'โรงเรียน', 'ชั้น']
				: ['กลุ่มเรียน', 'ครู', 'โรงเรียน', 'ชั้น']) {
				await page.getByRole('button', { name: view, exact: true }).click();
				if (view === 'โรงเรียน') {
					await page.getByRole('combobox', { name: 'เลือกวันดูภาพรวม' }).click();
					await page.getByRole('option', { name: 'วันอังคาร', exact: true }).click();
				}
				await expect.poll(async () => (await colorMetrics(saved, theme)).fill).toBe(initial.fill);
			}
			await page.reload();
			await expect(page.getByTestId('timetable-board-ready')).toBeVisible();
			await assertTheme(page, theme);
			await expect.poll(async () => (await colorMetrics(saved, theme)).fill).toBe(initial.fill);
			expect(mock.createRequestCount() + mock.synchronizedCreateRequestCount()).toBe(1);
			expect(mock.updateRequestCount()).toBe(1);
			expect(mock.swapRequestCount() + mock.deleteRequestCount()).toBe(0);
		});
	}

	for (const width of [1920, 390]) {
		test(`different groups and learner activities have readable whole fills at ${width}px in ${theme}`, async ({
			page
		}, testInfo) => {
			await page.setViewportSize({ width, height: width === 390 ? 844 : 1080 });
			const groups = [
				group(timetableIds.groupA),
				group(timetableIds.groupB),
				group('71000000-0000-4000-8000-000000000203', 'activity')
			];
			const blocks = groups.map((learningGroup, index) => {
				const block = makeTimetableBlock(
					`color-block-${index}`,
					[timetableIds.period1, timetableIds.period2, timetableIds.period3][index],
					{
						groupId: learningGroup.id,
						offeringId: learningGroup.learningOfferingId,
						code: learningGroup.offeringCode,
						name: learningGroup.offeringName
					}
				);
				block.blockKind = learningGroup.offeringKind === 'activity' ? 'activity' : 'course';
				block.schedulingMode = block.blockKind === 'activity' ? 'independent' : null;
				return block;
			});
			const sync = makeSynchronizedTimetableBlock('color-sync', timetableIds.period1, [
				timetableIds.teacherA
			]);
			sync.dayOfWeek = 'TUE';
			const mock = await installTimetableMock(page, {
				learningGroups: groups,
				ordinaryDemands: groups.map(demand),
				blocks: [...blocks, sync],
				includeSynchronizedDemand: true
			});
			await page.goto(boardUrl('teacher', timetableIds.teacherA));
			await setTheme(page, theme);
			await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
			const cards = page.locator('[data-timetable-lesson-card]');
			await expect(cards).toHaveCount(4);
			const metrics = await Promise.all(
				Array.from({ length: 4 }, (_, index) => colorMetrics(cards.nth(index), theme))
			);
			for (const metric of metrics) {
				expect(metric.textContrast).toBeGreaterThanOrEqual(4.5);
				if (metric.mutedContrast !== null) expect(metric.mutedContrast).toBeGreaterThanOrEqual(4.5);
				expect(metric.backgroundImage).toBe('none');
				expect(metric.leftWidth).toBe(metric.topWidth);
			}
			for (let first = 0; first < metrics.length; first++) {
				for (let second = first + 1; second < metrics.length; second++) {
					const distance = Math.hypot(
						...metrics[first].background.map(
							(value, channel) => value - metrics[second].background[channel]
						)
					);
					expect(distance).toBeGreaterThan(10);
				}
			}
			for (const block of blocks) {
				const exactTrayCard = page.locator(
					`aside article[data-lesson-color="group:${block.groups[0].learningGroupId}"]`
				);
				await expect
					.poll(async () => (await colorMetrics(exactTrayCard, theme)).fill)
					.toBe((await colorMetrics(page.locator(`[data-block-id="${block.id}"]`), theme)).fill);
			}
			await page.locator('[data-timetable-scroll-container]').scrollIntoViewIfNeeded();
			await assertTheme(page, theme);
			await page.screenshot({
				path: testInfo.outputPath(`lesson-colors-${width}-${theme}.png`),
				fullPage: true
			});
			expect(
				mock.createRequestCount() +
					mock.synchronizedCreateRequestCount() +
					mock.updateRequestCount() +
					mock.swapRequestCount() +
					mock.deleteRequestCount()
			).toBe(0);
		});
	}

	test(`blocked preview retains its lesson fill and destructive border in ${theme}`, async ({
		page
	}) => {
		const mock = await installTimetableMock(page, { blockedPeriodId: timetableIds.period2 });
		await page.goto(boardUrl());
		await setTheme(page, theme);
		await page.getByRole('button', { name: 'แก้ไข', exact: true }).click();
		const trayCard = page.locator('aside article').first();
		const fill = (await colorMetrics(trayCard, theme)).fill;
		const dataTransfer = await page.evaluateHandle(() => new DataTransfer());
		await trayCard.dispatchEvent('dragstart', { dataTransfer });
		await page
			.locator(`td[data-timetable-day="MON"][data-timetable-period-id="${timetableIds.period2}"]`)
			.dispatchEvent('dragover', { dataTransfer });
		const preview = page.locator('[data-timetable-placement-preview]');
		await expect(preview).toHaveAttribute('data-placement-state', 'blocked');
		const metrics = await colorMetrics(preview, theme);
		expect(metrics.fill).toBe(fill);
		expect(metrics.border).toEqual(metrics.destructive);
		await trayCard.dispatchEvent('dragend', { dataTransfer });
		expect(
			mock.createRequestCount() +
				mock.synchronizedCreateRequestCount() +
				mock.updateRequestCount() +
				mock.swapRequestCount() +
				mock.deleteRequestCount()
		).toBe(0);
	});
}
