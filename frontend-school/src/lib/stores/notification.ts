import { PUBLIC_BACKEND_URL } from '$env/static/public';
import { toast } from 'svelte-sonner';
import { writable } from 'svelte/store';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

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
        }
    };
}

export const notificationStore = createNotificationStore();
