// Web Push Service Worker

self.addEventListener('push', function (event) {
    if (event.data) {
        const data = event.data.json();

        const options = {
            body: data.body,
            icon: '/icons/icon-192x192.png', // ต้องมี icon นี้ใน static
            badge: '/icons/badge-72x72.png', // icon เล็กๆ สีขาวล้วน (ถ้ามี)
            vibrate: [100, 50, 100],
            data: {
                link: data.link || '/'
            },
            actions: [
                {
                    action: 'open',
                    title: 'เปิดดู'
                }
            ]
        };

        event.waitUntil(
            self.registration.showNotification(data.title, options)
        );
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
