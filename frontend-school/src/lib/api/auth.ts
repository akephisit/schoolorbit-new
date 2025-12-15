import { authStore, type User } from '$lib/stores/auth';
import { toast } from 'svelte-sonner';

const API_BASE_URL = '/api'; // Proxy ผ่าน SvelteKit

export interface LoginRequest {
    nationalId: string;
    password: string;
    rememberMe?: boolean;
}

export interface LoginResponse {
    success: boolean;
    message: string;
    user: User;
}

export interface ApiError {
    error: string;
    success?: boolean;
}

class AuthAPI {
    /**
     * Login user
     */
    async login(data: LoginRequest): Promise<User> {
        try {
            const response = await fetch(`${API_BASE_URL}/auth/login`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                credentials: 'include', // Important: include cookies
                body: JSON.stringify({
                    nationalId: data.nationalId,
                    password: data.password,
                    rememberMe: data.rememberMe
                })
            });

            const result = await response.json();

            if (!response.ok) {
                const error = result as ApiError;
                throw new Error(error.error || 'เกิดข้อผิดพลาดในการเข้าสู่ระบบ');
            }

            const loginResponse = result as LoginResponse;
            authStore.setUser(loginResponse.user);
            toast.success(loginResponse.message || 'เข้าสู่ระบบสำเร็จ');

            return loginResponse.user;
        } catch (error) {
            const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
            toast.error(message);
            throw error;
        }
    }

    /**
     * Logout user
     */
    async logout(): Promise<void> {
        try {
            const response = await fetch(`${API_BASE_URL}/auth/logout`, {
                method: 'POST',
                credentials: 'include'
            });

            if (!response.ok) {
                throw new Error('ออกจากระบบไม่สำเร็จ');
            }

            authStore.clearUser();
            toast.success('ออกจากระบบสำเร็จ');
        } catch (error) {
            const message = error instanceof Error ? error.message : 'เกิดข้อผิดพลาด';
            toast.error(message);
            throw error;
        }
    }

    /**
     * Get current authenticated user
     */
    async me(): Promise<User | null> {
        try {
            const response = await fetch(`${API_BASE_URL}/auth/me`, {
                credentials: 'include'
            });

            if (!response.ok) {
                if (response.status === 401) {
                    authStore.clearUser();
                    return null;
                }
                throw new Error('ไม่สามารถดึงข้อมูลผู้ใช้ได้');
            }

            const user = (await response.json()) as User;
            authStore.setUser(user);
            return user;
        } catch (error) {
            authStore.clearUser();
            return null;
        }
    }

    /**
     * Check if user is authenticated (called on app init)
     */
    async checkAuth(): Promise<boolean> {
        authStore.setLoading(true);
        const user = await this.me();
        authStore.setLoading(false);
        return user !== null;
    }
}

export const authAPI = new AuthAPI();
