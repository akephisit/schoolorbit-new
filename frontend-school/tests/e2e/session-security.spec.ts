import { expect, test, type BrowserContext, type Page } from '@playwright/test';

const configuredSessionUsername = process.env.E2E_SESSION_USERNAME;
const configuredSessionPassword = process.env.E2E_SESSION_PASSWORD;

if (!configuredSessionUsername || !configuredSessionPassword) {
	throw new Error(
		'E2E_SESSION_USERNAME and E2E_SESSION_PASSWORD must identify a dedicated disposable account.'
	);
}
const sessionUsername = configuredSessionUsername;
const sessionPassword = configuredSessionPassword;

const baseURL =
	process.env.E2E_BASE_URL ||
	process.env.SMOKE_TENANT_URL ||
	`https://${process.env.SMOKE_SUBDOMAIN || 'sandbox'}.schoolorbit.app`;
const apiURL = (
	process.env.E2E_API_URL ||
	process.env.SMOKE_API_URL ||
	'https://school-api.schoolorbit.app'
).replace(/\/$/, '');
const otherTenantURL = process.env.E2E_OTHER_TENANT_URL?.replace(/\/$/, '');
const primaryOrigin = new URL(baseURL).origin;
const schoolSubdomain =
	process.env.SMOKE_SUBDOMAIN || new URL(primaryOrigin).hostname.split('.')[0];

async function login(context: BrowserContext, page?: Page): Promise<Page> {
	const loginPage = page ?? (await context.newPage());
	await loginPage.goto(`${primaryOrigin}/login`);
	await expect(loginPage.getByRole('heading', { name: 'เข้าสู่ระบบ' })).toBeVisible();
	await loginPage.getByLabel('ชื่อผู้ใช้งาน (Username)').fill(sessionUsername);
	await loginPage.getByLabel('รหัสผ่าน').fill(sessionPassword);

	await Promise.all([
		loginPage.waitForURL(/\/(staff|student|parent)\/?(?:[?#].*)?$/, { timeout: 15_000 }),
		loginPage.getByRole('button', { name: 'เข้าสู่ระบบ' }).click()
	]);

	return loginPage;
}

async function currentSessionId(page: Page): Promise<string> {
	const current = page.locator('[data-testid^="session-row-"][data-current="true"]');
	await expect(current).toHaveCount(1);
	const testId = await current.getAttribute('data-testid');
	if (!testId?.startsWith('session-row-')) {
		throw new Error('Current session row has no stable session identifier.');
	}
	return testId.slice('session-row-'.length);
}

async function expectProtectedNavigationToRequireLogin(page: Page): Promise<void> {
	await page.goto(`${primaryOrigin}/account/security`);
	await expect(page).toHaveURL(/\/login\/?(?:[?#].*)?$/, { timeout: 15_000 });
}

async function bestEffortCurrentLogout(context: BrowserContext): Promise<void> {
	try {
		const headers = {
			Origin: primaryOrigin,
			'X-School-Subdomain': schoolSubdomain
		};
		const current = await context.request.get(`${apiURL}/api/auth/me`, { headers });
		if (!current.ok()) return;
		const csrf = current.headers()['x-csrf-token'];
		if (!csrf) return;
		await context.request.post(`${apiURL}/api/auth/logout`, {
			headers: { ...headers, 'X-CSRF-Token': csrf }
		});
	} catch {
		// Cleanup is deliberately best-effort so the original assertion remains visible.
	}
}

test.describe.serial('school session security', () => {
	test('revokes one selected browser context and then all contexts', async ({ browser }) => {
		const contextA = await browser.newContext({ baseURL: primaryOrigin });
		const contextB = await browser.newContext({ baseURL: primaryOrigin });

		try {
			const pageA = await login(contextA);
			const pageB = await login(contextB);

			await pageB.goto(`${primaryOrigin}/account/security`);
			const bSessionId = await currentSessionId(pageB);

			await pageA.goto(`${primaryOrigin}/account/security`);
			const sessionB = pageA.getByTestId(`session-row-${bSessionId}`);
			await expect(sessionB).toHaveAttribute('data-current', 'false');
			await sessionB.getByRole('button', { name: 'นำอุปกรณ์ออก' }).click();
			await expect(sessionB).toHaveCount(0);

			await expectProtectedNavigationToRequireLogin(pageB);
			await login(contextB, pageB);

			await pageA.goto(`${primaryOrigin}/account/security`);
			await pageA.getByTestId('logout-all-sessions').click();
			await pageA
				.getByRole('alertdialog')
				.getByRole('button', { name: 'ออกจากระบบทุกอุปกรณ์' })
				.click();
			await expect(pageA).toHaveURL(/\/login\/?(?:[?#].*)?$/);
			await expectProtectedNavigationToRequireLogin(pageB);
		} finally {
			await bestEffortCurrentLogout(contextA);
			await bestEffortCurrentLogout(contextB);
			await contextA.close();
			await contextB.close();
		}
	});

	test('rejects a legacy JWT cookie without an opaque session', async ({ browser }) => {
		const context = await browser.newContext({ baseURL: primaryOrigin });
		try {
			await context.addCookies([
				{
					name: 'auth_token',
					value: 'synthetic.legacy.jwt',
					url: `${new URL(apiURL).origin}/`,
					httpOnly: true,
					secure: new URL(apiURL).protocol === 'https:',
					sameSite: 'Lax'
				}
			]);
			const page = await context.newPage();
			await expectProtectedNavigationToRequireLogin(page);
		} finally {
			await context.close();
		}
	});

	test('rejects a primary-tenant session on another tenant', async ({ browser }) => {
		test.skip(!otherTenantURL, 'Set E2E_OTHER_TENANT_URL to verify tenant isolation.');
		const context = await browser.newContext({ baseURL: primaryOrigin });
		try {
			const page = await login(context);
			await page.goto(`${otherTenantURL}/account/security`);
			await expect(page).toHaveURL(/\/login\/?(?:[?#].*)?$/, { timeout: 15_000 });
		} finally {
			await bestEffortCurrentLogout(context);
			await context.close();
		}
	});
});
