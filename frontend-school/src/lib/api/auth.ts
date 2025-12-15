import { authStore, type User } from '$lib/stores/auth';
import { toast } from 'svelte-sonner';
import { browser } from '$app/environment';

// Get backend URL from environment (Cloudflare Workers will inject this)
// or use production URL as fallback
const BACKEND_URL =
    (browser && (window as any).PUBLIC_BACKEND_URL) ||
    'https://school-api.schoolorbit.app';

export interface LoginRequest {
    nationalId: string;
    password: string;
    rememberMe?: boolean;
}

class AuthAPI {
    /**
     * Login - Direct to backend (client-side)
     */
    async login(data: LoginRequest): Promise<User> {
        authStore.setLoading(true);

        try {
            const response = await fetch(`${BACKEND_URL}/api/auth/login`, {
                method: 'POST',
                credentials: 'include', // Send/receive cookies
                headers: {
                    'Content-Type': 'application/json'
                    // No X-School-Subdomain needed - backend extracts from Origin
                },
                body: JSON.stringify(data)
            });

            const result = await response.json();

            if (!response.ok) {
                throw new Error(result.error || 'เกิดข้อผิดพลาดในการเข้าสู่ระบบ');
            }

            // Update store
            authStore.setUser(result.user);
            toast.success(result.message || 'เข้าสู่ระบบสำเร็จ');

            return result.user;
        } catch (error: any) {
            const message = error.message || 'ไม่สามารถเชื่อมต่อกับเซิร์ฟเวอร์ได้';
            toast.error(message);
            throw error;
        } finally {
            authStore.setLoading(false);
        }
    }

    /**
     * Logout - Direct to backend (client-side)
     */
    async logout(): Promise<void> {
        try {
            const response = await fetch(`${BACKEND_URL}/api/auth/logout`, {
                method: 'POST',
                credentials: 'include'
            });

            if (response.ok) {
                authStore.clearUser();
                toast.success('ออกจากระบบสำเร็จ');
            }
        } catch (error) {
            console.error('Logout error:', error);
            // Clear store anyway
            authStore.clearUser();
        }
    }

    /**
     * Check authentication status - Direct to backend (client-side)
     */
    async checkAuth(): Promise<boolean> {
        authStore.setLoading(true);

        try {
            const response = await fetch(`${BACKEND_URL}/api/auth/me`, {
                credentials: 'include'
            });

            if (!response.ok) {
                if (response.status === 401) {
                    authStore.clearUser();
                    return false;
                }
                throw new Error('Failed to check auth');
            }

            const user = await response.json();
            authStore.setUser(user);
            return true;
        } catch (error) {
            console.error('Auth check error:', error);
            authStore.clearUser();
            return false;
        } finally {
            authStore.setLoading(false);
        }
    }
}

export const authAPI = new AuthAPI();
