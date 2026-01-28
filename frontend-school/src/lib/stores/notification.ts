import { PUBLIC_BACKEND_URL, PUBLIC_VAPID_KEY } from '$env/static/public';
import { toast } from 'svelte-sonner';
import { writable } from 'svelte/store';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

// Helper for VAPID key conversion
function urlBase64ToUint8Array(base64String: string) {
    const padding = '='.repeat((4 - base64String.length % 4) % 4);
    const base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
    const rawData = window.atob(base64);
    const outputArray = new Uint8Array(rawData.length);
    for (let i = 0; i < rawData.length; ++i) {
        outputArray[i] = rawData.charCodeAt(i);
    }
    return outputArray;
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

let eventSource: EventSource | null = null;

function createNotificationStore() {
    const { subscribe, set, update } = writable<NotificationState>({
        notifications: [],
        unreadCount: 0,
        loading: false
    });

    return {
        subscribe,

        async fetchNotifications(limit = 10) {
            update(s => ({ ...s, loading: true }));
            try {
                const res = await fetch(`${BACKEND_URL}/api/notifications?limit=${limit}`, {
                    credentials: 'include'
                });

                if (res.ok) {
                    const data = await res.json();
                    set({
                        notifications: data.data,
                        unreadCount: data.unread_count,
                        loading: false
                    });
                }
            } catch (err) {
                console.error('Failed to fetch notifications', err);
                update(s => ({ ...s, loading: false }));
            }
        },

        initSSE() {
            if (typeof EventSource === 'undefined') return;
            if (eventSource?.readyState === 1) return; // Already connected

            eventSource = new EventSource(`${BACKEND_URL}/api/notifications/stream`, {
                withCredentials: true
            });

            eventSource.onopen = () => {
                console.log('✅ SSE Connected');
            };

            eventSource.onmessage = (event) => {
                try {
                    const newNotif: Notification = JSON.parse(event.data);

                    update(s => {
                        // Avoid duplicates
                        if (s.notifications.some(n => n.id === newNotif.id)) return s;

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

            eventSource.onerror = (err) => {
                console.error('SSE Error', err);
                eventSource?.close();
                // Reconnect logic usually handled by browser for SSE
            };
        },

        closeSSE() {
            if (eventSource) {
                eventSource.close();
                eventSource = null;
            }
        },

        async markAsRead(id: string) {
            try {
                // Optimistic update
                update(s => {
                    const notifs = s.notifications.map(n =>
                        n.id === id ? { ...n, read_at: new Date().toISOString() } : n
                    );
                    const unread = notifs.filter(n => !n.read_at).length;
                    return { ...s, notifications: notifs, unreadCount: unread };
                });

                await fetch(`${BACKEND_URL}/api/notifications/${id}/read`, {
                    method: 'POST',
                    credentials: 'include'
                });
            } catch (err) {
                console.error('Failed to mark as read', err);
            }
        },

        async markAllAsRead() {
            try {
                // Optimistic update
                update(s => {
                    const notifs = s.notifications.map(n => ({ ...n, read_at: new Date().toISOString() }));
                    return { ...s, notifications: notifs, unreadCount: 0 };
                });

                await fetch(`${BACKEND_URL}/api/notifications/read-all`, {
                    method: 'POST',
                    credentials: 'include'
                });

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
            if (!('serviceWorker' in navigator) || !('PushManager' in window)) {
                console.warn('Push messaging is not supported');
                return false;
            }

            try {
                // First, check and unregister old service worker if exists
                const oldReg = await navigator.serviceWorker.getRegistration('/service-worker.js');
                if (oldReg) {
                    await oldReg.unregister();
                    console.log('Unregistered old service worker');
                }

                // Register New Service Worker
                const registration = await navigator.serviceWorker.register('/sw.js');
                await navigator.serviceWorker.ready;

                // Check existing subscription
                let subscription = await registration.pushManager.getSubscription();

                // If force update or no subscription, ensure clean state
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

                // ... (rest same logic to send backend)

                const p256dh = subscription.getKey('p256dh');
                const auth = subscription.getKey('auth');

                if (!p256dh || !auth) return false;

                function arrayBufferToUrlSafeBase64(buffer: ArrayBuffer): string {
                    let binary = '';
                    const bytes = new Uint8Array(buffer);
                    for (let i = 0; i < bytes.byteLength; i++) {
                        binary += String.fromCharCode(bytes[i]);
                    }
                    return window.btoa(binary)
                        .replace(/\+/g, '-')
                        .replace(/\//g, '_')
                        .replace(/=+$/, '');
                }

                const body = {
                    endpoint: subscription.endpoint,
                    p256dh: arrayBufferToUrlSafeBase64(p256dh),
                    auth: arrayBufferToUrlSafeBase64(auth)
                };

                await fetch(`${BACKEND_URL}/api/notifications/subscribe`, {
                    method: 'POST',
                    body: JSON.stringify(body),
                    headers: { 'Content-Type': 'application/json' },
                    credentials: 'include'
                });

                console.log('✅ Push Notification Subscribed (Synced to Backend)');
                return true;

            } catch (err) {
                console.error('Failed to subscribe to push', err);
                return false;
            }
        }
    };
}

export const notificationStore = createNotificationStore();
