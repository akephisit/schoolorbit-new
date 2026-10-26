import assert from 'node:assert/strict';
import test from 'node:test';
import {
	deviceHarness,
	deferred,
	flush,
	PushBindingChangedError
} from '../helpers/notification-device-harness.mjs';

test('existing subscription sync uses runtime key and persists server binding without requesting permission', async () => {
	const h = deviceHarness();
	assert.equal(await h.store.enablePushFromUserAction(), true);
	assert.equal(h.calls.register[0].expected_version, undefined);
	assert.equal(await h.store.syncExistingPushSubscription(), true);
	assert.equal(h.calls.register[1].expected_version, 'version-1');
	assert.equal(h.calls.permission, 0);
	assert.equal(h.calls.subscribe.length, 0);
	assert.equal(h.calls.register[0].key_id, 'key-1');
	assert.equal(h.state.binding.binding_token, 'token-1');
	assert.equal((await h.store.getPushStatus()).synced, true);
});
test('unavailable key is degraded even with a browser subscription', async () => {
	const h = deviceHarness({ config: { available: false, public_key: null, key_id: null } });
	assert.equal(await h.store.syncExistingPushSubscription(), false);
	const status = await h.store.getPushStatus();
	assert.equal(status.synced, false);
	assert.equal(status.serverAvailable, false);
	assert.equal(h.calls.register.length, 0);
});
test('key rotation is silent until user action then recreates the exact old subscription', async () => {
	const h = deviceHarness({ oldKey: true });
	assert.equal(await h.store.syncExistingPushSubscription(), false);
	assert.equal(h.calls.unsubscribe, 0);
	assert.equal(h.calls.subscribe.length, 0);
	assert.equal(await h.store.enablePushFromUserAction(), true);
	assert.equal(h.calls.unsubscribe, 1);
	assert.deepEqual([...h.calls.subscribe[0].applicationServerKey], [1, 2, 3]);
});
test('logout clears local binding, revokes only current version and closes browser notifications', async () => {
	const h = deviceHarness();
	await h.store.enablePushFromUserAction();
	await h.store.prepareLogout();
	assert.equal(h.state.binding, null);
	assert.deepEqual(h.calls.revoke, [{ id: 'device-1', version: 'version-1' }]);
	assert.equal(h.calls.unsubscribe, 1);
	assert.equal(h.calls.close, 1);
});
test('logout or switch during registration rejects late binding and revokes its exact old version', async () => {
	for (const action of ['logout', 'switch']) {
		const pending = deferred();
		const h = deviceHarness({ register: () => pending.promise });
		const registration = h.store.enablePushFromUserAction();
		await flush();
		if (action === 'logout') await h.store.prepareLogout();
		else h.switchUser('user-b');
		pending.resolve({
			id: 'old-device',
			version: 'old-version',
			binding_token: 'old-token',
			installation_id: 'installation',
			expires_at: new Date(Date.now() + 86400_000).toISOString()
		});
		assert.equal(await registration, false);
		assert.equal(h.state.binding, null);
		assert.deepEqual(h.calls.revoke, [{ id: 'old-device', version: 'old-version' }]);
	}
});
test('same-identity overlapping registrations do not revoke the preserved server generation', async () => {
	const first = deferred();
	const second = deferred();
	const h = deviceHarness({
		register: (_, count) => (count === 1 ? first.promise : second.promise)
	});
	const a = h.store.enablePushFromUserAction();
	await flush();
	const b = h.store.enablePushFromUserAction();
	await flush();
	const device = {
		id: 'device',
		version: 'stable',
		binding_token: 'stable-token',
		installation_id: 'installation',
		expires_at: new Date(Date.now() + 86400_000).toISOString()
	};
	second.resolve(device);
	assert.equal(await b, true);
	first.resolve(device);
	assert.equal(await a, false);
	assert.equal(h.state.binding.version, 'stable');
	assert.equal(h.calls.revoke.length, 0);
});
test('revoking another device leaves this installation subscribed', async () => {
	const h = deviceHarness();
	await h.store.enablePushFromUserAction();
	await h.store.revokePushDevice({ id: 'other', version: 'other-version' });
	assert.equal(h.state.binding.id, 'device-1');
	assert.equal(h.calls.unsubscribe, 0);
	assert.deepEqual(h.calls.revoke, [{ id: 'other', version: 'other-version' }]);
});

test('a delayed pre-disable register response cannot revoke a newer identical same-account registration', async () => {
	const stale = deferred();
	const device = (version) => ({
		id: 'device',
		version,
		binding_token: version,
		installation_id: 'installation',
		expires_at: new Date(Date.now() + 86400_000).toISOString()
	});
	const h = deviceHarness({
		register: (_, count) =>
			count === 2 ? stale.promise : device(count === 1 ? 'initial' : 'renewed')
	});
	await h.store.enablePushFromUserAction();
	const pending = h.store.syncExistingPushSubscription();
	await flush();
	await h.store.revokePushDevice(device('initial'));
	assert.equal(await h.store.enablePushFromUserAction(), true);
	stale.resolve(device('renewed'));
	assert.equal(await pending, false);
	assert.equal(h.state.binding.version, 'renewed');
	assert.deepEqual(h.calls.revoke, [{ id: 'device', version: 'initial' }]);
});

