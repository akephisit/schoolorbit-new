import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import ts from 'typescript';
import { writable, get } from 'svelte/store';
import * as helpers from '../../src/lib/tools/file-transfer.ts';
import { recoveryDelay } from '../../src/lib/tools/file-transfer-recovery.ts';
import {
	createVisibilityIdle,
	browserVisibilityDependencies
} from '../../src/lib/realtime/visibility-idle.ts';

const flush = async () => {
	for (let i = 0; i < 35; i++) await Promise.resolve();
};
async function harness({ loading = false } = {}) {
	const original = new Map(
		[
			'window',
			'document',
			'navigator',
			'EventSource',
			'RTCPeerConnection',
			'setTimeout',
			'clearTimeout'
		].map((key) => [key, Object.getOwnPropertyDescriptor(globalThis, key)])
	);
	const windowTarget = new EventTarget(),
		documentTarget = new EventTarget();
	documentTarget.hidden = false;
	const timers = new Map();
	let nextTimer = 0;
	const streams = [],
		transports = [],
		devices = new Map();
	let registrations = 0,
		authReads = 0,
		requests = 0,
		epoch = loading ? 0 : 1;
	const state = writable({
		isLoading: loading,
		isAuthenticated: !loading,
		isUnavailable: false,
		user: loading ? null : { id: 'A', user_type: 'staff' }
	});
	const authStore = {
		subscribe: state.subscribe,
		get sessionEpoch() {
			return epoch;
		}
	};
	class ApiClientError extends Error {
		constructor(status) {
			super('fixture');
			this.status = status;
		}
	}
	let failRegister = false,
		authResult = 'authenticated',
		authGate = null,
		acceptGate = null,
		configureGate = null,
		registerGate = null;
	class Source extends EventTarget {
		constructor() {
			super();
			streams.push(this);
		}
		close() {
			this.closed = true;
			if (this.id) devices.delete(this.id);
		}
		ready() {
			this.id = [...devices.keys()].at(-1);
			this.dispatchEvent(
				new MessageEvent('file_transfer', {
					data: JSON.stringify({ type: 'ready', data: devices.get(this.id) })
				})
			);
		}
		emit(event) {
			this.dispatchEvent(new MessageEvent('file_transfer', { data: JSON.stringify(event) }));
		}
	}
	class Store {
		static async open(batch) {
			return new Store(batch);
		}
		constructor(batch) {
			this.batch = batch;
			this.completedCount = 0;
		}
		async dispose() {
			this.disposed = true;
		}
		async configureDestination(destination) {
			await configureGate;
			this.destination = destination;
			this.destinationLabel = 'fixture';
		}
	}
	class Transport {
		constructor(pair, files, store, ports) {
			Object.assign(this, { pair, store, ports });
			transports.push(this);
		}
		offer() {}
		dispose() {
			this.stopped = true;
		}
		signal() {}
		acceptFiles() {
			this.receivedReady = (this.receivedReady ?? 0) + 1;
		}
		manifest() {
			this.ports.progress({
				phase: 'review',
				bytes: 0,
				fileIndex: 0,
				fileBytes: 0,
				completedFiles: 0,
				megabytesPerSecond: 0
			});
			this.ports.manifest();
		}
	}
	const transferAPI = {
		async register(label) {
			const attempt = ++registrations;
			await registerGate;
			if (failRegister) throw new Error('unknown register outcome');
			const id = `device-${attempt}`,
				snapshot = {
					protocolVersion: 3,
					device: { id, label },
					pairingCode: '012345ABCD',
					limits: { maxOutgoingTransfers: 6 },
					peers: [],
					pairs: []
				};
			devices.set(id, snapshot);
			return snapshot;
		},
		async snapshot(id) {
			if (!devices.has(id)) throw new ApiClientError(404);
			return devices.get(id);
		},
		async remove(id) {
			devices.delete(id);
		},
		eventsUrl: () => '/events',
		request() {
			requests++;
			return new Promise(() => {});
		},
		async accept(id) {
			await acceptGate;
			return pair(id);
		},
		async close() {},
		async signal() {}
	};
	const authAPI = {
		async refreshCurrentUser() {
			authReads++;
			await authGate;
			if (authResult === 'unavailable' || authResult === 'authenticated')
				state.update((value) => ({ ...value, isUnavailable: authResult === 'unavailable' }));
			return authResult;
		}
	};
	Object.defineProperty(globalThis, 'window', { configurable: true, value: windowTarget });
	Object.defineProperty(globalThis, 'document', { configurable: true, value: documentTarget });
	Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { onLine: true } });
	Object.defineProperty(globalThis, 'EventSource', { configurable: true, value: Source });
	Object.defineProperty(globalThis, 'RTCPeerConnection', { configurable: true, value: class {} });
	globalThis.setTimeout = (callback, delay) => {
		const id = ++nextTimer;
		timers.set(id, { callback, delay });
		return id;
	};
	globalThis.clearTimeout = (id) => timers.delete(id);
	const dependencies = {
		'#lib/api/auth.js': { authAPI },
		'./file-transfer-recovery.js': { recoveryDelay },
		'./file-transfer-destination.js': {
			directoryPickerSupported: () => typeof windowTarget.showDirectoryPicker === 'function',
			pickReceiveDirectory: () => windowTarget.showDirectoryPicker(),
			destinationSelectionMessage: () => 'ยกเลิกการเลือกโฟลเดอร์'
		},
		'./file-transfer-download-pump.js': { AutomaticDownloadPump: class {} },
		'#lib/api/client.js': { ApiClientError },
		'svelte/store': { get },
		'#lib/stores/auth.js': { authStore },
		'#lib/realtime/visibility-idle.js': { createVisibilityIdle, browserVisibilityDependencies },
		'#lib/api/file-transfer.js': { transferAPI, parseTransferEvent: JSON.parse },
		'./file-transfer.js': helpers,
		'./file-transfer-storage.js': {
			ReceivedBatchStore: Store,
			receivingSupported: () => true,
			RECEIVING_UNSUPPORTED: 'unsupported',
			storageMessage: (error) => error.message,
			collectAbandonedTransferStorage: async () => {},
			checkStorageSpace: async () => null
		},
		'./file-transfer-archive.js': { createTransferArchive: async () => {} },
		'./file-transfer-transport.js': { FileTransferTransport: Transport }
	};
	const source = await readFile(
		new URL('../../src/lib/tools/file-transfer-session.ts', import.meta.url),
		'utf8'
	);
	const compiled = ts.transpileModule(source, {
			compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 }
		}).outputText,
		exports = {};
	new Function('require', 'exports', compiled)((id) => {
		assert.ok(id in dependencies, id);
		return dependencies[id];
	}, exports);
	let view;
	const session = new exports.FileTransferSession((value) => {
		view = value;
	});
	function pair(id = 'pair-1') {
		const device = devices.get([...devices.keys()].at(-1)).device;
		return {
			id,
			clientRequestId: 'request-1',
			batch: { fileCount: 1, totalBytes: 3 },
			initiator: { id: 'sender', label: 'Sender', displayName: 'Sender' },
			recipient: device,
			status: 'pending'
		};
	}
	return {
		session,
		streams,
		transports,
		devices,
		get view() {
			return view;
		},
		get registrations() {
			return registrations;
		},
		get authReads() {
			return authReads;
		},
		get requests() {
			return requests;
		},
		async ready() {
			await session.start();
			streams.at(-1).ready();
			await flush();
		},
		async hidden(value, expire = false) {
			documentTarget.hidden = value;
			documentTarget.dispatchEvent(new Event('visibilitychange'));
			if (expire) {
				for (const [id, timer] of timers)
					if (timer.delay === 60000) {
						timers.delete(id);
						timer.callback();
						break;
					}
			}
			await flush();
		},
		async wake() {
			windowTarget.dispatchEvent(new Event('focus'));
			await flush();
		},
		async tick() {
			const found = [...timers].find(([, timer]) => timer.delay < 20000);
			if (found) {
				timers.delete(found[0]);
				found[1].callback();
			}
			await flush();
		},
		setUser(id) {
			epoch++;
			state.set({
				isLoading: false,
				isAuthenticated: !!id,
				isUnavailable: false,
				user: id ? { id, user_type: 'staff' } : null
			});
		},
		async deadline() {
			const found = [...timers].find(([, timer]) => timer.delay === 30000);
			assert.ok(found, 'registration deadline is armed');
			timers.delete(found[0]);
			found[1].callback();
			await flush();
		},
		setUnavailable() {
			state.update((value) => ({ ...value, isUnavailable: true }));
		},
		set registerGate(value) {
			registerGate = value;
		},
		set failRegister(value) {
			failRegister = value;
		},
		set authResult(value) {
			authResult = value;
		},
		set authGate(value) {
			authGate = value;
		},
		set acceptGate(value) {
			acceptGate = value;
		},
		set configureGate(value) {
			configureGate = value;
		},
		incoming() {
			const value = pair();
			devices.get(value.recipient.id).pairs = [value];
			streams.at(-1).emit({ type: 'pair_changed', data: value });
			return view.jobs.at(-1).id;
		},
		picker(callback) {
			windowTarget.showDirectoryPicker = callback;
		},
		async restore() {
			session.dispose();
			await flush();
			for (const [key, descriptor] of original)
				if (descriptor) Object.defineProperty(globalThis, key, descriptor);
				else delete globalThis[key];
		}
	};
}

