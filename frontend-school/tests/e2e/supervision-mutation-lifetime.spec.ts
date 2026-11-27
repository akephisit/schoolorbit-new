import { expect, test, type Page } from '@playwright/test';
import { revalidatePage } from './fixtures/page-revalidation';
import { id, year, mock, navigate, path } from './fixtures/supervision-route-data';

test.use({ serviceWorkers: 'block' });
test.beforeEach(async ({ page }) => {
	await page.route('https://fonts.googleapis.com/**', (route) =>
		route.fulfill({ status: 200, contentType: 'text/css', body: '' })
	);
});

const permissions = ['supervision.manage.school', 'supervision.read.school'];
const detailPath = `/staff/academic/supervision/${id(30)}?academicYearId=${year}`;
const availabilityPath = `/api/supervision/observations/${id(30)}/evaluator-availability`;
const conflict = 'ข้อมูลขัดแย้งจากการดำเนินการในหน้าก่อน';
type Owner = 'workspace' | 'detail';
type Outcome = 'success' | 'error';

async function holdMutation(page: Page, owner: Owner, outcome: Outcome) {
	const api = await mock(page, { observationStatus: owner === 'detail' ? 'planned' : 'requested' });
	const endpoint = `/api/supervision/observations/${id(30)}/${owner === 'workspace' ? 'approve-request' : 'evaluators'}`;
	const started = Promise.withResolvers<void>();
	const gate = Promise.withResolvers<void>();
	await page.route(`**${endpoint}`, async (route) => {
		started.resolve();
		await gate.promise;
		await route.fulfill({
			status: outcome === 'error' ? 409 : 200,
			contentType: 'application/json',
			body: JSON.stringify(
				outcome === 'error'
					? { success: false, error: conflict }
					: { success: true, data: { ...api.observation(), status: 'planned' } }
			)
		});
	});
	await page.goto(owner === 'workspace' ? path('requests') : detailPath);
	if (owner === 'detail') {
		await page.getByRole('button', { name: 'แก้ผู้ประเมิน', exact: true }).click();
	}
	await page.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน', exact: true }).click();
	await page.getByRole('option', { name: /ผู้ประเมินที่ว่าง/ }).click();
	await page.keyboard.press('Escape');
	await page
		.getByRole('button', {
			name: owner === 'workspace' ? 'อนุมัติและมอบหมาย' : 'บันทึกผู้ประเมิน',
			exact: true
		})
		.click();
	await started.promise;
	return {
		api,
		async settle() {
			const response = page.waitForResponse(
				(response) => new URL(response.url()).pathname === endpoint
			);
			gate.resolve();
			await (await response).finished();
			// Let fetch continuations and Svelte/toast rendering finish before negative assertions.
			await page.evaluate(
				() =>
					new Promise<void>((resolve) =>
						requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
					)
			);
		}
	};
}

async function expectNoOldEffects(
	page: Page,
	api: Awaited<ReturnType<typeof mock>>,
	reads: number
) {
	await expect(page.getByText(conflict, { exact: true })).toHaveCount(0);
	await expect(page.getByText('อนุมัติคำขอและมอบหมายผู้ประเมินแล้ว', { exact: true })).toHaveCount(
		0
	);
	await expect(page.getByText('แก้ไขผู้ประเมินแล้ว', { exact: true })).toHaveCount(0);
	expect(api.count(availabilityPath)).toBe(reads);
}

for (const owner of ['workspace', 'detail'] as const) {
	for (const outcome of ['success', 'error'] as const) {
		test(`${owner} disposal rejects a held mutation ${outcome} and its follow-up effects`, async ({
			page
		}) => {
			const { api, settle } = await holdMutation(page, owner, outcome);
			if (owner === 'detail') {
				await page.keyboard.press('Escape');
				await expect(page.getByRole('dialog')).toHaveCount(0);
			}
			await navigate(page, owner === 'workspace' ? detailPath : path('cycles'));
			if (owner === 'workspace') {
				await expect(page.getByTestId('supervision-detail')).toHaveAttribute('aria-busy', 'false');
				await expect(page.getByTestId('supervision-observations')).toHaveCount(0);
			} else {
				await expect(
					page
						.getByTestId('supervision-cycles-ready')
						.getByRole('listitem')
						.filter({ hasText: 'รอบแรก' })
				).toBeVisible();
				await expect(page.getByTestId('supervision-detail')).toHaveCount(0);
			}
			const reads = api.count(availabilityPath);
			await settle();
			await expectNoOldEffects(page, api, reads);
		});
	}

	for (const change of ['permissions', 'session'] as const) {
		test(`${owner} rejects a held mutation after ${change} replacement`, async ({ page }) => {
			const { api, settle } = await holdMutation(
				page,
				owner,
				owner === 'workspace' ? 'success' : 'error'
			);
			const freshPermissions =
				change === 'permissions' ? [...permissions, 'supervision.read.own'] : permissions;
			await api.replaceIdentity(
				freshPermissions,
				change === 'session' ? '01900000-0000-7000-8000-000000000002' : undefined
			);
			await expect(
				page.getByRole('combobox', { name: 'เพิ่ม/เลือกผู้ประเมิน', exact: true })
			).toHaveCount(owner === 'detail' ? 0 : 1);
			const reads = api.count(availabilityPath);
			await settle();
			await expectNoOldEffects(page, api, reads);
		});
	}

	test(`${owner} keeps a same-context mutation through a background refresh`, async ({ page }) => {
		const { api, settle } = await holdMutation(page, owner, 'success');
		const primary =
			owner === 'workspace'
				? '/api/supervision/observations'
				: `/api/supervision/observations/${id(30)}`;
		const reads = api.count(primary);
		if (owner === 'detail') {
			await page.keyboard.press('Escape');
			await expect(page.getByRole('dialog')).toHaveCount(0);
		}
		await revalidatePage(page);
		await expect.poll(() => api.count(primary)).toBe(reads + 1);
		await settle();
		await expect(
			page.getByText(
				owner === 'workspace' ? 'อนุมัติคำขอและมอบหมายผู้ประเมินแล้ว' : 'แก้ไขผู้ประเมินแล้ว',
				{ exact: true }
			)
		).toBeVisible();
		if (owner === 'detail') await expect(page.getByRole('dialog')).toHaveCount(0);
		else
			await expect(
				page.getByRole('button', { name: 'อนุมัติและมอบหมาย', exact: true })
			).toHaveCount(0);
	});
}
