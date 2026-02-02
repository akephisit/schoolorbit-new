import { authStore, type User } from '$lib/stores/auth';
import { toast } from 'svelte-sonner';
import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

export interface LoginRequest {
	username: string;
	password: string;
	rememberMe?: boolean;
}

export interface ProfileResponse {
	// Read-only fields
	id: string;
	username?: string;
	nationalId?: string;
	firstName: string;
	lastName: string;
	userType: string;
	status: string;
	createdAt: string;
	updatedAt: string;
	primaryRoleName?: string;

	// Editable fields
	title?: string;
	nickname?: string;
	email?: string;
	phone?: string;
	emergencyContact?: string;
	lineId?: string;
	dateOfBirth?: string;
	gender?: string;
	address?: string;
	profileImageUrl?: string;
	hiredDate?: string;
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

			// Map backend response to User interface
			const user: User = {
				...result.user,
				user_type: result.user.userType || result.user.user_type
			};

			// Update store
			authStore.setUser(user);
			toast.success(result.message || 'เข้าสู่ระบบสำเร็จ');

			return user;
		} catch (error: unknown) {
			const message =
				error instanceof Error ? error.message : 'ไม่สามารถเชื่อมต่อกับเซิร์ฟเวอร์ได้';
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

			const userData = await response.json();

			// Map backend response to User interface
			const user: User = {
				...userData,
				user_type: userData.userType || userData.user_type
			};

			authStore.setUser(user);
			return true;
		} catch (error) {
			// Only log actual errors, not 401/403 which are expected state checks
			// Or just suppress log for auth check entirely to keep console clean
			// console.error('Auth check error:', error);
			authStore.clearUser();
			return false;
		} finally {
			authStore.setLoading(false);
		}
	}

	/**
	 * Get full user profile with all fields
	 */
	async getFullProfile(): Promise<ProfileResponse> {
		const response = await fetch(`${BACKEND_URL}/api/auth/me/profile`, {
			credentials: 'include'
		});

		if (!response.ok) {
			if (response.status === 401) {
				authStore.clearUser();
				throw new Error('กรุณาเข้าสู่ระบบอีกครั้ง');
			}
			throw new Error('ไม่สามารถโหลดข้อมูลได้');
		}

		return await response.json();
	}

	/**
	 * Update user profile
	 */
	async updateProfile(data: {
		title?: string;
		nickname?: string;
		email?: string;
		phone?: string;
		emergencyContact?: string;
		lineId?: string;
		dateOfBirth?: string;
		gender?: string;
		address?: string;
		profileImageUrl?: string;
	}): Promise<ProfileResponse> {
		const response = await fetch(`${BACKEND_URL}/api/auth/me/profile`, {
			method: 'PUT',
			credentials: 'include',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify(data)
		});

		if (!response.ok) {
			if (response.status === 401) {
				authStore.clearUser();
				throw new Error('กรุณาเข้าสู่ระบบอีกครั้ง');
			}
			const error = await response.json();
			throw new Error(error.error || 'ไม่สามารถบันทึกข้อมูลได้');
		}

		return await response.json();
	}

	/**
	 * Change password
	 */
	async changePassword(data: {
		currentPassword: string;
		newPassword: string;
	}): Promise<{ success: boolean; message: string }> {
		const response = await fetch(`${BACKEND_URL}/api/auth/me/change-password`, {
			method: 'POST',
			credentials: 'include',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify(data)
		});

		if (!response.ok) {
			if (response.status === 401) {
				const error = await response.json();
				throw new Error(error.error || 'รหัสผ่านปัจจุบันไม่ถูกต้อง');
			}
			const error = await response.json();
			throw new Error(error.error || 'ไม่สามารถเปลี่ยนรหัสผ่านได้');
		}

		return await response.json();
	}
}

export const authAPI = new AuthAPI();
