import { expect, test } from '@playwright/test';
import {
	failSubject,
	installSubjectCatalog,
	openSubjectDetails,
	subjectFixture,
	subjectPath
} from './fixtures/subject-catalog';

test.use({ serviceWorkers: 'block' });

for (const [width, mode] of [
	[1280, 'light'],
	[390, 'dark']
] as const) {
	test(`${width}px ${mode}: new subject keeps zero hours and standard periods through retry, publication and prefill`, async ({
		page
	}, testInfo) => {
		await page.setViewportSize({ width, height: 900 });
		const fixture = subjectFixture(0);
		let attempts = 0;
		const requests = await installSubjectCatalog(page, fixture, {
			mode,
			onRequest: async (route, url) => {
				if (
					url.pathname.endsWith('/versions') &&
					route.request().method() === 'POST' &&
					++attempts === 1
				) {
					await failSubject(route, 409, 'บันทึกไม่สำเร็จ กรุณาตรวจสอบร่าง');
					return true;
				}
				return false;
			}
		});
		await page.goto(subjectPath);
		await page.getByRole('button', { name: 'เพิ่มรายวิชา', exact: true }).click();
		await page.getByLabel('เพิ่มรหัสรายวิชา', { exact: true }).fill('คไม่มีการสอน');
		await page.getByRole('button', { name: 'เพิ่มและกำหนดรายละเอียด', exact: true }).click();
		const form = page.getByRole('form', { name: 'สร้างรุ่นใหม่', exact: true });
		const credit = form.getByLabel('หน่วยกิต', { exact: true });
		const periods = form.getByLabel('คาบมาตรฐานต่อสัปดาห์', { exact: true });
		const hours = form.getByLabel('ชั่วโมงรวมต่อภาคเรียน', { exact: true });
		await expect(credit).toHaveValue('1.00');
		await expect(periods).toHaveValue('1');
		await expect(hours).toHaveValue('20.00');
		await expect(form).toContainText('กำหนดคาบต่อสัปดาห์และชั่วโมงรวมเป็น 0 ได้');
		await form.getByLabel('ชื่อภาษาไทย', { exact: true }).fill('รายวิชาในหลักสูตรไม่มีการจัดสอน');
		await credit.fill('1.25');
		await periods.fill('0');
		await hours.fill('0');
		await form.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
		await expect(form.getByRole('alert')).toContainText('บันทึกไม่สำเร็จ');
		await expect(periods).toHaveValue('0');
		await expect(hours).toHaveValue('0');
		await expect(credit).toHaveValue('1.25');
		await form.getByRole('button', { name: 'บันทึกร่างรุ่นใหม่', exact: true }).click();
		await expect
			.poll(() => requests.writes.filter((write) => write.path.endsWith('/versions')).length)
			.toBe(2);
		for (const write of requests.writes.filter((write) => write.path.endsWith('/versions'))) {
			expect(write.body).toMatchObject({ credit: '1.25', periodsPerWeek: 0, hoursPerSemester: 0 });
		}
		await page.getByRole('button', { name: 'เผยแพร่', exact: true }).click();
		await expect(page.getByRole('region', { name: 'ประวัติรุ่น', exact: true })).toContainText(
			'เผยแพร่แล้ว'
		);
		const version = fixture.histories.get('subject-created')![0];
		expect(version.status).toBe('published');
		expect(version.periodsPerWeek).toBe(0);
		expect(version.hoursPerSemester).toBe(0);
		await page.getByRole('dialog').getByRole('button', { name: 'Close', exact: true }).click();
		await openSubjectDetails(page, 'คไม่มีการสอน');
		await expect(form.getByLabel('ชื่อภาษาไทย', { exact: true })).toHaveValue(
			'รายวิชาในหลักสูตรไม่มีการจัดสอน'
		);
		await expect(periods).toHaveValue('0');
		await expect(hours).toHaveValue('0');
		await expect(credit).toHaveValue('1.25');
		await expect(page.getByRole('region', { name: 'ประวัติรุ่น', exact: true })).toContainText(
			'มาตรฐาน 0 คาบ/สัปดาห์'
		);
		await expect(page.getByRole('region', { name: 'ประวัติรุ่น', exact: true })).toContainText(
			'รวม 0 ชม./ภาคเรียน'
		);
		expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
			width
		);
		await hours.scrollIntoViewIfNeeded();
		await page.screenshot({
			path: testInfo.outputPath(`subject-zero-hours-${width}-${mode}.png`),
			animations: 'disabled'
		});
		expect(requests.pageErrors).toEqual([]);
		expect(requests.unexpected).toEqual([]);
	});
}
