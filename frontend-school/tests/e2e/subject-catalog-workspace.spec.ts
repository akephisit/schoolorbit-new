import { mkdir } from 'node:fs/promises';
import { expect, test } from '@playwright/test';
import {
	chooseSubjectDate,
	failSubject,
	fillSubjectVersion,
	installSubjectCatalog,
	openSubjectDetails,
	replySubject,
	subjectApi,
	subjectFixture,
	subjectGate,
	subjectGrade,
	subjectName,
	subjectPath,
	subjectVersion
} from './fixtures/subject-catalog';

test.use({ serviceWorkers: 'block', timezoneId: 'Asia/Bangkok', hasTouch: true });

test('filters the complete authorized catalog, resets paging, and separates read and manage group choices', async ({
	page
}) => {
	await page.setViewportSize({ width: 1029, height: 888 });
	const fixture = subjectFixture(31);
	const requests = await installSubjectCatalog(page, fixture);
	await page.goto(subjectPath);
	const table = page.getByRole('table', { name: 'ทะเบียนรายวิชา', exact: true });
	await expect(table.getByRole('row')).toHaveCount(26);
	expect(requests.historyReads.size).toBe(0);
	await page.getByRole('button', { name: 'ถัดไป', exact: true }).click();
	await expect(table.getByRole('row')).toHaveCount(7);
	await page.getByLabel('ค้นหารายวิชา', { exact: true }).fill('ค00031');
	await expect(table.getByRole('row')).toHaveCount(2);
	await expect(table.getByRole('button', { name: 'ค00031', exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ล้างตัวกรอง', exact: true }).click();
	await page.getByRole('button', { name: /^ตัวกรอง/ }).click();
	await page.getByRole('button', { name: 'กรองกลุ่มสาระ', exact: true }).click();
	await page.getByRole('option', { name: 'ภาษาไทย', exact: true }).click();
	await expect(table.getByRole('row')).toHaveCount(11);
	await page.getByRole('button', { name: /^ตัวกรอง/ }).click();
	await expect(page.getByRole('region', { name: 'ค้นหาและกรองรายวิชา' })).toContainText('ภาษาไทย');
	await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
	await page.getByRole('dialog').getByLabel('กลุ่มสาระ', { exact: true }).click();
	await expect(page.getByRole('option', { name: 'ภาษาไทย', exact: true })).toHaveCount(0);
	await expect(page.getByRole('option', { name: 'คณิตศาสตร์', exact: true })).toBeVisible();
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
	await page.getByLabel('ค้นหารายวิชา', { exact: true }).fill('ไม่มีรายวิชาตรงคำค้น');
	await expect(page.getByText('ไม่พบรายวิชาที่ตรงกับตัวกรอง', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ล้างตัวกรอง', exact: true }).first().click();
	const open = page.getByRole('button', { name: 'เปิด ค00001', exact: true });
	await open.focus();
	await page.keyboard.press('Enter');
	await expect(page.getByRole('region', { name: 'ประวัติรุ่น', exact: true })).toContainText(
		subjectName
	);
	await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
	expect(requests.overviewReads()).toBe(1);
	expect(requests.writes).toEqual([]);
	expect(requests.pageErrors).toEqual([]);
	expect(requests.unexpected).toEqual([]);
});

test('create failure preserves its draft and rejects duplicate pending submissions', async ({
	page
}) => {
	const fixture = subjectFixture(1);
	const gate = subjectGate();
	let attempts = 0;
	const requests = await installSubjectCatalog(page, fixture, {
		onRequest: async (route, url) => {
			if (url.pathname === subjectApi && route.request().method() === 'POST' && ++attempts === 1) {
				await gate.promise;
				await failSubject(route, 409, 'รหัสรายวิชาซ้ำ');
				return true;
			}
			return false;
		}
	});
	await page.goto(subjectPath);
	await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
	await page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true }).fill('คทดสอบ99');
	await page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด', exact: true }).click();
	try {
		await expect.poll(() => requests.writes.length).toBe(1);
		await expect(page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true })).toBeDisabled();
		await expect(page.getByRole('button', { name: 'กำลังเพิ่ม...', exact: true })).toBeDisabled();
		await page.getByRole('dialog').locator('form').dispatchEvent('submit');
		expect(requests.writes).toHaveLength(1);
	} finally {
		gate.release();
	}
	await expect(page.getByRole('alert')).toContainText('รหัสรายวิชาซ้ำ');
	await expect(page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true })).toHaveValue('คทดสอบ99');
	await page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด', exact: true }).click();
	await expect(page.getByRole('dialog', { name: 'คทดสอบ99', exact: true })).toBeVisible();
	await expect.poll(() => requests.historyReads.get('subject-created')).toBe(1);
	expect(requests.writes).toHaveLength(2);
	expect(requests.pageErrors).toEqual([]);
});

