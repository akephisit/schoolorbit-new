import { expect, test, type Page, type Response } from '@playwright/test';
import {
	sessionCookie,
	startNotificationRecoveryProxy
} from './fixtures/notification-recovery-proxy';

let fixture: Awaited<ReturnType<typeof startNotificationRecoveryProxy>>;
let streamResponse: Response;

test.describe.configure({ mode: 'serial' });
test.use({
	serviceWorkers: 'block',
	launchOptions: {
		args: ['--no-proxy-server']
	}
});
test.beforeAll(async () => {
	fixture = await startNotificationRecoveryProxy();
});
test.afterAll(async () => {
	await fixture?.close();
});
test.beforeEach(async ({ context, page }) => {
	await expect.poll(() => fixture.openStreams).toBe(0);
	fixture.reset();
	await context.addCookies([
		{
			name: '__Host-schoolorbit_session',
			value: sessionCookie,
			domain: '127.0.0.1',
			path: '/',
			httpOnly: true,
			secure: true,
			sameSite: 'Lax'
		}
	]);
	await page.goto(`${fixture.baseUrl}/__notification-recovery-proxy`);
	const streamReady = page.waitForResponse(
		(response) =>
			new URL(response.url()).pathname === '/api/notifications/stream' &&
			response.request().method() === 'GET'
	);
	await page.getByRole('button', { name: 'Start notifications' }).click();
	await expect(page.locator('#ready')).toHaveText('connected');
	await expect(page.locator('#identity')).toHaveText('synthetic-user');
	await expect(page.locator('#unread')).toHaveText('1');
	await expect(page.locator('#work')).toHaveText('2');
	await expect.poll(() => fixture.openStreams).toBe(1);
	streamResponse = await streamReady;
	await expect(page.locator('#error')).toBeEmpty();
});

function pathsAfter(index: number) {
	return fixture.requests
		.slice(index)
		.filter((request) => request.method !== 'OPTIONS')
		.map((request) => request.path);
}

async function expectState(page: Page, unread: number, work: number) {
	await expect(page.locator('#unread')).toHaveText(String(unread));
	await expect(page.locator('#work')).toHaveText(String(work));
	await expect(page.locator('#inbox')).toHaveText(`snapshot-${unread}`);
	await expect(page.locator('#error')).toBeEmpty();
}

test('the exact Nginx SSE location streams small messages with credentialed CORS', async ({
	page,
	context
}) => {
	const stream = fixture.requests.find((request) => request.path === '/api/notifications/stream');
	expect(stream?.cookie).toBe(`__Host-schoolorbit_session=${sessionCookie}`);
	expect(stream?.origin).toBe(fixture.baseUrl);
	expect(streamResponse.status()).toBe(200);
	expect(await streamResponse.allHeaders()).toMatchObject({
		'access-control-allow-origin': fixture.baseUrl,
		'access-control-allow-credentials': 'true',
		'content-type': 'text/event-stream',
		'transfer-encoding': 'chunked'
	});
	const cookie = (await context.cookies()).find(
		(item) => item.name === '__Host-schoolorbit_session'
	);
	expect(cookie).toMatchObject({ httpOnly: true, secure: true, sameSite: 'Lax', path: '/' });
	expect(await page.evaluate(() => isSecureContext)).toBe(true);
	expect(await page.evaluate(() => document.cookie)).not.toContain(sessionCookie);
	// Less than one proxy buffer, delivered while the upstream response remains open indefinitely.
	fixture.send('', { id: 'live-small', title: 'Live', message: 'Synthetic streamed message' });
	await expect(page.locator('#inbox')).toContainText('live-small', { timeout: 1500 });
	expect(fixture.openStreams).toBe(1);
	expect(fixture.connections).toBe(1);
	const preflight = await page.evaluate(async (url) => {
		const response = await fetch(`${url}/api/notifications/stream`, {
			method: 'OPTIONS',
			credentials: 'include',
			headers: { 'X-School-Subdomain': 'fixture' }
		});
		return response.status;
	}, fixture.apiUrl);
	expect(preflight).toBe(204);
});

