/** Shared by the inbox owner and push worker; no application requests are cached. */
export interface DeviceBinding {
	identity: string;
	intent: string;
	id: string;
	version: string;
	binding_token: string;
	installation_id: string;
	expires_at: string;
}
interface BrowserState {
	installation_id: string;
	identity: string | null;
	intent: string;
	binding: DeviceBinding | null;
}
const DATABASE = 'schoolorbit-notifications';
const IDENTITY_CHANNEL = 'schoolorbit-notification-identity';
function publishIdentityChange() {
	if (typeof BroadcastChannel === 'undefined') return;
	const channel = new BroadcastChannel(IDENTITY_CHANNEL);
	channel.postMessage('changed');
	channel.close();
}
export function watchIdentityChanges(listener: () => void): () => void {
	if (typeof BroadcastChannel === 'undefined') return () => {};
	const channel = new BroadcastChannel(IDENTITY_CHANNEL);
	channel.onmessage = listener;
	return () => channel.close();
}
const MAX_CLAIMS = 2048;
const CLAIM_TTL = 24 * 60 * 60 * 1000;
let database: Promise<IDBDatabase> | null = null;
function openDatabase(): Promise<IDBDatabase> {
	if (!database) {
		database = new Promise<IDBDatabase>((resolve, reject) => {
			const request = indexedDB.open(DATABASE, 1);
			request.onupgradeneeded = () => {
				request.result.createObjectStore('state');
				request.result.createObjectStore('claims');
			};
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		}).catch((error) => {
			database = null;
			throw error;
		});
	}
	return database;
}
async function stateTransaction<T>(change: (state: BrowserState) => T): Promise<T> {
	const db = await openDatabase();
	return new Promise((resolve, reject) => {
		const transaction = db.transaction('state', 'readwrite');
		const store = transaction.objectStore('state');
		const request = store.get('current');
		let result: T;
		request.onsuccess = () => {
			const state: BrowserState = request.result ?? {
				installation_id: crypto.randomUUID(),
				identity: null,
				intent: '',
				binding: null
			};
			result = change(state);
			store.put(state, 'current');
		};
		transaction.oncomplete = () => resolve(result);
		transaction.onerror = () => reject(transaction.error);
		transaction.onabort = () => reject(transaction.error);
	});
}
export function readBrowserState() {
	return stateTransaction((state) => ({ ...state }));
}
export async function beginBinding(identity: string | null, intent: string) {
	const result = await stateTransaction((state) => {
		const changed = state.identity !== identity;
		if (changed) state.binding = null;
		state.identity = identity;
		state.intent = intent;
		return { state: { ...state }, changed };
	});
	if (result.changed) publishIdentityChange();
	return result.state;
}
/** Reserve a refresh only while the exact prior binding and intent still own this installation. */
export function beginBindingRefresh(
	binding: DeviceBinding,
	previousIntent: string,
	intent: string
) {
	return stateTransaction((state) => {
		if (
			state.identity !== binding.identity ||
			state.intent !== previousIntent ||
			state.binding?.id !== binding.id ||
			state.binding.version !== binding.version ||
			state.binding.binding_token !== binding.binding_token
		)
			return null;
		state.intent = intent;
		return { ...state };
	});
}
/** Forget a rejected push registration without changing the authenticated inbox identity. */
export function forgetBinding(binding: DeviceBinding, intent: string) {
	return stateTransaction((state) => {
		if (
			state.identity !== binding.identity ||
			state.intent !== intent ||
			state.binding?.id !== binding.id ||
			state.binding.version !== binding.version ||
			state.binding.binding_token !== binding.binding_token
		)
			return null;
		state.binding = null;
		state.intent = crypto.randomUUID();
		return state.intent;
	});
}
export function saveBinding(binding: DeviceBinding) {
	return stateTransaction((state) => {
		if (state.intent !== binding.intent || state.identity !== binding.identity) return false;
		state.binding = binding;
		return true;
	});
}
export async function clearBinding(identity: string, intent?: string) {
	const result = await stateTransaction((state) => {
		if (state.identity !== identity || (intent && state.intent !== intent))
			return { binding: null, changed: false };
		const binding = state.binding;
		state.binding = null;
		state.identity = null;
		state.intent = crypto.randomUUID();
		return { binding, changed: true };
	});
	if (result.changed) publishIdentityChange();
	return result.binding;
}
export function bindingMatches(
	binding: DeviceBinding | null,
	payload: {
		device_id: string;
		device_version: string;
		binding_token: string;
		expires_at: string;
	},
	now = Date.now()
) {
	return Boolean(
		binding &&
		binding.id === payload.device_id &&
		binding.version === payload.device_version &&
		binding.binding_token === payload.binding_token &&
		Date.parse(binding.expires_at) > now &&
		Date.parse(payload.expires_at) > now
	);
}
/** Atomic across tabs and the worker. Failure to coordinate means presentation is suppressed. */
export async function claimPresentation(id: string, expiresAt: number): Promise<boolean> {
	if (!id || !Number.isFinite(expiresAt) || expiresAt <= Date.now()) return false;
	const db = await openDatabase();
	return new Promise((resolve, reject) => {
		const transaction = db.transaction('claims', 'readwrite');
		const store = transaction.objectStore('claims');
		const request = store.getAllKeys();
		let claimed = false;
		request.onsuccess = () => {
			const keys = request.result;
			const values = store.getAll();
			values.onsuccess = () => {
				const now = Date.now();
				const retained = keys.filter((key, index) => {
					if (values.result[index] <= now) {
						store.delete(key);
						return false;
					}
					return true;
				});
				if (retained.includes(id)) return;
				while (retained.length >= MAX_CLAIMS) store.delete(retained.shift()!);
				store.put(Math.min(expiresAt, now + CLAIM_TTL), id);
				claimed = true;
			};
		};
		transaction.oncomplete = () => resolve(claimed);
		transaction.onerror = () => reject(transaction.error);
		transaction.onabort = () => reject(transaction.error);
	});
}