for (const [instant, expectedDate, dateLabel] of [
	['2026-10-20T16:59:00Z', '2026-10-20', '20 ต.ค. 2569'],
	['2026-10-20T17:01:00Z', '2026-10-21', '21 ต.ค. 2569']
] as const) {
	test(`version decimals retain keyboard input and default to Bangkok day ${expectedDate}`, async ({
		page
	}) => {
		const fixture = subjectFixture(1);
		fixture.items[0].displayVersion = null;
		fixture.histories.set(fixture.items[0].subject.id, []);
		const requests = await installSubjectCatalog(page, fixture);
		await page.clock.setFixedTime(new Date(instant));
		await page.goto(subjectPath);
		await openSubjectDetails(page);
		await expect(
			page.getByRole('button', { name: 'เลือกวันที่เริ่มใช้', exact: true })
		).toContainText(dateLabel);
		await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('รุ่นทศนิยม');
		const credit = page.getByLabel('หน่วยกิต', { exact: true });
		await credit.clear();
		await credit.pressSequentially('1');
		await credit.pressSequentially('.');
		await expect(credit).toHaveValue('1.');
		await credit.pressSequentially('5');
		await expect(credit).toHaveValue('1.5');
		await page.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
		await expect.poll(() => requests.writes.length).toBe(1);
		expect(requests.writes[0].body).toMatchObject({ credit: '1.5', effectiveFrom: expectedDate });
		expect(requests.pageErrors).toEqual([]);
	});
}

test('invalid version values block writes and failed saves preserve exact decimals and chosen date', async ({
	page
}) => {
	let attempts = 0;
	const requests = await installSubjectCatalog(page, subjectFixture(1), {
		onRequest: async (route, url) => {
			if (
				url.pathname.endsWith('/versions') &&
				route.request().method() === 'POST' &&
				++attempts === 1
			) {
				await failSubject(route, 409, 'บันทึกรุ่นไม่สำเร็จ');
				return true;
			}
			return false;
		}
	});
	await page.clock.setFixedTime(new Date('2026-10-20T17:01:00Z'));
	await page.goto(subjectPath);
	await openSubjectDetails(page);
	await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('แบบร่างทศนิยม');
	const credit = page.getByLabel('หน่วยกิต', { exact: true });
	const save = page.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true });
	for (const value of ['', '0', '-1', '1.234', '1.', '.']) {
		await credit.clear();
		await credit.pressSequentially(value);
		expect(await credit.evaluate((element: HTMLInputElement) => element.checkValidity())).toBe(
			false
		);
		await save.click();
		expect(requests.writes).toEqual([]);
	}
	await credit.clear();
	await credit.pressSequentially('1.50');
	for (const label of ['คาบมาตรฐานต่อสัปดาห์', 'ชั่วโมงรวมต่อภาคเรียน']) {
		const input = page.getByLabel(label, { exact: true });
		await input.clear();
		await input.pressSequentially('1.5');
		await save.click();
		expect(requests.writes).toEqual([]);
		await input.clear();
		await input.pressSequentially('2');
	}
	await chooseSubjectDate(page, '2026-10-23');
	await save.click();
	await expect(page.getByRole('form', { name: 'สร้างรุ่นใหม่' }).getByRole('alert')).toBeVisible();
	await expect(credit).toHaveValue('1.50');
	await expect(page.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue('แบบร่างทศนิยม');
	await expect(
		page.getByRole('button', { name: 'เลือกวันที่เริ่มใช้', exact: true })
	).toContainText('23 ต.ค. 2569');
	await save.click();
	await expect.poll(() => requests.writes.length).toBe(2);
	for (const write of requests.writes)
		expect(write.body).toMatchObject({ credit: '1.50', effectiveFrom: '2026-10-23' });
	expect(requests.pageErrors).toEqual([]);
});

