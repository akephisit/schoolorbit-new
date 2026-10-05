/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />

// Push-only worker. Requests use the browser network directly; no application cache.
const sw = self as unknown as ServiceWorkerGlobalScope;

sw.addEventListener('install', (event) => {
	event.waitUntil(sw.skipWaiting());
});

sw.addEventListener('activate', (event) => {
	event.waitUntil(sw.clients.claim());
});

// Handle push notifications (optional - for future use)
// Handle push notifications
sw.addEventListener('push', (event) => {
	if (event.data) {
		try {
			const data = event.data.json();

			const options: NotificationOptions = {
				body: data.body,
				icon: '/icon-192.png',
				badge: '/notification-badge.png',
				silent: false,
				// @ts-expect-error vibrate is not in NotificationOptions typing
				vibrate: [200, 100, 200, 100, 200],
				requireInteraction: true,
				timestamp: Date.now(),
				actions: [
					{
						action: 'open',
						title: 'เปิดดู'
					}
				],
				data: {
					link: data.link || '/'
				}
			};

			event.waitUntil(sw.registration.showNotification(data.title, options));
		} catch (e) {
			console.error('Error parsing push data', e);
		}
	}
});

sw.addEventListener('notificationclick', (event) => {
	event.notification.close();

	if (event.action === 'open' || !event.action) {
		const link = event.notification.data.link;
		event.waitUntil(
			sw.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((windowClients) => {
				for (let i = 0; i < windowClients.length; i++) {
					const client = windowClients[i];
					if (client.url === link && 'focus' in client) {
						return (client as WindowClient).focus();
					}
				}
				if (sw.clients.openWindow) {
					return sw.clients.openWindow(link);
				}
			})
		);
	}
});

console.log('[ServiceWorker] Push worker loaded');