test('mounted visibility owner resumes a fresh device after sixty seconds and never replays source jobs', async () => {
	const h = await harness();
	try {
		await h.ready();
		const file = new File(['abc'], 'source.txt');
		h.session.send([file], [{ type: 'device', deviceId: 'receiver' }]);
		assert.equal(h.requests, 1);
		await h.hidden(true);
		await h.hidden(false);
		assert.equal(h.registrations, 1);
		await h.hidden(true, true);
		assert.equal(h.view.status, 'paused');
		assert.equal(h.view.sourceFiles[0].name, 'source.txt');
		await h.hidden(false);
		assert.equal(h.authReads, 1);
		assert.equal(h.registrations, 2);
		h.streams.at(-1).ready();
		await flush();
		assert.equal(h.view.status, 'online');
		assert.equal(h.requests, 1);
		assert.equal(h.view.jobs[0].phase, 'cancelled');
		h.session.send([file], [{ type: 'device', deviceId: 'receiver' }]);
		assert.equal(h.requests, 2);
		assert.notEqual(h.view.jobs[0].clientRequestId, h.view.jobs[1].clientRequestId);
	} finally {
		await h.restore();
	}
});
test('ambiguous registration has no automatic POST retry on focus or timers', async () => {
	const h = await harness();
	try {
		h.failRegister = true;
		await h.session.start();
		assert.equal(h.registrations, 1);
		for (let i = 0; i < 6; i++) {
			await h.wake();
			await h.tick();
		}
		assert.equal(h.registrations, 1);
		assert.equal(h.view.status, 'offline');
		h.failRegister = false;
		await h.session.start();
		assert.equal(h.registrations, 2);
	} finally {
		await h.restore();
	}
});
test('logout then another account cannot resurrect the old mounted feature, while initial auth settling can start once', async () => {
	const h = await harness();
	try {
		await h.ready();
		h.setUser(null);
		h.setUser('B');
		await h.wake();
		await h.tick();
		assert.equal(h.registrations, 1);
		assert.ok(h.view.draftEpoch > 0);
	} finally {
		await h.restore();
	}
	const initial = await harness({ loading: true });
	try {
		await initial.session.start();
		assert.equal(initial.registrations, 0);
		initial.setUser('A');
		await flush();
		assert.equal(initial.registrations, 1);
	} finally {
		await initial.restore();
	}
});
test('dispose fences pending auth reconciliation before fresh device creation', async () => {
	const h = await harness();
	try {
		await h.ready();
		let release;
		h.authGate = new Promise((resolve) => {
			release = resolve;
		});
		h.streams.at(-1).onerror();
		await h.tick();
		h.session.dispose();
		release();
		await flush();
		await h.wake();
		assert.equal(h.registrations, 1);
	} finally {
		await h.restore();
	}
});
test('one-click intent starts picker synchronously and manifest progress outruns held accept HTTP', async () => {
	const h = await harness();
	try {
		await h.ready();
		let pickerCalls = 0,
			releaseAccept,
			releaseConfigure;
		h.picker(() => {
			pickerCalls++;
			return Promise.resolve({ name: 'target' });
		});
		const id = h.incoming();
		h.session.setReceiveMode(id, 'folder');
		h.acceptGate = new Promise((resolve) => {
			releaseAccept = resolve;
		});
		h.configureGate = new Promise((resolve) => {
			releaseConfigure = resolve;
		});
		const receiving = h.session.receive(id);
		assert.equal(pickerCalls, 1);
		await flush();
		const transport = h.transports.at(-1);
		transport.manifest();
		await flush();
		assert.equal(h.view.jobs[0].pending, true);
		releaseAccept();
		await receiving;
		assert.equal(h.view.jobs[0].pending, true);
		releaseConfigure();
		await flush();
		assert.equal(transport.receivedReady, 1);
		assert.equal(h.view.jobs[0].pending, false);
	} finally {
		await h.restore();
	}
});
test('safe recovery backoff is bounded and jitter stays within its declared interval', () => {
	assert.deepEqual(
		[0, 1, 2, 3, 4].map((i) => recoveryDelay(i, 0)),
		[400, 1200, 3200, 8000, null]
	);
	assert.deepEqual(
		[0, 1, 2, 3].map((i) => recoveryDelay(i, 1)),
		[600, 1800, 4800, 12000]
	);
});

