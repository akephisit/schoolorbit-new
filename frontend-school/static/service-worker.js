// Web Push Service Worker

self.addEventListener('push', function (event) {
    console.log('[ServiceWorker] Push Received', event);

    if (event.data) {
        try {
            const data = event.data.json();
            console.log('[ServiceWorker] Push Data:', data);

            const options = {
                body: data.body,
                vibrate: [100, 50, 100],
                data: {
                    link: data.link || '/'
                }
                // Comment out icons for now to test basic functionality
                // icon: '/icons/icon-192x192.png',
                // badge: '/icons/badge-72x72.png',
            };

            event.waitUntil(
                self.registration.showNotification(data.title, options)
            );
        } catch (e) {
            console.error('[ServiceWorker] Error parsing push data', e);
        }
    } else {
        console.log('[ServiceWorker] Push event but no data');
    }
});

self.addEventListener('notificationclick', function (event) {
    event.notification.close();

    if (event.action === 'open' || !event.action) {
        event.waitUntil(
            clients.matchAll({ type: 'window', includeUncontrolled: true }).then(windowClients => {
                // ถ้ามีหน้าเว็บเปิดอยู่แล้ว ให้ focus
                for (let i = 0; i < windowClients.length; i++) {
                    const client = windowClients[i];
                    if (client.url === event.notification.data.link && 'focus' in client) {
                        return client.focus();
                    }
                }
                // ถ้าไม่มี ให้เปิดหน้าใหม่
                if (clients.openWindow) {
                    return clients.openWindow(event.notification.data.link);
                }
            })
        );
    }
});
