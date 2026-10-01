import { expect, test } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';
import { PERMISSIONS } from '../../src/lib/permissions/registry';
test.use({ trace: 'off', screenshot: 'off', video: 'off' });
process.env.PLAYWRIGHT_NO_COPY_PROMPT = '1';
const enabled = process.env.E2E_PERSONNEL_LIVE === '1';
test('deployed personnel overview and drilldown are consistent through the proxy', async ({
	page,
	context
}) => {
	test.setTimeout(120_000);
	test.skip(!enabled, 'Set E2E_PERSONNEL_LIVE=1 for authorized read-only acceptance.');
	const username = process.env.E2E_STAFF_USERNAME || process.env.SMOKE_USERNAME;
	const password = process.env.E2E_STAFF_PASSWORD || process.env.SMOKE_PASSWORD;
	const base = process.env.E2E_BASE_URL || process.env.SMOKE_TENANT_URL;
	const api = (
		process.env.E2E_API_URL ||
		process.env.SMOKE_API_URL ||
		'https://school-api.schoolorbit.app'
	).replace(/\/$/, '');
	if (!username || !password || !base || new URL(base).protocol !== 'https:')
		throw new Error('PERSONNEL_LIVE_CONFIGURATION_REQUIRED');
	try {
		await page.goto('/login');
		await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill(username);
		await page.getByLabel('รหัสผ่าน', { exact: true }).fill(password);
		const login = page.waitForResponse((r) => new URL(r.url()).pathname === '/api/auth/login');
		await page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true }).click();
		if (!(await login).ok()) throw new Error('LOGIN_FAILED');
		const headers = {
			Origin: new URL(base).origin,
			'X-School-Subdomain': new URL(base).hostname.split('.')[0]
		};
		async function read<T>(path: string): Promise<T> {
			const response = await context.request.get(api + path, { headers });
			if (!response.ok()) throw new Error('READ_FAILED');
			const reply = (await response.json()) as { success: boolean; data: T };
			if (!reply.success) throw new Error('READ_FAILED');
			return reply.data;
		}
		type Schemas = components['schemas'];
		const overview = await read<Schemas['PersonnelOverview']>(
			'/api/staff/personnel-overview?status=active'
		);
		expect(overview.active + overview.otherStatuses).toBe(overview.total);
		expect(overview.statuses.reduce((n, b) => n + b.count, 0)).toBe(overview.total);
		for (const buckets of [overview.jobPositions, overview.academicRanks, overview.educationLevels])
			expect(buckets.reduce((n, b) => n + b.count, 0)).toBe(overview.filteredTotal);
		await page.goto('/staff/manage/overview');
		const region = page.getByTestId('personnel-overview');
		await expect(region).toHaveAttribute('aria-busy', 'false');
		await expect(page.getByRole('heading', { name: 'ภาพรวมงานบุคคล', exact: true })).toBeVisible();
		const bucket = overview.jobPositions.find((b) => b.count > 0);
		if (bucket) {
			const query = new URLSearchParams({
				status: 'active',
				job_position_id: bucket.key,
				page_size: '1'
			});
			const list = await read<Schemas['StaffListData']>('/api/staff?' + query);
			expect(list.total).toBe(bucket.count);
			await region
				.getByRole('link', { name: `${bucket.label} ${bucket.count} คน`, exact: true })
				.click();
			await expect(page.getByTestId('staff-directory')).toHaveAttribute('aria-busy', 'false');
			expect(new URL(page.url()).searchParams.get('job_position_id')).toBe(bucket.key);
		}
		const actor = await read<Schemas['CurrentUserResponse']>('/api/auth/me');
		if (actor.permissions.includes(PERMISSIONS.STAFF_UPDATE_ALL)) {
			await page.goto('/staff/manage/reference-data');
			await expect(page.getByTestId('staff-reference-catalog')).toHaveAttribute(
				'aria-busy',
				'false'
			);
		}
	} catch {
		throw new Error('PERSONNEL_LIVE_ACCEPTANCE_FAILED');
	}
});