test('late creation cannot close or clear a reopened creation draft', async ({ page }) => {
	const fixture = subjectFixture(1);
	const gate = subjectGate();
	const requests = await installSubjectCatalog(page, fixture, {
		onRequest: async (route, url) => {
			if (url.pathname === subjectApi && route.request().method() === 'POST') {
				await gate.promise;
				const item = structuredClone(fixture.items[0]);
				item.subject = { ...item.subject, id: 'created-late', code: 'คเก่า99' };
				item.displayVersion = null;
				item.displayState = 'unpublished';
				fixture.items.push(item);
				fixture.histories.set(item.subject.id, []);
				await replySubject(route, item.subject);
				return true;
			}
			return false;
		}
	});
	await page.goto(subjectPath);
	await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
	await page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true }).fill('คเก่า99');
	await page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด', exact: true }).click();
	try {
		await expect.poll(() => requests.writes.length).toBe(1);
		await page.keyboard.press('Escape');
		await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
		await page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true }).fill('คแบบร่างใหม่');
		await expect(
			page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด', exact: true })
		).toBeEnabled();
	} finally {
		gate.release();
	}
	await expect.poll(requests.overviewReads).toBe(2);
	await expect(page.getByRole('dialog', { name: 'เพิ่มรายวิชา', exact: true })).toBeVisible();
	await expect(page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true })).toHaveValue('คแบบร่างใหม่');
	expect(requests.historyReads.size).toBe(0);
	expect(requests.pageErrors).toEqual([]);
});

test('a delayed creation overview cannot replace a later subject selection', async ({ page }) => {
	const fixture = subjectFixture(2);
	const gate = subjectGate();
	let reads = 0;
	const requests = await installSubjectCatalog(page, fixture, {
		onRequest: async (route, url) => {
			if (url.pathname === `${subjectApi}/overview` && ++reads === 2) {
				await gate.promise;
				await replySubject(route, fixture.overview());
				return true;
			}
			return false;
		}
	});
	await page.goto(subjectPath);
	await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
	await page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true }).fill('คสร้างแล้ว');
	await page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด', exact: true }).click();
	try {
		await expect.poll(requests.overviewReads).toBe(2);
		await expect(page.getByRole('dialog', { name: 'เพิ่มรายวิชา', exact: true })).toBeHidden();
		await openSubjectDetails(page, 'ค00002');
		await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('แบบร่างของรายการที่เลือกภายหลัง');
	} finally {
		gate.release();
	}
	await expect(page.getByTestId('catalog-subjects-ready')).toHaveAttribute('aria-busy', 'false');
	await expect(page.getByRole('dialog', { name: /^ค00002 ·/ })).toBeVisible();
	await expect(page.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue(
		'แบบร่างของรายการที่เลือกภายหลัง'
	);
	expect(requests.historyReads.get('subject-created')).toBeUndefined();
	expect(requests.pageErrors).toEqual([]);
});

