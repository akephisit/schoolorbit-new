import { PUBLIC_VAPID_KEY } from '$env/static/public';
import { apiClient, BACKEND_URL, getSchoolSubdomainHint } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';
import { realtimeAuthRecovery } from '$lib/realtime/auth-recovery';
import { workStore } from '$lib/stores/work';
import { toast } from 'svelte-sonner';
import { writable } from 'svelte/store';

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
	await navigator.serviceWorker.register('/service-worker.js');
	return navigator.serviceWorker.ready;
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

async function syncPushSubscription(subscription: PushSubscription) {
	const body = subscriptionPayload(subscription);
	if (!body) return false;

	await apiClient.post<Record<string, never>>('/api/notifications/subscribe', body);
	return true;
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
		return shouldMaintainSSE && eventSource === source && sseGeneration === generation;
	}

	function notificationStreamUrl(): string {
		const url = new URL('/api/notifications/stream', BACKEND_URL);
		const schoolSubdomain = getSchoolSubdomainHint();
		if (schoolSubdomain) url.searchParams.set('school_subdomain', schoolSubdomain);
		return url.toString();
	}

	function scheduleSseTask(generation: number, callback: () => void) {
		if (!shouldMaintainSSE || generation !== sseGeneration || reconnectTimeout !== null) {
			return;
		}

		const delay = reconnectDelay;
		reconnectTimeout = setTimeout(() => {
			reconnectTimeout = null;
			if (!shouldMaintainSSE || generation !== sseGeneration) return;
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
		if (!shouldMaintainSSE || generation !== sseGeneration) return;
		if (recoveryInFlight?.generation === generation) return recoveryInFlight.promise;

		clearReconnectTimer();
		closeEventSource();

		const promise = (async () => {
			try {
				const { authAPI } = await import('$lib/api/auth');
				const recoveryAction = await realtimeAuthRecovery(() =>
					authAPI.refreshCurrentUser({ silent: true })
				);
				if (!shouldMaintainSSE || generation !== sseGeneration) return;

				if (recoveryAction === 'reconnect') {
					scheduleSseReconnect(generation);
				} else if (recoveryAction === 'retry') {
					scheduleAuthRecovery(generation);
				} else if (recoveryAction === 'stop') {
					clearReconnectTimer();
					shouldMaintainSSE = false;
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

	function openSSE(generation: number) {
		if (typeof EventSource === 'undefined' || !shouldMaintainSSE || generation !== sseGeneration) {
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
		};

		source.onmessage = (event) => {
			if (!ownsEventSource(source, generation)) return;
			try {
				const newNotif: Notification = JSON.parse(event.data);

				update((state) => {
					if (state.notifications.some((notification) => notification.id === newNotif.id)) {
						return state;
					}

					return {
						...state,
						notifications: [newNotif, ...state.notifications],
						unreadCount: state.unreadCount + 1
					};
				});

				toast.success(newNotif.title, {
					description: newNotif.message,
					duration: 5000,
					action: {
						label: 'ดู',
						onClick: () => {
							if (newNotif.link) window.location.href = newNotif.link;
						}
					}
				});
			} catch (error) {
				console.error('Failed to parse SSE message', error);
			}
		};

		source.addEventListener('permission_changed', async () => {
			if (!ownsEventSource(source, generation)) return;
			try {
				const { authAPI } = await import('$lib/api/auth');
				await authAPI.refreshCurrentUser({ silent: true });
			} catch (error) {
				console.error('Failed to refresh auth context after permission change', error);
			}
		});

		source.addEventListener('work_items_changed', () => {
			if (ownsEventSource(source, generation)) void workStore.refreshSilently();
		});

		source.addEventListener('workflow_window_changed', () => {
			if (ownsEventSource(source, generation)) void workStore.refreshSilently();
		});

		const recover = () => {
			if (!ownsEventSource(source, generation)) return;
			void recoverAfterSessionSignal(generation);
		};
		source.addEventListener('session_invalid', recover);
		source.addEventListener('session_unavailable', recover);

		source.onerror = () => {
			if (!ownsEventSource(source, generation)) return;
			if (source.readyState !== EventSource.CLOSED) {
				console.log('🔄 SSE Reconnecting...');
				return;
			}
			void recoverAfterSessionSignal(generation);
		};
	}

	function initSSE() {
		if (typeof EventSource === 'undefined') return;
		shouldMaintainSSE = true;
		if (eventSource && (eventSource.readyState === 1 || eventSource.readyState === 0)) return;

		clearReconnectTimer();
		sseGeneration += 1;
		openSSE(sseGeneration);
	}

	function closeSSE() {
		shouldMaintainSSE = false;
		sseGeneration += 1;
		clearReconnectTimer();
		closeEventSource();
		reconnectDelay = INITIAL_SSE_RECONNECT_DELAY_MS;
	}

	return {
		subscribe,

		async fetchNotifications(limit = 10) {
			update((s) => ({ ...s, loading: true }));
			try {
				const response = await apiClient.get<ListNotificationsResponse>(
					`/api/notifications?limit=${limit}`
				);

				if (response.success && response.data) {
					set({
						notifications: response.data.items,
						unreadCount: response.data.unread_count,
						loading: false
					});
				}
			} catch (err) {
				console.error('Failed to fetch notifications', err);
				update((s) => ({ ...s, loading: false }));
			}
		},

		initSSE,
		closeSSE,

		async markAsRead(id: string) {
			try {
				// Optimistic update
				update((s) => {
					const notifs = s.notifications.map((n) =>
						n.id === id ? { ...n, read_at: new Date().toISOString() } : n
					);
					const unread = notifs.filter((n) => !n.read_at).length;
					return { ...s, notifications: notifs, unreadCount: unread };
				});

				await apiClient.post<Record<string, never>>(`/api/notifications/${id}/read`);
			} catch (err) {
				console.error('Failed to mark as read', err);
			}
		},

		async markAllAsRead() {
			try {
				// Optimistic update
				update((s) => {
					const notifs = s.notifications.map((n) => ({ ...n, read_at: new Date().toISOString() }));
					return { ...s, notifications: notifs, unreadCount: 0 };
				});

				await apiClient.post<Record<string, never>>('/api/notifications/read-all');

				toast.success('อ่านทั้งหมดแล้ว');
			} catch (err) {
				console.error('Failed to mark all as read', err);
			}
		},

		async unsubscribe() {
			try {
				const registration = await navigator.serviceWorker.ready;
				const subscription = await registration.pushManager.getSubscription();
				if (subscription) {
					await subscription.unsubscribe();
					console.log('Unsubscribed from push');
				}
				return true;
			} catch (err) {
				console.error('Failed to unsubscribe', err);
				return false;
			}
		},

		async subscribeToPush(force = false) {
			return this.enablePushFromUserAction(force);
		},

		async syncExistingPushSubscription() {
			if (!('serviceWorker' in navigator) || !('PushManager' in window)) {
				console.warn('Push messaging is not supported');
				return false;
			}

			try {
				const registration = await getPushRegistration();
				const subscription = await registration.pushManager.getSubscription();
				if (!subscription) return false;

				await syncPushSubscription(subscription);

				console.log('✅ Existing Push Notification Synced to Backend');
				return true;
			} catch (err) {
				console.error('Failed to sync existing push subscription', err);
				return false;
			}
		},

		async enablePushFromUserAction(force = false) {
			if (!isPushMessagingSupported()) {
				console.warn('Push messaging is not supported');
				return false;
			}
			if (!PUBLIC_VAPID_KEY) {
				console.warn('VAPID public key is not configured');
				return false;
			}

			try {
				const registration = await getPushRegistration();
				let subscription = await registration.pushManager.getSubscription();

				if (force && subscription) {
					await subscription.unsubscribe();
					subscription = null;
				}

				if (!subscription) {
					const permission = await Notification.requestPermission();
					if (permission !== 'granted') {
						console.warn('Notification permission denied');
						return false;
					}

					subscription = await registration.pushManager.subscribe({
						userVisibleOnly: true,
						applicationServerKey: urlBase64ToUint8Array(PUBLIC_VAPID_KEY)
					});
				}

				await syncPushSubscription(subscription);

				console.log('✅ Push Notification Subscribed (Synced to Backend)');
				return true;
			} catch (err) {
				console.error('Failed to subscribe to push', err);
				return false;
			}
		},

		async getPushStatus(): Promise<PushNotificationDeviceStatus> {
			const status: PushNotificationDeviceStatus = {
				supported: isPushMessagingSupported(),
				permission: isPushMessagingSupported() ? Notification.permission : 'unsupported',
				hasSubscription: false,
				isIOS: isIOSDevice(),
				isStandalone: isStandalonePWA()
			};

			if (!status.supported) return status;

			try {
				const registration = await getPushRegistration();
				const subscription = await registration.pushManager.getSubscription();
				status.hasSubscription = Boolean(subscription);
				status.permission = Notification.permission;
			} catch (err) {
				console.error('Failed to read push notification status', err);
			}

			return status;
		}
	};
}

export const notificationStore = createNotificationStore();
