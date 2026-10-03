import { expect, test } from '@playwright/test';
import type { components } from '../../src/lib/api/generated/school-api';
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
	let phase = 'login';
	let csrf: string | undefined;
	let failedPhase: string | null = null;
	let cleanupFailed = false;
	try {
		await page.goto('/login');
		await page.getByLabel('ชื่อผู้ใช้งาน (Username)').fill(username);
		await page.getByLabel('รหัสผ่าน', { exact: true }).fill(password);
		const login = page.waitForResponse((r) => new URL(r.url()).pathname === '/api/auth/login');
		await page.getByRole('button', { name: 'เข้าสู่ระบบ', exact: true }).click();
		const loginResponse = await login;
		if (!loginResponse.ok()) throw new Error('LOGIN_FAILED');
		csrf = loginResponse.headers()['x-csrf-token'];
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
		phase = 'overview_api';
		const overview = await read<Schemas['PersonnelOverview']>(
			'/api/staff/personnel-overview?status=active'
		);
		expect(overview.active + overview.otherStatuses).toBe(overview.total);
		expect(overview.statuses.reduce((n, b) => n + b.count, 0)).toBe(overview.total);
		for (const buckets of [overview.jobPositions, overview.academicRanks, overview.educationLevels])
			expect(buckets.reduce((n, b) => n + b.count, 0)).toBe(overview.filteredTotal);
		phase = 'overview_ui';
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
		phase = 'career_detail';
		const directory = await read<Schemas['StaffListData']>('/api/staff?status=active&page_size=1');
		if (directory.items[0]) {
			const personId = directory.items[0].id;
			const profile = await read<Schemas['StaffProfileResponse']>(`/api/staff/${personId}`);
			const history = await read<Schemas['StaffCareerHistoryPage']>(
				`/api/staff/${personId}/career-history`
			);
			if (profile.staff_info) expect(history.current).toEqual(profile.staff_info.current_career);
			for (const entry of history.items) {
				expect(entry.staffId).toBe(personId);
				if (entry.source === 'existing_record' && entry.revision === 1) {
					expect(entry.effectiveDate).toBeNull();
					expect(entry.orderDate).toBeNull();
					expect(entry.orderNumber).toBeNull();
				}
			}
			await page.goto(`/staff/manage/${personId}`);
			const career = page.getByRole('region', { name: 'ประวัติตำแหน่งและวิทยฐานะ', exact: true });
			await expect(career).toHaveAttribute('aria-busy', 'false');
			await expect(career.getByTestId('career-current-academic_rank')).toBeVisible();
		}
		phase = 'career_own';
		const me = await read<Schemas['CurrentUserResponse']>('/api/auth/me');
		const ownReadable = me.permissions.some((code) =>
			[
				'staff_profile.read.own',
				'staff_profile.read.school',
				'staff_profile.read.organization_unit',
				'staff_profile.read.organization_tree'
			].includes(code)
		);
		await page.goto('/staff/profile');
		const ownHistory = page.getByRole('region', { name: 'ประวัติตำแหน่งและวิทยฐานะ', exact: true });
		if (ownReadable) {
			await read<Schemas['StaffCareerHistoryPage']>(`/api/staff/${me.id}/career-history`);
			await expect(ownHistory).toHaveAttribute('aria-busy', 'false');
			await expect(ownHistory.getByTestId('career-current-academic_rank')).toBeVisible();
		} else await expect(ownHistory).toHaveCount(0);
		phase = 'positions_api';
		const positions = await read<Schemas['JobPositionPage']>(
			'/api/staff/job-positions?selectableOnly=true'
		);
		expect(positions.items.every((p) => p.isActive && p.isSelectable)).toBe(true);
		phase = 'retired_api';
		const retired = await context.request.get(api + '/api/staff/reference-items', { headers });
		// The removed collection slug now matches the UUID profile route and is rejected.
		expect(retired.status()).toBe(400);
		const retiredItem = await context.request.get(
			api + '/api/staff/reference-items/00000000-0000-0000-0000-000000000000',
			{ headers }
		);
		expect(retiredItem.status()).toBe(404);
		phase = 'directory_ui';
		await page.goto('/staff/manage');
		await expect(page.getByRole('link', { name: 'จัดการรายการกลาง', exact: true })).toHaveCount(0);
		phase = 'retired_ui';
		await page.goto('/staff/manage/reference-data');
		await expect(page.getByText('404', { exact: true })).toBeVisible();
	} catch {
		failedPhase = phase;
	} finally {
		if (csrf) {
			try {
				const logout = await context.request.post(api + '/api/auth/logout', {
					headers: { Origin: new URL(base).origin, 'X-CSRF-Token': csrf },
					data: {}
				});
				cleanupFailed = !logout.ok();
			} catch {
				cleanupFailed = true;
			}
		}
	}
	if (failedPhase || cleanupFailed)
		throw new Error(
			'PERSONNEL_LIVE_ACCEPTANCE_FAILED_' +
				(failedPhase ?? 'session_cleanup') +
				(cleanupFailed ? '_SESSION_CLEANUP_FAILED' : '')
		);
});
