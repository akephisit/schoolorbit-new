import assert from 'node:assert/strict';
import test from 'node:test';
import { deviceHarness, deferred, flush } from '../helpers/notification-device-harness.mjs';

test('existing subscription sync uses runtime key and persists server binding without requesting permission', async () => {
	const h = deviceHarness();
	assert.equal(await h.store.syncExistingPushSubscription(), true);
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
	await h.store.syncExistingPushSubscription();
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
		const registration = h.store.syncExistingPushSubscription();
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
	const a = h.store.syncExistingPushSubscription();
	await flush();
	const b = h.store.syncExistingPushSubscription();
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
	await h.store.syncExistingPushSubscription();
	await h.store.revokePushDevice({ id: 'other', version: 'other-version' });
	assert.equal(h.state.binding.id, 'device-1');
	assert.equal(h.calls.unsubscribe, 0);
	assert.deepEqual(h.calls.revoke, [{ id: 'other', version: 'other-version' }]);
});
