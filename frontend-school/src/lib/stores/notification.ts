import {
	createVisibilityIdle,
	browserVisibilityDependencies
} from '#lib/realtime/visibility-idle.js';
import { getServiceWorkerRegistration } from '#lib/pwa/service-worker-registration.js';
import {
	beginBinding,
	clearBinding,
	readBrowserState,
	saveBinding,
	bindingMatches,
	claimPresentation,
	watchIdentityChanges,
	type DeviceBinding
} from '#lib/notifications/browser-state.js';
import {
	getPushConfig,
	listPushDevices,
	registerPushDevice,
	unregisterPushDevice,
	type PushConfig,
	type PushDevice
} from '#lib/api/notifications.js';
import { authStore } from '#lib/stores/auth.js';
import { apiClient, BACKEND_URL, getSchoolSubdomainHint } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';
import { realtimeAuthRecovery } from '#lib/realtime/auth-recovery.js';
import { workStore } from '#lib/stores/work.js';
import { toast } from 'svelte-sonner';
import { get, writable } from 'svelte/store';

// Helper for VAPID key conversion
function urlBase64ToUint8Array(base64String: string) {
	const padding = '='.repeat((4 - (base64String.length % 4)) % 4);
	const base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
	const rawData = window.atob(base64);
	const outputArray = new Uint8Array(rawData.length);
	for (let i = 0; i < rawData.length; ++i) {
		outputArray[i] = rawData.charCodeAt(i);
	}
	return outputArray;
}

