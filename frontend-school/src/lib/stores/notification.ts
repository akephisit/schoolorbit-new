import { PUBLIC_VAPID_KEY } from '$env/static/public';
import { apiClient, BACKEND_URL } from '$lib/api/client';
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

export interface Notification {
	id: string;
	title: string;
	message: string;
	type_: 'info' | 'success' | 'warning' | 'error';
	link?: string;
	read_at?: string;
	created_at: string;
}

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
let reconnectDelay = 3000; // ms, doubles on each failure up to 60s

function createNotificationStore() {
	const { subscribe, set, update } = writable<NotificationState>({
		notifications: [],
		unreadCount: 0,
		loading: false
	});

	return {
		subscribe,

		async fetchNotifications(limit = 10) {
			update((s) => ({ ...s, loading: true }));
			try {
				const response = await apiClient.get<{
					items: Notification[];
					unread_count: number;
				}>(`/api/notifications?limit=${limit}`);

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

		initSSE() {
			if (typeof EventSource === 'undefined') return;
			// 0 = CONNECTING, 1 = OPEN
			if (eventSource && (eventSource.readyState === 1 || eventSource.readyState === 0)) return;

			// Clear any pending reconnect before creating a new connection
			if (reconnectTimeout) {
				clearTimeout(reconnectTimeout);
				reconnectTimeout = null;
			}

			eventSource = new EventSource(`${BACKEND_URL}/api/notifications/stream`, {
				withCredentials: true
			});

			eventSource.onopen = () => {
				console.log('✅ SSE Connected');
				reconnectDelay = 3000; // reset backoff on successful connect
			};

			eventSource.onmessage = (event) => {
				try {
					const newNotif: Notification = JSON.parse(event.data);

					update((s) => {
						// Avoid duplicates
						if (s.notifications.some((n) => n.id === newNotif.id)) return s;

						return {
							...s,
							notifications: [newNotif, ...s.notifications],
							unreadCount: s.unreadCount + 1
						};
					});

					// Show toast
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
				} catch (e) {
					console.error('Failed to parse SSE message', e);
				}
			};

			eventSource.addEventListener('permission_changed', async () => {
				try {
					const { authAPI } = await import('$lib/api/auth');
					await authAPI.refreshCurrentUser({ silent: true });
				} catch (error) {
					console.error('Failed to refresh auth context after permission change', error);
				}
			});

			eventSource.addEventListener('work_items_changed', () => {
				void workStore.refreshSilently();
			});

			eventSource.addEventListener('workflow_window_changed', () => {
				void workStore.refreshSilently();
			});

			eventSource.onerror = () => {
				if (!eventSource) return;

				if (eventSource.readyState === 0) {
					// Browser is actively trying to reconnect (network blip)
					console.log('🔄 SSE Reconnecting...');
				} else {
					// readyState === 2 (CLOSED) — server closed or HTTP error (e.g. 401)
					// Browser will NOT auto-retry; we must do it manually
					console.log(`🔄 SSE closed, retrying in ${reconnectDelay / 1000}s...`);
					eventSource.close();
					eventSource = null;

					reconnectTimeout = setTimeout(() => {
						reconnectDelay = Math.min(reconnectDelay * 2, 60000);
						this.initSSE();
					}, reconnectDelay);
				}
			};
		},

		closeSSE() {
			if (reconnectTimeout) {
				clearTimeout(reconnectTimeout);
				reconnectTimeout = null;
			}
			if (eventSource) {
				eventSource.close();
				eventSource = null;
			}
			reconnectDelay = 3000;
		},

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