test('metadata failures stay with their owning subject and preserve another subjects drafts', async ({
	page
}) => {
	const fixture = subjectFixture(2);
	const gate = subjectGate();
	let responseFinished = false;
	const requests = await installSubjectCatalog(page, fixture, {
		onRequest: async (route, url) => {
			if (
				url.pathname === `${subjectApi}/${fixture.items[0].subject.id}` &&
				route.request().method() === 'PATCH'
			) {
				await gate.promise;
				await failSubject(route, 409, 'ข้อมูลกลุ่มสาระเก่าเปลี่ยนแล้ว');
				responseFinished = true;
				return true;
			}
			return false;
		}
	});
	await page.goto(subjectPath);
	await openSubjectDetails(page);
	await page.getByRole('button', { name: 'กลุ่มสาระของรายวิชา', exact: true }).click();
	await page.getByRole('option', { name: 'วิทยาศาสตร์และเทคโนโลยี', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกกลุ่มสาระ', exact: true }).click();
	try {
		await expect.poll(() => requests.writes.length).toBe(1);
		await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
		await openSubjectDetails(page, 'ค00002');
		await page.getByRole('button', { name: 'กลุ่มสาระของรายวิชา', exact: true }).click();
		await page.getByRole('option', { name: 'คณิตศาสตร์', exact: true }).click();
		await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('แบบร่างรายวิชาที่สอง');
	} finally {
		gate.release();
	}
	await expect.poll(() => responseFinished).toBe(true);
	await expect(page.getByRole('alert')).toHaveCount(0);
	await expect(page.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue('แบบร่างรายวิชาที่สอง');
	await expect(page.getByRole('button', { name: 'กลุ่มสาระของรายวิชา', exact: true })).toHaveText(
		'คณิตศาสตร์'
	);
	await expect(page.getByRole('button', { name: 'บันทึกกลุ่มสาระ', exact: true })).toBeEnabled();
	expect(requests.writes[0].body).toEqual({
		code: 'ค00001',
		subjectGroupId: 'group-science',
		archived: false,
		rowVersion: 2
	});
	expect(requests.pageErrors).toEqual([]);
});

for (const kind of ['version', 'publish'] as const) {
	test(`late ${kind} success invalidates its own history and preserves another subjects draft`, async ({
		page
	}) => {
		const fixture = subjectFixture(2);
		const subjectId = fixture.items[0].subject.id;
		const draft = subjectVersion(subjectId, 2);
		if (kind === 'publish') {
			fixture.histories.get(subjectId)!.unshift(draft);
			fixture.items[0].draftCount = 1;
		}
		const gate = subjectGate();
		const requests = await installSubjectCatalog(page, fixture, {
			onRequest: async (route, url) => {
				const expected =
					kind === 'version'
						? `${subjectApi}/${subjectId}/versions`
						: `/api/academic/catalog/subject-versions/${draft.id}/publish`;
				if (url.pathname === expected && route.request().method() === 'POST') {
					await gate.promise;
					if (kind === 'version') {
						draft.nameTh = 'รุ่นใหม่จากคำขอเก่า';
						fixture.histories.get(subjectId)!.unshift(draft);
					} else {
						draft.status = 'published';
						draft.rowVersion += 1;
					}
					await replySubject(route, draft);
					return true;
				}
				return false;
			}
		});
		await page.goto(subjectPath);
		await openSubjectDetails(page);
		if (kind === 'version') {
			await fillSubjectVersion(page, 'รุ่นใหม่จากคำขอเก่า');
			await page.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
		} else await page.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
		try {
			await expect.poll(() => requests.writes.length).toBe(1);
			await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
			await openSubjectDetails(page, 'ค00002');
			await page
				.getByLabel('ชื่อภาษาไทย', { exact: true })
				.fill('แบบร่างที่ต้องเก็บของรายวิชาที่สอง');
		} finally {
			gate.release();
		}
		await expect.poll(requests.overviewReads).toBe(2);
		await expect(page.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue(
			'แบบร่างที่ต้องเก็บของรายวิชาที่สอง'
		);
		expect(requests.historyReads.get(fixture.items[1].subject.id)).toBe(1);
		await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
		await openSubjectDetails(page);
		await expect.poll(() => requests.historyReads.get(subjectId)).toBe(2);
		const article = page
			.getByRole('region', { name: 'ประวัติรุ่น', exact: true })
			.getByRole('article')
			.filter({ hasText: draft.nameTh });
		await expect(article).toBeVisible();
		if (kind === 'publish') await expect(article).toContainText('เผยแพร่แล้ว');
		expect(requests.pageErrors).toEqual([]);
	});
}

test('publication uses the displayed revision, shows local pending/error, and keeps the version draft', async ({
	page
}) => {
	const fixture = subjectFixture(1);
	const draft = subjectVersion(fixture.items[0].subject.id, 2);
	fixture.histories.get(draft.subjectId)!.unshift(draft);
	fixture.items[0].draftCount = 1;
	const gate = subjectGate();
	let attempts = 0;
	const requests = await installSubjectCatalog(page, fixture, {
		onRequest: async (route, url) => {
			if (
				url.pathname.endsWith('/publish') &&
				route.request().method() === 'POST' &&
				++attempts === 1
			) {
				await gate.promise;
				await failSubject(route, 409, 'รุ่นข้อมูลเปลี่ยนแล้ว');
				return true;
			}
			return false;
		}
	});
	await page.goto(subjectPath);
	await openSubjectDetails(page);
	await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('แบบร่างที่ยังไม่บันทึก');
	await page.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
	try {
		await expect.poll(() => requests.writes.length).toBe(1);
		await expect(page.getByRole('button', { name: 'กำลังเผยแพร่...', exact: true })).toBeDisabled();
		await expect(page.getByLabel('ชื่อภาษาไทย', { exact: true })).toBeDisabled();
	} finally {
		gate.release();
	}
	await expect(
		page.getByRole('region', { name: 'ประวัติรุ่น', exact: true }).getByRole('alert')
	).toContainText('กรุณาโหลดข้อมูลล่าสุด');
	await expect(page.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue(
		'แบบร่างที่ยังไม่บันทึก'
	);
	expect(requests.writes[0].body).toEqual({ rowVersion: 4 });
	await page.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
	await expect(page.getByRole('article').filter({ hasText: draft.nameTh })).toContainText(
		'เผยแพร่แล้ว'
	);
	expect(requests.writes).toHaveLength(2);
	expect(requests.pageErrors).toEqual([]);
});

test('readers see metadata while history fails and retry only history without mutation controls', async ({
	page
}) => {
	const fixture = subjectFixture(1, true);
	let first = true;
	const requests = await installSubjectCatalog(page, fixture, {
		readOnly: true,
		onRequest: async (route, url) => {
			if (url.pathname.endsWith('/versions') && first) {
				first = false;
				await failSubject(route, 503, 'ประวัติยังไม่พร้อม');
				return true;
			}
			return false;
		}
	});
	await page.goto(subjectPath);
	await expect(page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true })).toHaveCount(0);
	await openSubjectDetails(page);
	await expect(
		page.getByRole('region', { name: 'กลุ่มสาระของรายวิชา', exact: true })
	).toContainText('คณิตศาสตร์');
	await expect(page.getByText('โหลดประวัติไม่สำเร็จ', { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'ลองอีกครั้ง', exact: true }).click();
	await expect(page.getByRole('region', { name: 'ประวัติรุ่น', exact: true })).toContainText(
		subjectName
	);
	await expect(page.getByRole('form', { name: 'สร้างรุ่นใหม่' })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'บันทึกกลุ่มสาระ', exact: true })).toHaveCount(0);
	expect(requests.overviewReads()).toBe(1);
	expect(requests.historyReads.get(fixture.items[0].subject.id)).toBe(2);
	expect(requests.writes).toEqual([]);
	expect(requests.pageErrors).toEqual([]);
});

for (const width of [1029, 390]) {
	for (const mode of ['light', 'dark'] as const) {
		test(`${width}px ${mode}: readable list, create dialog and version form use school appearance`, async ({
			page
		}) => {
			await page.setViewportSize({ width, height: 888 });
			const requests = await installSubjectCatalog(page, subjectFixture(), { mode });
			await page.goto(subjectPath);
			const search = page.getByLabel('ค้นหารายวิชา', { exact: true });
			await expect(search).toBeVisible();
			await expect(search).toHaveCSS('font-family', /School Noto Sans Thai/);
			const box = await search.boundingBox();
			expect(box?.width).toBeGreaterThan(width === 390 ? 180 : 400);
			await expect(page.locator('html')).toHaveClass(mode === 'dark' ? /dark/ : /^(?!.*dark)/);
			expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
				true
			);
			await mkdir('/workspace/artifacts/staff-catalog-facility', { recursive: true });
			const prefix = `/workspace/artifacts/staff-catalog-facility/subjects-${width}-${mode}`;
			await page.screenshot({ path: `${prefix}-list.png`, fullPage: true, animations: 'disabled' });
			if (width === 1029) {
				const scroll = page
					.getByRole('table', { name: 'ทะเบียนรายวิชา', exact: true })
					.locator('..');
				expect(await scroll.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(
					true
				);
			} else {
				const list = page.getByRole('list', { name: 'รายวิชา', exact: true });
				await expect(list).toContainText(subjectName);
				await expect(list).toContainText('หน่วยกิต');
				await expect(list).toContainText('v1');
				await expect(list).toContainText('เก็บถาวร');
			}
			await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
			await page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true }).fill('คตัวอย่าง');
			await page.screenshot({
				path: `${prefix}-create.png`,
				fullPage: true,
				animations: 'disabled'
			});
			await page.getByRole('button', { name: 'ยกเลิก', exact: true }).click();
			await openSubjectDetails(page);
			await page.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('แบบร่างสำหรับตรวจรูปแบบ');
			await expect(
				page.getByRole('button', { name: 'เลือกวันที่เริ่มใช้', exact: true })
			).not.toContainText('เลือกวันเริ่มใช้');
			await expect(page.getByLabel('หน่วยกิต', { exact: true })).toHaveCSS(
				'font-family',
				/School Noto Sans Thai/
			);
			await expect(
				page.getByRole('button', { name: 'เลือกวันที่เริ่มใช้', exact: true })
			).toBeVisible();
			await page.screenshot({
				path: `${prefix}-version-form.png`,
				fullPage: true,
				animations: 'disabled'
			});
			expect(
				await page
					.getByRole('dialog')
					.evaluate((element) => element.scrollWidth <= element.clientWidth)
			).toBe(true);
			expect(requests.writes).toEqual([]);
			expect(requests.pageErrors).toEqual([]);
		});
	}
}