test('ordinary EOF uses native reconnect then authenticates and catches up inbox/work', async ({
	page
}) => {
	const index = fixture.requests.length;
	fixture.setState(7, 9);
	fixture.drop();
	await expectState(page, 7, 9);
	const sequence = pathsAfter(index);
	expect(sequence[0]).toBe('/api/notifications/stream');
	expect(sequence[1]).toBe('/api/auth/me');
	expect(sequence.slice(2).sort()).toEqual(['/api/me/work-items/counts', '/api/notifications']);
	expect(fixture.connections).toBe(2);
	await expect(page.locator('#windows')).toHaveText('1');
	expect(
		fixture.requests
			.filter((request) => request.path === '/api/notifications/stream')
			.every((request) => request.cookie.includes(sessionCookie))
	).toBe(true);
});

test('reconcile_required catches up over the still-open stream', async ({ page }) => {
	const index = fixture.requests.length;
	fixture.setState(11, 13);
	fixture.send('reconcile_required');
	await expectState(page, 11, 13);
	expect(pathsAfter(index)[0]).toBe('/api/auth/me');
	expect(pathsAfter(index).slice(1).sort()).toEqual([
		'/api/me/work-items/counts',
		'/api/notifications'
	]);
	expect(fixture.connections).toBe(1);
	expect(fixture.openStreams).toBe(1);
});

test('unavailable session validation closes the stream and retries before protected catch-up', async ({
	page
}) => {
	const index = fixture.requests.length;
	fixture.setAuthStatus(503);
	fixture.send('session_unavailable');
	await expect(page.locator('#identity')).toHaveText('unavailable');
	await expect.poll(() => fixture.openStreams).toBe(0);
	expect(pathsAfter(index)).toEqual(['/api/auth/me']);
	await expectState(page, 1, 2);
	fixture.setState(17, 19);
	fixture.setAuthStatus(200);
	await expectState(page, 17, 19);
	expect(pathsAfter(index).slice(0, 4)).toEqual([
		'/api/auth/me',
		'/api/auth/me',
		'/api/notifications/stream',
		'/api/auth/me'
	]);
	expect(pathsAfter(index).slice(4).sort()).toEqual([
		'/api/me/work-items/counts',
		'/api/notifications'
	]);
	await expect(page.locator('#identity')).toHaveText('synthetic-user');
});

test('revoked session validation signs out and never reconnects', async ({ page }) => {
	const index = fixture.requests.length;
	fixture.setAuthStatus(401);
	fixture.send('session_invalid');
	await expect(page).toHaveURL(`${fixture.baseUrl}/login`);
	await expect(page.getByText('Signed out', { exact: true })).toBeVisible();
	await expect.poll(() => fixture.openStreams).toBe(0);
	const sequence = pathsAfter(index);
	expect(sequence).toEqual(['/api/auth/me']);
	// Let the real application reconnect deadline pass; a revoked identity must stay closed.
	await page.waitForTimeout(3500);
	expect(pathsAfter(index)).toEqual(sequence);
	expect(fixture.connections).toBe(1);
});

test('logout fences pending authoritative catch-up and clears its session cookie', async ({
	page,
	context
}) => {
	fixture.setState(23, 29);
	fixture.holdSnapshots();
	fixture.send('reconcile_required');
	await expect.poll(() => fixture.pendingSnapshots).toBe(2);
	await page.getByRole('button', { name: 'Log out', exact: true }).click();
	await expect(page.locator('#ready')).toHaveText('logged-out');
	await expect(page.locator('#identity')).toHaveText('signed-out');
	await expect.poll(() => fixture.openStreams).toBe(0);
	const staleResponses = Promise.all([
		page.waitForResponse((response) => new URL(response.url()).pathname === '/api/notifications'),
		page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/me/work-items/counts'
		)
	]);
	fixture.releaseSnapshots();
	for (const response of await staleResponses) await response.finished();
	await expect(page.locator('#unread')).toHaveText('0');
	await expect(page.locator('#inbox')).toBeEmpty();
	await expect(page.locator('#work')).toHaveText('0');
	expect(
		(await context.cookies()).some((cookie) => cookie.name === '__Host-schoolorbit_session')
	).toBe(false);
	const logout = fixture.requests.find((request) => request.path === '/api/auth/logout');
	expect(logout?.csrf).toBe('synthetic-csrf-token');
	const sequence = pathsAfter(0);
	await page.waitForTimeout(3500);
	expect(pathsAfter(0)).toEqual(sequence);
	expect(fixture.connections).toBe(1);
	await expect(page.locator('#identity')).toHaveText('signed-out');
	await expect(page.locator('#unread')).toHaveText('0');
	await expect(page.locator('#inbox')).toBeEmpty();
	await expect(page.locator('#work')).toHaveText('0');
});
