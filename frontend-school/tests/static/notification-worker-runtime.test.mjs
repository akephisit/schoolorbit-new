import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { bindingMatches } from '../../src/lib/notifications/browser-state.ts';
const source = await readFile(new URL('../../src/service-worker.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, {
	compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
}).outputText;
function harness(options = {}) {
	const now = new Date(Date.now() + 60000).toISOString();
	let binding = { id: 'device', version: 'v1', binding_token: 'token', expires_at: now };
	const events = {};
	const shown = [];
	const opened = [];
	const claims = new Set();
	const sw = {
		location: { origin: 'https://school.invalid' },
		addEventListener(name, listener) {
			events[name] = listener;
		},
		registration: {
			async showNotification(...args) {
				shown.push(args);
			}
		},
		clients: {
			async matchAll() {
				return options.clients ?? [];
			},
			async openWindow(link) {
				opened.push(link);
			}
		}
	};
	const exports = {};
	new Function('require', 'exports', 'self', compiled)(
		(id) => {
			assert.equal(id, './lib/notifications/browser-state.js');
			return {
				bindingMatches,
				async readBrowserState() {
					return { binding };
				},
				async claimPresentation(id) {
					if (options.logoutDuringClaim) binding = null;
					if (claims.has(id)) return false;
					claims.add(id);
					return true;
				}
			};
		},
		exports,
		sw
	);
	return {
		shown,
		opened,
		events,
		setBinding(value) {
			binding = value;
		},
		async push(extra = {}) {
			let pending;
			events.push({
				data: {
					json: () => ({
						id: 'notification',
						device_id: 'device',
						device_version: 'v1',
						binding_token: 'token',
						expires_at: now,
						link: '/staff/work',
						...extra
					})
				},
				waitUntil(work) {
					pending = work;
				}
			});
			await pending;
		},
		async click(data) {
			let pending;
			events.notificationclick({
				notification: { data, close() {} },
				waitUntil(work) {
					pending = work;
				}
			});
			await pending;
		}
	};
}
test('push worker suppresses expired, missing, old-version and old-account bindings', async () => {
	for (const extra of [
		{ expires_at: new Date(Date.now() - 1).toISOString() },
		{ device_version: 'v0' },
		{ binding_token: 'old-account' },
		{ device_id: 'other' }
	]) {
		const h = harness();
		await h.push(extra);
		assert.equal(h.shown.length, 0);
	}
	const h = harness();
	h.setBinding(null);
	await h.push();
	assert.equal(h.shown.length, 0);
});
test('duplicate pushes use one generic OS notification with stable id tag and renotify disabled', async () => {
	const h = harness();
	await h.push({ title: 'private', body: 'private' });
	await h.push();
	assert.equal(h.shown.length, 1);
	const [title, options] = h.shown[0];
	assert.equal(title, 'SchoolOrbit');
	assert.equal(options.renotify, false);
	assert.equal(options.tag, 'schoolorbit:notification');
	assert.doesNotMatch(options.body, /private/);
});
test('logout during asynchronous presentation coordination invalidates the queued push', async () => {
	const h = harness({ logoutDuringClaim: true });
	await h.push();
	assert.equal(h.shown.length, 0);
});
test('notification clicks reject stale account bindings and constrain navigation to the app origin', async () => {
	const h = harness();
	await h.click({ device_id: 'device', device_version: 'old', link: '/private' });
	assert.equal(h.opened.length, 0);
	await h.click({ device_id: 'device', device_version: 'v1', link: 'https://evil.invalid' });
	assert.deepEqual(h.opened, ['https://school.invalid/']);
});
test('worker owns no fetch handler or application caching', () => {
	assert.doesNotMatch(source, /addEventListener\(['"]fetch|caches\./);
});

test('focused owned client claims foreground presentation and suppresses the OS duplicate', async () => {
	let messages = 0;
	const client = {
		visibilityState: 'visible',
		focused: true,
		postMessage(message, ports) {
			messages++;
			assert.equal(message.type, 'schoolorbit:push');
			ports[0].postMessage({ handled: true });
			ports[0].close();
		}
	};
	const h = harness({ clients: [client] });
	await h.push();
	assert.equal(messages, 1);
	assert.equal(h.shown.length, 0);
});
test('visible unfocused clients do not suppress OS presentation', async () => {
	const h = harness({
		clients: [
			{
				visibilityState: 'visible',
				focused: false,
				postMessage() {
					assert.fail('unfocused client was asked to present');
				}
			}
		]
	});
	await h.push();
	assert.equal(h.shown.length, 1);
});
