import type { HandleClientError } from '@sveltejs/kit';

// เมื่อ deploy ใหม่ chunk hash เปลี่ยน แต่ browser ยังโหลด app shell เก่าอยู่
// client-side navigation จะ fetch chunk เก่า → 404 → "Failed to fetch dynamically imported module"
// แก้โดย force reload เพื่อให้ browser โหลด app shell ใหม่
export const handleError: HandleClientError = ({ error }) => {
    if (
        error instanceof TypeError &&
        error.message.includes('Failed to fetch dynamically imported module')
    ) {
        window.location.reload();
    }
};
