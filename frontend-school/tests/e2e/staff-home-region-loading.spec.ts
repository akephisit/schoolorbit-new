import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import {
	mockStaffHome,
	homePath,
	publicPath,
	nextYear,
	firstStaff,
	secondStaff
} from './fixtures/staff-home-route-data';
import { navigate } from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});
for (const hold of ['menu', 'counts', 'overview']) {
	test(`home siblings render independently of ${hold}`, async ({ page }) => {
		const api = await mockStaffHome(page, { hold });
		await page.goto(homePath());
		const ready = hold === 'overview' ? 'staff-work-counts' : 'staff-overview';
		await expect(page.getByTestId(ready)).toContainText(hold === 'overview' ? '7' : '18');
		const pending =
			hold === 'menu'
				? 'staff-services'
				: hold === 'counts'
					? 'staff-work-counts'
					: 'staff-overview';
		await expect(page.getByTestId(pending).getByRole('status')).toBeVisible();
		api.release();
		await expect(page.getByTestId(pending).getByRole('status')).toHaveCount(0);
		expect(api.count('/api/menu/user')).toBe(1);
		expect(api.count('/api/me/work-items/counts')).toBe(1);
		expect(api.count('/api/auth/me')).toBe(1);
	});
}
for (const fail of ['menu', 'counts', 'overview']) {
	test(`${fail} retry does not reload successful home siblings`, async ({ page }) => {
		const api = await mockStaffHome(page, { fail });
		await page.goto(homePath());
		const region = page.getByTestId(
			fail === 'menu'
				? 'staff-services'
				: fail === 'counts'
					? 'staff-work-counts'
					: 'staff-overview'
		);
		await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
		await region.getByRole('button', { name: /ลอง/ }).click();
		await expect(region.getByText('region ไม่พร้อม')).toHaveCount(0);
		expect(api.count('/api/menu/user')).toBe(fail === 'menu' ? 2 : 1);
		expect(api.count('/api/me/work-items/counts')).toBe(fail === 'counts' ? 2 : 1);
		expect(api.count('/api/staff/dashboard')).toBe(fail === 'overview' ? 2 : 1);
	});
}
test('dashboard year supersession rejects a late aggregate', async ({ page }) => {
	const api = await mockStaffHome(page, { hold: 'overview' });
	await page.goto(homePath());
	await expect(page.getByTestId('staff-overview').getByRole('status')).toBeVisible();
	await navigate(page, homePath(nextYear));
	await expect(page.getByTestId('staff-overview')).toContainText('42');
	api.release();
	await expect(page.getByTestId('staff-overview')).not.toContainText('18');
	expect(api.count('/api/menu/user')).toBe(1);
});
test('home aggregate retains values through refresh failure', async ({ page }) => {
	const api = await mockStaffHome(page, { fail: 'overview', failAt: 2 });
	await page.goto(homePath());
	const region = page.getByTestId('staff-overview');
	await expect(region).toContainText('18');
	await region.getByRole('button', { name: 'รีเฟรช' }).click();
	await expect(region).toContainText('region ไม่พร้อม');
	await expect(region).toContainText('18');
	await region.getByRole('button', { name: /ลอง/ }).click();
	await expect(region).not.toContainText('region ไม่พร้อม');
	expect(api.count('/api/staff/dashboard')).toBe(3);
});
test('profile startup is one typed read and image tools stay lazy', async ({ page }) => {
	const cropperChunk = JSON.parse(
		readFileSync('.svelte-kit/output/client/.vite/manifest.json', 'utf8')
	)['src/lib/components/forms/ImageCropper.svelte'].file as string;
	const assets: string[] = [];
	page.on('request', (request) => {
		if (new URL(request.url()).pathname.endsWith(cropperChunk)) assets.push(request.url());
	});
	const api = await mockStaffHome(page);
	await page.goto('/staff/profile');
	await expect(page.locator('#nickname')).toHaveValue('ชื่อเล่นเดิม');
	expect(api.count('/api/auth/me/profile')).toBe(1);
	expect(api.count('/api/auth/me')).toBe(1);
	expect(assets).toEqual([]);
	await page.locator('input[type=file]').setInputFiles({
		name: 'test.png',
		mimeType: 'image/png',
		buffer: Buffer.from(
			'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl6Bv8AAAAASUVORK5CYII=',
			'base64'
		)
	});
	await expect.poll(() => assets.length).toBeGreaterThan(0);
	await expect(page.getByRole('dialog')).toBeVisible();
});
test('profile first read shows a skeleton without editable blank fields', async ({ page }) => {
	const api = await mockStaffHome(page, { hold: 'profile' });
	await page.goto('/staff/profile');
	await expect(page.getByTestId('staff-own-profile').getByRole('status')).toBeVisible();
	await expect(page.locator('#nickname')).toHaveCount(0);
	api.release();
	await expect(page.locator('#nickname')).toHaveValue('ชื่อเล่นเดิม');
});
test('profile error retries only its primary region', async ({ page }) => {
	const api = await mockStaffHome(page, { fail: 'profile' });
	await page.goto('/staff/profile');
	const region = page.getByTestId('staff-own-profile');
	await expect(region.getByText('region ไม่พร้อม')).toBeVisible();
	await region.getByRole('button', { name: 'ลองอีกครั้ง' }).click();
	await expect(page.locator('#nickname')).toHaveValue('ชื่อเล่นเดิม');
	expect(api.count('/api/auth/me/profile')).toBe(2);
	expect(api.count('/api/auth/me')).toBe(1);
	expect(api.count('/api/menu/user')).toBe(1);
});
test('typed profile save patches without a primary or current-user reread', async ({ page }) => {
	const api = await mockStaffHome(page);
	await page.goto('/staff/profile');
	await page.locator('#nickname').fill('ชื่อใหม่');
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง' }).click();
	await expect(page.getByText('บันทึกข้อมูลสำเร็จ', { exact: true })).toBeVisible();
	await expect(page.locator('#nickname')).toHaveValue('ชื่อใหม่');
	expect(api.count('/api/auth/me/profile')).toBe(1);
	expect(api.count('/api/auth/me')).toBe(1);
});
test('late profile save does not toast or patch after navigation away', async ({ page }) => {
	const api = await mockStaffHome(page, { hold: 'save' });
	await page.goto('/staff/profile');
	await page.locator('#nickname').fill('ร่างเก่า');
	await page.getByRole('button', { name: 'บันทึกการเปลี่ยนแปลง' }).click();
	await expect.poll(() => api.writes.length).toBe(1);
	await navigate(page, homePath());
	await expect(page.getByRole('heading', { name: 'หน้าหลักของฉัน', exact: true })).toBeVisible();
	api.release();
	await expect(page.getByText('บันทึกข้อมูลสำเร็จ', { exact: true })).toHaveCount(0);
});
for (const hold of ['public-profile', 'achievements']) {
	test(`public view renders the ready sibling before ${hold}`, async ({ page }) => {
		const api = await mockStaffHome(page, { hold });
		await page.goto(publicPath());
		await expect(
			page.getByTestId(
				hold === 'public-profile' ? 'staff-public-achievements' : 'staff-public-profile'
			)
		).toContainText(hold === 'public-profile' ? 'ผลงานที่พร้อมก่อน' : 'บุคลากรแรก');
		await expect(
			page
				.getByTestId(
					hold === 'public-profile' ? 'staff-public-profile' : 'staff-public-achievements'
				)
				.getByRole('status')
		).toBeVisible();
		api.release();
	});
}
test('public achievements failure retries without identity reread', async ({ page }) => {
	const api = await mockStaffHome(page, { fail: 'achievements' });
	await page.goto(publicPath());
	const region = page.getByTestId('staff-public-achievements');
	await expect(region).toContainText('region ไม่พร้อม');
	await region.getByRole('button', { name: 'ลองใหม่' }).click();
	await expect(region).toContainText('ผลงานที่พร้อมก่อน');
	expect(api.count('/api/achievements')).toBe(2);
	expect(api.count(`/api/staff/${firstStaff}/public-profile`)).toBe(1);
});
test('own-only achievements cannot show a different persons own-scope result', async ({ page }) => {
	const api = await mockStaffHome(page, { permissions: ['achievement.read.own'] });
	await page.goto(publicPath());
	await expect(page.getByTestId('staff-public-profile')).toContainText('บุคลากรแรก');
	await expect(page.getByText('ไม่มีสิทธิ์ดูผลงานของบุคลากรนี้')).toBeVisible();
	expect(api.count('/api/achievements')).toBe(0);
});
test('public view ignores a late prior person', async ({ page }) => {
	const api = await mockStaffHome(page, { hold: 'public-profile' });
	await page.goto(publicPath());
	await expect(page.getByTestId('staff-public-profile').getByRole('status')).toBeVisible();
	await navigate(page, publicPath(secondStaff));
	await expect(page.getByTestId('staff-public-profile')).toContainText('บุคลากรสอง');
	await expect(page.getByTestId('staff-public-achievements')).toContainText('ผลงานคนที่สอง');
	api.release();
	await expect(page.getByText('บุคลากรแรก', { exact: true })).toHaveCount(0);
});
test('parent cannot start staff profile reads', async ({ page }) => {
	const api = await mockStaffHome(page, { userType: 'parent' });
	await page.goto(publicPath());
	await expect(page).toHaveURL(/403/);
	expect(api.count(`/api/staff/${firstStaff}/public-profile`)).toBe(0);
	expect(api.count('/api/achievements')).toBe(0);
});

test('shared counts keep a successful retry across child navigation', async ({ page }) => {
	const api = await mockStaffHome(page, { fail: 'counts' });
	await page.goto(homePath());
	const region = page.getByTestId('staff-work-counts');
	await expect(region).toContainText('region ไม่พร้อม');
	await region.getByRole('button', { name: /ลอง/ }).click();
	await expect(region).toContainText('8');
	await navigate(page, '/staff/profile');
	await expect(page.locator('#nickname')).toHaveValue('ชื่อเล่นเดิม');
	await page.getByRole('link', { name: 'test navigation' }).evaluate((link) => link.remove());
	await navigate(page, homePath());
	await expect(region).toContainText('8');
	await expect(region).not.toContainText('region ไม่พร้อม');
	expect(api.count('/api/menu/user')).toBe(1);
	expect(api.count('/api/me/work-items/counts')).toBe(2);
});
