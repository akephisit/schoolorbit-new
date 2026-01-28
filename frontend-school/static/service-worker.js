// Web Push Service Worker

// Force immediate update
self.addEventListener('install', (event) => {
    self.skipWaiting();
});

self.addEventListener('activate', (event) => {
    event.waitUntil(clients.claim());
});

self.addEventListener('push', function (event) {
    if (event.data) {
        try {
            const data = event.data.json();

            const options = {
                body: data.body,
                icon: '/icon-192.png',
                vibrate: [100, 50, 100],
                data: {
                    link: data.link || '/'
                }
            };
            // Removed actions/badge to maximize compatibility

            event.waitUntil(
                self.registration.showNotification(data.title, options)
            );
        } catch (e) {
            console.error('Error parsing push data', e);
        }
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