test('terminal stream invalidation resets the draft before an unavailable central refresh changes identity', async () => {
	const h = await harness();
	try {
		await h.ready();
		h.session.send([new File(['abc'], 'private.txt')], [{ type: 'device', deviceId: 'receiver' }]);
		const marker = h.view.draftEpoch;
		h.authResult = 'unavailable';
		h.streams.at(-1).emit({ type: 'session_invalid', data: {} });
		await flush();
		assert.ok(h.view.draftEpoch > marker);
		assert.equal(h.view.sourceFiles.length, 0);
		assert.equal(h.view.jobs.length, 0);
		await h.wake();
		await h.tick();
		assert.equal(h.registrations, 1);
	} finally {
		await h.restore();
	}
});
test('same-identity auth unavailability retains the source draft and bounds safe reconciliation without registration loops', async () => {
	const h = await harness();
	try {
		await h.ready();
		h.session.send([new File(['abc'], 'draft.txt')], [{ type: 'device', deviceId: 'receiver' }]);
		const marker = h.view.draftEpoch;
		h.authResult = 'unavailable';
		h.streams.at(-1).emit({ type: 'session_unavailable', data: {} });
		for (let i = 0; i < 6; i++) await h.tick();
		assert.equal(h.registrations, 1);
		assert.equal(h.authReads, 4);
		assert.equal(h.view.status, 'offline');
		assert.equal(h.view.draftEpoch, marker);
		assert.equal(h.view.sourceFiles[0].name, 'draft.txt');
		assert.equal(h.view.authBlocked, true);
		h.authResult = 'authenticated';
		await h.wake();
		assert.equal(h.registrations, 1);
		await h.session.start();
		assert.equal(h.registrations, 2);
		assert.equal(h.requests, 1);
	} finally {
		await h.restore();
	}
});

