import type { HandleClientError } from '@sveltejs/kit';

// SvelteKit prefetch chunk บน hover link — ถ้า chunk ไม่มีบน server (deploy ไม่สมบูรณ์
// หรือ stale cache หลัง deploy ใหม่) จะได้ "Failed to fetch dynamically imported module"
// reload ครั้งเดียวเพื่อโหลด manifest ใหม่ — guard ด้วย sessionStorage ป้องกัน reload วน
export const handleError: HandleClientError = ({ error }) => {
    if (
        error instanceof TypeError &&
        error.message.includes('Failed to fetch dynamically imported module')
    ) {
        const key = '_sk_chunk_reload';
        if (!sessionStorage.getItem(key)) {
            sessionStorage.setItem(key, '1');
            window.location.reload();
        }
    }
};