test('grade-only same-day revisions prefill the displayed published source and preserve the original', async ({
	page
}) => {
	const fixture = subjectFixture(2);
	const source = fixture.items[0].displayVersion!;
	source.description = 'คำอธิบายที่เก็บไว้';
	source.termCode = '1';
	source.effectiveUntil = '2027-03-31';
	const original = structuredClone(source);
	const future = {
		...source,
		id: 'future-v3',
		versionNo: 3,
		effectiveFrom: '2027-05-01',
		effectiveUntil: null,
		nameTh: 'รุ่นอนาคต'
	};
	fixture.histories.get(source.subjectId)!.unshift(future);
	const requests = await installSubjectCatalog(page, fixture);
	await page.goto(subjectPath);
	await openSubjectDetails(page);
	const form = page.getByRole('form', { name: 'สร้างรุ่นใหม่' });
	await expect(form).toContainText('เริ่มจากรุ่นที่เผยแพร่ v1');
	await expect(form.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue(original.nameTh);
	await expect(form.getByLabel('ชื่อภาษาอังกฤษ', { exact: true })).toHaveValue(original.nameEn!);
	await expect(form.getByLabel('หน่วยกิต', { exact: true })).toHaveValue('1.50');
	await expect(
		form.getByRole('button', { name: 'เลือกวันที่เริ่มใช้', exact: true })
	).toContainText('1 พ.ค. 2569');
	await expect(form.getByRole('button', { name: 'เลือกวันที่สิ้นสุด', exact: true })).toContainText(
		'31 มี.ค. 2570'
	);
	await expect(page.getByRole('button', { name: /แก้ไขรุ่น|คัดลอกรุ่น/ })).toHaveCount(0);
	await form.getByRole('combobox', { name: /เลือกระดับชั้นสำหรับ/ }).click();
	await page.getByRole('option', { name: /มัธยมศึกษาปีที่ 1/ }).click();
	await page.keyboard.press('Escape');
	await form.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
	await expect.poll(() => requests.writes.length).toBe(1);
	expect(requests.writes[0].body).toEqual({
		nameTh: original.nameTh,
		nameEn: original.nameEn,
		credit: original.credit,
		description: original.description,
		effectiveFrom: original.effectiveFrom,
		effectiveUntil: original.effectiveUntil,
		gradeLevelIds: [],
		hoursPerSemester: 60,
		periodsPerWeek: 3,
		subjectType: 'BASIC',
		termCode: '1'
	});
	await expect(form.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue('');
	await form.dispatchEvent('submit');
	expect(requests.writes).toHaveLength(1);
	expect(fixture.histories.get(source.subjectId)!.find((item) => item.id === original.id)).toEqual(
		original
	);
	await page.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
	await expect(page.getByRole('dialog')).toHaveAccessibleName(`ค00001 · ${original.nameTh}`);
	await expect(page.getByRole('dialog').getByText('v3', { exact: true }).first()).toBeVisible();
	await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
	await openSubjectDetails(page, 'ค00002');
	await expect(form.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue('รายวิชาทดสอบ 02');
	await expect(form.getByRole('combobox', { name: /เลือกระดับชั้นสำหรับ/ })).toContainText(
		subjectGrade.short_name
	);
	expect(requests.pageErrors).toEqual([]);
});

test('server semantic duplicate errors preserve the prefilled revision and refreshes preserve unsaved edits', async ({
	page
}) => {
	const fixture = subjectFixture(1);
	const requests = await installSubjectCatalog(page, fixture, {
		onRequest: async (route, url) => {
			if (url.pathname.endsWith('/versions') && route.request().method() === 'POST') {
				await failSubject(
					route,
					400,
					'ข้อมูลรุ่นนี้เหมือนกับรุ่นที่มีอยู่ กรุณาปรับข้อมูลก่อนสร้างรุ่นใหม่'
				);
				return true;
			}
			return false;
		}
	});
	await page.goto(subjectPath);
	await openSubjectDetails(page);
	const form = page.getByRole('form', { name: 'สร้างรุ่นใหม่' });
	await form.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
	await expect(form.getByRole('alert')).toContainText('กรุณาปรับข้อมูลก่อนสร้างรุ่นใหม่');
	await expect(form.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue(subjectName);
	await form.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('แบบร่างที่ยังไม่บันทึก');
	await page.getByRole('button', { name: 'กลุ่มสาระของรายวิชา', exact: true }).click();
	await page.getByRole('option', { name: 'วิทยาศาสตร์และเทคโนโลยี', exact: true }).click();
	await page.getByRole('button', { name: 'บันทึกกลุ่มสาระ', exact: true }).click();
	await expect.poll(requests.overviewReads).toBe(2);
	await expect(page.getByTestId('catalog-subjects-ready')).toHaveAttribute('aria-busy', 'false');
	await expect(form.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue(
		'แบบร่างที่ยังไม่บันทึก'
	);
	await expect(
		form.getByRole('button', { name: 'เลือกวันที่เริ่มใช้', exact: true })
	).toContainText('1 พ.ค. 2569');
	expect(requests.pageErrors).toEqual([]);
});
