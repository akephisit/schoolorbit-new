// SchoolOrbit Service Worker
// Version: 1.0.0
// Strategy: Network-Only (No caching - always online)

const CACHE_NAME = 'schoolorbit-v1';

// Install event - just activate immediately
self.addEventListener('install', (event) => {
    console.log('[ServiceWorker] Installing...');
    // Skip waiting to activate immediately
    self.skipWaiting();
});

// Activate event - clean up old caches if any
self.addEventListener('activate', (event) => {
    console.log('[ServiceWorker] Activating...');
    event.waitUntil(
        caches.keys().then((cacheNames) => {
            return Promise.all(
                cacheNames.map((cacheName) => {
                    // Delete all caches (we don't use any)
                    console.log('[ServiceWorker] Deleting cache:', cacheName);
                    return caches.delete(cacheName);
                })
            );
        })
    );
    // Take control of all pages immediately
    return self.clients.claim();
});

// Fetch event - Network-Only strategy (no caching)
self.addEventListener('fetch', (event) => {
    // Always fetch from network, never cache
    event.respondWith(
        fetch(event.request).catch((error) => {
            console.error('[ServiceWorker] Fetch failed:', error);
            // If network fails, return error response
            return new Response('Network error', {
                status: 503,
                statusText: 'Service Unavailable',
                headers: new Headers({
                    'Content-Type': 'text/plain'
                })
            });
        })
    );
});

// Push notification event (optional - for future use)
self.addEventListener('push', (event) => {
    console.log('[ServiceWorker] Push received');
    // Handle push notifications here if needed
});

console.log('[ServiceWorker] Script loaded');