test('logout and current-device revoke reject a subscription read that finishes after an account switch', async () => {
	for (const action of ['logout', 'revoke']) {
		const pending = deferred();
		let hold = false;
		const h = deviceHarness({ getSubscription: () => (hold ? pending.promise : undefined) });
		await h.store.enablePushFromUserAction();
		hold = true;
		const operation =
			action === 'logout' ? h.store.prepareLogout() : h.store.revokePushDevice(h.state.binding);
		await flush();
		h.switchUser('user-b');
		pending.resolve({
			async unsubscribe() {
				assert.fail('superseded browser subscription must not be removed');
			}
		});
		await operation;
		assert.equal(h.calls.unsubscribe, 0);
	}
});

test('an account change while the permission prompt is open cancels the original enable action', async () => {
	const permission = deferred();
	const h = deviceHarness({
		permission: 'default',
		requestPermission: () => permission.promise,
		subscription: false
	});
	const enable = h.store.enablePushFromUserAction();
	await flush();
	h.switchUser('user-b');
	permission.resolve('granted');
	assert.equal(await enable, false);
	assert.equal(h.calls.register.length, 0);
	assert.equal(h.calls.subscribe.length, 0);
});

test('an unbound existing browser subscription is never automatically registered', async () => {
	const h = deviceHarness();
	assert.equal(await h.store.syncExistingPushSubscription(), false);
	assert.equal(h.calls.register.length, 0);
	assert.equal(h.calls.unsubscribe, 0);
	assert.equal(h.state.binding, null);
});
test('remote revoke persists through repeated auth sync and requires explicit enable to return', async () => {
	const h = deviceHarness();
	await h.store.enablePushFromUserAction();
	h.devices.splice(0);
	assert.equal(await h.store.syncExistingPushSubscription(), false);
	assert.equal(h.calls.register[1].expected_version, 'version-1');
	assert.equal(h.state.binding, null);
	assert.equal(h.state.identity, 'school:user-a:staff');
	assert.equal(h.calls.unsubscribe, 1);
	assert.equal(await h.store.syncExistingPushSubscription(), false);
	assert.equal(h.calls.register.length, 2);
	assert.equal(await h.store.enablePushFromUserAction(), true);
	assert.equal(h.calls.register[2].expected_version, undefined);
	assert.equal(h.devices.length, 1);
	assert.equal(h.state.binding.version, 'version-2');
});
test('a revoke racing the refresh is confirmed atomically and cannot be recreated', async () => {
	const gate = deferred();
	const h = deviceHarness({
		beforeRegister: (_, count) => (count === 2 ? gate.promise : undefined)
	});
	await h.store.enablePushFromUserAction();
	const refresh = h.store.syncExistingPushSubscription();
	await flush();
	h.devices.splice(0);
	gate.resolve();
	assert.equal(await refresh, false);
	assert.equal(h.devices.length, 0);
	assert.equal(h.state.binding, null);
	assert.equal(h.calls.unsubscribe, 1);
});
test('late refresh conflict does not clear or unsubscribe a newer explicit enable', async () => {
	const gate = deferred();
	const h = deviceHarness({
		beforeRegister: (_, count) => (count === 2 ? gate.promise : undefined)
	});
	await h.store.enablePushFromUserAction();
	const refresh = h.store.syncExistingPushSubscription();
	await flush();
	h.devices.splice(0);
	assert.equal(await h.store.enablePushFromUserAction(), true);
	gate.resolve();
	assert.equal(await refresh, false);
	assert.equal(h.state.binding.version, 'version-2');
	assert.equal(h.calls.unsubscribe, 0);
	assert.equal(h.devices.length, 1);
});
test('a stale-generation refresh conflict cannot affect the next account binding', async () => {
	const gate = deferred();
	const h = deviceHarness({
		beforeRegister: async (_, count) => {
			if (count === 2) {
				await gate.promise;
				throw new PushBindingChangedError();
			}
		}
	});
	await h.store.enablePushFromUserAction();
	const refresh = h.store.syncExistingPushSubscription();
	await flush();
	h.switchUser('user-b');
	await h.store.enablePushFromUserAction();
	const binding = h.state.binding;
	gate.resolve();
	assert.equal(await refresh, false);
	assert.equal(h.state.binding, binding);
	assert.equal(h.calls.unsubscribe, 0);
});
test('network, service and unrelated conflict errors preserve a good binding for later refresh', async () => {
	for (const message of ['network error', '503 unavailable', '409 push_key_changed']) {
		const h = deviceHarness({
			allowErrors: true,
			beforeRegister: (_, count) => {
				if (count === 2) throw new Error(message);
			}
		});
		await h.store.enablePushFromUserAction();
		const binding = h.state.binding;
		assert.equal(await h.store.syncExistingPushSubscription(), false);
		assert.equal(h.state.binding, binding);
		assert.equal(h.calls.unsubscribe, 0);
		assert.equal(await h.store.syncExistingPushSubscription(), true);
		h.config.available = false;
		assert.equal(await h.store.syncExistingPushSubscription(), false);
		assert.notEqual(h.state.binding, null);
		assert.equal(h.calls.unsubscribe, 0);
	}
});
