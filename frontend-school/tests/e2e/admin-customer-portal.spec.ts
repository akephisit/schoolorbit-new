import { test, expect, type Page } from '@playwright/test';

// Dedicated local Admin fixture; never points at a School tenant or sends email.
const origin = process.env.E2E_ADMIN_URL || 'https://127.0.0.1:4174';
test.use({ ignoreHTTPSErrors: true });
const school = {
	id: '0195db54-1234-7000-8000-000000000001',
	name: 'โรงเรียนตัวอย่าง',
	subdomain: 'fixture',
	cadence: 'monthly',
	endsAt: '2026-01-01T00:00:00Z',
	expired: true
};
async function fixture(page: Page, role: 'customer' | 'operator' | null) {
	await page.route('https://fonts.googleapis.com/**', (route) => route.abort());
	await page.route('https://fonts.gstatic.com/**', (route) => route.abort());
	const calls: {
		path: string;
		method: string;
		body: Record<string, string> | null;
		csrf?: string;
	}[] = [];
	let signed = role !== null;
	const user = {
		id: '0195db54-1234-7000-8000-000000000002',
		email: 'owner@example.test',
		name: 'ผู้ดูแลตัวอย่าง',
		role: role || 'customer'
	};
	let requestStatus = 'pending';
	await page.route('http://127.0.0.1:8089/**', async (route) => {
		const req = route.request();
		const path = new URL(req.url()).pathname;
		if (req.method() === 'OPTIONS') {
			await route.fulfill({
				status: 204,
				headers: {
					'Access-Control-Allow-Origin': origin,
					'Access-Control-Allow-Credentials': 'true',
					'Access-Control-Allow-Headers': 'Content-Type,X-CSRF-Token'
				}
			});
			return;
		}
		calls.push({
			path,
			method: req.method(),
			body: req.postData() ? req.postDataJSON() : null,
			csrf: req.headers()['x-csrf-token']
		});
		let data: object | undefined;
		if (path === '/api/v1/auth/login') {
			signed = true;
			data = { user, csrfToken: 'fixture-csrf' };
		} else if (path === '/api/v1/auth/me' && signed) data = { user, csrfToken: 'fixture-csrf' };
		else if (path === '/api/v1/portal/schools') data = [school];
		else if (path === '/api/v1/portal/requests')
			data = [
				{
					id: '0195db54-1234-7000-8000-000000000003',
					accountId: user.id,
					email: user.email,
					schoolId: null,
					kind: 'open_school',
					schoolName: school.name,
					subdomain: school.subdomain,
					cadence: 'monthly',
					status: requestStatus,
					decisionReason: null,
					createdAt: '2026-10-15T00:00:00Z'
				}
			];
		else if (path === '/api/v1/schools')
			data = { schools: [school], total: 1, page: 1, limit: 100, totalPages: 1 };
		else if (path.includes('/decision')) requestStatus = 'approved';
		const unauthorized = path === '/api/v1/auth/me' && !signed;
		await route.fulfill({
			status: unauthorized ? 401 : 200,
			headers: {
				'Access-Control-Allow-Origin': origin,
				'Access-Control-Allow-Credentials': 'true'
			},
			contentType: 'application/json',
			body: JSON.stringify(
				unauthorized ? { success: false, error: 'Unauthorized' } : { success: true, data }
			)
		});
	});
	return calls;
}

test('Admin registration uses email and a scanner-safe explicit verification', async ({ page }) => {
	const calls = await fixture(page, null);
	await page.goto(`${origin}/register`);
	await page.getByLabel('ชื่อผู้ติดต่อ').fill('ผู้ดูแลตัวอย่าง');
	await page.getByLabel('อีเมล', { exact: true }).fill('owner@example.test');
	await page.getByLabel('รหัสผ่าน', { exact: true }).fill('fixture-password-123');
	await page.getByRole('button', { name: 'สร้างบัญชี' }).click();
	await expect(page.getByRole('status')).toContainText('ตรวจสอบกล่องจดหมาย');
	expect(calls.find((call) => call.path.endsWith('/register'))?.body?.email).toBe(
		'owner@example.test'
	);
	await page.goto(`${origin}/verify-email#token=${'a'.repeat(64)}`);
	await expect(page).toHaveURL(`${origin}/verify-email`);
	expect(calls.filter((call) => call.path.endsWith('/verify-email')).length).toBe(0);
	await page.getByRole('button', { name: 'ยืนยันอีเมล', exact: true }).click();
	await expect(page.getByRole('status')).toContainText('ยืนยันอีเมลแล้ว');
	expect(calls.find((call) => call.path.endsWith('/verify-email'))?.body?.token).toBe(
		'a'.repeat(64)
	);
});

test('Admin customer sees warning-only expiry and cannot render operator tools', async ({
	page
}) => {
	const calls = await fixture(page, 'customer');
	await page.goto(`${origin}/dashboard`);
	await expect(page.getByRole('heading', { name: school.name })).toBeVisible();
	await expect(page.getByText('โรงเรียนยังใช้งานได้ตามปกติ', { exact: false })).toBeVisible();
	await expect(page.getByRole('link', { name: 'จัดการโรงเรียนทั้งหมด' })).toHaveCount(0);
	await page.goto(`${origin}/dashboard/schools`);
	await expect(page.getByRole('alert')).toContainText('สำหรับทีม SchoolOrbit เท่านั้น');
	expect(calls.filter((call) => call.path === '/api/v1/schools').length).toBe(0);
});

test('Admin login sends email and session CSRF is used for operator approval', async ({ page }) => {
	const calls = await fixture(page, 'operator');
	await page.goto(`${origin}/login`);
	await page.getByLabel('อีเมล', { exact: true }).fill('owner@example.test');
	await page.getByLabel('รหัสผ่าน', { exact: true }).fill('fixture-password-123');
	await page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true }).click();
	await expect(page).toHaveURL(`${origin}/dashboard`);
	await page.getByRole('link', { name: 'คำขอจากลูกค้า' }).click();
	await page.getByLabel('โรงเรียนที่สร้างและตรวจสอบแล้ว').selectOption(school.id);
	await page.getByLabel('เหตุผล / หลักฐานการอนุมัติ').fill('ตรวจสอบผู้ขอและการชำระเงินแล้ว');
	await page.getByRole('button', { name: 'อนุมัติและเริ่มรอบบริการ' }).click();
	await expect(page.getByText('อนุมัติแล้ว', { exact: true })).toBeVisible();
	const decision = calls.find((call) => call.path.endsWith('/decision'));
	expect(decision?.csrf).toBe('fixture-csrf');
	expect(decision?.body?.schoolId).toBe(school.id);
});

test('Admin reset keeps its secret out of the URL and auth pages fit mobile', async ({ page }) => {
	const calls = await fixture(page, null);
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto(`${origin}/reset-password#token=${'b'.repeat(64)}`);
	await expect(page).toHaveURL(`${origin}/reset-password`);
	await page.getByLabel('รหัสผ่าน', { exact: true }).fill('fixture-password-456');
	await page.getByRole('button', { name: 'บันทึกรหัสผ่านใหม่' }).click();
	await expect(page.getByRole('status')).toContainText('กรุณาเข้าสู่ระบบใหม่');
	expect(calls.find((call) => call.path.endsWith('/reset-password'))?.body?.token).toBe(
		'b'.repeat(64)
	);
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
		true
	);
});
