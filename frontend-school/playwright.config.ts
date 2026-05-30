import { defineConfig, devices } from '@playwright/test';

const subdomain = process.env.SMOKE_SUBDOMAIN || 'sandbox';
const baseURL =
	process.env.E2E_BASE_URL ||
	process.env.SMOKE_TENANT_URL ||
	`https://${subdomain}.schoolorbit.app`;

export default defineConfig({
	testDir: './tests/e2e',
	fullyParallel: true,
	forbidOnly: Boolean(process.env.CI),
	retries: process.env.CI ? 2 : 0,
	reporter: process.env.CI ? [['github'], ['list']] : 'list',
	timeout: 30_000,
	expect: {
		timeout: 10_000
	},
	use: {
		baseURL,
		screenshot: 'only-on-failure',
		trace: 'retain-on-failure',
		video: 'retain-on-failure'
	},
	projects: [
		{
			name: 'chromium',
			use: { ...devices['Desktop Chrome'] }
		}
	]
});