test('renaming a stopped device validates its preferred label without rearming automatic readiness', async () => {
	const h = await harness();
	try {
		await h.ready();
		h.session.stop();
		await h.session.rename('ชื่อใหม่');
		await h.wake();
		assert.equal(h.registrations, 1);
		await h.session.start('ชื่อใหม่');
		assert.equal(h.registrations, 2);
		assert.equal([...h.devices.values()].at(-1).device.label, 'ชื่อใหม่');
		h.streams.at(-1).ready();
		await h.session.rename('');
		assert.equal(h.registrations, 2);
		assert.equal(h.view.status, 'online');
	} finally {
		await h.restore();
	}
});

test('a held registration reaches its deadline, releases single-flight and cleans a late known result without automatic POST replay', async () => {
	const h = await harness();
	let release;
	try {
		h.registerGate = new Promise((resolve) => {
			release = resolve;
		});
		const starting = h.session.start();
		await flush();
		assert.equal(h.registrations, 1);
		await h.deadline();
		await starting;
		assert.equal(h.view.status, 'offline');
		await h.wake();
		assert.equal(h.registrations, 1);
		h.registerGate = null;
		await h.session.start();
		assert.equal(h.registrations, 2);
		h.streams.at(-1).ready();
		release();
		await flush();
		assert.equal(h.devices.has('device-1'), false);
		assert.equal(h.view.snapshot.device.id, 'device-2');
	} finally {
		release?.();
		await h.restore();
	}
});
test('auth unavailability aborts a held registration and defers known-device cleanup until central reconciliation', async () => {
	const h = await harness();
	let release;
	try {
		h.registerGate = new Promise((resolve) => {
			release = resolve;
		});
		const starting = h.session.start();
		await flush();
		h.setUnavailable();
		await starting;
		release();
		await flush();
		assert.equal(h.streams.length, 0);
		assert.equal(h.view.authBlocked, true);
		assert.equal(h.devices.has('device-1'), true);
		await h.wake();
		assert.equal(h.registrations, 1);
		h.registerGate = null;
		await h.session.start();
		assert.equal(h.devices.has('device-1'), false);
		assert.equal(h.registrations, 2);
	} finally {
		release?.();
		await h.restore();
	}
});
test('dispose settles a hung registration immediately and a late success creates no stream', async () => {
	const h = await harness();
	let release;
	try {
		h.registerGate = new Promise((resolve) => {
			release = resolve;
		});
		const starting = h.session.start();
		await flush();
		h.session.dispose();
		await starting;
		assert.equal(h.registrations, 1);
		release();
		await flush();
		assert.equal(h.streams.length, 0);
		assert.equal(h.devices.size, 0);
	} finally {
		release?.();
		await h.restore();
	}
});

test('late registration cleanup after disposal cannot borrow a replacement account authority', async () => {
	const h = await harness();
	let release;
	try {
		h.registerGate = new Promise((resolve) => {
			release = resolve;
		});
		const starting = h.session.start();
		await flush();
		h.session.dispose();
		await starting;
		h.setUser('B');
		release();
		await flush();
		assert.equal(h.streams.length, 0);
		assert.equal(h.devices.has('device-1'), true);
	} finally {
		release?.();
		await h.restore();
	}
});