function arrayBufferToUrlSafeBase64(buffer: ArrayBuffer): string {
	let binary = '';
	const bytes = new Uint8Array(buffer);
	for (let i = 0; i < bytes.byteLength; i++) {
		binary += String.fromCharCode(bytes[i]);
	}
	return window.btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function isIOSDevice() {
	const navigatorWithPlatform = navigator as Navigator & { platform?: string };
	return (
		/iPad|iPhone|iPod/.test(navigator.userAgent) ||
		(navigatorWithPlatform.platform === 'MacIntel' && navigator.maxTouchPoints > 1)
	);
}

function isStandalonePWA() {
	const standaloneNavigator = navigator as Navigator & { standalone?: boolean };
	return (
		window.matchMedia('(display-mode: standalone)').matches ||
		standaloneNavigator.standalone === true
	);
}

function isPushMessagingSupported() {
	return 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window;
}

async function getPushRegistration() {
	return getServiceWorkerRegistration();
}

function subscriptionPayload(subscription: PushSubscription) {
	const p256dh = subscription.getKey('p256dh');
	const auth = subscription.getKey('auth');

	if (!p256dh || !auth) return null;

	return {
		endpoint: subscription.endpoint,
		p256dh: arrayBufferToUrlSafeBase64(p256dh),
		auth: arrayBufferToUrlSafeBase64(auth)
	};
}

type Schemas = components['schemas'];

export type Notification = Schemas['Notification'];
type ListNotificationsResponse = Schemas['ListNotificationsResponse'];

export interface NotificationState {
	notifications: Notification[];
	unreadCount: number;
	loading: boolean;
}

export interface PushNotificationDeviceStatus {
	supported: boolean;
	permission: NotificationPermission | 'unsupported';
	hasSubscription: boolean;
	isIOS: boolean;
	isStandalone: boolean;
	serverAvailable: boolean;
	synced: boolean;
	currentDeviceId: string | null;
	devices: PushDevice[];
	config: PushConfig | null;
	error: string | null;
}

let eventSource: EventSource | null = null;
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
const INITIAL_SSE_RECONNECT_DELAY_MS = 3000;
const MAX_SSE_RECONNECT_DELAY_MS = 60000;
let reconnectDelay = INITIAL_SSE_RECONNECT_DELAY_MS;
let sseGeneration = 0;
let shouldMaintainSSE = false;
let recoveryInFlight: { generation: number; promise: Promise<void> } | null = null;

function createNotificationStore() {
	const { subscribe, set, update } = writable<NotificationState>({
		notifications: [],
		unreadCount: 0,
		loading: false
	});

	let identity: string | null = null;
	let stopIdentityListener: (() => void) | null = null;
	let identityReady: Promise<unknown> = Promise.resolve();
	let bindingIntent = '';
	let pushGeneration = 0;
	function currentAuthIdentity(): string | null {
		const auth = get(authStore);
		return auth.isAuthenticated && auth.user
			? `${getSchoolSubdomainHint() ?? ''}:${auth.user.id}:${auth.user.user_type ?? ''}:${authStore.sessionEpoch}`
			: null;
	}
	function deviceIdentity(): string | null {
		const auth = get(authStore);
		return auth.isAuthenticated && auth.user
			? `${getSchoolSubdomainHint() ?? ''}:${auth.user.id}:${auth.user.user_type ?? ''}`
			: null;
	}
	function bindIdentity() {
		const next = currentAuthIdentity();
		if (next === identity) return;
		const previous = identity;
		closeSSE();
		pushGeneration++;
		identity = next;
		bindingIntent = crypto.randomUUID();
		identityReady = beginBinding(deviceIdentity(), bindingIntent)
			.then(async () => {
				if (previous && previous !== next) await closeDeviceNotifications();
			})
			.catch((error) => console.error('Failed to clear notification device identity', error));
	}
	async function closeDeviceNotifications() {
		if (typeof navigator === 'undefined' || !('serviceWorker' in navigator)) return;
		try {
			const registration = await getPushRegistration();
			for (const notification of await registration.getNotifications()) notification.close();
		} catch (error) {
			console.error('Failed to close device notifications', error);
		}
	}
	const foregroundPush = (event: MessageEvent) => {
		if (event.data?.type !== 'schoolorbit:push' || !event.ports[0]) return;
		const port = event.ports[0];
		const generation = pushGeneration;
		void (async () => {
			let handled = false;
			try {
				const state = await readBrowserState();
				if (
					identity &&
					state.identity === deviceIdentity() &&
					bindingMatches(state.binding, event.data.payload) &&
					!document.hidden &&
					document.hasFocus()
				) {
					handled = await claimPresentation(
						`${deviceIdentity()}:${event.data.payload.id}`,
						Date.parse(event.data.payload.expires_at)
					);
					if (handled && generation === pushGeneration && !document.hidden && document.hasFocus()) {
						toast.success('SchoolOrbit', {
							description: 'มีการแจ้งเตือนใหม่ กรุณาเปิดดูในกล่องแจ้งเตือน'
						});
						void notificationStore.fetchNotifications();
					}
				}
			} catch (error) {
				console.error('Failed to handle foreground push', error);
			}
			port.postMessage({ handled });
			port.close();
		})();
	};
	const seen = new Map<string, string | null>();
	const seenCreated = new Map<string, number>();
	let snapshotWatermark = 0;
	let evictionWatermark = 0;
	const remember = (notification: Notification) => {
		seen.set(notification.id, notification.read_at ?? seen.get(notification.id) ?? null);
		seenCreated.set(notification.id, Date.parse(notification.created_at));
		while (seen.size > 2048) {
			const oldest = seen.keys().next().value!;
			const created = seenCreated.get(oldest);
			if (created && Number.isFinite(created))
				evictionWatermark = Math.max(evictionWatermark, created);
			seen.delete(oldest);
			seenCreated.delete(oldest);
		}
	};
	let needsReconcile = false;
	let dataGeneration = 0;
	let signalVersion = 0;
	let reconcileInFlight: EventSource | null = null;
	const idle = createVisibilityIdle({
		...browserVisibilityDependencies(),
		onPause: () => {
			sseGeneration++;
			markReconcileRequired();
			clearReconnectTimer();
			closeEventSource();
		},
		onResume: () => {
			void recoverAfterSessionSignal(sseGeneration);
		}
	});

	function markReconcileRequired() {
		needsReconcile = true;
		// A gap invalidates even an HTTP snapshot already in flight on this source.
		dataGeneration++;
		signalVersion++;
	}

	function clearReconnectTimer() {
		if (!reconnectTimeout) return;
		clearTimeout(reconnectTimeout);
		reconnectTimeout = null;
	}

	function closeEventSource() {
		const source = eventSource;
		eventSource = null;
		if (source) source.close();
	}

	function ownsEventSource(source: EventSource, generation: number): boolean {
		return (
			shouldMaintainSSE && !idle.paused && eventSource === source && sseGeneration === generation
		);
	}

	function notificationStreamUrl(): string {
		const url = new URL('/api/notifications/stream', BACKEND_URL);
		const schoolSubdomain = getSchoolSubdomainHint();
		if (schoolSubdomain) url.searchParams.set('school_subdomain', schoolSubdomain);
		return url.toString();
	}

	function scheduleSseTask(generation: number, callback: () => void) {
		if (
			!shouldMaintainSSE ||
			idle.paused ||
			generation !== sseGeneration ||
			reconnectTimeout !== null
		) {
			return;
		}

		const delay = reconnectDelay;
		reconnectTimeout = setTimeout(() => {
			reconnectTimeout = null;
			if (!shouldMaintainSSE || idle.paused || generation !== sseGeneration) return;
			reconnectDelay = Math.min(reconnectDelay * 2, MAX_SSE_RECONNECT_DELAY_MS);
			callback();
		}, delay);
	}

	function scheduleSseReconnect(generation: number) {
		scheduleSseTask(generation, () => openSSE(generation));
	}

	function scheduleAuthRecovery(generation: number) {
		scheduleSseTask(generation, () => {
			void recoverAfterSessionSignal(generation);
		});
	}

	async function recoverAfterSessionSignal(generation = sseGeneration): Promise<void> {
		if (!shouldMaintainSSE || idle.paused || generation !== sseGeneration) return;
		if (recoveryInFlight?.generation === generation) return recoveryInFlight.promise;

		markReconcileRequired();
		clearReconnectTimer();
		closeEventSource();

		const promise = (async () => {
			try {
				const { authAPI } = await import('#lib/api/auth.js');
				if (!shouldMaintainSSE || idle.paused || generation !== sseGeneration) return;
				const recoveryAction = await realtimeAuthRecovery(() =>
					authAPI.refreshCurrentUser({ silent: true })
				);
				if (!shouldMaintainSSE || idle.paused || generation !== sseGeneration) return;

				if (recoveryAction === 'reconnect') {
					scheduleSseReconnect(generation);
				} else if (recoveryAction === 'retry') {
					scheduleAuthRecovery(generation);
				} else if (recoveryAction === 'stop') {
					clearReconnectTimer();
					closeSSE();
				}
			} catch (error) {
				console.error('Failed to recover notification stream authentication', error);
				if (shouldMaintainSSE && generation === sseGeneration) {
					scheduleAuthRecovery(generation);
				}
			}
		})();
		const recovery = { generation, promise };
		recoveryInFlight = recovery;
		try {
			await promise;
		} finally {
			if (recoveryInFlight === recovery) recoveryInFlight = null;
		}
	}

	async function reconcileAfterOpen(source: EventSource, generation: number) {
		if (
			!ownsEventSource(source, generation) ||
			source.readyState !== 1 ||
			!needsReconcile ||
			reconcileInFlight === source ||
			reconnectTimeout !== null
		)
			return;
		reconcileInFlight = source;
		const version = signalVersion;
		const ownsSnapshot = () =>
			ownsEventSource(source, generation) && source.readyState === 1 && version === signalVersion;
		try {
			const { authAPI } = await import('#lib/api/auth.js');
			if (!ownsEventSource(source, generation)) return;
			const action = await realtimeAuthRecovery(() =>
				authAPI.refreshCurrentUser({ silent: true, invalidate: true })
			);
			if (!ownsEventSource(source, generation)) return;
			if (action === 'stop') {
				closeSSE();
				return;
			}
			if (action === 'reconnect' && ownsSnapshot()) {
				const results = await Promise.all([
					notificationStore.fetchNotifications(10, ownsSnapshot),
					workStore.refreshSilently({ isCurrent: ownsSnapshot, windowsChanged: true })
				]);
				if (ownsSnapshot() && results.every(Boolean)) {
					needsReconcile = false;
					reconnectDelay = INITIAL_SSE_RECONNECT_DELAY_MS;
				}
			}
		} catch (error) {
			console.error('Failed to reconcile notification stream', error);
		} finally {
			if (reconcileInFlight === source) reconcileInFlight = null;
			if (ownsEventSource(source, generation) && source.readyState === 1 && needsReconcile) {
				scheduleSseTask(generation, () => {
					void reconcileAfterOpen(source, generation);
				});
			}
		}
	}

	function openSSE(generation: number) {
		if (
			typeof EventSource === 'undefined' ||
			!shouldMaintainSSE ||
			idle.paused ||
			generation !== sseGeneration
		) {
			return;
		}
		if (eventSource && (eventSource.readyState === 1 || eventSource.readyState === 0)) return;

		clearReconnectTimer();
		closeEventSource();
		const source = new EventSource(notificationStreamUrl(), { withCredentials: true });
		eventSource = source;

		source.onopen = () => {
			if (!ownsEventSource(source, generation)) return;
			console.log('✅ SSE Connected');
			reconnectDelay = INITIAL_SSE_RECONNECT_DELAY_MS;
			void reconcileAfterOpen(source, generation);
		};

		source.onmessage = (event) => {
			if (!ownsEventSource(source, generation)) return;
			try {
				const newNotif: Notification = JSON.parse(event.data);
				signalVersion++;

				const known = seen.has(newNotif.id);
				const readAt = newNotif.read_at ?? seen.get(newNotif.id) ?? null;
				const created = Date.parse(newNotif.created_at);
				const replay =
					Number.isFinite(created) && created <= Math.max(snapshotWatermark, evictionWatermark);
				remember({ ...newNotif, read_at: readAt });
				if (known) return;
				update((state) => ({
					...state,
					notifications: [{ ...newNotif, read_at: readAt }, ...state.notifications].slice(0, 10),
					unreadCount:
						state.unreadCount +
						(readAt || replay || (Number.isFinite(created) && created + 15 * 60_000 <= Date.now())
							? 0
							: 1)
				}));
				const expiresAt = created + 15 * 60_000;
				if (!readAt && !replay && expiresAt > Date.now()) {
					void presentLive(newNotif, expiresAt, () => ownsEventSource(source, generation));
				}
			} catch (error) {
				console.error('Failed to parse SSE message', error);
			}
		};

		source.addEventListener('permission_changed', async () => {
			if (!ownsEventSource(source, generation)) return;
			signalVersion++;
			try {
				const { authAPI } = await import('#lib/api/auth.js');
				if (!ownsEventSource(source, generation)) return;
				await authAPI.refreshCurrentUser({ silent: true, invalidate: true });
			} catch (error) {
				console.error('Failed to refresh auth context after permission change', error);
			}
		});

		source.addEventListener('work_items_changed', () => {
			if (!ownsEventSource(source, generation)) return;
			signalVersion++;
			void workStore.refreshSilently();
		});

		source.addEventListener('workflow_window_changed', () => {
			if (!ownsEventSource(source, generation)) return;
			signalVersion++;
			void workStore.refreshSilently({ windowsChanged: true });
		});

		source.addEventListener('reconcile_required', () => {
			if (!ownsEventSource(source, generation)) return;
			markReconcileRequired();
			void reconcileAfterOpen(source, generation);
		});

		const recover = () => {
			if (!ownsEventSource(source, generation)) return;
			void recoverAfterSessionSignal(generation);
		};
		source.addEventListener('session_invalid', recover);
		source.addEventListener('session_unavailable', recover);

		source.onerror = () => {
			if (!ownsEventSource(source, generation)) return;
			markReconcileRequired();
			clearReconnectTimer();
			if (source.readyState !== EventSource.CLOSED) {
				console.log('🔄 SSE Reconnecting...');
				return;
			}
			void recoverAfterSessionSignal(generation);
		};
	}

	async function presentLive(
		notification: Notification,
		expiresAt: number,
		isCurrent: () => boolean
	) {
		if (typeof document === 'undefined' || document.hidden || !document.hasFocus()) return;
		try {
			const browserState = await readBrowserState();
			if (browserState.identity !== deviceIdentity()) return;
			if (
				!(await claimPresentation(`${deviceIdentity()}:${notification.id}`, expiresAt)) ||
				!isCurrent() ||
				document.hidden ||
				!document.hasFocus()
			)
				return;
			toast.success(notification.title, {
				description: notification.message,
				duration: 5000,
				action: {
					label: 'ดู',
					onClick: () => {
						if (notification.link) window.location.href = notification.link;
					}
				}
			});
		} catch (error) {
			console.error('Failed to coordinate notification presentation', error);
		}
	}

	function initSSE() {
		if (typeof EventSource === 'undefined') return;
		stopIdentityListener ??= watchIdentityChanges(() => {
			void readBrowserState()
				.then((state) => {
					if (state.identity === deviceIdentity() || !shouldMaintainSSE) return;
					seen.clear();
					seenCreated.clear();
					set({ notifications: [], unreadCount: 0, loading: false });
					void recoverAfterSessionSignal(sseGeneration);
				})
				.catch((error) => console.error('Failed to reconcile notification identity', error));
		});
		if (typeof navigator !== 'undefined' && 'serviceWorker' in navigator)
			navigator.serviceWorker.addEventListener('message', foregroundPush);
		shouldMaintainSSE = true;
		idle.start();
		if (idle.paused || recoveryInFlight?.generation === sseGeneration) return;
		if (eventSource && (eventSource.readyState === 1 || eventSource.readyState === 0)) return;

		clearReconnectTimer();
		sseGeneration += 1;
		openSSE(sseGeneration);
	}

	function closeSSE() {
		dataGeneration++;
		stopIdentityListener?.();
		stopIdentityListener = null;
		if (typeof navigator !== 'undefined' && 'serviceWorker' in navigator)
			navigator.serviceWorker.removeEventListener('message', foregroundPush);
		idle.stop();
		needsReconcile = false;
		shouldMaintainSSE = false;
		sseGeneration += 1;
		clearReconnectTimer();
		closeEventSource();
		reconnectDelay = INITIAL_SSE_RECONNECT_DELAY_MS;
		seen.clear();
		seenCreated.clear();
		snapshotWatermark = 0;
		evictionWatermark = 0;
		set({ notifications: [], unreadCount: 0, loading: false });
	}

	// This singleton follows authoritative identity changes even when the bell is not mounted.
	if (typeof window !== 'undefined') {
		authStore.subscribe((state) => {
			if (state.isAuthenticated || (!state.isLoading && !state.isUnavailable)) {
				if (!state.isAuthenticated && identity === null) {
					bindingIntent = crypto.randomUUID();
					identityReady = beginBinding(null, bindingIntent)
						.then(() => closeDeviceNotifications())
						.catch((error) => console.error('Failed to clear notification identity', error));
				} else bindIdentity();
			}
		});
	}

	return {
		subscribe,
		bindIdentity,

		async fetchNotifications(limit = 10, isCurrent: () => boolean = () => true) {
			const generation = dataGeneration;
			const version = signalVersion;
			const ownsRequest = () =>
				generation === dataGeneration && version === signalVersion && isCurrent();
			update((s) => ({ ...s, loading: true }));
			try {
				const response = await apiClient.get<ListNotificationsResponse>(
					`/api/notifications?limit=${limit}`
				);

				if (!ownsRequest()) return false;
				if (response.success && response.data) {
					const items = response.data.items.map((item) => ({
						...item,
						read_at: item.read_at ?? seen.get(item.id) ?? null
					}));
					for (const item of items) {
						remember(item);
						const created = Date.parse(item.created_at);
						if (Number.isFinite(created)) snapshotWatermark = Math.max(snapshotWatermark, created);
					}
					set({
						notifications: items,
						unreadCount: response.data.unread_count,
						loading: false
					});
					return true;
				}
				update((s) => ({ ...s, loading: false }));
				return false;
			} catch (err) {
				if (!ownsRequest()) return false;
				console.error('Failed to fetch notifications', err);
				update((s) => ({ ...s, loading: false }));
				return false;
			}
		},

		initSSE,
		closeSSE,

		async markAsRead(id: string) {
			const generation = dataGeneration;
			try {
				await apiClient.post<Record<string, never>>(`/api/notifications/${id}/read`);
				if (generation !== dataGeneration) return;
				signalVersion++;
				update((state) => {
					const unread =
						(seen.has(id) && !seen.get(id)) ||
						state.notifications.some((item) => item.id === id && !item.read_at);
					const readAt = new Date().toISOString();
					seen.set(id, readAt);
					return {
						...state,
						notifications: state.notifications.map((item) =>
							item.id === id ? { ...item, read_at: readAt } : item
						),
						unreadCount: Math.max(0, state.unreadCount - (unread ? 1 : 0))
					};
				});
			} catch (error) {
				console.error('Failed to mark as read', error);
			}
		},

		async markAllAsRead() {
			const generation = dataGeneration;
			const version = signalVersion;
			try {
				await apiClient.post<Record<string, never>>('/api/notifications/read-all');
				if (generation !== dataGeneration) return;
				if (version !== signalVersion) {
					await notificationStore.fetchNotifications();
					return;
				}
				signalVersion++;
				const readAt = new Date().toISOString();
				for (const id of seen.keys()) seen.set(id, readAt);
				update((state) => ({
					...state,
					notifications: state.notifications.map((item) => ({ ...item, read_at: readAt })),
					unreadCount: 0
				}));
				toast.success('อ่านทั้งหมดแล้ว');
			} catch (error) {
				console.error('Failed to mark all as read', error);
			}
		},

		async prepareLogout() {
			const previous = deviceIdentity();
			const generation = ++pushGeneration;
			closeSSE();
			if (!previous) return;
			try {
				await identityReady;
				const binding = await clearBinding(previous);
				await closeDeviceNotifications();
				if (binding) {
					try {
						await unregisterPushDevice(binding.id, binding.version);
					} catch (error) {
						console.error('Failed to revoke notification device during logout', error);
					}
				}
				if (generation !== pushGeneration) return;
				if (isPushMessagingSupported()) {
					const registration = await getPushRegistration();
					if (generation !== pushGeneration) return;
					await (await registration.pushManager.getSubscription())?.unsubscribe();
				}
			} catch (error) {
				console.error('Failed to clear local notification device', error);
			}
		},

		async unsubscribe() {
			await this.prepareLogout();
			return true;
		},
		async revokePushDevice(device: PushDevice) {
			const generation = pushGeneration;
			await unregisterPushDevice(device.id, device.version);
			if (generation !== pushGeneration) return;
			const state = await readBrowserState();
			if (
				state.binding &&
				state.binding.id === device.id &&
				state.binding.version === device.version
			) {
				pushGeneration++;
				await clearBinding(state.identity!, state.intent);
				await closeDeviceNotifications();
				const registration = await getPushRegistration();
				await (await registration.pushManager.getSubscription())?.unsubscribe();
			}
		},
		async syncExistingPushSubscription() {
			bindIdentity();
			if (!identity || !isPushMessagingSupported() || Notification.permission !== 'granted')
				return false;
			const generation = pushGeneration;
			try {
				await identityReady;
				const config = await getPushConfig();
				if (
					!config.available ||
					!config.public_key ||
					!config.key_id ||
					generation !== pushGeneration
				)
					return false;
				const registration = await getPushRegistration();
				const subscription = await registration.pushManager.getSubscription();
				if (!subscription || !subscriptionKeyMatches(subscription, config.public_key)) return false;
				return await syncPushSubscription(subscription, config, generation);
			} catch (error) {
				console.error('Failed to sync existing push subscription', error);
				return false;
			}
		},
		async enablePushFromUserAction(force = false) {
			bindIdentity();
			if (!identity || !isPushMessagingSupported()) return false;
			// Request permission within the user's action, before asynchronous configuration reads.
			const permission =
				Notification.permission === 'granted' ? 'granted' : await Notification.requestPermission();
			if (permission !== 'granted') return false;
			const generation = pushGeneration;
			try {
				await identityReady;
				const config = await getPushConfig();
				if (
					!config.available ||
					!config.public_key ||
					!config.key_id ||
					generation !== pushGeneration
				)
					return false;
				const registration = await getPushRegistration();
				let subscription = await registration.pushManager.getSubscription();
				if (subscription && (force || !subscriptionKeyMatches(subscription, config.public_key))) {
					if (generation !== pushGeneration) return false;
					await subscription.unsubscribe();
					subscription = null;
				}
				if (generation !== pushGeneration) return false;
				if (!subscription)
					subscription = await registration.pushManager.subscribe({
						userVisibleOnly: true,
						applicationServerKey: urlBase64ToUint8Array(config.public_key)
					});
				return await syncPushSubscription(subscription, config, generation);
			} catch (error) {
				console.error('Failed to enable push subscription', error);
				return false;
			}
		},
		async getPushStatus(): Promise<PushNotificationDeviceStatus> {
			bindIdentity();
			const generation = pushGeneration;
			const status: PushNotificationDeviceStatus = {
				supported: isPushMessagingSupported(),
				permission: isPushMessagingSupported() ? Notification.permission : 'unsupported',
				hasSubscription: false,
				isIOS: isIOSDevice(),
				isStandalone: isStandalonePWA(),
				serverAvailable: false,
				synced: false,
				currentDeviceId: null,
				devices: [],
				config: null,
				error: null
			};
			try {
				await identityReady;
				const [config, devices, state] = await Promise.all([
					getPushConfig(),
					listPushDevices(),
					readBrowserState()
				]);
				if (generation !== pushGeneration) return status;
				status.config = config;
				status.serverAvailable = config.available;
				status.devices = devices;
				if (status.supported) {
					const registration = await getPushRegistration();
					const subscription = await registration.pushManager.getSubscription();
					if (generation !== pushGeneration) return { ...status, devices: [] };
					status.hasSubscription = Boolean(subscription);
					const current = devices.find(
						(device) => device.installation_id === state.installation_id
					);
					status.currentDeviceId = current?.id ?? null;
					status.synced = Boolean(
						config.available &&
						config.public_key &&
						subscription &&
						subscriptionKeyMatches(subscription, config.public_key) &&
						current &&
						state.binding?.id === current.id &&
						state.binding.version === current.version &&
						Date.parse(current.expires_at) > Date.now()
					);
				}
			} catch (error) {
				console.error('Failed to read push notification status', error);
				status.error = 'ไม่สามารถตรวจสอบการเชื่อมต่อกับระบบได้';
			}
			return status;
		}
	};

	function subscriptionKeyMatches(subscription: PushSubscription, publicKey: string) {
		const actual = subscription.options.applicationServerKey;
		if (!actual) return false;
		const expected = urlBase64ToUint8Array(publicKey);
		const bytes = new Uint8Array(actual);
		return (
			bytes.length === expected.length && bytes.every((value, index) => value === expected[index])
		);
	}
	async function syncPushSubscription(
		subscription: PushSubscription,
		config: PushConfig,
		generation: number
	) {
		const payload = subscriptionPayload(subscription);
		if (!payload || !identity || !config.key_id || generation !== pushGeneration) return false;
		const scope = deviceIdentity();
		if (!scope) return false;
		const intent = crypto.randomUUID();
		const state = await beginBinding(scope, intent);
		if (generation !== pushGeneration) return false;
		const device = await registerPushDevice({
			...payload,
			installation_id: state.installation_id,
			label: isIOSDevice() ? 'iPhone / iPad' : 'เว็บเบราว์เซอร์',
			key_id: config.key_id
		});
		const binding: DeviceBinding = { ...device, identity: scope, intent };
		if (generation !== pushGeneration || !(await saveBinding(binding))) {
			if (generation !== pushGeneration) {
				try {
					await unregisterPushDevice(device.id, device.version);
				} catch (error) {
					console.error('Failed to revoke superseded device binding', error);
				}
			}
			return false;
		}
		return true;
	}
}

export const notificationStore = createNotificationStore();
